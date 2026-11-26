//! The HTTP client this crate brings into a build leaves a gzip answer as
//! it came: its default `gzip` decoding, once on, is on for every crate.

use {
    super::*,
    std::{io::Write, thread},
};

#[test]
fn a_gzip_answer_keeps_its_encoding_and_bytes() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let body = b"\x1f\x8b not decoded";
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut chunk = [0; 1024];
        while !request.windows(4).any(|end| end == b"\r\n\r\n") {
            let read = stream.read(&mut chunk).unwrap();
            assert_ne!(read, 0);
            request.extend_from_slice(&chunk[..read]);
        }
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).unwrap();
        stream.write_all(body).unwrap();
        String::from_utf8_lossy(&request).to_lowercase()
    });
    let mut answer = ureq::agent().get(&url).call().unwrap();
    assert_eq!(
        answer
            .headers()
            .get("content-encoding")
            .and_then(|value| value.to_str().ok()),
        Some("gzip")
    );
    assert_eq!(answer.body_mut().read_to_vec().unwrap(), body);
    let request = server.join().unwrap();
    assert!(!request.contains("accept-encoding"), "{request}");
}
