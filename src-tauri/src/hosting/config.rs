//! Hosting endpoints derived from `settings.json` (GitHub / GitHub
//! Enterprise host + API base, GitLab and Gitea base URLs).

use crate::commands::settings::{settings_file, AppSettings};

/// Where the GitHub (or GitHub Enterprise Server) instance lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubEndpoint {
    /// Lowercase hostname used in remote URLs, e.g. `github.com`.
    pub host: String,
    /// REST API base without trailing slash, e.g. `https://api.github.com`.
    pub api_base: String,
    /// GraphQL endpoint.
    pub graphql_url: String,
    /// Web base, e.g. `https://github.com`.
    pub web_base: String,
}

impl GitHubEndpoint {
    #[cfg(test)]
    pub fn dotcom() -> Self {
        Self::new("github.com", "")
    }

    /// Build from the configured host and optional REST API override.
    /// GitHub Enterprise Server serves REST at `/api/v3` and GraphQL at
    /// `/api/graphql` on the instance host.
    pub fn new(host: &str, api_override: &str) -> Self {
        let host = normalize_host(host).unwrap_or_else(|| "github.com".to_string());
        let dotcom = host == "github.com";
        let api_override = api_override.trim().trim_end_matches('/');
        let api_base = if !api_override.is_empty() && is_http_url(api_override) {
            api_override.to_string()
        } else if dotcom {
            "https://api.github.com".to_string()
        } else {
            format!("https://{host}/api/v3")
        };
        let graphql_url = if api_base == "https://api.github.com" {
            "https://api.github.com/graphql".to_string()
        } else if let Some(prefix) = api_base.strip_suffix("/api/v3") {
            format!("{prefix}/api/graphql")
        } else {
            format!("{api_base}/graphql")
        };
        Self {
            web_base: format!("https://{host}"),
            host,
            api_base,
            graphql_url,
        }
    }

    pub fn is_dotcom(&self) -> bool {
        self.host == "github.com"
    }
}

/// Snapshot of all hosting-related settings.
#[derive(Debug, Clone)]
pub struct HostingConfig {
    pub github: GitHubEndpoint,
    pub github_https_auth: bool,
    /// GitLab base URL without trailing slash (e.g. `https://gitlab.com`).
    pub gitlab_base: Option<String>,
    /// Gitea/Forgejo base URL without trailing slash.
    pub gitea_base: Option<String>,
}

impl HostingConfig {
    pub fn from_settings(s: &AppSettings) -> Self {
        Self {
            github: GitHubEndpoint::new(&s.github_host, &s.github_api_url),
            github_https_auth: s.github_https_auth,
            gitlab_base: normalize_base_url(&s.gitlab_base_url),
            gitea_base: normalize_base_url(&s.gitea_base_url),
        }
    }
}

/// Read the hosting settings from disk. Falls back to defaults when the
/// settings file is missing or unreadable (the UI surfaces those errors).
pub fn load(app: &tauri::AppHandle) -> HostingConfig {
    let settings = settings_file(app)
        .ok()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|json| serde_json::from_str::<AppSettings>(&json).ok())
        .unwrap_or_default();
    HostingConfig::from_settings(&settings)
}

fn is_http_url(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

/// Accept `host`, `https://host`, `https://host/` → `host` (lowercase).
/// Rejects values with whitespace or userinfo.
pub fn normalize_host(input: &str) -> Option<String> {
    let s = input.trim();
    let s = s
        .split_once("://")
        .map_or(s, |(_, rest)| rest)
        .split('/')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if s.is_empty() || s.contains('@') || s.chars().any(|c| c.is_whitespace()) {
        return None;
    }
    Some(s)
}

/// `gitlab.example.com/` or `https://gitlab.example.com/` →
/// `https://gitlab.example.com`. A sub-path (`https://host/gitlab`) is kept.
pub fn normalize_base_url(input: &str) -> Option<String> {
    let s = input.trim().trim_end_matches('/');
    if s.is_empty() || s.chars().any(|c| c.is_whitespace()) {
        return None;
    }
    let with_scheme = if is_http_url(s) {
        s.to_string()
    } else if s.contains("://") {
        return None;
    } else {
        format!("https://{s}")
    };
    let rest = with_scheme.split_once("://").map(|(_, r)| r)?;
    if rest.is_empty() || rest.starts_with('/') || rest.contains('@') {
        return None;
    }
    Some(with_scheme)
}

/// Split a base URL into (lowercase host without port, path prefix).
pub fn base_host_and_prefix(base: &str) -> Option<(String, String)> {
    let rest = base.split_once("://").map(|(_, r)| r)?;
    let (host, prefix) = rest.split_once('/').unwrap_or((rest, ""));
    let host = host.split_once(':').map_or(host, |(h, _)| h);
    Some((host.to_ascii_lowercase(), prefix.trim_matches('/').to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_endpoints() {
        let d = GitHubEndpoint::dotcom();
        assert_eq!(d.api_base, "https://api.github.com");
        assert_eq!(d.graphql_url, "https://api.github.com/graphql");
        assert_eq!(d.web_base, "https://github.com");
        assert!(d.is_dotcom());

        let e = GitHubEndpoint::new("https://GHE.corp.io/", "");
        assert_eq!(e.host, "ghe.corp.io");
        assert_eq!(e.api_base, "https://ghe.corp.io/api/v3");
        assert_eq!(e.graphql_url, "https://ghe.corp.io/api/graphql");
        assert!(!e.is_dotcom());

        let o = GitHubEndpoint::new("ghe.corp.io", "https://api.ghe.corp.io/");
        assert_eq!(o.api_base, "https://api.ghe.corp.io");
        assert_eq!(o.graphql_url, "https://api.ghe.corp.io/graphql");

        // Garbage falls back to github.com / the derived API base.
        assert_eq!(GitHubEndpoint::new("  ", "").host, "github.com");
        assert_eq!(
            GitHubEndpoint::new("a b", "not a url").api_base,
            "https://api.github.com"
        );
    }

    #[test]
    fn base_urls() {
        assert_eq!(
            normalize_base_url("gitlab.com/").as_deref(),
            Some("https://gitlab.com")
        );
        assert_eq!(
            normalize_base_url(" https://git.x.io/gitlab/ ").as_deref(),
            Some("https://git.x.io/gitlab")
        );
        assert_eq!(normalize_base_url(""), None);
        assert_eq!(normalize_base_url("ftp://x"), None);
        assert_eq!(normalize_base_url("https://u:p@x"), None);
        assert_eq!(
            base_host_and_prefix("https://Git.X.io:8443/gitlab"),
            Some(("git.x.io".to_string(), "gitlab".to_string()))
        );
        assert_eq!(
            base_host_and_prefix("https://gitea.io"),
            Some(("gitea.io".to_string(), String::new()))
        );
    }
}
