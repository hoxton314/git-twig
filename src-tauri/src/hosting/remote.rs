//! Remote URL parsing and hosting-provider detection.

use serde::Serialize;

use super::config::{base_host_and_prefix, HostingConfig};

/// A remote URL broken into host and repository path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitRemote {
    /// Lowercase host without port or userinfo.
    pub host: String,
    /// Repository path without surrounding slashes or a `.git` suffix.
    pub path: String,
    /// Whether the URL is `https://` (the only scheme we inject auth for).
    pub is_https: bool,
}

/// Split HTTPS (`https://[user@]host[:port]/path.git`), URL-style SSH/git
/// (`ssh://git@host[:port]/path.git`) and scp-like (`git@host:path.git`)
/// remote URLs. Local paths and other schemes return `None`.
pub fn split_remote_url(url: &str) -> Option<SplitRemote> {
    let url = url.trim();
    let (host, path, is_https) = if let Some((scheme, rest)) = url.split_once("://") {
        let scheme = scheme.to_ascii_lowercase();
        if !matches!(
            scheme.as_str(),
            "https" | "http" | "ssh" | "git" | "git+ssh" | "ssh+git"
        ) {
            return None;
        }
        let (authority, path) = rest.split_once('/')?;
        // Strip userinfo (`user@` / `user:token@`) and port.
        let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        let host = host.split_once(':').map_or(host, |(h, _)| h);
        (host, path, scheme == "https")
    } else {
        // scp-like syntax: [user@]host:path
        let (authority, path) = url.split_once(':')?;
        if authority.contains('/') || authority.is_empty() {
            return None; // local path, not a remote
        }
        let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        (host, path, false)
    };
    if host.is_empty() {
        return None;
    }
    let path = path.trim().trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path).trim_end_matches('/');
    if path.is_empty() {
        return None;
    }
    Some(SplitRemote {
        host: host.to_ascii_lowercase(),
        path: path.to_string(),
        is_https,
    })
}

fn github_host_matches(remote_host: &str, configured: &str) -> bool {
    remote_host == configured || (configured == "github.com" && remote_host == "www.github.com")
}

/// Parse `owner/repo` from a remote URL on the given GitHub host.
pub fn parse_github_remote(url: &str, host: &str) -> Option<(String, String)> {
    let split = split_remote_url(url)?;
    if !github_host_matches(&split.host, host) {
        return None;
    }
    let mut parts = split.path.splitn(3, '/');
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim();
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
}

/// Whether `url` is an `https://` URL on `host` (auth may be injected).
pub fn is_https_on_host(url: &str, host: &str) -> bool {
    split_remote_url(url).is_some_and(|s| s.is_https && github_host_matches(&s.host, host))
}

/// Strip an instance sub-path (`https://host/gitlab`) from a remote path.
/// SSH remotes never include it, so it is optional.
fn strip_prefix_path<'a>(path: &'a str, prefix: &str) -> &'a str {
    if prefix.is_empty() {
        return path;
    }
    path.strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(path)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    GitHub,
    GitLab,
    Gitea,
}

/// A git remote that points at a configured hosting provider.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HostedRemote {
    pub provider: ProviderKind,
    pub remote_name: String,
    pub host: String,
    /// Namespace (GitHub/Gitea owner, or GitLab group path incl. subgroups).
    pub owner: String,
    pub repo: String,
    /// Full `owner/repo` (GitLab: `group/sub/project`).
    pub project_path: String,
    /// Browser URL of the repository.
    pub web_url: String,
}

/// Match a remote URL against the configured providers.
pub fn match_remote(cfg: &HostingConfig, remote_name: &str, url: &str) -> Option<HostedRemote> {
    let split = split_remote_url(url)?;

    if github_host_matches(&split.host, &cfg.github.host) {
        let (owner, repo) = parse_github_remote(url, &cfg.github.host)?;
        return Some(HostedRemote {
            provider: ProviderKind::GitHub,
            remote_name: remote_name.to_string(),
            host: cfg.github.host.clone(),
            web_url: format!("{}/{owner}/{repo}", cfg.github.web_base),
            project_path: format!("{owner}/{repo}"),
            owner,
            repo,
        });
    }

    let try_base = |base: &Option<String>, kind: ProviderKind| -> Option<HostedRemote> {
        let base = base.as_ref()?;
        let (host, prefix) = base_host_and_prefix(base)?;
        if split.host != host {
            return None;
        }
        let path = strip_prefix_path(&split.path, &prefix);
        let (owner, repo) = path.rsplit_once('/')?;
        if owner.is_empty() || repo.is_empty() {
            return None;
        }
        // Gitea has no nested namespaces.
        if kind == ProviderKind::Gitea && owner.contains('/') {
            return None;
        }
        Some(HostedRemote {
            provider: kind,
            remote_name: remote_name.to_string(),
            host,
            owner: owner.to_string(),
            repo: repo.to_string(),
            project_path: path.to_string(),
            web_url: format!("{base}/{path}"),
        })
    };

    try_base(&cfg.gitlab_base, ProviderKind::GitLab)
        .or_else(|| try_base(&cfg.gitea_base, ProviderKind::Gitea))
}

