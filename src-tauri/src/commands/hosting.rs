//! Provider-agnostic hosting commands: pull/merge requests, CI status,
//! GitHub OAuth device flow, and GitLab / Gitea tokens and repo lists.
//! Tokens are read from the OS keyring here and never cross into the UI.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::State;

use super::github::{build_client, endpoint, get_token as github_token};
use super::staging::CommandResult;
use crate::credentials;
use crate::error::TwigError;
use crate::git::writer::{self, run_git, safe_ref};
use crate::github::{self, GitHubPullRequest, GitHubUser, RepoListPage};
use crate::hosting::config::{self, base_host_and_prefix, HostingConfig};
use crate::hosting::gitea::Gitea;
use crate::hosting::github_api::{self, DeviceFlowStart, PollOutcome, GITHUB_OAUTH_CLIENT_ID};
use crate::hosting::gitlab::GitLab;
use crate::hosting::net_auth::with_network_auth;
use crate::hosting::remote::{hosted_remotes, HostedRemote, ProviderKind};
use crate::hosting::types::{CiStatus, PrDetail, PrFile, PrPage};
use crate::state::AppState;

// ── Resolution helpers ──────────────────────────────────────────────

/// Instance base URL of a self-hosted provider (GitLab / Gitea).
fn base_for(cfg: &HostingConfig, kind: ProviderKind) -> Result<String, TwigError> {
    let base = match kind {
        ProviderKind::GitLab => cfg.gitlab_base.clone(),
        ProviderKind::Gitea => cfg.gitea_base.clone(),
        ProviderKind::GitHub => Some(cfg.github.web_base.clone()),
    };
    base.ok_or_else(|| {
        TwigError::Hosting(format!(
            "No {} instance URL configured. Set one in Settings.",
            provider_label(kind)
        ))
    })
}

fn provider_label(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::GitHub => "GitHub",
        ProviderKind::GitLab => "GitLab",
        ProviderKind::Gitea => "Gitea",
    }
}

fn parse_provider(provider: &str) -> Result<ProviderKind, TwigError> {
    match provider {
        "github" => Ok(ProviderKind::GitHub),
        "gitlab" => Ok(ProviderKind::GitLab),
        "gitea" => Ok(ProviderKind::Gitea),
        other => Err(TwigError::InvalidArgument(format!("unknown provider '{other}'"))),
    }
}

/// Keyring account for a GitLab/Gitea instance.
fn account_for(cfg: &HostingConfig, kind: ProviderKind) -> Result<String, TwigError> {
    let base = base_for(cfg, kind)?;
    let (host, _) = base_host_and_prefix(&base)
        .ok_or_else(|| TwigError::Hosting("Invalid instance URL".into()))?;
    let name = match kind {
        ProviderKind::GitLab => "gitlab",
        ProviderKind::Gitea => "gitea",
        ProviderKind::GitHub => "github",
    };
    Ok(credentials::provider_account(name, &host))
}

async fn token_for(app: &tauri::AppHandle, cfg: &HostingConfig, kind: ProviderKind) -> Result<String, TwigError> {
    if kind == ProviderKind::GitHub {
        return github_token(app).await;
    }
    credentials::get_token_for(account_for(cfg, kind)?)
        .await?
        .ok_or_else(|| {
            TwigError::Hosting(format!(
                "No {} token configured. Set one in Settings > Hosting.",
                provider_label(kind)
            ))
        })
}

/// Pick the hosted remote by name, or the preferred one.
async fn resolve_remote(
    state: &State<'_, AppState>,
    cfg: &HostingConfig,
    path: &str,
    remote_name: Option<&str>,
) -> Result<HostedRemote, TwigError> {
    let cfg2 = cfg.clone();
    let remotes = state.read_repo(path, move |repo| Ok(hosted_remotes(repo, &cfg2))).await?;
    let found = match remote_name {
        Some(name) => remotes.into_iter().find(|r| r.remote_name == name),
        None => remotes.into_iter().next(),
    };
    found.ok_or_else(|| {
        TwigError::Hosting(
            "No GitHub, GitLab or Gitea remote found for this repository (check the hosts in Settings)."
                .into(),
        )
    })
}

/// Everything needed to call the provider API for one remote.
struct Ctx {
    cfg: HostingConfig,
    remote: HostedRemote,
    token: String,
    client: reqwest::Client,
}

