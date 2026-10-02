//! Shared HTTP plumbing for the GitLab and Gitea clients.

use crate::error::TwigError;

/// Whether a redirect from `prev` to `next` may be followed. Tokens travel
/// in headers reqwest does not know are sensitive (GitLab's `PRIVATE-TOKEN`,
/// Gitea's `Authorization: token`), so they would be forwarded to any host a
/// redirect points to. Only same-host redirects are followed, and never from
/// HTTPS down to plain HTTP.
fn redirect_allowed(prev: &reqwest::Url, next: &reqwest::Url) -> bool {
    let same_host = prev
        .host_str()
        .zip(next.host_str())
        .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b));
    let downgrade = prev.scheme() == "https" && next.scheme() != "https";
    same_host && !downgrade && matches!(next.scheme(), "https" | "http")
}

pub fn build_client() -> Result<reqwest::Client, TwigError> {
    let policy = reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() > 10 {
            return attempt.error("too many redirects");
        }
        match attempt.previous().last() {
            Some(prev) if !redirect_allowed(prev, attempt.url()) => attempt.stop(),
            _ => attempt.follow(),
        }
    });
    reqwest::Client::builder()
        .user_agent(concat!("Twig/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .redirect(policy)
        .build()
        .map_err(|e| TwigError::Http(e.to_string()))
}

/// Extract a human-readable message from a provider error body
/// (`{"message": ...}`, `{"error": ...}`, `{"message": {"field": [..]}}`).
pub fn error_message(raw: &str) -> String {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) else {
        return raw.chars().take(300).collect();
    };
    let field = v.get("message").or_else(|| v.get("error"));
    match field {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(a)) => a
            .iter()
            .filter_map(|x| x.as_str())
            .collect::<Vec<_>>()
            .join("; "),
        Some(serde_json::Value::Object(o)) => o
            .iter()
            .map(|(k, val)| match val {
                serde_json::Value::Array(a) => format!(
                    "{k} {}",
                    a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", ")
                ),
                other => format!("{k}: {other}"),
            })
            .collect::<Vec<_>>()
            .join("; "),
        _ => raw.chars().take(300).collect(),
    }
}

/// Turn a non-2xx response into a `TwigError::Hosting` naming the provider.
pub async fn check(provider: &str, response: reqwest::Response) -> Result<reqwest::Response, TwigError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let raw = response.text().await.unwrap_or_default();
    let msg = error_message(&raw);
    Err(TwigError::Hosting(match status.as_u16() {
        401 => format!("{provider}: invalid or expired token. Update it in Settings."),
        403 => format!("{provider}: access denied: {msg}"),
        404 => format!("{provider}: not found (or the token lacks access): {msg}"),
        409 | 422 => format!("{provider}: {msg}"),
        429 => format!("{provider}: rate limit hit, try again later."),
        _ => format!("{provider} API error ({status}): {msg}"),
    }))
}

/// Minimal percent-encoding for a URL path segment (GitLab project ids
/// like `group/sub/project` must be passed as `group%2Fsub%2Fproject`).
pub fn encode_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Percent-encode a query value.
pub fn encode_query(s: &str) -> String {
    encode_segment(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages() {
        assert_eq!(error_message(r#"{"message":"nope"}"#), "nope");
        assert_eq!(error_message(r#"{"error":"bad"}"#), "bad");
        assert_eq!(
            error_message(r#"{"message":{"title":["is too long"]}}"#),
            "title is too long"
        );
        assert_eq!(error_message(r#"{"message":["a","b"]}"#), "a; b");
        assert_eq!(error_message("plain"), "plain");
    }

    #[test]
    fn redirects_stay_on_host_and_never_downgrade() {
        let u = |s: &str| reqwest::Url::parse(s).unwrap();
        let ok = |a: &str, b: &str| redirect_allowed(&u(a), &u(b));
        assert!(ok("https://gitlab.example.com/api/v4/x", "https://gitlab.example.com/api/v4/y"));
        assert!(ok("http://gitea.lan/api", "https://gitea.lan/api"));
        assert!(ok("https://Git.Example.com/a", "https://git.example.com/b"));
        assert!(!ok("https://gitlab.example.com/a", "https://evil.example.net/a"));
        assert!(!ok("https://gitlab.example.com/a", "https://gitlab.example.com.evil.net/a"));
        assert!(!ok("https://gitlab.example.com/a", "http://gitlab.example.com/a"));
    }

    /// End to end: a cross-host redirect is not followed, so the custom
    /// token header never reaches the other host.
    #[tokio::test]
    async fn cross_host_redirect_is_not_followed() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_port = target.local_addr().unwrap().port();
        let origin = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin_port = origin.local_addr().unwrap().port();
        // `localhost` vs `127.0.0.1`: same machine, different host.
        let location = format!("http://localhost:{target_port}/steal");
        tokio::spawn(async move {
            if let Ok((mut sock, _)) = origin.accept().await {
                let mut buf = [0u8; 2048];
                let _ = sock.read(&mut buf).await;
                let resp = format!(
                    "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = sock.write_all(resp.as_bytes()).await;
            }
        });
        let hit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = hit.clone();
        tokio::spawn(async move {
            if target.accept().await.is_ok() {
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        });
        let client = build_client().unwrap();
        let resp = client
            .get(format!("http://127.0.0.1:{origin_port}/api"))
            .header("PRIVATE-TOKEN", "secret")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 302);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert!(!hit.load(std::sync::atomic::Ordering::SeqCst), "token header followed the redirect");
    }

    #[test]
    fn encoding() {
        assert_eq!(encode_segment("grp/sub/my proj"), "grp%2Fsub%2Fmy%20proj");
        assert_eq!(encode_segment("a-b_c.d~"), "a-b_c.d~");
    }
}
