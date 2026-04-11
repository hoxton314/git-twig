use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum TwigError {
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("Git CLI error: {0}")]
    GitCli(String),

    #[error("Repository not found: {0}")]
    RepoNotFound(String),

    #[error("Not a git repository: {0}")]
    NotARepo(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Lock error: failed to acquire state lock")]
    Lock,
}

// Tauri requires command errors to implement Serialize
impl Serialize for TwigError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