async fn ctx(
    app: &tauri::AppHandle,
    state: &State<'_, AppState>,
    path: &str,
    remote_name: Option<&str>,
) -> Result<Ctx, TwigError> {
    let cfg = config::load(app);
    let remote = resolve_remote(state, &cfg, path, remote_name).await?;
    let token = token_for(app, &cfg, remote.provider).await?;
    Ok(Ctx {
        cfg,
        remote,
        token,
        client: build_client()?,
    })
}

impl Ctx {
    fn gitlab(&self) -> Result<GitLab<'_>, TwigError> {
        Ok(GitLab {
            client: &self.client,
            base: self.cfg.gitlab_base.as_deref().ok_or_else(|| TwigError::Hosting("GitLab URL not set".into()))?,
            token: &self.token,
        })
    }

    fn gitea(&self) -> Result<Gitea<'_>, TwigError> {
        Ok(Gitea {
            client: &self.client,
            base: self.cfg.gitea_base.as_deref().ok_or_else(|| TwigError::Hosting("Gitea URL not set".into()))?,
            token: &self.token,
        })
    }
}

// ── Remotes & provider info ─────────────────────────────────────────

/// Hosted remotes of a repo (preferred first: upstream, origin, others).
#[tauri::command]
pub async fn hosting_list_remotes(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<HostedRemote>, TwigError> {
    let cfg = config::load(&app);
    state.read_repo(&path, move |repo| Ok(hosted_remotes(repo, &cfg))).await
}

#[derive(Debug, Serialize)]
pub struct HostingInfo {
    pub github_host: String,
    pub github_api_base: String,
    /// Whether "Sign in with GitHub" (device flow) can be offered.
    pub github_oauth_available: bool,
    pub gitlab_base: Option<String>,
    pub gitea_base: Option<String>,
}

#[tauri::command]
pub async fn hosting_info(app: tauri::AppHandle) -> Result<HostingInfo, TwigError> {
    let cfg = config::load(&app);
    Ok(HostingInfo {
        github_oauth_available: !GITHUB_OAUTH_CLIENT_ID.is_empty() && cfg.github.is_dotcom(),
        github_host: cfg.github.host,
        github_api_base: cfg.github.api_base,
        gitlab_base: cfg.gitlab_base,
        gitea_base: cfg.gitea_base,
    })
}

// ── Pull / merge requests ───────────────────────────────────────────

#[tauri::command]
pub async fn hosting_list_prs(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
    remote_name: Option<String>,
    filter: String,
    cursor: Option<String>,
) -> Result<PrPage, TwigError> {
    let c = ctx(&app, &state, &path, remote_name.as_deref()).await?;
    let r = &c.remote;
    match r.provider {
        ProviderKind::GitHub => {
            github_api::list_pull_requests(&c.client, &c.cfg.github, &c.token, &r.owner, &r.repo, &filter, cursor.as_deref())
                .await
        }
        ProviderKind::GitLab => c.gitlab()?.list_merge_requests(&r.project_path, &filter, cursor.as_deref()).await,
        ProviderKind::Gitea => c.gitea()?.list_pull_requests(&r.owner, &r.repo, &filter, cursor.as_deref()).await,
    }
}

#[tauri::command]
pub async fn hosting_get_pr(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
    remote_name: Option<String>,
    number: u64,
) -> Result<PrDetail, TwigError> {
    let c = ctx(&app, &state, &path, remote_name.as_deref()).await?;
    let r = &c.remote;
    match r.provider {
        ProviderKind::GitHub => {
            github_api::get_pull_request(&c.client, &c.cfg.github, &c.token, &r.owner, &r.repo, number).await
        }
        ProviderKind::GitLab => c.gitlab()?.get_merge_request(&r.project_path, number).await,
        ProviderKind::Gitea => c.gitea()?.get_pull_request(&r.owner, &r.repo, number).await,
    }
}

#[tauri::command]
pub async fn hosting_pr_files(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
    remote_name: Option<String>,
    number: u64,
) -> Result<Vec<PrFile>, TwigError> {
    let c = ctx(&app, &state, &path, remote_name.as_deref()).await?;
    let r = &c.remote;
    match r.provider {
        ProviderKind::GitHub => {
            github_api::list_pull_request_files(&c.client, &c.cfg.github, &c.token, &r.owner, &r.repo, number).await
        }
        ProviderKind::GitLab => c.gitlab()?.merge_request_files(&r.project_path, number).await,
        ProviderKind::Gitea => c.gitea()?.pull_request_files(&r.owner, &r.repo, number).await,
    }
}

/// Ref under which the provider exposes a PR/MR head.
fn pr_head_ref(kind: ProviderKind, number: u64) -> String {
    match kind {
        ProviderKind::GitLab => format!("refs/merge-requests/{number}/head"),
        _ => format!("refs/pull/{number}/head"),
    }
}

/// Fetch a PR/MR head and check it out as local branch `pr/<number>`.
/// An existing `pr/<number>` is fast-forwarded, never reset, so local
/// commits on it are not lost.
#[tauri::command]
pub async fn hosting_checkout_pr(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
    remote_name: Option<String>,
    number: u64,
) -> Result<CommandResult, TwigError> {
    // No token needed: fetching uses git's own (or our HTTPS) auth.
    let cfg = config::load(&app);
    let remote = resolve_remote(&state, &cfg, &path, remote_name.as_deref()).await?;
    safe_ref(&remote.remote_name)?;
    let repo_path = state.repo_path(&path)?;
    let head_ref = pr_head_ref(remote.provider, number);
    let branch = format!("pr/{number}");

    let fetched = with_network_auth(
        &app,
        &repo_path,
        crate::git::remotes::fetch_ref_commit(
            &repo_path,
            &remote.remote_name,
            &head_ref,
            &format!("pr-{number}"),
        ),
    )
    .await?;
    let sha = match fetched {
        Ok(sha) => sha,
        Err(fetch) => {
            return Ok(CommandResult {
                success: false,
                message: format!("Fetching {head_ref} from {} failed:\n{}", remote.remote_name, fetch.stderr),
            })
        }
    };

    let local_ref = format!("refs/heads/{branch}");
    if !writer::rev_exists(&repo_path, &local_ref).await? {
        let created = run_git(&repo_path, &["branch", &branch, &sha]).await?;
        if !created.success {
            return Ok(created.into());
        }
        let co = writer::checkout_branch(&repo_path, &branch).await?;
        return Ok(if co.success {
            CommandResult {
                success: true,
                message: format!("Checked out #{number} as '{branch}'"),
            }
        } else {
            co.into()
        });
    }

    let co = writer::checkout_branch(&repo_path, &branch).await?;
    if !co.success {
        return Ok(co.into());
    }
    let ff = run_git(&repo_path, &["merge", "--ff-only", &sha]).await?;
    Ok(if ff.success {
        CommandResult {
            success: true,
            message: format!("Checked out '{branch}' and updated it to the latest #{number} head"),
        }
    } else {
        CommandResult {
            success: true,
            message: format!(
                "Checked out '{branch}', but it has diverged from #{number} (local commits?), so it was not updated."
            ),
        }
    })
}

// ── Create PR / branches for any provider ───────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn hosting_create_pr(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
    remote_name: Option<String>,
    title: String,
    body: String,
    head: String,
    base: String,
) -> Result<GitHubPullRequest, TwigError> {
    let c = ctx(&app, &state, &path, remote_name.as_deref()).await?;
    let r = &c.remote;
    match r.provider {
        ProviderKind::GitHub => {
            github::create_pull_request(&c.client, &c.cfg.github.api_base, &c.token, &r.owner, &r.repo, &title, &body, &head, &base)
                .await
        }
        ProviderKind::GitLab => {
            if head.contains(':') {
                return Err(TwigError::Hosting(
                    "GitLab merge requests from forks must be created from the fork's project.".into(),
                ));
            }
            c.gitlab()?.create_merge_request(&r.project_path, &title, &body, &head, &base).await
        }
        ProviderKind::Gitea => c.gitea()?.create_pull_request(&r.owner, &r.repo, &title, &body, &head, &base).await,
    }
}

