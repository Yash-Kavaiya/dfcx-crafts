//! A minimal HTTP/1.1 server and client built on `std::net`.
//!
//! The server is thread-per-connection with keep-alive support, which is
//! plenty for a development console and API. The client speaks plain HTTP
//! (TLS is out of scope for a dependency-free build; put a local TLS
//! terminating proxy in front of HTTPS webhooks if needed).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

const MAX_BODY: usize = 64 * 1024 * 1024;
const MAX_HEADER_LINE: usize = 64 * 1024;
const MAX_CONNECTIONS: usize = 512;

#[derive(Debug, Clone, Default)]
pub struct Request {
    pub method: String,
    /// Percent-decoded path, without the query string.
    pub path: String,
    pub raw_path: String,
    pub query: Vec<(String, String)>,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub remote: Option<SocketAddr>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    pub fn query_param(&self, name: &str) -> Option<&str> {
        self.query.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
    pub fn body_str(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn new(status: u16, content_type: &str, body: impl Into<Vec<u8>>) -> Self {
        Response { status, headers: vec![("Content-Type".into(), content_type.into())], body: body.into() }
    }
    pub fn json(status: u16, body: impl Into<String>) -> Self {
        Self::new(status, "application/json; charset=utf-8", body.into())
    }
    pub fn text(status: u16, body: impl Into<String>) -> Self {
        Self::new(status, "text/plain; charset=utf-8", body.into())
    }
    pub fn html(body: impl Into<String>) -> Self {
        Self::new(200, "text/html; charset=utf-8", body.into())
    }
    pub fn with_header(mut self, k: &str, v: &str) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }
    pub fn redirect(location: &str) -> Self {
        Response::text(302, "").with_header("Location", location)
    }
}

pub fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Unknown",
    }
}

pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let h = std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
                match h {
                    Some(b) => {
                        out.push(b);
                        i += 3;
                    }
                    None => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

pub fn parse_query(q: &str) -> Vec<(String, String)> {
    q.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (percent_decode(k), percent_decode(v)),
            None => (percent_decode(p), String::new()),
        })
        .collect()
}

fn read_line<R: BufRead>(r: &mut R) -> std::io::Result<Option<String>> {
    let mut buf = Vec::new();
    let n = r.by_ref().take(MAX_HEADER_LINE as u64).read_until(b'\n', &mut buf)?;
    if n == 0 {
        return Ok(None);
    }
    while buf.last().map_or(false, |b| *b == b'\n' || *b == b'\r') {
        buf.pop();
    }
    Ok(Some(String::from_utf8_lossy(&buf).into_owned()))
}

fn read_headers<R: BufRead>(r: &mut R) -> std::io::Result<Vec<(String, String)>> {
    let mut headers = Vec::new();
    loop {
        let line = read_line(r)?.unwrap_or_default();
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
        if headers.len() > 200 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "too many headers"));
        }
    }
    Ok(headers)
}

fn read_body<R: BufRead>(r: &mut R, headers: &[(String, String)]) -> std::io::Result<Vec<u8>> {
    let get = |n: &str| headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(n)).map(|(_, v)| v.clone());
    if get("Transfer-Encoding").map_or(false, |v| v.to_ascii_lowercase().contains("chunked")) {
        let mut body = Vec::new();
        loop {
            let line = read_line(r)?.unwrap_or_default();
            let size = usize::from_str_radix(line.split(';').next().unwrap_or("0").trim(), 16).unwrap_or(0);
            if size == 0 {
                // trailers
                while let Some(l) = read_line(r)? {
                    if l.is_empty() {
                        break;
                    }
                }
                break;
            }
            if body.len() + size > MAX_BODY {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "body too large"));
            }
            let mut chunk = vec![0; size];
            r.read_exact(&mut chunk)?;
            body.extend_from_slice(&chunk);
            read_line(r)?;
        }
        return Ok(body);
    }
    match get("Content-Length").and_then(|v| v.parse::<usize>().ok()) {
        Some(len) if len > MAX_BODY => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "body too large")),
        Some(len) => {
            let mut body = vec![0; len];
            r.read_exact(&mut body)?;
            Ok(body)
        }
        None => Ok(Vec::new()),
    }
}

