//! Opt-in HTTPS authentication for network git commands (fetch/pull/push).
//!
//! The keyring token is handed to git as a one-off `http.<url>.extraHeader`
//! through `GIT_CONFIG_COUNT`/`GIT_CONFIG_KEY_n`/`GIT_CONFIG_VALUE_n`
//! environment variables, so it never appears in the process list, is never
//! written to any git config file, and — because the key is scoped to
//! `https://<github host>/` — git only sends it to that host, even for
//! `fetch --all` across remotes on other hosts.
//!
//! Commands opt in by wrapping their `writer::*` call in
//! [`with_network_auth`]; `writer::run_git` picks the variables up from a
//! task-local, so the writer functions keep their signatures.

use std::future::Future;
use std::path::Path;

use base64::Engine;

use super::config;
use super::remote::is_https_on_host;

tokio::task_local! {
    static GIT_ENV: Vec<(String, String)>;
}

/// Apply the auth environment of the current [`with_network_auth`] scope
/// (if any) to a git command. Called by `writer::run_git`.
pub(crate) fn apply_env(cmd: &mut tokio::process::Command) {
    let _ = GIT_ENV.try_with(|vars| {
        for (k, v) in vars {
            cmd.env(k, v);
        }
    });
}

/// Environment that makes git send `Authorization` to `https://{host}/`
/// only. `existing_count` is a `GIT_CONFIG_COUNT` already present in the
/// environment, which we extend instead of clobbering.
pub(crate) fn auth_header_env(
    host: &str,
    token: &str,
    existing_count: Option<&str>,
) -> Vec<(String, String)> {
    let n = existing_count
        .and_then(|c| c.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let basic = base64::engine::general_purpose::STANDARD.encode(format!("x-access-token:{token}"));
    vec![
        ("GIT_CONFIG_COUNT".to_string(), (n + 1).to_string()),
        (format!("GIT_CONFIG_KEY_{n}"), format!("http.https://{host}/.extraheader")),
        (format!("GIT_CONFIG_VALUE_{n}"), format!("AUTHORIZATION: basic {basic}")),
    ]
}

/// Same as [`auth_header_env`], extending the process' own `GIT_CONFIG_COUNT`.
pub(crate) fn auth_env_for(host: &str, token: &str) -> Vec<(String, String)> {
    let existing = std::env::var("GIT_CONFIG_COUNT").ok();
    auth_header_env(host, token, existing.as_deref())
}

/// Whether any remote (fetch or push URL) of the repo is HTTPS on `host`.
fn repo_uses_https_host(repo_path: &Path, host: &str) -> bool {
    let Ok(repo) = git2::Repository::open(repo_path) else {
        return false;
    };
    let Ok(names) = repo.remotes() else {
        return false;
    };
    names.iter().flatten().any(|name| {
        repo.find_remote(name).is_ok_and(|r| {
            r.url().is_some_and(|u| is_https_on_host(u, host))
                || r.pushurl().is_some_and(|u| is_https_on_host(u, host))
        })
    })
}

/// The auth environment for network commands in `repo_path`: empty unless
/// the setting is on, a token is stored, and the repo has an HTTPS remote
/// on the configured GitHub host.
pub async fn network_env(app: &tauri::AppHandle, repo_path: &Path) -> Vec<(String, String)> {
    let cfg = config::load(app);
    if !cfg.github_https_auth {
        return Vec::new();
    }
    let host = cfg.github.host.clone();
    let path = repo_path.to_path_buf();
    let check_host = host.clone();
    let uses_host = tauri::async_runtime::spawn_blocking(move || repo_uses_https_host(&path, &check_host))
        .await
        .unwrap_or(false);
    if !uses_host {
        return Vec::new();
    }
    match crate::commands::github::get_token(app).await {
        Ok(token) => auth_env_for(&host, &token),
        Err(_) => Vec::new(), // fall back to the user's credential helper
    }
}

/// Run `fut` with the auth environment for `repo_path` applied to every git
/// command it spawns through `writer::run_git`.
pub async fn with_network_auth<F: Future>(
    app: &tauri::AppHandle,
    repo_path: &Path,
    fut: F,
) -> F::Output {
    let env = network_env(app, repo_path).await;
    GIT_ENV.scope(env, fut).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_env_is_host_scoped() {
        let env = auth_header_env("github.com", "tok", None);
        assert_eq!(env[0], ("GIT_CONFIG_COUNT".into(), "1".into()));
        assert_eq!(
            env[1],
            ("GIT_CONFIG_KEY_0".into(), "http.https://github.com/.extraheader".into())
        );
        let expected = base64::engine::general_purpose::STANDARD.encode("x-access-token:tok");
        assert_eq!(
            env[2],
            ("GIT_CONFIG_VALUE_0".into(), format!("AUTHORIZATION: basic {expected}"))
        );
    }

    #[test]
    fn header_env_extends_existing_count() {
        let env = auth_header_env("ghe.corp.io", "t", Some("2"));
        assert_eq!(env[0].1, "3");
        assert_eq!(env[1].0, "GIT_CONFIG_KEY_2");
        assert_eq!(env[1].1, "http.https://ghe.corp.io/.extraheader");
        assert_eq!(env[2].0, "GIT_CONFIG_VALUE_2");
        // Garbage count is treated as absent.
        assert_eq!(auth_header_env("h", "t", Some("x"))[1].0, "GIT_CONFIG_KEY_0");
    }

    #[tokio::test]
    async fn scope_applies_env_to_run_git() {
        let dir = std::env::temp_dir().join(format!("twig-netauth-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let env = vec![
            ("GIT_CONFIG_COUNT".to_string(), "1".to_string()),
            ("GIT_CONFIG_KEY_0".to_string(), "twig.selftest".to_string()),
            ("GIT_CONFIG_VALUE_0".to_string(), "yes".to_string()),
        ];
        let inside = GIT_ENV
            .scope(env, crate::git::writer::run_git(&dir, &["config", "--get", "twig.selftest"]))
            .await
            .unwrap();
        assert_eq!(inside.stdout.trim(), "yes");
        let outside = crate::git::writer::run_git(&dir, &["config", "--get", "twig.selftest"])
            .await
            .unwrap();
        assert!(outside.stdout.trim().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
