/**
 * Typed wrappers for all Tauri invoke() calls.
 * One function per Tauri command — no direct invoke() elsewhere in the frontend.
 */
import { invoke } from "@tauri-apps/api/core";
import type {
  RepoInfo,
  CommitGraph,
  BranchInfo,
  DiffFile,
  CommandResult,
  WorkingStatus,
  Session,
  AppSettings,
  GitConfig,
  StashEntry,
} from "./types/git";
import type {
  GitHubUser,
  GitHubRepo,
  GitHubPullRequest,
  RepoListPage,
  GitHubRemoteInfo,
} from "./types/github";

// ── Repo management ───────────────────────────────────────────────────

export function openRepo(path: string): Promise<RepoInfo> {
  return invoke<RepoInfo>("open_repo", { path });
}

export function closeRepo(path: string): Promise<void> {
  return invoke<void>("close_repo", { path });
}

export function getRepoInfo(path: string): Promise<RepoInfo> {
  return invoke<RepoInfo>("get_repo_info", { path });
}

export function listOpenRepos(): Promise<string[]> {
  return invoke<string[]>("list_open_repos");
}

export function listReposInDir(dir: string): Promise<RepoInfo[]> {
  return invoke<RepoInfo[]>("list_repos_in_dir", { dir });
}

// ── Session persistence ───────────────────────────────────────────────

export function saveSession(
  paths: string[],
  active: string | null,
  sidebarWidth: number | null,
  stagingWidth: number | null,
  diffPanelRatio: number | null,
): Promise<void> {
  return invoke<void>("save_session", {
    paths,
    active,
    sidebarWidth,
    stagingWidth,
    diffPanelRatio,
  });
}

export function loadSession(): Promise<Session> {
  return invoke<Session>("load_session");
}

// ── Window ────────────────────────────────────────────────────────────

export function isTilingWm(): Promise<boolean> {
  return invoke<boolean>("is_tiling_wm");
}

/** False for installs the in-app updater must not touch (AUR, dev builds). */
export function updaterSupported(): Promise<boolean> {
  return invoke<boolean>("updater_supported");
}

// ── Commit graph ──────────────────────────────────────────────────────

export function getCommitGraph(
  path: string,
  maxCommits?: number
): Promise<CommitGraph> {
  return invoke<CommitGraph>("get_commit_graph", {
    path,
    maxCommits: maxCommits ?? null,
  });
}

// ── Branches ──────────────────────────────────────────────────────────

export function getBranches(path: string): Promise<BranchInfo[]> {
  return invoke<BranchInfo[]>("get_branches", { path });
}

export function checkoutBranch(
  path: string,
  branchName: string
): Promise<CommandResult> {
  return invoke<CommandResult>("checkout_branch", { path, branchName });
}

export function checkoutRemoteBranch(
  path: string,
  branchName: string
): Promise<CommandResult> {
  return invoke<CommandResult>("checkout_remote_branch", { path, branchName });
}

export function createBranch(
  path: string,
  branchName: string,
  startPoint?: string
): Promise<CommandResult> {
  return invoke<CommandResult>("create_branch", {
    path,
    branchName,
    startPoint: startPoint ?? null,
  });
}

export function renameBranch(
  path: string,
  oldName: string,
  newName: string
): Promise<CommandResult> {
  return invoke<CommandResult>("rename_branch", { path, oldName, newName });
}

export function deleteBranch(
  path: string,
  branchName: string,
  force: boolean = false
): Promise<CommandResult> {
  return invoke<CommandResult>("delete_branch", { path, branchName, force });
}

export function deleteRemoteBranch(
  path: string,
  remote: string,
  branchName: string
): Promise<CommandResult> {
  return invoke<CommandResult>("delete_remote_branch", {
    path,
    remote,
    branchName,
  });
}

export function pushBranch(
  path: string,
  branchName: string,
  remote?: string,
  setUpstream: boolean = false
): Promise<CommandResult> {
  return invoke<CommandResult>("push_branch", {
    path,
    remote: remote ?? null,
    branchName,
    setUpstream,
  });
}

export function mergeBranch(
  path: string,
  branchName: string
): Promise<CommandResult> {
  return invoke<CommandResult>("merge_branch", { path, branchName });
}

export function fetchAll(path: string): Promise<CommandResult> {
  return invoke<CommandResult>("fetch_all", { path });
}

// ── Diffs ─────────────────────────────────────────────────────────────

