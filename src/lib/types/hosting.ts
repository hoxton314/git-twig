// ── Hosting integrations (GitHub / GitLab / Gitea) ──────────────────
// Mirrors src-tauri/src/hosting/{types,remote}.rs and commands/hosting.rs.

import type { DiffHunk } from "./git";

export type ProviderKind = "github" | "gitlab" | "gitea";

export interface HostedRemote {
  provider: ProviderKind;
  remote_name: string;
  host: string;
  owner: string;
  repo: string;
  project_path: string;
  web_url: string;
}

export interface HostingInfo {
  github_host: string;
  github_api_base: string;
  github_oauth_available: boolean;
  gitlab_base: string | null;
  gitea_base: string | null;
}

export interface PrLabel {
  name: string;
  color: string | null;
}

export type PrState = "open" | "closed" | "merged";
export type PrFilter = "open" | "closed" | "merged" | "all";
export type ReviewState = "approved" | "changes_requested" | "review_required";
export type CiState = "success" | "failure" | "pending" | "neutral" | "none";

export interface PrSummary {
  number: number;
  title: string;
  state: PrState;
  draft: boolean;
  author: string;
  author_avatar: string | null;
  head_ref: string;
  head_sha: string;
  head_repo: string | null;
  base_ref: string;
  html_url: string;
  created_at: string;
  updated_at: string;
  comments: number;
  review_state: ReviewState | null;
  ci_state: CiState | null;
  labels: PrLabel[];
}

export interface PrPage {
  items: PrSummary[];
  next_cursor: string | null;
}

export interface PrDetail {
  summary: PrSummary;
  body: string;
  additions: number | null;
  deletions: number | null;
  changed_files: number | null;
  commits: number | null;
  mergeable_state: string | null;
}

export interface PrFile {
  path: string;
  old_path: string | null;
  status: "added" | "deleted" | "modified" | "renamed";
  additions: number;
  deletions: number;
  hunks: DiffHunk[];
  patch_missing: boolean;
}

export interface CiCheck {
  name: string;
  state: Exclude<CiState, "none">;
  description: string | null;
  url: string | null;
}

export interface CiStatus {
  sha: string;
  state: CiState;
  checks: CiCheck[];
}

export interface DeviceFlowStart {
  flow_id: number;
  user_code: string;
  verification_uri: string;
  expires_in: number;
}
