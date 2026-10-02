//! Shared HTTP plumbing for the GitLab and Gitea clients.

use crate::error::TwigError;

pub fn build_client() -> Result<reqwest::Client, TwigError> {
    reqwest::Client::builder()
        .user_agent(concat!("Twig/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
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
    fn encoding() {
        assert_eq!(encode_segment("grp/sub/my proj"), "grp%2Fsub%2Fmy%20proj");
        assert_eq!(encode_segment("a-b_c.d~"), "a-b_c.d~");
    }
}
