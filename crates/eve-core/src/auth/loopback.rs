//! A one-shot loopback HTTP server that captures the EVE SSO redirect.
//!
//! Step 3 of the PKCE flow (see [`super`]): after the user authorizes in their
//! system browser, EVE SSO redirects to our registered loopback URI
//! (`http://127.0.0.1:<port>/callback?code=...&state=...`). This server binds
//! that port, accepts the single redirect request, parses `code` + `state`,
//! shows the user a "you can close this tab" page, and hands the values back to
//! [`LoginManager::complete`](super::flow::LoginManager::complete) — which is
//! where the authoritative CSRF `state` check happens.
//!
//! Deliberately dependency-free (`std::net` only, no async runtime, no WebView)
//! so it compiles and is integration-tested in headless environments. The
//! desktop shell runs [`wait_for_callback`](LoopbackServer::wait_for_callback)
//! on a blocking task.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

use url::Url;

use crate::error::{Error, Result};

/// The values extracted from the OAuth redirect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthCallback {
    pub code: String,
    pub state: String,
}

/// A bound loopback listener awaiting exactly one OAuth redirect.
pub struct LoopbackServer {
    listener: TcpListener,
}

impl LoopbackServer {
    /// Bind `127.0.0.1:<port>`. Pass `0` to let the OS pick a free port (handy
    /// for tests; production uses the fixed registered port).
    pub fn bind(port: u16) -> Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
        Ok(Self { listener })
    }

    /// The actual bound address (resolves the real port when bound with `0`).
    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }

    /// The bound port.
    pub fn port(&self) -> Result<u16> {
        Ok(self.local_addr()?.port())
    }

    /// The loopback redirect URI to register / send to SSO, e.g.
    /// `http://127.0.0.1:8787/callback`.
    pub fn redirect_uri(&self, path: &str) -> Result<String> {
        Ok(format!("http://127.0.0.1:{}{}", self.port()?, path))
    }

    /// Block until the browser delivers the redirect, then return its
    /// `code` + `state`. Responds to the browser with a success page (or an
    /// error page if SSO reported `error=...`).
    pub fn wait_for_callback(&self) -> Result<OAuthCallback> {
        let (stream, _) = self.listener.accept()?;
        self.handle(stream)
    }

    /// Like [`wait_for_callback`](Self::wait_for_callback) but gives up after
    /// `timeout` (e.g. the user closed the browser) instead of blocking forever.
    pub fn wait_for_callback_timeout(&self, timeout: Duration) -> Result<OAuthCallback> {
        self.listener.set_nonblocking(true)?;
        let deadline = Instant::now() + timeout;
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false)?;
                    return self.handle(stream);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(Error::Auth("loopback callback timed out".into()));
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// Parse one accepted connection and write the browser response.
    fn handle(&self, stream: TcpStream) -> Result<OAuthCallback> {
        let request_line = read_request_line(&stream)?;
        let target = request_target(&request_line)
            .ok_or_else(|| Error::Auth("malformed redirect request".into()))?;

        match parse_callback_query(target) {
            Ok(cb) => {
                respond(&stream, "200 OK", SUCCESS_PAGE)?;
                Ok(cb)
            }
            Err(e) => {
                respond(&stream, "200 OK", &error_page(&e.to_string()))?;
                Err(e)
            }
        }
    }
}

/// Read just the HTTP request line (the first line) — everything we need is in
/// it. Bounded by a read timeout so a half-open socket can't hang us.
fn read_request_line(stream: &TcpStream) -> Result<String> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(line)
}

/// Extract the request target from a request line:
/// `"GET /callback?code=abc&state=xyz HTTP/1.1"` → `"/callback?code=abc&state=xyz"`.
fn request_target(request_line: &str) -> Option<&str> {
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?;
    if !method.eq_ignore_ascii_case("GET") {
        return None;
    }
    parts.next()
}

/// Parse `code` + `state` out of the redirect target's query string. Returns an
/// auth error if SSO reported `error=...`, or if `code`/`state` are absent.
fn parse_callback_query(target: &str) -> Result<OAuthCallback> {
    // Resolve against a dummy base so a path-only target parses; `url` handles
    // percent-decoding of the query pairs.
    let url = Url::parse("http://127.0.0.1")
        .and_then(|base| base.join(target))
        .map_err(|e| Error::Auth(format!("bad redirect URL: {e}")))?;

    let mut code = None;
    let mut state = None;
    let mut oauth_error = None;
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "code" => code = Some(v.into_owned()),
            "state" => state = Some(v.into_owned()),
            "error" => oauth_error = Some(v.into_owned()),
            _ => {}
        }
    }

    if let Some(err) = oauth_error {
        return Err(Error::Auth(format!("authorization denied: {err}")));
    }
    match (code, state) {
        (Some(code), Some(state)) => Ok(OAuthCallback { code, state }),
        (None, _) => Err(Error::Auth("callback missing authorization code".into())),
        (_, None) => Err(Error::Auth("callback missing state".into())),
    }
}