/// All hosted remotes of a repository, with the preferred one first:
/// `upstream` (PRs of a fork live there), then `origin`, then the rest.
pub fn hosted_remotes(repo: &git2::Repository, cfg: &HostingConfig) -> Vec<HostedRemote> {
    let mut out = Vec::new();
    if let Ok(names) = repo.remotes() {
        for name in names.iter().flatten() {
            if let Ok(remote) = repo.find_remote(name) {
                if let Some(hosted) = remote.url().and_then(|u| match_remote(cfg, name, u)) {
                    out.push(hosted);
                }
            }
        }
    }
    let rank = |r: &HostedRemote| match r.remote_name.as_str() {
        "upstream" => 0,
        "origin" => 1,
        _ => 2,
    };
    out.sort_by_key(rank);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosting::config::GitHubEndpoint;

    fn p(url: &str) -> Option<(String, String)> {
        parse_github_remote(url, "github.com")
    }

    fn ok(owner: &str, repo: &str) -> Option<(String, String)> {
        Some((owner.to_string(), repo.to_string()))
    }

    #[test]
    fn parses_common_remote_forms() {
        assert_eq!(p("https://github.com/o/r.git"), ok("o", "r"));
        assert_eq!(p("https://github.com/o/r"), ok("o", "r"));
        assert_eq!(p("https://github.com/o/r/"), ok("o", "r"));
        assert_eq!(p("https://github.com/o/r.git/"), ok("o", "r"));
        assert_eq!(p("http://GitHub.com/o/r.git"), ok("o", "r"));
        assert_eq!(p("https://user:tok@github.com/o/r.git"), ok("o", "r"));
        assert_eq!(p("git@github.com:o/r.git"), ok("o", "r"));
        assert_eq!(p("git@github.com:o/r"), ok("o", "r"));
        assert_eq!(p("ssh://git@github.com/o/r.git"), ok("o", "r"));
        assert_eq!(p("ssh://git@github.com:22/o/r.git"), ok("o", "r"));
        assert_eq!(p("git://github.com/o/my.repo.git"), ok("o", "my.repo"));
        assert_eq!(p("https://www.github.com/o/r"), ok("o", "r"));
    }

    #[test]
    fn rejects_non_github() {
        assert_eq!(p("https://gitlab.com/o/r.git"), None);
        assert_eq!(p("git@gitlab.com:o/r.git"), None);
        assert_eq!(p("https://github.com.evil.io/o/r"), None);
        assert_eq!(p("/home/me/github.com:o/r"), None);
        assert_eq!(p("https://github.com/o"), None);
        assert_eq!(p("file:///github.com/o/r"), None);
    }

    #[test]
    fn enterprise_host() {
        let h = "ghe.corp.io";
        assert_eq!(parse_github_remote("git@ghe.corp.io:o/r.git", h), ok("o", "r"));
        assert_eq!(parse_github_remote("https://github.com/o/r", h), None);
        assert_eq!(parse_github_remote("https://www.ghe.corp.io/o/r", h), None);
    }

    #[test]
    fn https_host_check() {
        assert!(is_https_on_host("https://github.com/o/r.git", "github.com"));
        assert!(is_https_on_host("https://x@GitHub.com/o/r", "github.com"));
        assert!(!is_https_on_host("http://github.com/o/r", "github.com"));
        assert!(!is_https_on_host("git@github.com:o/r", "github.com"));
        assert!(!is_https_on_host("https://github.com.evil.io/o/r", "github.com"));
        assert!(!is_https_on_host("https://gitlab.com/o/r", "github.com"));
    }

    fn cfg() -> HostingConfig {
        HostingConfig {
            github: GitHubEndpoint::dotcom(),
            github_https_auth: true,
            gitlab_base: Some("https://git.example.com/gitlab".into()),
            gitea_base: Some("https://codeberg.org".into()),
        }
    }

    #[test]
    fn matches_providers() {
        let c = cfg();
        let gh = match_remote(&c, "origin", "git@github.com:o/r.git").unwrap();
        assert_eq!(gh.provider, ProviderKind::GitHub);
        assert_eq!(gh.web_url, "https://github.com/o/r");

        let gl = match_remote(&c, "origin", "https://git.example.com/gitlab/grp/sub/proj.git").unwrap();
        assert_eq!(gl.provider, ProviderKind::GitLab);
        assert_eq!(gl.owner, "grp/sub");
        assert_eq!(gl.repo, "proj");
        assert_eq!(gl.project_path, "grp/sub/proj");
        assert_eq!(gl.web_url, "https://git.example.com/gitlab/grp/sub/proj");

        let gl_ssh = match_remote(&c, "o", "git@git.example.com:grp/proj.git").unwrap();
        assert_eq!(gl_ssh.project_path, "grp/proj");

        let gt = match_remote(&c, "up", "https://codeberg.org/owner/repo").unwrap();
        assert_eq!(gt.provider, ProviderKind::Gitea);
        assert_eq!(gt.project_path, "owner/repo");
        assert!(match_remote(&c, "up", "https://codeberg.org/a/b/c").is_none());

        assert!(match_remote(&c, "x", "https://bitbucket.org/o/r").is_none());
        assert!(match_remote(&c, "x", "/local/path").is_none());
    }
}
