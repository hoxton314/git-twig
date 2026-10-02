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
  HeadCommitInfo,
  AuthorInfo,
  IgnoreKind,
  IgnoreResult,
} from "./types/git";
// Commit operations, undo history, tags
import type { CommitOpResult, ReflogEntry, ResetMode, TagInfo } from "./types/git";
import type {
  GitHubUser,
  GitHubRepo,
  GitHubPullRequest,
  RepoListPage,
  GitHubRemoteInfo,
} from "./types/github";
// Branch list / remotes
import type { RemoteInfo, BranchComparison } from "./types/git";

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

// ── Commit operations & undo history (reflog) ─────────────────────────

export function checkoutCommit(path: string, oid: string): Promise<CommitOpResult> {
  return invoke<CommitOpResult>("checkout_commit", { path, oid });
}

export function cherryPickCommit(path: string, oid: string): Promise<CommitOpResult> {
  return invoke<CommitOpResult>("cherry_pick_commit", { path, oid });
}

export function revertCommit(path: string, oid: string): Promise<CommitOpResult> {
  return invoke<CommitOpResult>("revert_commit", { path, oid });
}

export function resetToCommit(path: string, oid: string, mode: ResetMode): Promise<CommitOpResult> {
  return invoke<CommitOpResult>("reset_to_commit", { path, oid, mode });
}

export function getHeadReflog(path: string, limit?: number): Promise<ReflogEntry[]> {
  return invoke<ReflogEntry[]>("get_head_reflog", { path, limit: limit ?? null });
}

/** Move HEAD back to `oid` (checking out `branch` if it points there). */
export function restoreHead(
  path: string,
  oid: string,
  branch: string | null,
  autoStash: boolean,
): Promise<CommitOpResult> {
  return invoke<CommitOpResult>("restore_head", { path, oid, branch, autoStash });
}

// ── Tags ──────────────────────────────────────────────────────────────

export function getTags(path: string): Promise<TagInfo[]> {
  return invoke<TagInfo[]>("get_tags", { path });
}

/** A non-empty `message` creates an annotated tag. */
export function createTag(
  path: string,
  name: string,
  target: string,
  message?: string,
): Promise<CommandResult> {
  return invoke<CommandResult>("create_tag", { path, name, target, message: message ?? null });
}

export function deleteTag(path: string, name: string): Promise<CommandResult> {
  return invoke<CommandResult>("delete_tag", { path, name });
}

/** Push one tag, or all tags when `name` is omitted. */
export function pushTag(path: string, name?: string, remote?: string): Promise<CommandResult> {
  return invoke<CommandResult>("push_tag", { path, name: name ?? null, remote: remote ?? null });
}

export function deleteRemoteTag(path: string, name: string, remote?: string): Promise<CommandResult> {
  return invoke<CommandResult>("delete_remote_tag", { path, name, remote: remote ?? null });
}

// ── Staging panel: commit helpers & file actions ─────────────────────

export function createCommitWithOptions(
  path: string,
  message: string,
  amend: boolean,
  signoff: boolean,
): Promise<CommandResult> {
  return invoke<CommandResult>("create_commit_with_options", { path, message, amend, signoff });
}

export function getHeadCommitInfo(path: string): Promise<HeadCommitInfo> {
  return invoke<HeadCommitInfo>("get_head_commit_info", { path });
}

export function getRecentAuthors(path: string, maxCommits?: number): Promise<AuthorInfo[]> {
  return invoke<AuthorInfo[]>("get_recent_authors", { path, maxCommits: maxCommits ?? null });
}

export function getCommitTemplate(path: string): Promise<string | null> {
  return invoke<string | null>("get_commit_template", { path });
}

export function openRepoFile(path: string, file: string): Promise<void> {
  return invoke<void>("open_repo_file", { path, file });
}

export function revealRepoFile(path: string, file: string): Promise<void> {
  return invoke<void>("reveal_repo_file", { path, file });
}