export function getCommitDiff(
  path: string,
  oid: string
): Promise<DiffFile[]> {
  return invoke<DiffFile[]>("get_commit_diff", { path, oid });
}

export function getWorkingDiff(path: string): Promise<DiffFile[]> {
  return invoke<DiffFile[]>("get_working_diff", { path });
}

export function getFileBlob(
  path: string,
  filePath: string,
  source: string,
): Promise<string | null> {
  return invoke<string | null>("get_file_blob", { path, filePath, source });
}

// ── Staging & working directory ───────────────────────────────────────

export function getWorkingStatus(path: string): Promise<WorkingStatus> {
  return invoke<WorkingStatus>("get_working_status", { path });
}

export function getStagedDiff(
  path: string,
  filePath?: string
): Promise<DiffFile[]> {
  return invoke<DiffFile[]>("get_staged_diff", {
    path,
    filePath: filePath ?? null,
  });
}

export function getUnstagedDiff(
  path: string,
  filePath?: string
): Promise<DiffFile[]> {
  return invoke<DiffFile[]>("get_unstaged_diff", {
    path,
    filePath: filePath ?? null,
  });
}

export function stageFiles(
  path: string,
  files: string[]
): Promise<CommandResult> {
  return invoke<CommandResult>("stage_files", { path, files });
}

export function unstageFiles(
  path: string,
  files: string[]
): Promise<CommandResult> {
  return invoke<CommandResult>("unstage_files", { path, files });
}

export function discardFiles(
  path: string,
  tracked: string[],
  untracked: string[]
): Promise<CommandResult> {
  return invoke<CommandResult>("discard_files", { path, tracked, untracked });
}

export function undoCommit(path: string): Promise<CommandResult> {
  return invoke<CommandResult>("undo_commit", { path });
}

export function createCommit(
  path: string,
  message: string
): Promise<CommandResult> {
  return invoke<CommandResult>("create_commit", { path, message });
}

export function pull(
  path: string,
  remote?: string,
  branch?: string
): Promise<CommandResult> {
  return invoke<CommandResult>("pull", {
    path,
    remote: remote ?? null,
    branch: branch ?? null,
  });
}

// ── Stash ────────────────────────────────────────────────────

export function stashList(path: string): Promise<StashEntry[]> {
  return invoke<StashEntry[]>("stash_list", { path });
}

export function stashPush(
  path: string,
  message?: string
): Promise<CommandResult> {
  return invoke<CommandResult>("stash_push", {
    path,
    message: message ?? null,
  });
}

export function stashPop(path: string, index: number): Promise<CommandResult> {
  return invoke<CommandResult>("stash_pop", { path, index });
}

export function stashApply(
  path: string,
  index: number
): Promise<CommandResult> {
  return invoke<CommandResult>("stash_apply", { path, index });
}

export function stashDrop(
  path: string,
  index: number
): Promise<CommandResult> {
  return invoke<CommandResult>("stash_drop", { path, index });
}

// ── Settings ─────────────────────────────────────────────────────────

export function loadSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("load_settings");
}

export function saveSettings(settings: AppSettings): Promise<void> {
  return invoke<void>("save_settings", { settings });
}

// ── Git Config ───────────────────────────────────────────────────────

export function getGitConfig(): Promise<GitConfig> {
  return invoke<GitConfig>("get_git_config");
}

export function setGitConfig(config: GitConfig): Promise<void> {
  return invoke<void>("set_git_config", { config });
}

// ── GitHub ───────────────────────────────────────────────────────────

/** Store the token in the OS keyring; `null` removes it. */
export function githubSetToken(token: string | null): Promise<void> {
  return invoke<void>("github_set_token", { token });
}

/** Whether a token is configured (the token itself never reaches the UI). */
export function githubHasToken(): Promise<boolean> {
  return invoke<boolean>("github_has_token");
}

export function githubValidateToken(): Promise<GitHubUser> {
  return invoke<GitHubUser>("github_validate_token");
}

export function githubListRepos(
  page: number,
  perPage: number = 30,
  sort: string = "updated",
): Promise<RepoListPage> {
  return invoke<RepoListPage>("github_list_repos", { page, perPage, sort });
}

export function githubCloneRepo(
  cloneUrl: string,
  destination: string,
): Promise<RepoInfo> {
  return invoke<RepoInfo>("github_clone_repo", { cloneUrl, destination });
}

