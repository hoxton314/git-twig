/** Repository tabs, session, window and app-shell commands. */
import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  RepoHistory,
  RepoInfo,
  RepoStatusSummary,
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
