//! The way out for a web page made into a PDF: a proxy on this machine that
//! WebKit sends every connection of the page's session through. It looks a
//! host up itself, refuses it when any of its addresses is off the public
//! internet, and connects to an address it checked. So neither a redirect,
//! a frame or picture by name, nor a name whose answer changes between a
//! check and the connection (DNS rebinding) reaches this machine or its
//! network: the content rules only see an address's text. It asks for a
//! password made when it starts, so no other program on this machine can
//! use it to reach the internet as this app.

use {
    std::{
        ffi::CStr,
        fs::File,
        io::{self, Read, Write},
        net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
        sync::{
            Arc, Condvar, Mutex, PoisonError,
            atomic::{AtomicU64, Ordering},
            mpsc,
        },
        thread,
        time::{Duration, Instant},
    },
    url::Url,
};

/// The user name the proxy asks for, beside its password.
pub(super) const USER: &CStr = c"moonframe-pdf";
/// Longest request head read.
const MAX_HEAD: usize = 16 * 1024;
/// Most connections served at once; more wait to be taken until one ends.
const MAX_OPEN: usize = 128;
/// How long a request's head, a lookup and its connection together, or a
/// tunnel quiet both ways waits.
const WAIT: Duration = Duration::from_secs(30);
/// How often a tunnel looks whether it has been quiet too long.
const TICK: Duration = Duration::from_secs(1);
/// Headers of one connection, never sent on (with those `Connection` names).
const HOP: [&str; 8] = [
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "proxy-connection",
    "te",
    "trailer",
    "upgrade",
];

/// Which addresses a connection may reach.
type Allow = fn(IpAddr) -> bool;

/// The proxy pages' sessions go through: where it listens, and the
/// password it asks for with [`USER`].
#[derive(Clone)]
pub(super) struct Proxy {
    pub(super) port: u16,
    pub(super) password: String,
}

/// The proxy, started the first time one asks (and asked again after it
/// could not start); it reaches only the public internet.
pub(super) fn proxy() -> Result<Proxy, String> {
    static STARTED: Mutex<Option<Proxy>> = Mutex::new(None);
    let mut started = STARTED.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(proxy) = &*started {
        return Ok(proxy.clone());
    }
    let proxy = start(moonframe_core::public_address)?;
    *started = Some(proxy.clone());
    Ok(proxy)
}

/// What each connection is checked against.
struct Gate {
    allow: Allow,
    /// The `Proxy-Authorization` a connection must send.
    authorization: String,
}

/// Connections being served, against [`MAX_OPEN`].
#[derive(Default)]
struct Slots {
    open: Mutex<usize>,
    freed: Condvar,
}

impl Slots {
    /// Waits for a slot, and takes it.
    fn take(&self) {
        let mut open = self.open.lock().unwrap_or_else(PoisonError::into_inner);
        while *open >= MAX_OPEN {
            open = self
                .freed
                .wait(open)
                .unwrap_or_else(PoisonError::into_inner);
        }
        *open += 1;
    }

    fn give_back(&self) {
        *self.open.lock().unwrap_or_else(PoisonError::into_inner) -= 1;
        self.freed.notify_one();
    }
}

/// Listens on a free port of this machine, serving each connection on a
/// thread of its own.
fn start(allow: Allow) -> Result<Proxy, String> {
    let failed = |error: io::Error| format!("cannot start the PDF proxy: {error}");
    let password = password().map_err(failed)?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(failed)?;
    let port = listener.local_addr().map_err(failed)?.port();
    let mut credential = USER.to_bytes().to_vec();
    credential.push(b':');
    credential.extend_from_slice(password.as_bytes());
    let gate = Arc::new(Gate {
        allow,
        authorization: format!("Basic {}", base64(&credential)),
    });
    let slots = Arc::new(Slots::default());
    thread::Builder::new()
        .name("pdf-proxy".to_owned())
        .spawn(move || {
            loop {
                // A connection past the most served waits in the listener's
                // queue, not refused, until one ends.
                slots.take();
                let Ok((client, _)) = listener.accept() else {
                    slots.give_back();
                    continue;
                };
                let (gate, held) = (Arc::clone(&gate), Arc::clone(&slots));
                let spawned = thread::Builder::new()
                    .name("pdf-proxy-connection".to_owned())
                    .spawn(move || {
                        if let Err(error) = serve(client, &gate) {
                            tracing::debug!(%error, "PDF proxy connection ended");
                        }
                        held.give_back();
                    });
                if spawned.is_err() {
                    slots.give_back();
                }
            }
        })
        .map_err(failed)?;
    Ok(Proxy { port, password })
}

