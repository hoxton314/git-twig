use git2::Repository;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Per-repo metadata stored alongside the git2 Repository handle.
pub struct OpenRepo {
    pub repository: Repository,
    pub path: PathBuf,
}

/// Thread-safe application state holding all currently open repositories.
/// Keyed by the canonical path string of each repo's workdir.
pub struct AppState {
    pub repos: Mutex<HashMap<String, OpenRepo>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            repos: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