/// Write a minimal HTTP/1.1 response and close the connection.
fn respond(stream: &TcpStream, status: &str, body: &str) -> Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {len}\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        len = body.len()
    );
    let mut w: &TcpStream = stream;
    w.write_all(response.as_bytes())?;
    w.flush()?;
    Ok(())
}

const SUCCESS_PAGE: &str = r#"<!doctype html><html><head><meta charset="utf-8">
<title>EVE Commander — signed in</title></head>
<body style="background:#0b0f17;color:#e6edf3;font-family:system-ui,sans-serif;display:grid;place-items:center;height:100vh;margin:0">
<div style="text-align:center">
<h1 style="color:#6cb6ff">Signed in</h1>
<p>You can close this tab and return to EVE Commander.</p>
</div></body></html>"#;

fn error_page(message: &str) -> String {
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8">
<title>EVE Commander — sign-in failed</title></head>
<body style="background:#0b0f17;color:#e6edf3;font-family:system-ui,sans-serif;display:grid;place-items:center;height:100vh;margin:0">
<div style="text-align:center">
<h1 style="color:#ff6b6b">Sign-in failed</h1>
<p>{message}</p>
<p style="color:#8b949e">You can close this tab and try again in EVE Commander.</p>
</div></body></html>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn request_target_extracts_path_and_query() {
        let line = "GET /callback?code=abc&state=xyz HTTP/1.1\r\n";
        assert_eq!(
            request_target(line),
            Some("/callback?code=abc&state=xyz")
        );
    }

    #[test]
    fn request_target_rejects_non_get() {
        assert_eq!(request_target("POST /callback HTTP/1.1"), None);
    }

    #[test]
    fn parse_query_decodes_code_and_state() {
        let cb = parse_callback_query("/callback?code=a%2Bb%2Fc&state=s%20t").unwrap();
        assert_eq!(cb.code, "a+b/c"); // percent-decoded
        assert_eq!(cb.state, "s t");
    }

    #[test]
    fn parse_query_surfaces_oauth_error() {
        let err = parse_callback_query("/callback?error=access_denied").unwrap_err();
        assert!(matches!(err, Error::Auth(m) if m.contains("access_denied")));
    }

    #[test]
    fn parse_query_requires_code_and_state() {
        assert!(parse_callback_query("/callback?state=only").is_err());
        assert!(parse_callback_query("/callback?code=only").is_err());
    }

    #[test]
    fn redirect_uri_uses_bound_port() {
        let server = LoopbackServer::bind(0).unwrap();
        let uri = server.redirect_uri("/callback").unwrap();
        assert!(uri.starts_with("http://127.0.0.1:"));
        assert!(uri.ends_with("/callback"));
    }

    /// Full socket round-trip: a simulated browser hits the redirect URL and the
    /// server returns the parsed code/state plus a success page.
    #[test]
    fn captures_code_and_state_over_real_socket() {
        let server = LoopbackServer::bind(0).unwrap();
        let port = server.port().unwrap();

        let client = std::thread::spawn(move || {
            let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
            s.write_all(
                b"GET /callback?code=abc123&state=st-42 HTTP/1.1\r\nHost: localhost\r\n\r\n",
            )
            .unwrap();
            let mut buf = String::new();
            s.read_to_string(&mut buf).unwrap();
            buf
        });

        let cb = server.wait_for_callback().unwrap();
        assert_eq!(cb.code, "abc123");
        assert_eq!(cb.state, "st-42");

        let response = client.join().unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("Signed in"));
    }

    /// An `error=` redirect yields an auth error and still serves an error page.
    #[test]
    fn oauth_error_redirect_returns_error_and_page() {
        let server = LoopbackServer::bind(0).unwrap();
        let port = server.port().unwrap();

        let client = std::thread::spawn(move || {
            let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
            s.write_all(b"GET /callback?error=access_denied HTTP/1.1\r\n\r\n")
                .unwrap();
            let mut buf = String::new();
            s.read_to_string(&mut buf).unwrap();
            buf
        });

        let err = server.wait_for_callback().unwrap_err();
        assert!(matches!(err, Error::Auth(_)));

        let response = client.join().unwrap();
        assert!(response.contains("Sign-in failed"));
    }

    #[test]
    fn timeout_when_no_browser_connects() {
        let server = LoopbackServer::bind(0).unwrap();
        let err = server
            .wait_for_callback_timeout(Duration::from_millis(120))
            .unwrap_err();
        assert!(matches!(err, Error::Auth(m) if m.contains("timed out")));
    }
}