fn write_response(stream: &mut TcpStream, resp: &Response, keep_alive: bool, head_only: bool) -> std::io::Result<()> {
    let mut out = format!("HTTP/1.1 {} {}\r\n", resp.status, reason(resp.status));
    for (k, v) in &resp.headers {
        out.push_str(&format!("{}: {}\r\n", k, v));
    }
    out.push_str(&format!("Content-Length: {}\r\n", resp.body.len()));
    out.push_str(if keep_alive { "Connection: keep-alive\r\n" } else { "Connection: close\r\n" });
    out.push_str("\r\n");
    stream.write_all(out.as_bytes())?;
    if !head_only {
        stream.write_all(&resp.body)?;
    }
    stream.flush()
}

pub type Handler = dyn Fn(&Request) -> Response + Send + Sync;

fn handle_conn(stream: TcpStream, handler: Arc<Handler>) {
    let remote = stream.peer_addr().ok();
    let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
    let _ = stream.set_nodelay(true);
    let mut writer = match stream.try_clone() {
        Ok(w) => w,
        Err(_) => return,
    };
    let mut reader = BufReader::new(stream);
    loop {
        let line = match read_line(&mut reader) {
            Ok(Some(l)) if !l.is_empty() => l,
            Ok(Some(_)) => continue,
            _ => return,
        };
        let mut parts = line.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let target = parts.next().unwrap_or("/").to_string();
        let version = parts.next().unwrap_or("HTTP/1.0").to_string();
        let headers = match read_headers(&mut reader) {
            Ok(h) => h,
            Err(_) => return,
        };
        let body = match read_body(&mut reader, &headers) {
            Ok(b) => b,
            Err(_) => {
                let _ = write_response(&mut writer, &Response::text(413, "request body too large"), false, false);
                return;
            }
        };
        let (raw_path, q) = match target.split_once('?') {
            Some((p, q)) => (p.to_string(), q.to_string()),
            None => (target.clone(), String::new()),
        };
        let conn_hdr = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("Connection")).map(|(_, v)| v.to_ascii_lowercase());
        let keep_alive = match conn_hdr.as_deref() {
            Some("close") => false,
            Some("keep-alive") => true,
            _ => version == "HTTP/1.1",
        };
        let req = Request {
            method: method.clone(),
            path: percent_decode(&raw_path.replace('+', "%2B")),
            raw_path,
            query: parse_query(&q),
            headers,
            body,
            remote,
        };
        let resp = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler(&req))) {
            Ok(r) => r,
            Err(_) => Response::json(500, r#"{"error":{"code":500,"message":"internal error","status":"INTERNAL"}}"#),
        };
        if write_response(&mut writer, &resp, keep_alive, method == "HEAD").is_err() || !keep_alive {
            return;
        }
    }
}

/// A running server handle.
pub struct Server {
    pub addr: SocketAddr,
    listener: TcpListener,
}

impl Server {
    pub fn bind(addr: &str) -> std::io::Result<Server> {
        let listener = TcpListener::bind(addr)?;
        Ok(Server { addr: listener.local_addr()?, listener })
    }

    /// Serves forever on the current thread.
    pub fn run(self, handler: Arc<Handler>) {
        let active = Arc::new(AtomicUsize::new(0));
        for stream in self.listener.incoming() {
            let Ok(stream) = stream else { continue };
            if active.load(Ordering::SeqCst) >= MAX_CONNECTIONS {
                let mut s = stream;
                let _ = write_response(&mut s, &Response::text(503, "server busy"), false, false);
                continue;
            }
            let h = handler.clone();
            let a = active.clone();
            a.fetch_add(1, Ordering::SeqCst);
            std::thread::spawn(move || {
                handle_conn(stream, h);
                a.fetch_sub(1, Ordering::SeqCst);
            });
        }
    }

    /// Serves on a background thread, returning the bound address.
    pub fn spawn(self, handler: Arc<Handler>) -> SocketAddr {
        let addr = self.addr;
        std::thread::spawn(move || self.run(handler));
        addr
    }
}

