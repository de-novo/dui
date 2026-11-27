//! A renderer that starts no browser, for maps without capture (docs/en/web-viewer.md#shared-screen-maps,
//! AGENTS.md#never). A visit reads the HTML the app's server sends and the links in it; nothing is
//! drawn and no script runs, so the cell stays `not-rendered`.

use crate::ports::Renderer;
use serde_json::{Value, json};
use std::{
    any::Any,
    io::{Read, Write},
    net::TcpStream,
    path::Path,
    time::Duration,
};

/// What the server is: started on the first visit and kept for the run. `None` means the app has
/// no HTML of its own to read (a Vite app's links come from its router).
pub(crate) type Start<'a> = Box<dyn FnMut() -> Result<Option<(u16, Box<dyn Any>)>, String> + 'a>;

pub(crate) struct HtmlLinks<'a> {
    start: Start<'a>,
    /// The server's port, and what keeps it running.
    host: Option<Option<(u16, Box<dyn Any>)>>,
}

impl<'a> HtmlLinks<'a> {
    pub(crate) fn new(start: Start<'a>) -> Self {
        Self { start, host: None }
    }

    /// Whether a manifest asks for a map without capture.
    pub(crate) fn wanted(manifest: &Path) -> bool {
        std::fs::read(manifest)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .is_some_and(|spec| spec["capture"] == false)
    }
}

impl Renderer for HtmlLinks<'_> {
    fn capture(
        &mut self,
        spec: &Value,
        _input: &Value,
        _width: u32,
        _image: &Path,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let started = std::time::Instant::now();
        if self.host.is_none() {
            self.host = Some((self.start)()?);
        }
        let path = spec["path"].as_str().unwrap_or("/");
        let Some(Some((port, _))) = &self.host else {
            return Ok(
                json!({"ready": true, "path": path, "links": [], "renderer": "html",
                "timing": {"totalMs": 0}}),
            );
        };
        let (reached, html) = fetch(*port, path)?;
        let origin = format!("http://127.0.0.1:{port}");
        let page = dui_core::html_links::read(&html, &reached, &origin);
        let mut observation = json!({
            "ready": true,
            "path": reached,
            "links": page.links,
            "renderer": "html",
            "timing": {"totalMs": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)},
        });
        if let Some(title) = page.title {
            observation["title"] = json!(title);
        }
        if reached != path {
            observation["redirectedTo"] = json!(reached);
        }
        Ok(observation)
    }
}

/// `GET path` on the loopback server at `port`, following same-origin redirects: the path the
/// page was served at and its HTML.
fn fetch(port: u16, path: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    let mut path = path.to_owned();
    for _ in 0..5 {
        let (status, headers, body) = get(port, &path)?;
        if (300..400).contains(&status)
            && let Some(location) = header(&headers, "location")
        {
            let origin = format!("http://127.0.0.1:{port}");
            let localhost = format!("http://localhost:{port}");
            let next = location
                .strip_prefix(&origin)
                .or_else(|| location.strip_prefix(&localhost))
                .unwrap_or(location);
            if !next.starts_with('/') {
                return Err(format!("{path} redirects to another origin: {location}").into());
            }
            path = next.to_owned();
            continue;
        }
        if status >= 400 {
            return Err(format!("{path} answered {status}").into());
        }
        return Ok((path, String::from_utf8_lossy(&body).into_owned()));
    }
    Err(format!("{path} redirects more than five times").into())
}

fn header<'h>(headers: &'h [(String, String)], name: &str) -> Option<&'h str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

type Headers = Vec<(String, String)>;
type Response = (u16, Headers, Vec<u8>);

/// One request, tried again when the server resets the connection.
fn get(port: u16, path: &str) -> Result<Response, Box<dyn std::error::Error>> {
    let mut attempt = 0;
    loop {
        match get_once(port, path) {
            Err(e)
                if attempt < 3
                    && e.downcast_ref::<std::io::Error>().is_some_and(|e| {
                        matches!(
                            e.kind(),
                            std::io::ErrorKind::ConnectionReset
                                | std::io::ErrorKind::ConnectionAborted
                        )
                    }) =>
            {
                attempt += 1;
            }
            result => return result,
        }
    }
}