/// A password no one can guess: 16 random bytes, in hex.
fn password() -> io::Result<String> {
    let mut bytes = [0; 16];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// `bytes` in Base64, padded, as Basic authorization carries them.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let joined = chunk.iter().enumerate().fold(0_u32, |joined, (at, byte)| {
            joined | u32::from(*byte) << (16 - 8 * at)
        });
        for at in 0..4 {
            out.push(if at <= chunk.len() {
                char::from(ALPHABET[(joined >> (18 - 6 * at)) as usize & 63])
            } else {
                '='
            });
        }
    }
    out
}

/// One connection: a `CONNECT host:port` tunnel (what WebKit asks for), or
/// a plain `http://` request in absolute form, sent on with `Connection:
/// close`. Without the password it is asked for, on the same connection.
fn serve(mut client: TcpStream, gate: &Gate) -> io::Result<()> {
    client.set_read_timeout(Some(WAIT))?;
    let mut read = Vec::new();
    let mut asked = 0;
    let (head, early) = loop {
        let (head, early) = read_head(&mut client, read)?;
        let head = String::from_utf8(head)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "a head not in UTF-8"))?;
        let authorized = head.split("\r\n").skip(1).any(|line| {
            line.split_once(':').is_some_and(|(name, value)| {
                name.trim().eq_ignore_ascii_case("proxy-authorization")
                    && value.trim() == gate.authorization
            })
        });
        if authorized {
            break (head, early);
        }
        asked += 1;
        if asked > 2 || has_body(&head) {
            return refuse(&mut client, "407 Proxy Authentication Required");
        }
        client.write_all(
            b"HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic \
              realm=\"moonframe-pdf\"\r\nContent-Length: 0\r\n\r\n",
        )?;
        read = early;
    };
    let mut lines = head.split("\r\n");
    let mut request = lines.next().unwrap_or_default().split(' ');
    let (Some(method), Some(target), Some(version), None) = (
        request.next(),
        request.next(),
        request.next(),
        request.next(),
    ) else {
        return refuse(&mut client, "400 Bad Request");
    };
    if method.eq_ignore_ascii_case("CONNECT") {
        let Some((host, port)) = authority(target) else {
            return refuse(&mut client, "400 Bad Request");
        };
        let upstream = match connect(&host, port, gate.allow) {
            Ok(upstream) => upstream,
            Err(status) => return refuse(&mut client, status),
        };
        client.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")?;
        return splice(client, upstream, &early, WAIT);
    }
    let Some((host, port, path)) = Url::parse(target)
        .ok()
        .filter(|url| url.scheme() == "http")
        .and_then(|url| {
            let host = match url.host()? {
                url::Host::Domain(name) => name.to_owned(),
                url::Host::Ipv4(ip) => ip.to_string(),
                url::Host::Ipv6(ip) => ip.to_string(),
            };
            let path = url[url::Position::BeforePath..url::Position::AfterQuery].to_owned();
            Some((host, url.port_or_known_default()?, path))
        })
    else {
        return refuse(&mut client, "400 Bad Request");
    };
    let headers: Vec<(&str, &str)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim(), value.trim()))
        .collect();
    let header = |wanted: &str| {
        headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(wanted))
            .map(|(_, value)| *value)
    };
    // A body is sent on by its length alone, so nothing after it (a
    // second request on the connection) reaches this host.
    if header("transfer-encoding").is_some() {
        return refuse(&mut client, "411 Length Required");
    }
    let Some(length): Option<u64> =
        header("content-length").map_or(Some(0), |length| length.parse().ok())
    else {
        return refuse(&mut client, "400 Bad Request");
    };
    let named: Vec<&str> = headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("connection"))
        .flat_map(|(_, value)| value.split(','))
        .map(str::trim)
        .collect();
    let mut upstream = match connect(&host, port, gate.allow) {
        Ok(upstream) => upstream,
        Err(status) => return refuse(&mut client, status),
    };
    let mut sent = format!("{method} {path} {version}\r\n");
    for (name, value) in &headers {
        let hop = HOP
            .iter()
            .chain(&named)
            .any(|hop| name.eq_ignore_ascii_case(hop));
        if !hop {
            sent.push_str(&format!("{name}: {value}\r\n"));
        }
    }
    sent.push_str("Connection: close\r\n\r\n");
    let early = &early[..early
        .len()
        .min(usize::try_from(length).unwrap_or(usize::MAX))];
    let mut sent = sent.into_bytes();
    sent.extend_from_slice(early);
    upstream.set_write_timeout(Some(WAIT))?;
    upstream.write_all(&sent)?;
    let rest = length.saturating_sub(early.len() as u64);
    if io::copy(&mut (&client).take(rest), &mut upstream)? < rest {
        return Err(io::ErrorKind::UnexpectedEof.into());
    }
    // The answer alone goes back; the connection then closes.
    client.set_write_timeout(Some(WAIT))?;
    upstream.set_read_timeout(Some(TICK))?;
    pump(&upstream, &client, &Quiet::new(WAIT));
    let _ = upstream.shutdown(Shutdown::Both);
    client.shutdown(Shutdown::Both)
}