#[tauri::command]
pub async fn hosting_list_branches(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
    remote_name: Option<String>,
) -> Result<Vec<String>, TwigError> {
    let c = ctx(&app, &state, &path, remote_name.as_deref()).await?;
    let r = &c.remote;
    match r.provider {
        ProviderKind::GitHub => github::list_branches(&c.client, &c.cfg.github.api_base, &c.token, &r.owner, &r.repo).await,
        ProviderKind::GitLab => c.gitlab()?.list_branches(&r.project_path).await,
        ProviderKind::Gitea => c.gitea()?.list_branches(&r.owner, &r.repo).await,
    }
}

// ── CI status ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn hosting_ci_status(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
    remote_name: Option<String>,
    sha: String,
) -> Result<CiStatus, TwigError> {
    if sha.is_empty() || !sha.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(TwigError::InvalidArgument(format!("'{sha}' is not a commit SHA")));
    }
    let c = ctx(&app, &state, &path, remote_name.as_deref()).await?;
    let r = &c.remote;
    match r.provider {
        ProviderKind::GitHub => {
            github_api::ci_status(&c.client, &c.cfg.github, &c.token, &r.owner, &r.repo, &sha).await
        }
        ProviderKind::GitLab => c.gitlab()?.ci_status(&r.project_path, &sha).await,
        ProviderKind::Gitea => c.gitea()?.ci_status(&r.owner, &r.repo, &sha).await,
    }
}