fn get_once(port: u16, path: &str) -> Result<Response, Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: text/html,*/*\r\nAccept-Encoding: identity\r\nUser-Agent: dui\r\nConnection: close\r\n\r\n"
    )?;
    // Read until the response is complete, not until the server closes: a server may keep the
    // connection open despite `Connection: close`.
    let mut raw = Vec::new();
    let mut buf = [0u8; 16 * 1024];
    // Where the body starts, the status and the headers, once the head has arrived.
    let mut parsed: Option<(usize, u16, Headers)> = None;
    loop {
        if parsed.is_none()
            && let Some(split) = raw.windows(4).position(|w| w == b"\r\n\r\n")
        {
            let head = String::from_utf8_lossy(&raw[..split]).into_owned();
            let mut lines = head.split("\r\n");
            let status: u16 = lines
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|code| code.parse().ok())
                .ok_or("invalid HTTP status line")?;
            let headers: Vec<(String, String)> = lines
                .filter_map(|line| line.split_once(':'))
                .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
                .collect();
            parsed = Some((split + 4, status, headers));
        }
        if let Some((start, status, headers)) = &parsed {
            let body = &raw[*start..];
            let chunked = header(headers, "transfer-encoding")
                .is_some_and(|v| v.eq_ignore_ascii_case("chunked"));
            let length = header(headers, "content-length").and_then(|v| v.parse::<usize>().ok());
            let complete = if chunked {
                body.ends_with(b"0\r\n\r\n")
            } else if let Some(length) = length {
                body.len() >= length
            } else {
                (100..200).contains(status) || *status == 204 || *status == 304
            };
            if complete {
                break;
            }
        }
        let n = stream.read(&mut buf)?;
        if n == 0 {
            break;
        }
        raw.extend_from_slice(&buf[..n]);
    }
    let (start, status, headers) = parsed.ok_or("incomplete HTTP response")?;
    let mut body = raw[start..].to_vec();
    if header(&headers, "transfer-encoding").is_some_and(|v| v.eq_ignore_ascii_case("chunked")) {
        body = dechunk(&body)?;
    } else if let Some(length) = header(&headers, "content-length").and_then(|v| v.parse().ok()) {
        body.truncate(length);
    }
    Ok((status, headers, body))
}

fn dechunk(mut body: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut out = Vec::new();
    loop {
        let line = body
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or("invalid chunk")?;
        let size_text = String::from_utf8_lossy(&body[..line]);
        let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("").trim(), 16)?;
        body = &body[line + 2..];
        if size == 0 {
            return Ok(out);
        }
        out.extend_from_slice(body.get(..size).ok_or("truncated chunk")?);
        body = body.get(size + 2..).unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// A server that answers each request with the next of `responses`.
    fn serve(responses: Vec<&'static str>) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            for response in responses {
                let (mut stream, _) = listener.accept().expect("accept");
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                stream.write_all(response.as_bytes()).expect("write");
            }
        });
        port
    }

    #[test]
    fn follows_a_redirect_and_reads_a_chunked_page() {
        let port = serve(vec![
            "HTTP/1.1 301 Moved\r\nLocation: /docs/\r\nContent-Length: 0\r\n\r\n",
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n10\r\n<a href=\"cli\">x<\r\n5\r\n/a>ok\r\n0\r\n\r\n",
        ]);
        let mut links = HtmlLinks::new(Box::new(move || Ok(Some((port, Box::new(()) as _)))));
        let obs = links
            .capture(
                &json!({"path": "/docs"}),
                &json!({}),
                1280,
                Path::new("x.png"),
            )
            .expect("visit");
        assert_eq!(obs["path"], "/docs/");
        assert_eq!(obs["redirectedTo"], "/docs/");
        assert_eq!(obs["links"], json!(["/docs/cli"]));
        assert_eq!(obs["renderer"], "html");
    }

    #[test]
    fn a_server_that_keeps_the_connection_open_still_answers() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let body = "<a href=\"/b\">b</a>";
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            )
            .expect("write");
            std::thread::sleep(Duration::from_secs(60));
        });
        let started = std::time::Instant::now();
        let (path, html) = fetch(port, "/").expect("page");
        assert_eq!(
            (path.as_str(), html.as_str()),
            ("/", "<a href=\"/b\">b</a>")
        );
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn an_app_without_html_of_its_own_has_no_links() {
        let mut links = HtmlLinks::new(Box::new(|| Ok(None)));
        let obs = links
            .capture(
                &json!({"path": "/orders"}),
                &json!({}),
                375,
                Path::new("x.png"),
            )
            .expect("visit");
        assert_eq!(obs["links"], json!([]));
        assert_eq!(obs["ready"], true);
    }
}
