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

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("GitHub API error: {0}")]
    GitHub(String),

    #[error("Lock error: failed to acquire state lock")]
    Lock,
}

impl From<reqwest::Error> for TwigError {
    fn from(e: reqwest::Error) -> Self {
        TwigError::Http(e.to_string())
    }
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