// ── GitLab / Gitea tokens and repo lists ────────────────────────────

/// Store (or with `None`/empty, remove) the token for `provider`
/// (`gitlab` / `gitea`) of the configured instance.
#[tauri::command]
pub async fn hosting_set_token(
    app: tauri::AppHandle,
    provider: String,
    token: Option<String>,
) -> Result<(), TwigError> {
    let kind = parse_provider(&provider)?;
    if kind == ProviderKind::GitHub {
        return credentials::set_github_token(token.map(|t| t.trim().to_string())).await;
    }
    let cfg = config::load(&app);
    credentials::set_token_for(account_for(&cfg, kind)?, token.map(|t| t.trim().to_string())).await
}

#[tauri::command]
pub async fn hosting_has_token(app: tauri::AppHandle, provider: String) -> Result<bool, TwigError> {
    let kind = parse_provider(&provider)?;
    let cfg = config::load(&app);
    Ok(token_for(&app, &cfg, kind).await.is_ok())
}

#[tauri::command]
pub async fn hosting_validate_token(app: tauri::AppHandle, provider: String) -> Result<GitHubUser, TwigError> {
    let kind = parse_provider(&provider)?;
    let cfg = config::load(&app);
    let token = token_for(&app, &cfg, kind).await?;
    let client = build_client()?;
    match kind {
        ProviderKind::GitHub => github::validate_token(&client, &cfg.github.api_base, &token).await,
        ProviderKind::GitLab => {
            let base = base_for(&cfg, kind)?;
            GitLab { client: &client, base: &base, token: &token }.validate().await
        }
        ProviderKind::Gitea => {
            let base = base_for(&cfg, kind)?;
            Gitea { client: &client, base: &base, token: &token }.validate().await
        }
    }
}

#[tauri::command]
pub async fn hosting_list_repos(
    app: tauri::AppHandle,
    provider: String,
    page: u32,
    per_page: u32,
) -> Result<RepoListPage, TwigError> {
    let kind = parse_provider(&provider)?;
    let cfg = config::load(&app);
    let token = token_for(&app, &cfg, kind).await?;
    let client = build_client()?;
    let per_page = per_page.clamp(1, 100);
    match kind {
        ProviderKind::GitHub => github::list_repos(&client, &cfg.github.api_base, &token, page, per_page, "updated").await,
        ProviderKind::GitLab => {
            let base = base_for(&cfg, kind)?;
            GitLab { client: &client, base: &base, token: &token }.list_repos(page, per_page).await
        }
        ProviderKind::Gitea => {
            let base = base_for(&cfg, kind)?;
            Gitea { client: &client, base: &base, token: &token }.list_repos(page, per_page).await
        }
    }
}

// ── GitHub OAuth device flow ────────────────────────────────────────

struct PendingFlow {
    id: u64,
    device_code: String,
    interval: u64,
    expires_at: Instant,
    cancelled: Arc<AtomicBool>,
}

static FLOWS: Mutex<Vec<PendingFlow>> = Mutex::new(Vec::new());
static NEXT_FLOW: AtomicU64 = AtomicU64::new(1);

