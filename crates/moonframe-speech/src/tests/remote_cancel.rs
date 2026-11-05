//! Cancellation must stop network I/O, not just discard the worker's reply.

use {
    super::*,
    std::{io::Write, net::TcpStream},
};

const WAIT: Duration = Duration::from_secs(5);

fn engine(url: &str) -> Engine {
    Engines::read(&from_json(&remote_settings(url)).unwrap())
        .named("cloud")
        .unwrap()
        .clone()
}

fn listener() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    (listener, url)
}

#[test]
fn cancellation_before_the_worker_starts_never_connects() {
    let (listener, url) = listener();
    listener.set_nonblocking(true).unwrap();
    let cancel = Cancel::default();
    cancel.set();
    assert_eq!(
        run_engine(&engine(&url), &speech_sound(), &cancel),
        Err("Cancelled".into())
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn cancellation_before_http_registration_shuts_down_the_agent() {
    let (listener, url) = listener();
    listener.set_nonblocking(true).unwrap();
    let agent = ureq::agent();
    let cancel = Cancel::default();
    cancel.set();
    assert!(cancel.http(&agent).is_err());
    assert!(agent.post(url).send("voice").is_err());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

fn read_take(stream: &mut TcpStream) {
    stream.set_read_timeout(Some(WAIT)).unwrap();
    let mut bytes = Vec::new();
    let mut chunk = [0; 8192];
    while !bytes.windows(4).any(|end| end == b"--\r\n") {
        let read = stream.read(&mut chunk).unwrap();
        assert_ne!(read, 0, "request ended before the take");
        bytes.extend_from_slice(&chunk[..read]);
    }
}

#[test]
fn cancellation_interrupts_response_headers_and_response_body_reads() {
    for body_started in [false, true] {
        let (listener, url) = listener();
        let (ready, waiting) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            read_take(&mut stream);
            if body_started {
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n{\"text\":\"")
                    .unwrap();
            }
            ready.send(()).unwrap();
            // Shutdown must close the peer, even while its response is incomplete.
            match stream.read(&mut [0; 1]) {
                Ok(0) => {},
                Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {},
                other => panic!("response connection stayed open: {other:?}"),
            }
        });
        let cancel = Cancel::default();
        let stopping = cancel.clone();
        let (done, result) = mpsc::channel();
        let worker = thread::spawn(move || {
            done.send(run_engine(&engine(&url), &speech_sound(), &stopping))
                .unwrap();
        });
        waiting.recv_timeout(WAIT).unwrap();
        cancel.set();
        assert!(result.recv_timeout(WAIT).unwrap().is_err());
        worker.join().unwrap();
        server.join().unwrap();
    }
}

#[test]
fn cancellation_interrupts_a_blocked_upload() {
    let (listener, url) = listener();
    let (ready, waiting) = mpsc::channel();
    let (drain, draining) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(WAIT)).unwrap();
        let mut chunk = [0; 8192];
        let mut received = stream.read(&mut chunk).unwrap();
        assert_ne!(received, 0);
        ready.send(()).unwrap();
        // Do not drain the upload until cancellation has shut the socket down.
        draining.recv_timeout(WAIT).unwrap();
        loop {
            match stream.read(&mut chunk) {
                Ok(0) => return received,
                Ok(count) => received += count,
                Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {
                    return received;
                },
                Err(error) => panic!("upload did not close: {error}"),
            }
        }
    });
    // Larger than a socket's buffers, without spending time recording audio.
    let file = tempfile::NamedTempFile::new().unwrap();
    const BYTES: u64 = 32 * 1024 * 1024;
    file.as_file().set_len(BYTES).unwrap();
    let cancel = Cancel::default();
    let stopping = cancel.clone();
    let (done, result) = mpsc::channel();
    let worker = thread::spawn(move || {
        done.send(engine(&url).transcribe(file.path(), &stopping))
            .unwrap();
    });
    waiting.recv_timeout(WAIT).unwrap();
    cancel.set();
    drain.send(()).unwrap();
    assert!(result.recv_timeout(WAIT).unwrap().is_err());
    worker.join().unwrap();
    assert!((server.join().unwrap() as u64) < BYTES);
}
