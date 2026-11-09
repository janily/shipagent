//! Native half of mkagent v0.2.6 preload/bootstrap.ts startChatGptOAuth.
use async_net::{TcpListener, TcpStream};
use secrecy::SecretString;
use smol::io::{AsyncReadExt, AsyncWriteExt};
use std::time::Duration;
/// Sanitized failures; authorization material never appears in diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LoginError {
    /// Another application owns the registered callback port.
    #[error("oauth callback port unavailable")]
    Occupied,
    /// The user cancelled or the owning window was closed.
    #[error("oauth cancelled")]
    Cancelled,
    /// No valid callback arrived in five minutes.
    #[error("oauth timed out")]
    Timeout,
    /// Provider denied authorization.
    #[error("oauth denied")]
    Denied,
    /// A backend operation could not be confirmed.
    #[error("oauth backend failed")]
    Backend,
    /// Browser launch failed or the authorization URL was invalid.
    #[error("oauth browser unavailable")]
    Browser,
}
/// An exclusively owned loopback listener, acquired before starting OAuth.
pub struct CallbackServer(TcpListener);
impl CallbackServer {
    /// Bind the provider-registered IPv4 loopback port; fail before creating a flow.
    pub async fn bind() -> Result<Self, LoginError> {
        TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 1455))
            .await
            .map(Self)
            .map_err(|_| LoginError::Occupied)
    }
    /// Await one state-validated callback, cancellation, or the five-minute deadline.
    pub async fn receive(
        self,
        state: &str,
        cancel: async_channel::Receiver<()>,
        page: String,
    ) -> Result<SecretString, LoginError> {
        wait_callback(self.0, state, cancel, Duration::from_secs(300), &page).await
    }
}
fn callback(target: &str, expected: &str) -> Option<Result<SecretString, LoginError>> {
    let u = url::Url::parse(&format!("http://localhost{target}")).ok()?;
    if u.path() != "/auth/callback" || !target.starts_with("/auth/callback?") {
        return None;
    }
    let p: Vec<_> = u.query_pairs().collect();
    for key in ["state", "code", "error"] {
        if p.iter().filter(|(k, _)| k == key).count() > 1 {
            return None;
        }
    }
    if !p.iter().any(|(k, v)| k == "state" && v == expected) {
        return None;
    }
    if p.iter().any(|(k, _)| k == "error") {
        return Some(Err(LoginError::Denied));
    }
    p.iter()
        .find(|(k, v)| k == "code" && !v.is_empty())
        .map(|(_, v)| Ok(v.to_string().into()))
}
async fn request(
    mut stream: TcpStream,
    expected: &str,
    port: u16,
    page: &str,
) -> Option<Result<SecretString, LoginError>> {
    let mut bytes = Vec::new();
    let mut buf = [0; 1024];
    while bytes.len() < 8192 {
        let n = stream.read(&mut buf).await.ok()?;
        if n == 0 {
            return None;
        }
        bytes.extend_from_slice(&buf[..n]);
        if bytes.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    if bytes.len() >= 8192 {
        return None;
    }
    let raw = std::str::from_utf8(&bytes).ok()?;
    let mut lines = raw.split("\r\n");
    let mut first = lines.next()?.split_whitespace();
    let method = first.next()?;
    let target = first.next()?;
    let version = first.next()?;
    let hosts: Vec<_> = lines
        .filter_map(|l| l.split_once(':'))
        .filter(|(k, _)| k.eq_ignore_ascii_case("host"))
        .map(|(_, v)| v.trim())
        .collect();
    let valid = method == "GET"
        && version == "HTTP/1.1"
        && hosts.len() == 1
        && (hosts[0] == format!("localhost:{port}") || hosts[0] == format!("127.0.0.1:{port}"));
    let result = valid.then(|| callback(target, expected)).flatten();
    let safe = page
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let body = if result.is_some() {
        safe.as_str()
    } else {
        "Invalid callback"
    };
    let status = if result.is_some() {
        "200 OK"
    } else {
        "400 Bad Request"
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'none'\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    result
}
async fn wait_callback(
    listener: TcpListener,
    expected: &str,
    cancel: async_channel::Receiver<()>,
    timeout: Duration,
    page: &str,
) -> Result<SecretString, LoginError> {
    let receive = async {
        loop {
            let (stream, _) = listener.accept().await.map_err(|_| LoginError::Backend)?;
            let result = smol::future::race(
                request(
                    stream,
                    expected,
                    listener
                        .local_addr()
                        .map_err(|_| LoginError::Backend)?
                        .port(),
                    page,
                ),
                async {
                    smol::Timer::after(Duration::from_secs(2)).await;
                    None
                },
            )
            .await;
            if let Some(result) = result {
                return result;
            }
        }
    };
    smol::future::race(
        receive,
        smol::future::race(
            async {
                let _ = cancel.recv().await;
                Err(LoginError::Cancelled)
            },
            async {
                smol::Timer::after(timeout).await;
                Err(LoginError::Timeout)
            },
        ),
    )
    .await
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn callback_requires_actual_unique_state_and_code() {
        assert!(callback("/auth/callback?code=private&state=wrong", "expected").is_none());
        assert!(
            callback(
                "/auth/callback?code=x&state=expected&state=expected",
                "expected"
            )
            .is_none()
        );
        assert!(callback("/other?code=x&state=expected", "expected").is_none());
        assert!(matches!(
            callback("/auth/callback?error=denied&state=expected", "expected"),
            Some(Err(LoginError::Denied))
        ));
        assert!(
            callback("/auth/callback?code=x&state=expected", "expected").is_some_and(|r| r.is_ok())
        );
    }
    #[test]
    fn listener_cancel_timeout_and_actual_http() {
        smol::block_on(async {
            let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
                .await
                .unwrap();
            let port = listener.local_addr().unwrap().port();
            let (tx, rx) = async_channel::bounded(1);
            let worker = smol::spawn(async move {
                wait_callback(
                    listener,
                    "fixture-state",
                    rx,
                    Duration::from_secs(2),
                    "Received",
                )
                .await
            });
            let mut stream = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
                .await
                .unwrap();
            stream.write_all(format!("GET /auth/callback?state=fixture-state&code=private-code HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n").as_bytes()).await.unwrap();
            let mut body = String::new();
            stream.read_to_string(&mut body).await.unwrap();
            assert!(!body.contains("private-code"));
            assert!(worker.await.is_ok());
            drop(tx);
            let l = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
                .await
                .unwrap();
            let (tx, rx) = async_channel::bounded(1);
            tx.send(()).await.unwrap();
            assert!(matches!(
                wait_callback(l, "state", rx, Duration::from_secs(1), "").await,
                Err(LoginError::Cancelled)
            ));
            let l = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
                .await
                .unwrap();
            let (_tx, rx) = async_channel::bounded(1);
            assert!(matches!(
                wait_callback(l, "state", rx, Duration::from_millis(10), "").await,
                Err(LoginError::Timeout)
            ));
        });
    }
}