/// Whether a request head says a body follows it.
fn has_body(head: &str) -> bool {
    head.split("\r\n").skip(1).any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.trim().eq_ignore_ascii_case("transfer-encoding")
                || (name.trim().eq_ignore_ascii_case("content-length") && value.trim() != "0")
        })
    })
}

/// The request's head, through its blank line, and what came after it;
/// `read` is what the connection sent already.
fn read_head(client: &mut TcpStream, mut read: Vec<u8>) -> io::Result<(Vec<u8>, Vec<u8>)> {
    let mut chunk = [0; 2048];
    loop {
        if let Some(end) = read.windows(4).position(|window| window == b"\r\n\r\n") {
            let early = read.split_off(end + 4);
            read.truncate(end);
            return Ok((read, early));
        }
        if read.len() > MAX_HEAD {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "a head too long",
            ));
        }
        let count = client.read(&mut chunk)?;
        if count == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        read.extend_from_slice(&chunk[..count]);
    }
}

/// `host:port` or `[v6]:port`.
fn authority(target: &str) -> Option<(String, u16)> {
    let (host, port) = match target.strip_prefix('[') {
        Some(rest) => {
            let (host, port) = rest.split_once("]:")?;
            (host, port)
        },
        None => target.rsplit_once(':')?,
    };
    let port = port.parse().ok().filter(|port| *port != 0)?;
    (!host.is_empty()).then(|| (host.to_owned(), port))
}

/// Looks `host` up and connects to one of its addresses, all of which must
/// be allowed, within [`WAIT`] all told; else the status to answer with.
fn connect(host: &str, port: u16, allow: Allow) -> Result<TcpStream, &'static str> {
    let deadline = Instant::now() + WAIT;
    let addresses = look_up(host, port, WAIT)?;
    tracing::debug!(host, port, "the PDF proxy looks a host up");
    if !addresses.iter().all(|address| allow(address.ip())) {
        tracing::debug!(host, "the PDF proxy refused a host off the public internet");
        return Err("403 Forbidden");
    }
    // Each address left shares the time left, so one that never answers
    // does not take it all from the next.
    for (tried, address) in addresses.iter().enumerate() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        let share = left / u32::try_from(addresses.len() - tried).unwrap_or(u32::MAX);
        if let Ok(upstream) =
            TcpStream::connect_timeout(address, share.max(Duration::from_millis(1)))
        {
            return Ok(upstream);
        }
    }
    Err("502 Bad Gateway")
}

/// `host`'s addresses, looked up on a thread of its own so a lookup that
/// never answers is given up after `wait`.
fn look_up(host: &str, port: u16, wait: Duration) -> Result<Vec<SocketAddr>, &'static str> {
    let (found, find) = mpsc::channel();
    let host = host.to_owned();
    thread::Builder::new()
        .name("pdf-proxy-lookup".to_owned())
        .spawn(move || {
            let addresses = (host.as_str(), port)
                .to_socket_addrs()
                .map(Iterator::collect::<Vec<_>>);
            let _ = found.send(addresses);
        })
        .map_err(|_| "502 Bad Gateway")?;
    match find.recv_timeout(wait) {
        Ok(Ok(addresses)) if !addresses.is_empty() => Ok(addresses),
        Err(mpsc::RecvTimeoutError::Timeout) => Err("504 Gateway Timeout"),
        _ => Err("502 Bad Gateway"),
    }
}

fn refuse(client: &mut TcpStream, status: &str) -> io::Result<()> {
    client.write_all(
        format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes(),
    )
}

/// When a tunnel last carried anything, either way, and how long it may
/// then be quiet.
struct Quiet {
    since: Instant,
    /// Milliseconds after `since`.
    last: AtomicU64,
    wait: Duration,
}

