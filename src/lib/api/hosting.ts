/** Provider-agnostic hosting: PRs/MRs, CI status, tokens, GitHub device flow. */
import { invoke } from "@tauri-apps/api/core";
import type {
  CommandResult,
} from "../types/git";
import type {
  GitHubPullRequest,
  GitHubUser,
  RepoListPage,
} from "../types/github";
import type {
  CiStatus,
  DeviceFlowStart,
  HostedRemote,
  HostingInfo,
  PrDetail,
  PrFile,
  PrFilter,
  PrPage,
  ProviderKind,
} from "../types/hosting";

// ── Hosting integrations (PRs, CI, OAuth device flow, GitLab/Gitea) ──


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
