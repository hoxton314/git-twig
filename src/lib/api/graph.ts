/** Commit graph paging, search and locate. */
import { invoke } from "@tauri-apps/api/core";
import type {
  SignatureInfo,
  CommitGraph,
  CommitSearchResult,
  GraphOptions,
  LocatedCommit,
} from "../types/git";

// ── Commit graph ──────────────────────────────────────────────────────

/**
 * One page of the commit graph: rows `[skip, skip + maxCommits)`. Lanes are
 * computed over the whole prefix, so pages with equal `tips` line up.
 */
export function getCommitGraph(
  path: string,
  maxCommits?: number,
  skip?: number,
  options?: GraphOptions,
): Promise<CommitGraph> {
  return invoke<CommitGraph>("get_commit_graph", {
    path,
    maxCommits: maxCommits ?? null,
    skip: skip ?? null,
    options: options ?? null,
  });
}

// ── Commit graph: search & locate ────────────────────────────────────


/** Search the full history by message, author name/email or SHA prefix. */
export function searchCommits(
  path: string,
  query: string,
  options?: GraphOptions,
  maxResults?: number,
): Promise<CommitSearchResult> {
  return invoke<CommitSearchResult>("search_commits", {
    path,
    query,
    options: options ?? null,
    maxResults: maxResults ?? null,
  });
}

/**
 * Search the history for code changes: commits that add or remove `query`
 * (`git log -S`), or with `regex`, whose changed lines match it (`-G`).
 */
export function searchChanges(
  path: string,
  query: string,
  opts: { regex: boolean; ignoreCase: boolean; paths: string[] },
  options?: GraphOptions,
  maxResults?: number,
): Promise<CommitSearchResult> {
  return invoke<CommitSearchResult>("search_changes", {
    path,
    query,
    regex: opts.regex,
    ignoreCase: opts.ignoreCase,
    paths: opts.paths,
    options: options ?? null,
    maxResults: maxResults ?? null,
  });
}

/** Stop a running code-change search (the search bar closed or switched). */
export function cancelSearchChanges(path: string): Promise<void> {
  return invoke<void>("cancel_search_changes", { path });
}

/** Resolve a revision (branch, tag, HEAD, SHA) to a commit and its graph row. */
export function locateCommit(
  path: string,
  rev: string,
  options?: GraphOptions,
): Promise<LocatedCommit> {
  return invoke<LocatedCommit>("locate_commit", {
    path,
    rev,
    options: options ?? null,
  });
}

/** Verify signatures of `oids`; unsigned commits are left out of the result. */
export function commitSignatures(path: string, oids: string[]): Promise<SignatureInfo[]> {
  return invoke<SignatureInfo[]>("commit_signatures", { path, oids });
}
