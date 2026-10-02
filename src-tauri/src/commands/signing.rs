//! Commit signing setup: list usable GPG / SSH keys and test signing.
//!
//! The choice itself is stored in git config (`gpg.format`,
//! `user.signingkey`, `commit.gpgsign`) by `git_config::set_git_config`.

use std::path::Path;

use serde::Serialize;
use tokio::process::Command;

use crate::error::TwigError;
use crate::git::writer::run_git;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SigningKey {
    /// Value for `user.signingkey`: a GPG key id, an SSH public key file
    /// path, or `key::<public key>` for a key that is only in the agent.
    pub value: String,
    /// Human-readable label (user id / comment and key type).
    pub label: String,
}

/// Parse `gpg --list-secret-keys --with-colons` output into signing-capable
/// keys, skipping expired, revoked and disabled ones.
pub(crate) fn parse_gpg_secret_keys(out: &str) -> Vec<SigningKey> {
    let mut keys = Vec::new();
    let mut current: Option<(String, bool)> = None; // (key id, usable)
    let mut have_uid = false;
    for line in out.lines() {
        let f: Vec<&str> = line.split(':').collect();
        match f.first().copied() {
            Some("sec") => {
                let validity = f.get(1).copied().unwrap_or("");
                let caps = f.get(11).copied().unwrap_or("");
                let usable = !matches!(validity, "e" | "r" | "d" | "i") && caps.contains(['s', 'S']);
                current = f.get(4).map(|id| (id.to_string(), usable));
                have_uid = false;
            }
            Some("uid") if !have_uid => {
                if let Some((id, true)) = &current {
                    let uid = f.get(9).copied().unwrap_or("").replace("\\x3a", ":");
                    keys.push(SigningKey { value: id.clone(), label: format!("{uid} ({id})") });
                    have_uid = true;
                }
            }
            _ => {}
        }
    }
    keys
}

/// SSH public keys usable for signing: `*.pub` files in `ssh_dir` and keys
/// held by the agent (`ssh-add -L` output), deduplicated by key material.
pub(crate) fn ssh_keys(ssh_dir: &Path, agent_listing: &str) -> Vec<SigningKey> {
    let mut keys: Vec<SigningKey> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let material = |line: &str| line.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
    let mut files: Vec<_> = std::fs::read_dir(ssh_dir)
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    files.sort();
    for path in files {
        if path.extension().and_then(|e| e.to_str()) != Some("pub") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let line = text.lines().next().unwrap_or("").trim();
        if !line.starts_with("ssh-") && !line.starts_with("ecdsa-") && !line.starts_with("sk-") {
            continue;
        }
        let kind = line.split_whitespace().next().unwrap_or("");
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        seen.push(material(line));
        keys.push(SigningKey { value: path.to_string_lossy().into_owned(), label: format!("{name} ({kind})") });
    }
    for line in agent_listing.lines().map(str::trim) {
        if line.is_empty() || line.starts_with("The agent has no") || seen.contains(&material(line)) {
            continue;
        }
        let mut parts = line.split_whitespace();
        let kind = parts.next().unwrap_or("");
        parts.next();
        let comment = parts.collect::<Vec<_>>().join(" ");
        seen.push(material(line));
        keys.push(SigningKey {
            value: format!("key::{}", material(line)),
            label: format!("{} ({kind}, from agent)", if comment.is_empty() { "agent key" } else { &comment }),
        });
    }
    keys
}

