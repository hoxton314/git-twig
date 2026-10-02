use git2::Repository;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::error::TwigError;

/// Per-repo metadata. git2 handles are opened per read (see `read_repo`).
pub struct OpenRepo {
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

    /// Resolve the on-disk path of an open repo. The state lock is held only
    /// for the lookup, never across git work or an `.await`.
    pub fn repo_path(&self, key: &str) -> Result<PathBuf, TwigError> {
        let repos = self.repos.lock().map_err(|_| TwigError::Lock)?;
        repos
            .get(key)
            .map(|open| open.path.clone())
            .ok_or_else(|| TwigError::RepoNotFound(key.to_string()))
    }

    /// Run a git2 read operation against an open repo on a blocking thread.
    ///
    /// A fresh `Repository` handle is opened for every call so that:
    ///   - the global state lock is not held while walking history or
    ///     building diffs (which would serialize every repo's reads), and
    ///     no blocking git2 work runs on the async runtime's worker threads;
    ///   - reads never observe stale libgit2 caches (index, config, refs)
    ///     after CLI writes made by `git::writer`.
    pub async fn read_repo<T, F>(&self, key: &str, f: F) -> Result<T, TwigError>
    where
        T: Send + 'static,
        F: FnOnce(&Repository) -> Result<T, TwigError> + Send + 'static,
    {
        let path = self.repo_path(key)?;
        tauri::async_runtime::spawn_blocking(move || {
            let repo = Repository::open(&path)?;
            f(&repo)
        })
        .await
        .map_err(|e| TwigError::Task(e.to_string()))?
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