// ---- client -----------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ClientResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl ClientResponse {
    pub fn body_str(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Url {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub path: String,
}

pub fn parse_url(url: &str) -> Result<Url, String> {
    let (scheme, rest) = url.split_once("://").ok_or_else(|| format!("invalid URL: {}", url))?;
    let scheme = scheme.to_ascii_lowercase();
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], rest[i..].to_string()),
        None => (rest, "/".to_string()),
    };
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let default_port = if scheme == "https" { 443 } else { 80 };
    let (host, port) = if authority.starts_with('[') {
        let end = authority.find(']').ok_or("invalid IPv6 host")?;
        let host = authority[1..end].to_string();
        let port = authority[end + 1..].strip_prefix(':').and_then(|p| p.parse().ok()).unwrap_or(default_port);
        (host, port)
    } else {
        match authority.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), p.parse().map_err(|_| "invalid port")?),
            None => (authority.to_string(), default_port),
        }
    };
    if host.is_empty() {
        return Err("missing host".into());
    }
    Ok(Url { scheme, host, port, path })
}

/// Performs a blocking HTTP request.
pub fn request(method: &str, url: &str, headers: &[(String, String)], body: &[u8], timeout: Duration) -> Result<ClientResponse, String> {
    let u = parse_url(url)?;
    if u.scheme != "http" {
        return Err(format!(
            "unsupported scheme '{}': this dependency-free build speaks plain HTTP only (use a local TLS proxy for HTTPS endpoints)",
            u.scheme
        ));
    }
    let addrs: Vec<SocketAddr> = (u.host.as_str(), u.port).to_socket_addrs().map_err(|e| format!("resolve {}: {}", u.host, e))?.collect();
    let addr = addrs.first().ok_or_else(|| format!("no address for {}", u.host))?;
    let mut stream = TcpStream::connect_timeout(addr, timeout).map_err(|e| format!("connect {}: {}", url, e))?;
    stream.set_read_timeout(Some(timeout)).ok();
    stream.set_write_timeout(Some(timeout)).ok();
    let host_hdr = if u.port == 80 { u.host.clone() } else { format!("{}:{}", u.host, u.port) };
    let mut req = format!("{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Length: {}\r\n", method, u.path, host_hdr, body.len());
    for (k, v) in headers {
        req.push_str(&format!("{}: {}\r\n", k, v));
    }
    if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("user-agent")) {
        req.push_str("User-Agent: cx-agents/0.1\r\n");
    }
    req.push_str("\r\n");
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    stream.write_all(body).map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream);
    let status_line = read_line(&mut reader).map_err(|e| e.to_string())?.ok_or("empty response")?;
    let status: u16 = status_line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).ok_or("invalid status line")?;
    let rheaders = read_headers(&mut reader).map_err(|e| e.to_string())?;
    let has_len = rheaders.iter().any(|(k, _)| k.eq_ignore_ascii_case("content-length") || k.eq_ignore_ascii_case("transfer-encoding"));
    let body = if has_len {
        read_body(&mut reader, &rheaders).map_err(|e| e.to_string())?
    } else {
        let mut b = Vec::new();
        reader.take(MAX_BODY as u64).read_to_end(&mut b).map_err(|e| e.to_string())?;
        b
    };
    Ok(ClientResponse { status, headers: rheaders, body })
}

pub fn post_json(url: &str, json: &str, extra_headers: &[(String, String)], timeout: Duration) -> Result<ClientResponse, String> {
    let mut h = vec![("Content-Type".to_string(), "application/json".to_string())];
    h.extend_from_slice(extra_headers);
    request("POST", url, &h, json.as_bytes(), timeout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_parsing() {
        let u = parse_url("http://localhost:8080/a/b?x=1").unwrap();
        assert_eq!((u.host.as_str(), u.port, u.path.as_str()), ("localhost", 8080, "/a/b?x=1"));
        assert_eq!(parse_url("https://example.com").unwrap().port, 443);
        assert_eq!(percent_decode("a%20b+c"), "a b c");
        assert_eq!(parse_query("a=1&b=x%2Fy"), vec![("a".into(), "1".into()), ("b".into(), "x/y".into())]);
    }

    #[test]
    fn roundtrip() {
        let server = Server::bind("127.0.0.1:0").unwrap();
        let addr = server.spawn(Arc::new(|r: &Request| Response::json(200, format!("{{\"m\":\"{}\",\"p\":\"{}\",\"b\":{}}}", r.method, r.path, r.body_str()))));
        let resp = post_json(&format!("http://{}/hello%20world", addr), "[1]", &[], Duration::from_secs(5)).unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body_str(), r#"{"m":"POST","p":"/hello world","b":[1]}"#);
    }
}