async fn output_of(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Keys usable for `format` (`openpgp` or `ssh`).
#[tauri::command]
pub async fn list_signing_keys(format: String) -> Result<Vec<SigningKey>, TwigError> {
    match format.as_str() {
        "openpgp" => {
            let gpg = run_git(&std::env::temp_dir(), &["config", "--global", "--get", "gpg.program"])
                .await
                .ok()
                .map(|o| o.stdout.trim().to_string())
                .filter(|p| !p.is_empty())
                .unwrap_or_else(|| "gpg".to_string());
            let out = output_of(&gpg, &["--list-secret-keys", "--with-colons"]).await.unwrap_or_default();
            Ok(parse_gpg_secret_keys(&out))
        }
        "ssh" => {
            let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
            let ssh_dir = home.map(|h| Path::new(&h).join(".ssh")).unwrap_or_default();
            let agent = output_of("ssh-add", &["-L"]).await.unwrap_or_default();
            Ok(ssh_keys(&ssh_dir, &agent))
        }
        other => Err(TwigError::InvalidArgument(format!("unknown signing format '{other}'"))),
    }
}

/// Sign a throwaway commit in a temporary repository with the global git
/// config (`gpg.format` / `user.signingkey`), or `global_config` instead
/// when given (tests).
async fn sign_test_commit(global_config: Option<&Path>) -> Result<crate::git::writer::GitOutput, TwigError> {
    let dir = std::env::temp_dir().join(format!(
        "twig-sign-test-{}-{}",
        std::process::id(),
        crate::git::remotes::unique_scratch_name("t")
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    let git = |args: &[&str]| {
        let mut cmd = Command::new("git");
        cmd.args(args).current_dir(&dir).stdin(std::process::Stdio::null());
        if let Some(cfg) = global_config {
            cmd.env("GIT_CONFIG_GLOBAL", cfg).env("GIT_CONFIG_NOSYSTEM", "1");
        }
        cmd
    };
    let result = async {
        let init = git(&["init", "-q"]).output().await?;
        if !init.status.success() {
            return Ok(init);
        }
        // Identity fallbacks only; the signing settings come from the config.
        git(&["-c", "user.name=Twig", "-c", "user.email=twig@localhost", "commit", "-q", "--allow-empty", "-S", "-m", "signing test"])
            .output()
            .await
    }
    .await;
    let _ = std::fs::remove_dir_all(&dir);
    let out = result.map_err(|e| TwigError::GitCli(format!("Failed to execute git: {e}")))?;
    Ok(crate::git::writer::GitOutput {
        success: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// Test the configured signing setup, so problems such as a missing key, a
/// locked agent or no pinentry show up here instead of on commit.
#[tauri::command]
pub async fn test_signing() -> Result<crate::commands::staging::CommandResult, TwigError> {
    let out = sign_test_commit(None).await?;
    Ok(crate::commands::staging::CommandResult {
        success: out.success,
        message: if out.success { "Signing works".to_string() } else { signing_hint(out.stderr.trim()) },
    })
}

/// `CommandResult` for a commit, with a hint when it failed to sign.
pub(crate) fn commit_result(out: crate::git::writer::GitOutput) -> crate::commands::staging::CommandResult {
    if out.success {
        out.into()
    } else {
        crate::commands::staging::CommandResult { success: false, message: signing_hint(out.stderr.trim()) }
    }
}

/// Turn git's signing failures into an actionable message (unchanged when
/// the error is not about signing).
pub(crate) fn signing_hint(stderr: &str) -> String {
    let lower = stderr.to_ascii_lowercase();
    let signing_error = ["failed to sign", "gpg failed", "signing failed", "error: load key", "couldn't load public key"]
        .iter()
        .any(|needle| lower.contains(needle));
    if signing_error {
        format!(
            "{stderr}\n\nCommit signing failed. Check the signing key in Settings > Git Configuration \
             (use \"Test signing\"), make sure your GPG/SSH agent is running and unlocked, or turn \
             signing off."
        )
    } else {
        stderr.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gpg_colon_output() {
        let out = "\
sec:u:255:22:AAAA1111BBBB2222:1700000000:::u:::scESC:::+:::ed25519:::0:
fpr:::::::::0123456789ABCDEF0123AAAA1111BBBB2222:
uid:u::::1700000000::HASH::Ada Lovelace <ada@example.com>::::::::::0:
uid:u::::1700000000::HASH::Ada (work) <ada@work.example>::::::::::0:
ssb:u:255:18:CCCC3333DDDD4444:1700000000::::::e:::+:::cv25519::
sec:e:255:22:EEEE5555FFFF6666:1500000000:1600000000::u:::scESC:::+:::ed25519:::0:
uid:e::::1500000000::HASH::Expired Key <old@example.com>::::::::::0:
sec:u:3072:1:9999888877776666:1700000000:::u:::eE:::+::::::23::0:
uid:u::::1700000000::HASH::Encrypt Only <enc@example.com>::::::::::0:
";
        assert_eq!(
            parse_gpg_secret_keys(out),
            vec![SigningKey {
                value: "AAAA1111BBBB2222".into(),
                label: "Ada Lovelace <ada@example.com> (AAAA1111BBBB2222)".into()
            }]
        );
        assert!(parse_gpg_secret_keys("").is_empty());
    }

    #[test]
    fn lists_ssh_pub_files_and_agent_keys() {
        let dir = std::env::temp_dir().join(format!("twig-sshkeys-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("id_ed25519.pub"), "ssh-ed25519 AAAAone me@laptop\n").unwrap();
        std::fs::write(dir.join("id_ed25519"), "PRIVATE").unwrap();
        std::fs::write(dir.join("known_hosts.pub"), "garbage\n").unwrap();
        let agent = "ssh-ed25519 AAAAone me@laptop\nssh-rsa AAAAtwo work key\n";
        let keys = ssh_keys(&dir, agent);
        assert_eq!(keys.len(), 2, "{keys:?}");
        assert!(keys[0].value.ends_with("id_ed25519.pub"));
        assert_eq!(keys[0].label, "id_ed25519.pub (ssh-ed25519)");
        assert_eq!(keys[1].value, "key::ssh-rsa AAAAtwo");
        assert_eq!(keys[1].label, "work key (ssh-rsa, from agent)");
        assert!(ssh_keys(&dir.join("missing"), "The agent has no identities.\n").is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn signing_hint_only_for_signing_errors() {
        assert!(signing_hint("error: gpg failed to sign the data\nfatal: failed to write commit object")
            .contains("Settings > Git Configuration"));
        assert_eq!(signing_hint("nothing to commit"), "nothing to commit");
    }

    /// End to end with a throwaway SSH key (skipped without ssh-keygen).
    #[tokio::test]
    async fn sign_test_commit_with_ssh_key() {
        let dir = std::env::temp_dir().join(format!("twig-sshsign-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let key = dir.join("id_test");
        let made = std::process::Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-C", "twig-test", "-f"])
            .arg(&key)
            .status();
        if !made.is_ok_and(|s| s.success()) {
            eprintln!("ssh-keygen unavailable; skipping");
            return;
        }
        let cfg = dir.join("gitconfig");
        let pubkey = format!("{}.pub", key.display());
        std::fs::write(&cfg, format!("[gpg]\n\tformat = ssh\n[user]\n\tsigningkey = {pubkey}\n")).unwrap();
        let out = sign_test_commit(Some(&cfg)).await.unwrap();
        assert!(out.success, "{}", out.stderr);

        // A missing key fails with the signing hint.
        std::fs::write(&cfg, "[gpg]\n\tformat = ssh\n[user]\n\tsigningkey = /nonexistent/key.pub\n").unwrap();
        let out = sign_test_commit(Some(&cfg)).await.unwrap();
        assert!(!out.success);
        assert!(signing_hint(out.stderr.trim()).contains("Settings > Git Configuration"), "{}", out.stderr);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
