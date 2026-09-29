//! Shared loopback OAuth callback server.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, oneshot, watch};

pub type OAuthCompletion<T> =
    Arc<dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<T, String>> + Send>> + Send + Sync>;
type CallbackResult<T> = Result<Option<T>, String>;
type CallbackReceiver<T> = oneshot::Receiver<CallbackResult<T>>;

pub struct OAuthCallbackOptions<T> {
    pub provider_name: String,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub redirect_host: Option<String>,
    pub state: Option<String>,
    pub complete: OAuthCompletion<T>,
    pub cancel: Option<watch::Receiver<bool>>,
    pub timeout_ms: Option<u64>,
}

pub struct OAuthCallbackServer<T> {
    pub redirect_uri: String,
    result: Mutex<Option<CallbackReceiver<T>>>,
    cancel: watch::Sender<bool>,
}

impl<T> OAuthCallbackServer<T> {
    pub async fn wait(&self) -> Result<Option<T>, String> {
        let receiver = self
            .result
            .lock()
            .await
            .take()
            .ok_or_else(|| "OAuth callback wait already consumed".to_string())?;
        receiver
            .await
            .map_err(|_| "OAuth callback server closed".to_string())?
    }

    pub fn cancel(&self) {
        let _ = self.cancel.send(true);
    }

    pub fn close(&self) {
        let _ = self.cancel.send(true);
    }
}

fn html(message: &str) -> String {
    format!(
        "<!doctype html><html><body><h1>{}</h1></body></html>",
        message.replace('&', "&amp;").replace('<', "&lt;")
    )
}

async fn send(stream: &mut TcpStream, status: u16, message: &str) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        409 => "Conflict",
        502 => "Bad Gateway",
        _ => "Internal Server Error",
    };
    let body = html(message);
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\ncontent-type: text/html; charset=utf-8\r\ncache-control: no-store\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

async fn request_url(stream: &mut TcpStream) -> Result<(String, String), String> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 2048];
    loop {
        let read = stream
            .read(&mut buffer)
            .await
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") || bytes.len() > 16 * 1024 {
            break;
        }
    }
    let request = String::from_utf8_lossy(&bytes);
    let line = request
        .lines()
        .next()
        .ok_or_else(|| "invalid callback request".to_string())?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default().to_string();
    Ok((method, target))
}

/// Start a loopback callback server on the requested address.
pub async fn start_oauth_callback_server<T: Send + 'static>(
    options: OAuthCallbackOptions<T>,
) -> Result<OAuthCallbackServer<T>, String> {
    if options
        .cancel
        .as_ref()
        .is_some_and(|cancel| *cancel.borrow())
    {
        return Err("Login cancelled".into());
    }
    let listener = TcpListener::bind((options.host.as_str(), options.port))
        .await
        .map_err(|error| error.to_string())?;
    let address = listener.local_addr().map_err(|error| error.to_string())?;
    let redirect_host = options.redirect_host.as_deref().unwrap_or(&options.host);
    let rendered_host = if redirect_host.contains(':') {
        format!("[{redirect_host}]")
    } else {
        redirect_host.to_string()
    };
    let redirect_uri = format!("http://{rendered_host}:{}{}", address.port(), options.path);
    let (cancel_tx, mut cancel_rx) = watch::channel(false);
    let (result_tx, result_rx) = oneshot::channel();
    let provider_name = options.provider_name;
    let expected_path = options.path;
    let expected_state = options.state;
    let completion = options.complete;
    let timeout_ms = options.timeout_ms;
    let mut external_cancel = options.cancel;
    tokio::spawn(async move {
        let mut result_tx = Some(result_tx);
        let timeout = async move {
            match timeout_ms {
                Some(ms) => tokio::time::sleep(std::time::Duration::from_millis(ms)).await,
                None => std::future::pending::<()>().await,
            }
        };
        tokio::pin!(timeout);
        loop {
            tokio::select! {
                _ = cancel_rx.changed() => {
                    if *cancel_rx.borrow() && let Some(sender) = result_tx.take() {
                        let _ = sender.send(Ok(None));
                    }
                    return;
                }
                _ = async {
                    match external_cancel.as_mut() {
                        Some(cancel) => { let _ = cancel.changed().await; },
                        None => std::future::pending::<()>().await,
                    }
                } => {
                    if let Some(sender) = result_tx.take() {
                        let _ = sender.send(Err("Login cancelled".into()));
                    }
                    return;
                }
                _ = &mut timeout => {
                    if let Some(sender) = result_tx.take() {
                        let _ = sender.send(Err(format!("{provider_name} sign-in timed out")));
                    }
                    return;
                }
                accepted = listener.accept() => {
                    let Ok((mut stream, _)) = accepted else {
                        if let Some(sender) = result_tx.take() {
                            let _ = sender.send(Err("OAuth callback server closed".into()));
                        }
                        return;
                    };
                    let Ok((method, target)) = request_url(&mut stream).await else {
                        send(&mut stream, 400, "Invalid callback request.").await;
                        continue;
                    };
                    let Ok(url) = url::Url::parse(&format!("http://localhost{target}")) else {
                        send(&mut stream, 400, "Invalid callback URL.").await;
                        continue;
                    };
                    if method != "GET" || url.path() != expected_path {
                        send(&mut stream, 404, "Callback route not found.").await;
                        continue;
                    }
                    if expected_state.as_deref().is_some_and(|state| url.query_pairs().find(|(key, _)| key == "state").map(|(_, value)| value != state).unwrap_or(true)) {
                        send(&mut stream, 400, "State mismatch.").await;
                        continue;
                    }
                    if result_tx.is_none() {
                        send(&mut stream, 409, "This sign-in has already been handled.").await;
                        continue;
                    }
                    if let Some(error) = url.query_pairs().find(|(key, _)| key == "error").map(|(_, value)| value.into_owned()) {
                        let description = url.query_pairs().find(|(key, _)| key == "error_description").map(|(_, value)| value.into_owned()).unwrap_or(error);
                        send(&mut stream, 400, &format!("{provider_name} authorization failed: {description}")).await;
                        if let Some(sender) = result_tx.take() {
                            let _ = sender.send(Err(format!("{provider_name} authorization failed: {description}")));
                        }
                        return;
                    }
                    let code = url.query_pairs().find(|(key, _)| key == "code").map(|(_, value)| value.into_owned());
                    let Some(code) = code else {
                        send(&mut stream, 400, "Missing authorization code.").await;
                        continue;
                    };
                    let sender = result_tx.take().unwrap();
                    match completion(code).await {
                        Ok(value) => {
                            send(&mut stream, 200, &format!("Authentication successful. Signed in to {provider_name}. You may now close this page.")).await;
                            let _ = sender.send(Ok(Some(value)));
                        }
                        Err(error) => {
                            send(&mut stream, 502, &format!("{provider_name} sign-in failed. {error}")).await;
                            let _ = sender.send(Err(error));
                        }
                    }
                    return;
                }
            }
        }
    });
    Ok(OAuthCallbackServer {
        redirect_uri,
        result: Mutex::new(Some(result_rx)),
        cancel: cancel_tx,
    })
}
