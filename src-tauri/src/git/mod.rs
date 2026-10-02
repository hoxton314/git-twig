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