export function addToGitignore(path: string, file: string, kind: IgnoreKind): Promise<IgnoreResult> {
  return invoke<IgnoreResult>("add_to_gitignore", { path, file, kind });
}

export function openExternalDiff(
  path: string,
  file: string,
  staged: boolean,
  tool: string | null,
): Promise<void> {
  return invoke<void>("open_external_diff", { path, file, staged, tool });
}

// ── Branch list / remotes ────────────────────────────────────────────

export function setBranchUpstream(
  path: string,
  branchName: string,
  upstream: string,
): Promise<CommandResult> {
  return invoke<CommandResult>("set_branch_upstream", { path, branchName, upstream });
}

export function unsetBranchUpstream(path: string, branchName: string): Promise<CommandResult> {
  return invoke<CommandResult>("unset_branch_upstream", { path, branchName });
}

/** Fast-forward a branch that is not checked out to its upstream. */
export function fastForwardBranch(path: string, branchName: string): Promise<CommandResult> {
  return invoke<CommandResult>("fast_forward_branch", { path, branchName });
}

/** Push to the branch's upstream, or to `remote` with --set-upstream. */
export function pushLocalBranch(
  path: string,
  branchName: string,
  remote?: string,
): Promise<CommandResult> {
  return invoke<CommandResult>("push_local_branch", {
    path,
    branchName,
    remote: remote ?? null,
  });
}

/** Rebase `branchName` (current branch if omitted) onto `onto`. */
export function rebaseBranch(
  path: string,
  onto: string,
  branchName?: string,
): Promise<CommandResult> {
  return invoke<CommandResult>("rebase_branch", {
    path,
    onto,
    branchName: branchName ?? null,
  });
}

export function rebaseBranchAbort(path: string): Promise<CommandResult> {
  return invoke<CommandResult>("rebase_branch_abort", { path });
}

/** Create a branch at `startPoint` without checking it out. */
export function createBranchAt(
  path: string,
  branchName: string,
  startPoint: string,
): Promise<CommandResult> {
  return invoke<CommandResult>("create_branch_at", { path, branchName, startPoint });
}

/** Checkout `<remote>/<branchName>` as a local tracking branch. */
export function checkoutRemoteTracking(
  path: string,
  remote: string,
  branchName: string,
): Promise<CommandResult> {
  return invoke<CommandResult>("checkout_remote_tracking", { path, remote, branchName });
}

export function compareBranches(
  path: string,
  base: string,
  other: string,
): Promise<BranchComparison> {
  return invoke<BranchComparison>("compare_branches", { path, base, other });
}

export function listRemotes(path: string): Promise<RemoteInfo[]> {
  return invoke<RemoteInfo[]>("list_remotes", { path });
}

export function addRemote(path: string, name: string, url: string): Promise<CommandResult> {
  return invoke<CommandResult>("add_remote", { path, name, url });
}

export function removeRemote(path: string, name: string): Promise<CommandResult> {
  return invoke<CommandResult>("remove_remote", { path, name });
}

export function renameRemote(
  path: string,
  oldName: string,
  newName: string,
): Promise<CommandResult> {
  return invoke<CommandResult>("rename_remote", { path, oldName, newName });
}

/** Set the fetch URL; a null/empty push URL removes a dedicated pushurl. */
export function setRemoteUrls(
  path: string,
  name: string,
  fetchUrl: string,
  pushUrl: string | null,
): Promise<CommandResult> {
  return invoke<CommandResult>("set_remote_urls", { path, name, fetchUrl, pushUrl });
}

export function fetchRemote(path: string, name: string): Promise<CommandResult> {
  return invoke<CommandResult>("fetch_remote", { path, name });
}

export function pruneRemote(path: string, name: string): Promise<CommandResult> {
  return invoke<CommandResult>("prune_remote", { path, name });
}
