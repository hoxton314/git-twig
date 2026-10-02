/** Branches, upstreams, remotes and branch comparison. */
import { invoke } from "@tauri-apps/api/core";
import type {
  BranchComparison,
  BranchInfo,
  CommandResult,
  RemoteInfo,
} from "../types/git";

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
