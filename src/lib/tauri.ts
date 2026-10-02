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
// File history & blame, stash extras, submodules, worktrees
import type {
  FileHistoryPage,
  BlameResult,
  StashDetail,
  StashDiff,
  SubmoduleInfo,
  WorktreeInfo,
} from "./types/git";

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

// ── File history & blame ─────────────────────────────────────────────

export function getFileHistory(
  path: string,
  filePath: string,
  skip: number,
  limit: number,
  rev?: string,
): Promise<FileHistoryPage> {
  return invoke<FileHistoryPage>("get_file_history", {
    path,
    filePath,
    rev: rev ?? null,
    skip,
    limit,
  });
}

export function getFileDiffAtCommit(
  path: string,
  oid: string,
  filePath: string,
  oldPath?: string | null,
): Promise<DiffFile[]> {
  return invoke<DiffFile[]>("get_file_diff_at_commit", {
    path,
    oid,
    filePath,
    oldPath: oldPath ?? null,
  });
}

export function getBlame(
  path: string,
  filePath: string,
  rev?: string,
): Promise<BlameResult> {
  return invoke<BlameResult>("get_blame", { path, filePath, rev: rev ?? null });
}

export function listTrackedFiles(path: string): Promise<string[]> {
  return invoke<string[]>("list_tracked_files", { path });
}

// ── Stash extras ─────────────────────────────────────────────────────

export function stashListDetailed(path: string): Promise<StashDetail[]> {
  return invoke<StashDetail[]>("stash_list_detailed", { path });
}

export function stashShow(path: string, oid: string): Promise<StashDiff> {
  return invoke<StashDiff>("stash_show", { path, oid });
}

/** Apply/pop/drop the stash with commit `oid` (index is resolved server-side). */
export function stashAct(
  path: string,
  oid: string,
  action: "apply" | "pop" | "drop",
): Promise<CommandResult> {
  return invoke<CommandResult>("stash_act", { path, oid, action });
}

export function stashRename(
  path: string,
  oid: string,
  message: string,
): Promise<CommandResult> {
  return invoke<CommandResult>("stash_rename", { path, oid, message });
}

export function stashBranch(
  path: string,
  oid: string,
  branch: string,
): Promise<CommandResult> {
  return invoke<CommandResult>("stash_branch", { path, oid, branch });
}

export function stashPushExt(
  path: string,
  opts: {
    message?: string;
    files?: string[];
    keepIndex?: boolean;
    includeUntracked?: boolean;
  },
): Promise<CommandResult> {
  return invoke<CommandResult>("stash_push_ext", {
    path,
    message: opts.message ?? null,
    files: opts.files ?? [],
    keepIndex: opts.keepIndex ?? false,
    includeUntracked: opts.includeUntracked ?? true,
  });
}

// ── Submodules ───────────────────────────────────────────────────────

export function listSubmodules(path: string): Promise<SubmoduleInfo[]> {
  return invoke<SubmoduleInfo[]>("list_submodules", { path });
}

export function submoduleUpdate(
  path: string,
  subPath?: string,
): Promise<CommandResult> {
  return invoke<CommandResult>("submodule_update", { path, subPath: subPath ?? null });
}

export function submoduleSync(
  path: string,
  subPath?: string,
): Promise<CommandResult> {
  return invoke<CommandResult>("submodule_sync", { path, subPath: subPath ?? null });
}

// ── Worktrees ────────────────────────────────────────────────────────

export function listWorktrees(path: string): Promise<WorktreeInfo[]> {
  return invoke<WorktreeInfo[]>("list_worktrees", { path });
}

export function worktreeAdd(
  path: string,
  worktreePath: string,
  opts: { commitish?: string; newBranch?: string },
): Promise<CommandResult> {
  return invoke<CommandResult>("worktree_add", {
    path,
    worktreePath,
    commitish: opts.commitish ?? null,
    newBranch: opts.newBranch ?? null,
  });
}

export function worktreeRemove(
  path: string,
  worktreePath: string,
  force: boolean = false,
): Promise<CommandResult> {
  return invoke<CommandResult>("worktree_remove", { path, worktreePath, force });
}

export function worktreePrune(path: string): Promise<CommandResult> {
  return invoke<CommandResult>("worktree_prune", { path });
}
