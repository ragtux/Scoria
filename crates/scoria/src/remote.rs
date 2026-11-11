//! The HTTP client gpui's `img()` fetches a remote image through.
//!
//! Obsidian requests every `http:` and `https:` image destination in
//! rendered markdown with no prompt, no allowlist and no setting, and sends
//! no `Referer`, `Origin` or `Cookie` with it
//! ([[Specification#Observed remote image behavior, Obsidian 1.14.4 (installer build 1.13.7)]]).
//! This client does the same: a plain `GET` with an image `Accept`, no
//! cookie store, redirects followed. gpui caches what it loads for the
//! session, so a URL drawn twice is fetched once.
//!
//! ureq is blocking, so each request runs on a thread of its own and hands
//! its answer back over a oneshot; the image loader awaits that on gpui's
//! background executor, never on the UI thread.

use futures::future::BoxFuture;
use http_client::http::{HeaderValue, Request, Response};
use http_client::{AsyncBody, HttpClient, Url};
use std::time::Duration;

/// What an image request accepts: Chromium's own list for `<img>`, which is
/// what Obsidian's requests carry.
const ACCEPT: &str = "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8";

/// The largest body read. A remote image bigger than this is a failed load,
/// drawn as its alt text, rather than an unbounded allocation.
const MAX_BODY: u64 = 64 * 1024 * 1024;

pub struct RemoteClient {
    agent: ureq::Agent,
    user_agent: HeaderValue,
}

impl RemoteClient {
    pub fn new() -> RemoteClient {
        let user_agent = format!("Scoria/{}", env!("CARGO_PKG_VERSION"));
        let agent = ureq::Agent::config_builder()
            // A 404 is an answer with a status, not a transport error: the
            // loader reads the status and fails the image itself.
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(60)))
            .user_agent(user_agent.as_str())
            .build()
            .into();
        RemoteClient {
            agent,
            user_agent: HeaderValue::from_str(&user_agent).expect("an ASCII user agent"),
        }
    }
}

impl HttpClient for RemoteClient {
    fn user_agent(&self) -> Option<&HeaderValue> {
        Some(&self.user_agent)
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    fn send(
        &self,
        req: Request<AsyncBody>,
    ) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        let agent = self.agent.clone();
        let uri = req.uri().to_string();
        let method = req.method().clone();
        let (tx, rx) = futures::channel::oneshot::channel();
        let spawned = std::thread::Builder::new()
            .name("scoria-remote-fetch".into())
            .spawn(move || {
                let _ = tx.send(fetch(&agent, &method, &uri));
            });
        Box::pin(async move {
            spawned?;
            rx.await?
        })
    }
}

/// One `GET` of an `http:` or `https:` URL; anything else is refused, so a
/// `file:` or `data:` destination can never reach the network layer here.
fn fetch(
    agent: &ureq::Agent,
    method: &http_client::http::Method,
    uri: &str,
) -> anyhow::Result<Response<AsyncBody>> {
    if method != http_client::http::Method::GET {
        anyhow::bail!("only GET is supported, not {method}");
    }
    if !is_remote(uri) {
        anyhow::bail!("not an http(s) URL: {uri}");
    }
    let response = agent.get(uri).header("Accept", ACCEPT).call()?;
    let status = response.status().as_u16();
    let body = response
        .into_body()
        .with_config()
        .limit(MAX_BODY)
        .read_to_vec()?;
    Ok(Response::builder()
        .status(status)
        .body(AsyncBody::from(body))?)
}

/// What a download brought back: the response's `Content-Type`, if any, and
/// the body.
pub struct Download {
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
}

/// One blocking `GET` for `editor:download-attachments`: the same agent
/// settings as an image load, and a status other than 2xx is a failure, as
/// a 404 was for Obsidian 1.15.0 (the run stopped there). Run it off the UI
/// thread.
pub fn download(url: &str) -> anyhow::Result<Download> {
    if !is_remote(url) {
        anyhow::bail!("not an http(s) URL: {url}");
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(60)))
        .user_agent(format!("Scoria/{}", env!("CARGO_PKG_VERSION")).as_str())
        .build()
        .into();
    let response = agent.get(url).header("Accept", ACCEPT).call()?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        anyhow::bail!("{url} answered {status}");
    }
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let bytes = response
        .into_body()
        .with_config()
        .limit(MAX_BODY)
        .read_to_vec()?;
    Ok(Download {
        content_type,
        bytes,
    })
}

/// Whether a destination is one Obsidian fetches: `http:` or `https:`,
/// scheme case ignored. A protocol-relative `//host/x.png` is not -- it
/// resolves against `app://` there and loads nothing.
pub fn is_remote(destination: &str) -> bool {
    let d = destination.trim_start();
    ["http://", "https://"].iter().any(|scheme| {
        d.get(..scheme.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(scheme))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_http_and_https_are_remote() {
        assert!(is_remote("https://e.com/x.png"));
        assert!(is_remote("http://127.0.0.1:8731/x"));
        assert!(is_remote("HTTPS://E.COM/X.PNG"));
        assert!(!is_remote("//e.com/x.png"));
        assert!(!is_remote("file:///c/x.png"));
        assert!(!is_remote("data:image/png;base64,AAAA"));
        assert!(!is_remote("app://local/x.png"));
        assert!(!is_remote("pic.png"));
        assert!(!is_remote("http:"));
    }
}
