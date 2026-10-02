pub mod branches;
pub mod diff;
pub mod git_config;
pub mod github;
pub mod graph;
pub mod repo;
pub mod session;
pub mod settings;
pub mod staging;
pub mod stash;
pub mod updater;
pub mod window;
// Commit context menu, undo history (reflog), tags
pub mod commit_ops;
pub mod tags;
// Staging panel: commit helpers & file actions
pub mod commit_tools;
pub mod file_ops;
// Branch list / remotes
pub mod branch_ops;
pub mod remotes;

// Conflict resolution & history rewriting (rebase, force push)
pub mod conflicts;
pub mod history;
// File history & blame, stash extras, submodules, worktrees
pub mod file_views;
pub mod stash_extra;
pub mod submodules;
pub mod worktrees;
// App shell (status bar, recent repos, settings import/export)
pub mod app_shell;
// Hosting integrations (PRs, CI, OAuth, GitLab/Gitea)
pub mod hosting;
