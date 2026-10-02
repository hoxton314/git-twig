//! Commit signature status for the graph's "verified" badge.
//!
//! git2 cheaply finds which commits carry a signature (`gpgsig` header);
//! only those are verified, with `git show --format=%G?…` so git uses the
//! user's own gpg / ssh (`gpg.ssh.allowedSignersFile`) setup.

use std::path::Path;

use git2::{Oid, Repository};
use serde::Serialize;

use crate::error::TwigError;
use crate::git::writer::run_git;

/// Most commits verified per call (the frontend asks for visible rows).
pub const MAX_BATCH: usize = 200;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SignatureInfo {
    pub oid: String,
    /// good | bad | untrusted (good, unknown validity) | expired |
    /// expired_key | revoked | unknown_key (cannot check) | unsigned
    pub status: String,
    /// Signer (`%GS`, e.g. "Ada <ada@example.com>"), when known.
    pub signer: Option<String>,
    /// Key (`%GK`, id or fingerprint), when known.
    pub key: Option<String>,
}

fn is_signed(repo: &Repository, oid: Oid) -> bool {
    repo.find_commit(oid).is_ok_and(|c| {
        c.header_field_bytes("gpgsig").is_ok() || c.header_field_bytes("gpgsig-sha256").is_ok()
    })
}

/// Of `oids`, the ones whose commit carries a signature.
pub fn signed_commits(repo: &Repository, oids: &[String]) -> Result<Vec<String>, TwigError> {
    let mut out = Vec::new();
    for s in oids.iter().take(MAX_BATCH) {
        let oid = Oid::from_str(s).map_err(|_| TwigError::InvalidArgument(format!("'{s}' is not a commit id")))?;
        if is_signed(repo, oid) {
            out.push(oid.to_string());
        }
    }
    Ok(out)
}

pub(crate) fn status_name(code: &str) -> &'static str {
    match code {
        "G" => "good",
        "B" => "bad",
        "U" => "untrusted",
        "X" => "expired",
        "Y" => "expired_key",
        "R" => "revoked",
        "E" => "unknown_key",
        _ => "unsigned",
    }
}

/// Parse `git show -s --format=%H%x00%G?%x00%GS%x00%GK%x1e` output.
pub(crate) fn parse_show(out: &str) -> Vec<SignatureInfo> {
    out.split('\u{1e}')
        .filter_map(|rec| {
            let rec = rec.trim_start_matches(['\n', '\r']);
            let f: Vec<&str> = rec.split('\0').collect();
            let oid = f.first()?.trim();
            if oid.len() < 40 {
                return None;
            }
            let opt = |i: usize| f.get(i).map(|s| s.trim()).filter(|s| !s.is_empty()).map(String::from);
            Some(SignatureInfo {
                oid: oid.to_string(),
                status: status_name(f.get(1).map(|s| s.trim()).unwrap_or("")).to_string(),
                signer: opt(2),
                key: opt(3),
            })
        })
        .collect()
}

/// Verify the given signed commits with the git CLI.
pub async fn verify(repo_path: &Path, signed: &[String]) -> Result<Vec<SignatureInfo>, TwigError> {
    if signed.is_empty() {
        return Ok(Vec::new());
    }
    let mut args: Vec<&str> = vec!["show", "-s", "--no-color", "--format=%H%x00%G?%x00%GS%x00%GK%x1e"];
    args.extend(signed.iter().map(String::as_str));
    let out = run_git(repo_path, &args).await?;
    if !out.success {
        return Err(TwigError::GitCli(out.stderr));
    }
    // Every input carries a signature (see `signed_commits`), so "N" means
    // git could not check it (e.g. SSH signing without
    // `gpg.ssh.allowedSignersFile`), not that it is unsigned.
    Ok(parse_show(&out.stdout)
        .into_iter()
        .map(|mut s| {
            if s.status == "unsigned" {
                s.status = "unknown_key".to_string();
            }
            s
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_show_records() {
        let a = "a".repeat(40);
        let b = "b".repeat(40);
        let out = format!("{a}\0G\0Ada <ada@x>\0ABCD\u{1e}\n{b}\0E\0\0EFGH\u{1e}\n");
        assert_eq!(
            parse_show(&out),
            vec![
                SignatureInfo { oid: a, status: "good".into(), signer: Some("Ada <ada@x>".into()), key: Some("ABCD".into()) },
                SignatureInfo { oid: b, status: "unknown_key".into(), signer: None, key: Some("EFGH".into()) },
            ]
        );
        assert_eq!(status_name("N"), "unsigned");
        assert_eq!(status_name("B"), "bad");
    }

    /// End to end with a throwaway SSH key and an allowed-signers file:
    /// unsigned commits are filtered out; the signed one verifies as good,
    /// and as unknown_key without the allowed-signers entry.
    #[tokio::test]
    async fn ssh_signed_commit_verifies() {
        let dir = std::env::temp_dir().join(format!("twig-sigs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let key = dir.join("key");
        let made = std::process::Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-C", "signer@example.com", "-f"])
            .arg(&key)
            .status();
        if !made.is_ok_and(|s| s.success()) {
            eprintln!("ssh-keygen unavailable; skipping");
            return;
        }
        let pubkey = std::fs::read_to_string(dir.join("key.pub")).unwrap();
        let allowed = dir.join("allowed_signers");
        std::fs::write(&allowed, format!("signer@example.com {}", pubkey.trim())).unwrap();
        let repo_dir = dir.join("repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        let git = |args: &[&str]| {
            let ok = std::process::Command::new("git")
                .args(["-c", "user.name=S", "-c", "user.email=signer@example.com"])
                .args(args)
                .current_dir(&repo_dir)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .status()
                .is_ok_and(|s| s.success());
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q"]);
        git(&["config", "gpg.format", "ssh"]);
        git(&["config", "user.signingkey", &format!("{}.pub", key.display())]);
        git(&["commit", "-q", "--allow-empty", "-m", "unsigned"]);
        git(&["commit", "-q", "--allow-empty", "-S", "-m", "signed"]);

        let repo = Repository::open(&repo_dir).unwrap();
        let head = repo.head().unwrap().target().unwrap().to_string();
        let parent = repo.find_commit(Oid::from_str(&head).unwrap()).unwrap().parent_id(0).unwrap().to_string();
        let signed = signed_commits(&repo, &[head.clone(), parent]).unwrap();
        assert_eq!(signed, vec![head.clone()]);

        let res = verify(&repo_dir, &signed).await.unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].status, "unknown_key", "no allowed signers yet: {res:?}");

        git(&["config", "gpg.ssh.allowedSignersFile", &allowed.to_string_lossy()]);
        let res = verify(&repo_dir, &signed).await.unwrap();
        assert_eq!(res[0].status, "good", "{res:?}");
        assert_eq!(res[0].signer.as_deref(), Some("signer@example.com"));

        assert!(signed_commits(&repo, &["nope".into()]).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