impl Quiet {
    fn new(wait: Duration) -> Self {
        Self {
            since: Instant::now(),
            last: AtomicU64::new(0),
            wait,
        }
    }

    fn heard(&self) {
        let now = u64::try_from(self.since.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.last.store(now, Ordering::Relaxed);
    }

    /// Whether nothing has passed for its wait.
    fn too_long(&self) -> bool {
        let last = Duration::from_millis(self.last.load(Ordering::Relaxed));
        self.since.elapsed().saturating_sub(last) >= self.wait
    }
}

/// Copies `from` to `to` until `from` ends, either fails, or the tunnel
/// `quiet` follows has carried nothing either way for its wait; `from`
/// reads with a short timeout, to look.
fn pump(mut from: &TcpStream, mut to: &TcpStream, quiet: &Quiet) {
    let mut chunk = [0; 16 * 1024];
    loop {
        match from.read(&mut chunk) {
            Ok(0) => return,
            Ok(count) => {
                if to.write_all(&chunk[..count]).is_err() {
                    return;
                }
                quiet.heard();
            },
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) =>
            {
                if quiet.too_long() {
                    return;
                }
            },
            Err(_) => return,
        }
    }
}

/// Copies both ways until either side ends, or nothing has passed either
/// way for `wait`, then closes both.
fn splice(client: TcpStream, upstream: TcpStream, early: &[u8], wait: Duration) -> io::Result<()> {
    for stream in [&client, &upstream] {
        stream.set_read_timeout(Some(TICK.min(wait / 4)))?;
        stream.set_write_timeout(Some(wait))?;
    }
    (&upstream).write_all(early)?;
    let quiet = Arc::new(Quiet::new(wait));
    let (client_in, upstream_out) = (client.try_clone()?, upstream.try_clone()?);
    let heard = Arc::clone(&quiet);
    let outward = thread::Builder::new()
        .name("pdf-proxy-out".to_owned())
        .spawn(move || {
            pump(&client_in, &upstream_out, &heard);
            let _ = client_in.shutdown(Shutdown::Both);
            let _ = upstream_out.shutdown(Shutdown::Both);
        })?;
    pump(&upstream, &client, &quiet);
    let _ = client.shutdown(Shutdown::Both);
    let _ = upstream.shutdown(Shutdown::Both);
    let _ = outward.join();
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// A proxy allowing `allow`, and the `Proxy-Authorization` line it asks for.
    fn proxy(allow: Allow) -> (u16, String) {
        let proxy = start(allow).unwrap();
        let credential = format!("{}:{}", USER.to_str().unwrap(), proxy.password);
        (
            proxy.port,
            format!(
                "Proxy-Authorization: Basic {}\r\n",
                base64(credential.as_bytes())
            ),
        )
    }

    /// Sends `request` to the proxy on `port`, and reads all it answers.
    fn send(port: u16, request: &str) -> String {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut answer = String::new();
        let _ = stream.read_to_string(&mut answer);
        answer
    }

    /// Sends `request` through a proxy allowing `allow`, its password put
    /// in where `{auth}` is.
    fn ask(allow: Allow, request: &str) -> String {
        let (port, auth) = proxy(allow);
        send(port, &request.replace("{auth}", &auth))
    }

    /// A server on this machine that answers each connection with `reply`
    /// after reading its first chunk, which it hands back.
    fn server(reply: &'static [u8]) -> (u16, mpsc::Receiver<String>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (heard, hear) = mpsc::channel();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut chunk = [0; 4096];
            let count = stream.read(&mut chunk).unwrap();
            heard
                .send(String::from_utf8_lossy(&chunk[..count]).into_owned())
                .unwrap();
            stream.write_all(reply).unwrap();
        });
        (port, hear)
    }

    #[test]
    fn base64_pads_as_basic_authorization_does() {
        assert_eq!(base64(b"moonframe:s3cret"), "bW9vbmZyYW1lOnMzY3JldA==");
        assert_eq!(base64(b"ab"), "YWI=");
        assert_eq!(base64(b"abc"), "YWJj");
        assert_eq!(base64(b""), "");
    }

    #[test]
    fn the_password_is_asked_for_on_the_same_connection() {
        let (server_port, heard) = server(b"pong");
        let (port, auth) = proxy(|_| true);
        let connect = format!("CONNECT 127.0.0.1:{server_port} HTTP/1.1\r\n");
        let answer = send(port, &format!("{connect}\r\n{connect}{auth}\r\nping"));
        assert_eq!(
            answer,
            "HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic \
             realm=\"moonframe-pdf\"\r\nContent-Length: 0\r\n\r\nHTTP/1.1 200 Connection \
             Established\r\n\r\npong"
        );
        assert_eq!(heard.recv().unwrap(), "ping");
        let wrong = "Proxy-Authorization: Basic bW9vbmZyYW1lOnMzY3JldA==\r\n";
        let answer = send(port, &format!("{connect}{wrong}\r\n").repeat(3));
        assert!(answer.ends_with("Connection: close\r\n\r\n"), "{answer}");
        assert!(!answer.contains("200"), "{answer}");
    }

    #[test]
    fn this_machine_and_its_network_are_refused_by_address_or_name() {
        for target in ["127.0.0.1:443", "localhost:443", "[::1]:443", "10.0.0.1:80"] {
            let answer = ask(
                moonframe_core::public_address,
                &format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n{{auth}}\r\n"),
            );
            assert!(answer.starts_with("HTTP/1.1 403"), "{target}: {answer}");
        }
        let answer = ask(
            moonframe_core::public_address,
            "GET http://localhost/ HTTP/1.1\r\nHost: localhost\r\n{auth}\r\n",
        );
        assert!(answer.starts_with("HTTP/1.1 403"), "{answer}");
        let answer = ask(moonframe_core::public_address, "nonsense\r\n{auth}\r\n");
        assert!(answer.starts_with("HTTP/1.1 400"), "{answer}");
    }

    #[test]
    fn an_allowed_host_is_tunnelled_to() {
        let (port, heard) = server(b"pong");
        let answer = ask(
            |_| true,
            &format!("CONNECT 127.0.0.1:{port} HTTP/1.1\r\n{{auth}}\r\nping"),
        );
        assert_eq!(answer, "HTTP/1.1 200 Connection Established\r\n\r\npong");
        assert_eq!(heard.recv().unwrap(), "ping");
    }

    /// Two connected sockets on this machine.
    fn pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let near = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        (near, listener.accept().unwrap().0)
    }

    #[test]
    fn a_tunnel_quiet_one_way_stays_open_while_the_other_carries() {
        // Long enough that a busy machine's late wake-up between two bytes
        // stays well inside it.
        let wait = Duration::from_millis(1000);
        let ((mut page, client), (upstream, mut site)) = (pair(), pair());
        let spliced = thread::spawn(move || splice(client, upstream, b"", wait));
        // The site answers slowly, a byte at a time, for two waits, while
        // the page, having asked, says nothing more.
        for _ in 0..8 {
            thread::sleep(wait / 4);
            site.write_all(b"x").unwrap();
        }
        page.set_read_timeout(Some(wait * 4)).unwrap();
        let mut heard = [0; 8];
        page.read_exact(&mut heard).unwrap();
        assert_eq!(&heard, b"xxxxxxxx");
        // Quiet both ways, it ends.
        spliced.join().unwrap().unwrap();
        let mut rest = Vec::new();
        assert_eq!(page.read_to_end(&mut rest).unwrap(), 0);
    }

    #[test]
    fn a_plain_request_is_sent_on_in_origin_form_without_hop_headers() {
        let (port, heard) = server(b"HTTP/1.1 204 No Content\r\n\r\n");
        let answer = ask(
            |_| true,
            &format!(
                "POST http://127.0.0.1:{port}/a?b=1 HTTP/1.1\r\nHost: x\r\nProxy-Connection: \
                 keep-alive\r\nConnection: keep-alive, X-Hop\r\nX-Hop: 1\r\nUpgrade: \
                 websocket\r\nTE: trailers\r\nContent-Length: 2\r\n{{auth}}\r\nhiGET \
                 http://example.com/ HTTP/1.1\r\n\r\n"
            ),
        );
        assert_eq!(answer, "HTTP/1.1 204 No Content\r\n\r\n");
        assert_eq!(
            heard.recv().unwrap(),
            "POST /a?b=1 HTTP/1.1\r\nHost: x\r\nContent-Length: 2\r\nConnection: \
             close\r\n\r\nhi"
        );
        let answer = ask(
            |_| true,
            "POST http://127.0.0.1:1/ HTTP/1.1\r\nTransfer-Encoding: chunked\r\n{auth}\r\n",
        );
        assert!(answer.starts_with("HTTP/1.1 411"), "{answer}");
    }
}