export function githubCreateRepo(
  name: string,
  description: string | null,
  private_: boolean,
  autoInit: boolean,
): Promise<GitHubRepo> {
  return invoke<GitHubRepo>("github_create_repo", {
    name,
    description,
    private: private_,
    autoInit,
  });
}

export function githubDetectRemote(
  path: string,
): Promise<GitHubRemoteInfo | null> {
  return invoke<GitHubRemoteInfo | null>("github_detect_remote", { path });
}

export function githubCreatePullRequest(
  owner: string,
  repo: string,
  title: string,
  body: string,
  head: string,
  base: string,
): Promise<GitHubPullRequest> {
  return invoke<GitHubPullRequest>("github_create_pull_request", {
    owner,
    repo,
    title,
    body,
    head,
    base,
  });
}

export function githubListBranches(
  owner: string,
  repo: string,
): Promise<string[]> {
  return invoke<string[]>("github_list_branches", { owner, repo });
}

// ── Hosting integrations (PRs, CI, OAuth device flow, GitLab/Gitea) ──

import type {
  HostedRemote,
  HostingInfo,
  PrPage,
  PrDetail,
  PrFile,
  PrFilter,
  CiStatus,
  DeviceFlowStart,
  ProviderKind,
} from "./types/hosting";

/** GitHub/GitLab/Gitea remotes of a repo, preferred (upstream, origin) first. */
export function hostingListRemotes(path: string): Promise<HostedRemote[]> {
  return invoke<HostedRemote[]>("hosting_list_remotes", { path });
}

export function hostingInfo(): Promise<HostingInfo> {
  return invoke<HostingInfo>("hosting_info");
}

export function hostingListPrs(
  path: string,
  remoteName: string | null,
  filter: PrFilter,
  cursor: string | null = null,
): Promise<PrPage> {
  return invoke<PrPage>("hosting_list_prs", { path, remoteName, filter, cursor });
}

export function hostingGetPr(path: string, remoteName: string | null, number: number): Promise<PrDetail> {
  return invoke<PrDetail>("hosting_get_pr", { path, remoteName, number });
}

export function hostingPrFiles(path: string, remoteName: string | null, number: number): Promise<PrFile[]> {
  return invoke<PrFile[]>("hosting_pr_files", { path, remoteName, number });
}

/** Fetch the PR head and check it out as local branch `pr/<number>`. */
export function hostingCheckoutPr(path: string, remoteName: string | null, number: number): Promise<CommandResult> {
  return invoke<CommandResult>("hosting_checkout_pr", { path, remoteName, number });
}

export function hostingCreatePr(
  path: string,
  remoteName: string | null,
  title: string,
  body: string,
  head: string,
  base: string,
): Promise<GitHubPullRequest> {
  return invoke<GitHubPullRequest>("hosting_create_pr", { path, remoteName, title, body, head, base });
}

export function hostingListBranches(path: string, remoteName: string | null): Promise<string[]> {
  return invoke<string[]>("hosting_list_branches", { path, remoteName });
}

export function hostingCiStatus(path: string, remoteName: string | null, sha: string): Promise<CiStatus> {
  return invoke<CiStatus>("hosting_ci_status", { path, remoteName, sha });
}

/** Store (null = remove) a token in the OS keyring for the configured instance. */
export function hostingSetToken(provider: ProviderKind, token: string | null): Promise<void> {
  return invoke<void>("hosting_set_token", { provider, token });
}

export function hostingHasToken(provider: ProviderKind): Promise<boolean> {
  return invoke<boolean>("hosting_has_token", { provider });
}

export function hostingValidateToken(provider: ProviderKind): Promise<GitHubUser> {
  return invoke<GitHubUser>("hosting_validate_token", { provider });
}

export function hostingListRepos(provider: ProviderKind, page: number, perPage: number = 30): Promise<RepoListPage> {
  return invoke<RepoListPage>("hosting_list_repos", { provider, page, perPage });
}

/** Start "Sign in with GitHub" (device flow); the device code stays in Rust. */
export function githubDeviceStart(): Promise<DeviceFlowStart> {
  return invoke<DeviceFlowStart>("github_device_start");
}

/** Resolves once the user authorized (token is then in the keyring). */
export function githubDeviceWait(flowId: number): Promise<GitHubUser> {
  return invoke<GitHubUser>("github_device_wait", { flowId });
}

export function githubDeviceCancel(flowId: number): Promise<void> {
  return invoke<void>("github_device_cancel", { flowId });
}
