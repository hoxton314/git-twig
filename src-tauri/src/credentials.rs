//! Secrets stored in the OS credential store (Secret Service / Keychain /
//! Windows Credential Manager) instead of `settings.json`.

use std::sync::Mutex;

use keyring_core::{Entry, Error as KeyringError};

use crate::error::TwigError;

const SERVICE: &str = "dev.twig.app";
const GITHUB_ACCOUNT: &str = "github-token";

fn keyring_error(e: KeyringError) -> TwigError {
    match e {
        KeyringError::PlatformFailure(_) | KeyringError::NoStorageAccess(_) => {
            TwigError::Keyring(format!(
                "{e}. On Linux, make sure a Secret Service provider \
                 (GNOME Keyring or KWallet) is running and unlocked."
            ))
        }
        other => TwigError::Keyring(other.to_string()),
    }
}

/// Whether the platform credential store has been installed as
/// `keyring_core`'s default store. Unlike keyring 3 (which connected per
/// call), the store is a long-lived object; if creating it fails (e.g. the
/// Secret Service isn't running yet at login) it is retried on the next use
/// instead of failing for the rest of the session.
static STORE_READY: Mutex<bool> = Mutex::new(false);

fn ensure_store() -> Result<(), KeyringError> {
    let mut ready = STORE_READY.lock().map_err(|_| {
        KeyringError::Invalid("store".into(), "credential store lock poisoned".into())
    })?;
    if *ready {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    let store = apple_native_keyring_store::keychain::Store::new()?;
    #[cfg(target_os = "windows")]
    let store = windows_native_keyring_store::Store::new()?;
    #[cfg(all(unix, not(target_os = "macos")))]
    let store = zbus_secret_service_keyring_store::Store::new()?;
    keyring_core::set_default_store(store);
    *ready = true;
    Ok(())
}

/// Keyring backends block (D-Bus round trips, Keychain prompts), so run
/// them off the async worker threads.
async fn run<T, F>(f: F) -> Result<T, TwigError>
where
    T: Send + 'static,
    F: FnOnce(&Entry) -> Result<T, KeyringError> + Send + 'static,
{
    run_as(GITHUB_ACCOUNT, f).await
}

async fn run_as<T, F>(account: impl Into<String>, f: F) -> Result<T, TwigError>
where
    T: Send + 'static,
    F: FnOnce(&Entry) -> Result<T, KeyringError> + Send + 'static,
{
    let account = account.into();
    tauri::async_runtime::spawn_blocking(move || {
        ensure_store().map_err(keyring_error)?;
        let entry = Entry::new(SERVICE, &account).map_err(keyring_error)?;
        f(&entry).map_err(keyring_error)
    })
    .await
    .map_err(|e| TwigError::Task(e.to_string()))?
}

/// Read the stored GitHub token, if any.
pub async fn get_github_token() -> Result<Option<String>, TwigError> {
    run(|entry| match entry.get_password() {
        Ok(token) if !token.is_empty() => Ok(Some(token)),
        Ok(_) | Err(KeyringError::NoEntry) => Ok(None),
        Err(e) => Err(e),
    })
    .await
}

/// Store the GitHub token, or delete it when `token` is `None` or empty.
pub async fn set_github_token(token: Option<String>) -> Result<(), TwigError> {
    run(move |entry| match token.filter(|t| !t.is_empty()) {
        Some(t) => entry.set_password(&t),
        None => match entry.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(e) => Err(e),
        },
    })
    .await
}

// ── Hosting integrations: GitLab / Gitea tokens ─────────────────────

/// Keyring account for a provider token, scoped to the instance host so a
/// token is never sent to a different server after the URL is changed.
pub fn provider_account(provider: &str, host: &str) -> String {
    format!("{provider}-token:{}", host.to_ascii_lowercase())
}

/// Read a provider token stored under `account` (see `provider_account`).
pub async fn get_token_for(account: String) -> Result<Option<String>, TwigError> {
    run_as(account, |entry| match entry.get_password() {
        Ok(token) if !token.is_empty() => Ok(Some(token)),
        Ok(_) | Err(KeyringError::NoEntry) => Ok(None),
        Err(e) => Err(e),
    })
    .await
}

/// Store (or with `None`/empty, delete) a provider token.
pub async fn set_token_for(account: String, token: Option<String>) -> Result<(), TwigError> {
    run_as(account, move |entry| match token.filter(|t| !t.is_empty()) {
        Some(t) => entry.set_password(&t),
        None => match entry.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
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
