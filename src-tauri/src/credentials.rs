//! Secrets stored in the OS credential store (Secret Service / Keychain /
//! Windows Credential Manager) instead of `settings.json`.

use keyring::Entry;

use crate::error::TwigError;

const SERVICE: &str = "dev.twig.app";
const GITHUB_ACCOUNT: &str = "github-token";

fn keyring_error(e: keyring::Error) -> TwigError {
    match e {
        keyring::Error::PlatformFailure(_) | keyring::Error::NoStorageAccess(_) => {
            TwigError::Keyring(format!(
                "{e}. On Linux, make sure a Secret Service provider \
                 (GNOME Keyring or KWallet) is running and unlocked."
            ))
        }
        other => TwigError::Keyring(other.to_string()),
    }
}

/// Keyring backends block (D-Bus round trips, Keychain prompts), so run
/// them off the async worker threads.
async fn run<T, F>(f: F) -> Result<T, TwigError>
where
    T: Send + 'static,
    F: FnOnce(&Entry) -> Result<T, keyring::Error> + Send + 'static,
{
    run_as(GITHUB_ACCOUNT, f).await
}

async fn run_as<T, F>(account: &'static str, f: F) -> Result<T, TwigError>
where
    T: Send + 'static,
    F: FnOnce(&Entry) -> Result<T, keyring::Error> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let entry = Entry::new(SERVICE, account).map_err(keyring_error)?;
        f(&entry).map_err(keyring_error)
    })
    .await
    .map_err(|e| TwigError::Task(e.to_string()))?
}

/// Read the stored GitHub token, if any.
pub async fn get_github_token() -> Result<Option<String>, TwigError> {
    run(|entry| match entry.get_password() {
        Ok(token) if !token.is_empty() => Ok(Some(token)),
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e),
    })
    .await
}

/// Store the GitHub token, or delete it when `token` is `None` or empty.
pub async fn set_github_token(token: Option<String>) -> Result<(), TwigError> {
    run(move |entry| match token.filter(|t| !t.is_empty()) {
        Some(t) => entry.set_password(&t),
        None => match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e),
        },
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trips through the real OS keyring, so it is opt-in:
    /// `cargo test -- --ignored keyring_round_trip`
    #[tokio::test]
    #[ignore]
    async fn keyring_round_trip() {
        const ACCOUNT: &str = "twig-selftest";
        run_as(ACCOUNT, |e| e.set_password("secret-123")).await.unwrap();
        let read = run_as(ACCOUNT, |e| e.get_password()).await.unwrap();
        assert_eq!(read, "secret-123");
        run_as(ACCOUNT, |e| e.delete_credential()).await.unwrap();
        let gone = run_as(ACCOUNT, |e| e.get_password()).await;
        assert!(matches!(gone, Err(TwigError::Keyring(_))));
    }
}
