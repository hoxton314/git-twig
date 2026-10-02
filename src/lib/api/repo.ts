/** Repository tabs, session, window and app-shell commands. */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  CloneProgress,
  AppSettings,
  RepoHistory,
  RepoInfo,
  RepoStatusSummary,
  RepoStatusRow,
  CommandResult,
  Session,
} from "../types/git";

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

// ── Repository dashboard (any repo path, not only open tabs) ─────────

export function getDashboardStatus(paths: string[]): Promise<RepoStatusRow[]> {
  return invoke<RepoStatusRow[]>("get_dashboard_status", { paths });
}

export function dashboardFetch(path: string): Promise<CommandResult> {
  return invoke<CommandResult>("dashboard_fetch", { path });
}

/** Fast-forward the current branch to its upstream (never merges). */
export function dashboardPull(path: string): Promise<CommandResult> {
  return invoke<CommandResult>("dashboard_pull", { path });
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

// ── App shell (status bar, recent repos, settings import/export) ──────


export function getRepoStatusSummary(path: string): Promise<RepoStatusSummary> {
  return invoke<RepoStatusSummary>("get_repo_status_summary", { path });
}

export function loadRepoHistory(): Promise<RepoHistory> {
  return invoke<RepoHistory>("load_repo_history");
}

export function saveRepoHistory(history: RepoHistory): Promise<void> {
  return invoke<void>("save_repo_history", { history });
}

/** Whether each path still exists as a directory. */
export function repoPathsExist(paths: string[]): Promise<boolean[]> {
  return invoke<boolean[]>("repo_paths_exist", { paths });
}

export function openInFileManager(path: string): Promise<void> {
  return invoke<void>("open_in_file_manager", { path });
}

/** Opens the app-data folder; resolves to its path. */
export function openSettingsFolder(): Promise<string> {
  return invoke<string>("open_settings_folder");
}

export function exportSettings(path: string, settings: AppSettings): Promise<void> {
  return invoke<void>("export_settings", { path, settings });
}

export function importSettings(path: string): Promise<AppSettings> {
  return invoke<AppSettings>("import_settings", { path });
}

// ── Create repositories (git init / clone from any URL) ─────────────

/** `git init` a folder (created if missing) and open it. */
export function initRepository(path: string, initialBranch: string | null): Promise<RepoInfo> {
  return invoke<RepoInfo>("init_repository", { path, initialBranch });
}

/** Clone any URL into `destination` (absolute, missing or empty) and open it. */
export function cloneRepository(url: string, destination: string, opId: number): Promise<RepoInfo> {
  return invoke<RepoInfo>("clone_repository", { url, destination, opId });
}

/** Subscribe to `git clone` progress lines (filter by `op_id`). */
export function onCloneProgress(handler: (p: CloneProgress) => void): Promise<UnlistenFn> {
  return listen<CloneProgress>("clone-progress", (e) => handler(e.payload));
}

/** Open a terminal in the repository folder. */
export function openInTerminal(path: string): Promise<void> {
  return invoke("open_in_terminal", { path });
}

/** Open the repository (or `file`, repo-relative) in the configured editor. */
export function openInEditor(path: string, file: string | null = null): Promise<void> {
  return invoke("open_in_editor", { path, file });
}

// ── Command line (`twig [path…]`) ───────────────────────────────────

/** Drain queued command-line paths (startup args and forwarded launches). */
export function takePendingPaths(): Promise<string[]> {
  return invoke<string[]>("take_pending_paths");
}

/** Ping when a later `twig <path>` queued paths; call `takePendingPaths`. */
export function onOpenPaths(handler: () => void): Promise<UnlistenFn> {
  return listen("open-paths", () => handler());
}
