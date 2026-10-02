pub mod reader;
#[allow(dead_code)]
pub mod writer;
// Commit context menu, undo history (reflog), tags
pub mod commit_ops;
pub mod reflog;
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

// Commit graph: pagination, search, locate
pub mod graph;
// File history & blame, stash extras, submodules, worktrees
pub mod file_history;
pub mod stash_extra;
pub mod submodules;
pub mod worktrees;
