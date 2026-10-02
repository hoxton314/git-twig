//! Git hosting integrations: provider configuration, remote URL parsing,
//! HTTPS auth for network git commands, and the provider-agnostic pull
//! request / CI model shared by GitHub, GitLab and Gitea/Forgejo.

pub mod config;
pub mod gitea;
pub mod github_api;
pub mod gitlab;
pub mod http;
pub mod net_auth;
pub mod patch;
pub mod remote;
pub mod types;
