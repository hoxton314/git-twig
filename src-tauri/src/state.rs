use git2::Repository;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use crate::error::TwigError;

/// Per-repo metadata. git2 handles are opened per read (see `read_repo`).
pub struct OpenRepo {
    pub path: PathBuf,
    /// Labels of the windows that have this repository open as a tab. The
    /// entry goes away when the last one closes it.
    pub windows: HashSet<String>,
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

impl AppState {
    /// Record that `window` has the repository `key` (at `path`) open.
    pub fn register(&self, key: String, path: PathBuf, window: &str) -> Result<(), TwigError> {
        let mut repos = self.repos.lock().map_err(|_| TwigError::Lock)?;
        let entry = repos.entry(key).or_insert_with(|| OpenRepo { path: path.clone(), windows: HashSet::new() });
        entry.path = path;
        entry.windows.insert(window.to_string());
        Ok(())
    }

    /// `window` closed its tab for `key`; forget the repository once no
    /// window has it open.
    pub fn release(&self, key: &str, window: &str) -> Result<(), TwigError> {
        let mut repos = self.repos.lock().map_err(|_| TwigError::Lock)?;
        if let Some(entry) = repos.get_mut(key) {
            entry.windows.remove(window);
            if entry.windows.is_empty() {
                repos.remove(key);
            }
        }
        Ok(())
    }

    /// A window went away: release everything it had open.
    pub fn release_window(&self, window: &str) -> Result<(), TwigError> {
        let mut repos = self.repos.lock().map_err(|_| TwigError::Lock)?;
        repos.retain(|_, entry| {
            entry.windows.remove(window);
            !entry.windows.is_empty()
        });
        Ok(())
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repositories_stay_open_while_any_window_uses_them() {
        let state = AppState::new();
        let p = PathBuf::from("/r");
        state.register("/r".into(), p.clone(), "main").unwrap();
        state.register("/r".into(), p.clone(), "win-2").unwrap();
        state.register("/s".into(), PathBuf::from("/s"), "win-2").unwrap();

        state.release("/r", "main").unwrap();
        assert!(state.repo_path("/r").is_ok(), "win-2 still has it");
        state.release("/r", "nope").unwrap();
        state.release_window("win-2").unwrap();
        assert!(state.repo_path("/r").is_err());
        assert!(state.repo_path("/s").is_err());

        // Closing the same tab twice is harmless.
        state.register("/r".into(), p, "main").unwrap();
        state.release("/r", "main").unwrap();
        state.release("/r", "main").unwrap();
        assert!(state.repo_path("/r").is_err());
    }
}

