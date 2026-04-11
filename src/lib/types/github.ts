// ── GitHub API types ─────────────────────────────────────────────────

export interface GitHubUser {
  login: string;
  name: string | null;
  avatar_url: string;
}

export interface GitHubOwner {
  login: string;
  avatar_url: string;
}

export interface GitHubRepo {
  id: number;
  full_name: string;
  name: string;
  owner: GitHubOwner;
  description: string | null;
  private: boolean;
  html_url: string;
  clone_url: string;
  ssh_url: string;
  default_branch: string;
  stargazers_count: number;
  updated_at: string;
  fork: boolean;
}

export interface GitHubPullRequest {
  number: number;
  html_url: string;
  title: string;
  state: string;
}

export interface RepoListPage {
  repos: GitHubRepo[];
  has_next_page: boolean;
}

export interface GitHubRemoteInfo {
  owner: string;
  repo: string;
  remote_name: string;
}
