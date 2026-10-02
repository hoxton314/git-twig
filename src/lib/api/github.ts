/** GitHub token and REST helpers (clone, create repo/PR). */
import { invoke } from "@tauri-apps/api/core";
import type {
  RepoInfo,
} from "../types/git";
import type {
  GitHubPullRequest,
  GitHubRemoteInfo,
  GitHubRepo,
  GitHubUser,
  RepoListPage,
} from "../types/github";

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