/// Start a device flow. The device code stays in Rust; the UI only gets
/// the user code and verification URL to show.
#[tauri::command]
pub async fn github_device_start(app: tauri::AppHandle) -> Result<DeviceFlowStart, TwigError> {
    if GITHUB_OAUTH_CLIENT_ID.is_empty() {
        return Err(TwigError::GitHub("Sign in with GitHub is not configured in this build.".into()));
    }
    let ep = endpoint(&app);
    let client = build_client()?;
    let code = github_api::request_device_code(&client, &ep).await?;
    let id = NEXT_FLOW.fetch_add(1, Ordering::Relaxed);
    let mut flows = FLOWS.lock().map_err(|_| TwigError::Lock)?;
    // Only one sign-in at a time: cancel older ones.
    for f in flows.iter() {
        f.cancelled.store(true, Ordering::Relaxed);
    }
    flows.push(PendingFlow {
        id,
        device_code: code.device_code,
        interval: code.interval.max(1),
        expires_at: Instant::now() + Duration::from_secs(code.expires_in),
        cancelled: Arc::new(AtomicBool::new(false)),
    });
    Ok(DeviceFlowStart {
        flow_id: id,
        user_code: code.user_code,
        verification_uri: code.verification_uri,
        expires_in: code.expires_in,
    })
}

fn take_flow_params(id: u64) -> Result<(String, u64, Instant, Arc<AtomicBool>), TwigError> {
    let flows = FLOWS.lock().map_err(|_| TwigError::Lock)?;
    flows
        .iter()
        .find(|f| f.id == id)
        .map(|f| (f.device_code.clone(), f.interval, f.expires_at, f.cancelled.clone()))
        .ok_or_else(|| TwigError::GitHub("Sign-in session not found. Please start again.".into()))
}

fn remove_flow(id: u64) {
    if let Ok(mut flows) = FLOWS.lock() {
        flows.retain(|f| f.id != id);
    }
}

async fn sleep_secs(secs: u64) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(Duration::from_secs(secs))).await;
}

/// Poll until the user authorizes (or the flow expires / is cancelled),
/// then store the token in the keyring and return the signed-in user.
#[tauri::command]
pub async fn github_device_wait(app: tauri::AppHandle, flow_id: u64) -> Result<GitHubUser, TwigError> {
    let result = device_wait_inner(&app, flow_id).await;
    remove_flow(flow_id);
    result
}

async fn device_wait_inner(app: &tauri::AppHandle, flow_id: u64) -> Result<GitHubUser, TwigError> {
    let (device_code, mut interval, expires_at, cancelled) = take_flow_params(flow_id)?;
    let ep = endpoint(app);
    let client = build_client()?;
    loop {
        // Sleep in 1s slices so a cancel takes effect promptly.
        for _ in 0..interval {
            if cancelled.load(Ordering::Relaxed) {
                return Err(TwigError::GitHub("Sign-in cancelled.".into()));
            }
            sleep_secs(1).await;
        }
        if cancelled.load(Ordering::Relaxed) {
            return Err(TwigError::GitHub("Sign-in cancelled.".into()));
        }
        if Instant::now() >= expires_at {
            return Err(TwigError::GitHub("The sign-in code expired. Please start again.".into()));
        }
        match github_api::poll_device_token(&client, &ep, &device_code).await? {
            PollOutcome::Pending => {}
            PollOutcome::SlowDown(new) => interval = new.unwrap_or(interval + 5),
            PollOutcome::Failed(msg) => return Err(TwigError::GitHub(msg)),
            PollOutcome::Token(token) => {
                credentials::set_github_token(Some(token.clone())).await?;
                return github::validate_token(&client, &ep.api_base, &token).await;
            }
        }
    }
}

#[tauri::command]
pub async fn github_device_cancel(flow_id: u64) -> Result<(), TwigError> {
    let flows = FLOWS.lock().map_err(|_| TwigError::Lock)?;
    if let Some(f) = flows.iter().find(|f| f.id == flow_id) {
        f.cancelled.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_refs() {
        assert_eq!(pr_head_ref(ProviderKind::GitHub, 4), "refs/pull/4/head");
        assert_eq!(pr_head_ref(ProviderKind::Gitea, 4), "refs/pull/4/head");
        assert_eq!(pr_head_ref(ProviderKind::GitLab, 4), "refs/merge-requests/4/head");
    }

    #[test]
    fn providers() {
        assert_eq!(parse_provider("gitlab").ok(), Some(ProviderKind::GitLab));
        assert!(parse_provider("bitbucket").is_err());
    }
}
