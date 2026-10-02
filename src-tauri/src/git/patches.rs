//! Patch files: save commits with `git format-patch` (one file each, or a
//! single mbox) and the working-tree changes with `git diff`; apply mbox
//! files with `git am --3way` and plain diffs with `git apply`.
//!
//! Patches are written by git itself (`-o` / `--output`), never through the
//! UTF-8 captured stdout, so files in other encodings stay byte-exact.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use serde::Serialize;

use crate::error::TwigError;
use crate::git::writer::{rev_exists, run_git};

/// Most commits saved in one go.
pub const MAX_PATCH_COMMITS: usize = 500;

fn invalid(msg: impl Into<String>) -> TwigError {
    TwigError::InvalidArgument(msg.into())
}

/// User-chosen files and folders must be absolute: they are passed to git as
/// arguments and must never be read as options or resolved against the repo.
fn absolute(p: &str) -> Result<&Path, TwigError> {
    let path = Path::new(p);
    if p.is_empty() || !path.is_absolute() {
        return Err(invalid(format!("'{p}' is not an absolute path")));
    }
    Ok(path)
}

fn is_hex_oid(s: &str) -> bool {
    (s.len() == 40 || s.len() == 64) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// A fresh scratch folder (removed by the caller).
fn scratch_dir() -> Result<PathBuf, TwigError> {
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "twig-patches-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// `git format-patch` one commit into `dir`, numbered `number`; returns the file.
async fn format_one(repo_path: &Path, oid: &str, number: usize, dir: &Path) -> Result<PathBuf, TwigError> {
    let start = format!("--start-number={number}");
    let dir_s = dir.to_string_lossy();
    let out = run_git(
        repo_path,
        &[
            "-c",
            "diff.noprefix=false",
            "-c",
            "diff.mnemonicPrefix=false",
            "format-patch",
            "-1",
            &start,
            "--src-prefix=a/",
            "--dst-prefix=b/",
            "-o",
            &dir_s,
            oid,
        ],
    )
    .await?;
    if !out.success {
        return Err(TwigError::GitCli(out.stderr));
    }
    let file = out.stdout.lines().map(str::trim).find(|l| !l.is_empty()).ok_or_else(|| {
        invalid(format!("git wrote no patch for {}", &oid[..7]))
    })?;
    Ok(PathBuf::from(file))
}

/// Save `oids` (oldest first) as patches: one numbered file per commit in the
/// folder `target`, or, with `single_file`, all of them in the mbox file
/// `target`. Returns the files written.
pub async fn format_patches(
    repo_path: &Path,
    oids: &[String],
    target: &str,
    single_file: bool,
) -> Result<Vec<String>, TwigError> {
    let target = absolute(target)?;
    if oids.is_empty() {
        return Err(invalid("no commits to save"));
    }
    if oids.len() > MAX_PATCH_COMMITS {
        return Err(invalid(format!("at most {MAX_PATCH_COMMITS} commits can be saved at once")));
    }
    for oid in oids {
        if !is_hex_oid(oid) {
            return Err(invalid(format!("'{oid}' is not a full commit id")));
        }
        let out = run_git(repo_path, &["rev-list", "--parents", "-n", "1", oid]).await?;
        if !out.success {
            return Err(invalid(format!("'{}' is not a commit", &oid[..7])));
        }
        if out.stdout.split_whitespace().count() > 2 {
            return Err(invalid(format!(
                "{} is a merge commit; patches can't represent merges",
                &oid[..7]
            )));
        }
    }

    if !single_file {
        std::fs::create_dir_all(target)?;
        let mut files = Vec::with_capacity(oids.len());
        for (i, oid) in oids.iter().enumerate() {
            files.push(format_one(repo_path, oid, i + 1, target).await?.to_string_lossy().into_owned());
        }
        return Ok(files);
    }

    let scratch = scratch_dir()?;
    let result = async {
        let mut mbox: Vec<u8> = Vec::new();
        for (i, oid) in oids.iter().enumerate() {
            let file = format_one(repo_path, oid, i + 1, &scratch).await?;
            mbox.extend(std::fs::read(&file)?);
        }
        std::fs::write(target, mbox)?;
        Ok(vec![target.to_string_lossy().into_owned()])
    }
    .await;
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

/// Save all uncommitted changes to tracked files (staged and unstaged,
/// against HEAD) as a patch file. Untracked files are not included.
pub async fn save_working_patch(repo_path: &Path, target: &str) -> Result<(), TwigError> {
    let target = absolute(target)?;
    if !rev_exists(repo_path, "HEAD").await? {
        return Err(invalid("there is no commit to diff against yet"));
    }
    let quiet = run_git(repo_path, &["diff", "HEAD", "--quiet"]).await?;
    if quiet.success {
        return Err(invalid("there are no changes to tracked files to save"));
    }
    let output = format!("--output={}", target.to_string_lossy());
    // Pin the format `git apply` expects, whatever the user's diff config
    // (`diff.noprefix`, textconv drivers, `diff.submodule=log`, …) says.
    let out = run_git(
        repo_path,
        &[
            // `diff.noprefix` beats `--src-prefix` on some git versions.
            "-c",
            "diff.noprefix=false",
            "-c",
            "diff.mnemonicPrefix=false",
            "diff",
            "HEAD",
            "--binary",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            "--submodule=short",
            "--no-relative",
            &output,
        ],
    )
    .await?;
    if !out.success {
        return Err(TwigError::GitCli(out.stderr));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct PatchInfo {
    /// "mbox" (format-patch output or a saved mail, applied with `git am`)
    /// or "diff".
    pub kind: String,
    /// Number of commits (mails) in an mbox.
    pub count: usize,
    /// Their subjects, in order (the first [`MAX_LISTED`]).
    pub commits: Vec<String>,
    /// `git apply --stat` summary.
    pub stat: String,
    /// Plain diffs: whether `git apply --check` passes (`None` for mbox,
    /// where later patches may depend on earlier ones).
    pub applies_cleanly: Option<bool>,
    /// Why the check failed, when it did.
    pub check_error: Option<String>,
}

/// Subjects read for the preview.
const MAX_LISTED: usize = 50;

/// An mbox (`From ` separator line, as `format-patch` writes) or a single
/// saved mail: a leading block of RFC 2822 headers with From and Subject.
fn is_mbox(bytes: &[u8]) -> bool {
    if bytes.starts_with(b"From ") {
        return true;
    }
    let text = String::from_utf8_lossy(&bytes[..bytes.len().min(16 * 1024)]);
    let mut from = false;
    let mut subject = false;
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            break;
        }
        if line.starts_with([' ', '\t']) {
            continue; // folded header
        }
        let Some((name, _)) = line.split_once(':') else {
            return false;
        };
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return false;
        }
        from |= name.eq_ignore_ascii_case("from");
        subject |= name.eq_ignore_ascii_case("subject");
    }
    from && subject
}

/// Count the mails in `file` and read their (decoded, unwrapped) subjects
/// with `git mailsplit` / `git mailinfo`, the way `git am` will.
async fn mbox_subjects(repo_path: &Path, file: &Path) -> Result<(usize, Vec<String>), TwigError> {
    let scratch = scratch_dir()?;
    let result = async {
        let out_dir = format!("-o{}", scratch.to_string_lossy());
        let split = run_git(repo_path, &["mailsplit", "-b", &out_dir, &file.to_string_lossy()]).await?;
        if !split.success {
            return Err(invalid(format!("not a mailbox git can read: {}", split.stderr.trim())));
        }
        let count: usize = split.stdout.trim().parse().unwrap_or(0);
        let mut subjects = Vec::new();
        for i in 1..=count.min(MAX_LISTED) {
            let mail = scratch.join(format!("{i:04}"));
            let info = mailinfo(repo_path, &mail, &scratch).await?;
            let subject = info
                .lines()
                .find_map(|l| l.strip_prefix("Subject: "))
                .unwrap_or("(no subject)")
                .trim()
                .to_string();
            subjects.push(subject);
        }
        Ok((count, subjects))
    }
    .await;
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

/// `git mailinfo <msg> <patch> < mail`: the header summary on stdout.
async fn mailinfo(repo_path: &Path, mail: &Path, scratch: &Path) -> Result<String, TwigError> {
    let input = std::fs::File::open(mail)?;
    let out = tokio::process::Command::new("git")
        .arg("mailinfo")
        .arg(scratch.join("msg"))
        .arg(scratch.join("patch"))
        .current_dir(repo_path)
        .stdin(std::process::Stdio::from(input))
        .output()
        .await?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Look at a patch file before applying it.
pub async fn inspect_patch(repo_path: &Path, file: &str) -> Result<PatchInfo, TwigError> {
    let path = absolute(file)?;
    let bytes = std::fs::read(path)?;
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(invalid("the patch file is empty"));
    }
    let file_s = path.to_string_lossy();
    let stat = run_git(repo_path, &["apply", "--stat", &file_s]).await?;
    if !stat.success {
        return Err(invalid(format!("not a patch git can read: {}", stat.stderr.trim())));
    }
    if is_mbox(&bytes) {
        let (count, commits) = mbox_subjects(repo_path, path).await?;
        return Ok(PatchInfo {
            kind: "mbox".into(),
            count,
            commits,
            stat: stat.stdout.trim_end().to_string(),
            applies_cleanly: None,
            check_error: None,
        });
    }
    let check = run_git(repo_path, &["apply", "--check", &file_s]).await?;
    Ok(PatchInfo {
        kind: "diff".into(),
        count: 0,
        commits: Vec::new(),
        stat: stat.stdout.trim_end().to_string(),
        applies_cleanly: Some(check.success),
        check_error: (!check.success).then(|| check.stderr.trim().to_string()),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplyPatchResult {
    pub success: bool,
    pub message: String,
    /// "am", "apply" or "apply-3way".
    pub mode: String,
    /// Left conflicted files to resolve.
    pub conflicted: bool,
    /// `git am` stopped part-way and is in progress (banner: continue /
    /// skip / abort), with or without conflicts.
    pub stopped: bool,
}

async fn unmerged_paths(repo_path: &Path) -> Result<Vec<String>, TwigError> {
    let out = run_git(repo_path, &["diff", "--name-only", "--diff-filter=U"]).await?;
    let mut v: Vec<String> = out.stdout.lines().map(String::from).collect();
    v.sort();
    Ok(v)
}

/// Apply a patch file: `git am --3way` for an mbox, otherwise `git apply`
/// to the working tree, falling back to `git apply --3way` (which leaves
/// conflict markers) when the plain check fails.
pub async fn apply_patch(repo_path: &Path, file: &str) -> Result<ApplyPatchResult, TwigError> {
    let path = absolute(file)?;
    let bytes = std::fs::read(path)?;
    let file_s = path.to_string_lossy();
    if is_mbox(&bytes) {
        if am_in_progress(repo_path).await? {
            return Err(invalid("a patch series is already being applied; continue or abort it first"));
        }
        let staged = run_git(repo_path, &["diff", "--cached", "--quiet"]).await?;
        if !staged.success {
            return Err(invalid(
                "applying a patch series needs a clean index: commit or unstage the staged changes first",
            ));
        }
        let out = crate::git::conflicts::run_git_noedit(repo_path, &["am", "--3way", &file_s], &[]).await?;
        let stopped = !out.success && am_in_progress(repo_path).await?;
        let conflicted = stopped && !unmerged_paths(repo_path).await?.is_empty();
        return Ok(ApplyPatchResult {
            success: out.success,
            message: if out.success { out.stdout } else { format!("{}{}", out.stdout, out.stderr) },
            mode: "am".into(),
            conflicted,
            stopped,
        });
    }
    let check = run_git(repo_path, &["apply", "--check", &file_s]).await?;
    if check.success {
        let out = run_git(repo_path, &["apply", &file_s]).await?;
        return Ok(ApplyPatchResult {
            success: out.success,
            message: if out.success { out.stdout } else { out.stderr },
            mode: "apply".into(),
            conflicted: false,
            stopped: false,
        });
    }
    // Only conflicts this apply created count (others may predate it).
    let before = unmerged_paths(repo_path).await?;
    let out = run_git(repo_path, &["apply", "--3way", &file_s]).await?;
    let after = unmerged_paths(repo_path).await?;
    let conflicted = after.iter().any(|p| !before.contains(p));
    Ok(ApplyPatchResult {
        success: out.success,
        message: if out.success { out.stdout } else { out.stderr },
        mode: "apply-3way".into(),
        conflicted,
        stopped: false,
    })
}

async fn am_in_progress(repo_path: &Path) -> Result<bool, TwigError> {
    let out = run_git(repo_path, &["rev-parse", "--git-path", "rebase-apply/applying"]).await?;
    Ok(out.success && repo_path.join(out.stdout.trim()).exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn git(dir: &Path, args: &[&str]) -> String {
        let out = run_git(dir, args).await.unwrap();
        assert!(out.success, "git {args:?}: {}", out.stderr);
        out.stdout.trim().to_string()
    }

    async fn repo(dir: &Path) {
        let _ = std::fs::remove_dir_all(dir);
        std::fs::create_dir_all(dir).unwrap();
        git(dir, &["init", "-q", "-b", "main"]).await;
        git(dir, &["config", "user.name", "t"]).await;
        git(dir, &["config", "user.email", "t@t"]).await;
        git(dir, &["config", "commit.gpgsign", "false"]).await;
        git(dir, &["config", "core.hooksPath", "/dev/null"]).await;
    }

    async fn commit(dir: &Path, file: &str, content: &[u8], msg: &str) -> String {
        std::fs::write(dir.join(file), content).unwrap();
        git(dir, &["add", "--", file]).await;
        git(dir, &["commit", "-q", "-m", msg]).await;
        git(dir, &["rev-parse", "HEAD"]).await
    }

    fn base(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("twig-patches-test-{name}-{}", std::process::id()))
    }

    #[tokio::test]
    async fn round_trips_commits_through_mbox_and_folder() {
        let root = base("roundtrip");
        let (src, dst) = (root.join("src"), root.join("dst"));
        repo(&src).await;
        let first = commit(&src, "a.txt", b"one\n", "first").await;
        // Latin-1 bytes must survive unchanged.
        let second = commit(&src, "a.txt", b"one\ncaf\xe9\n", "second: latin-1").await;
        let third = commit(&src, "b.txt", b"b\n", "third").await;

        let mbox = root.join("series.mbox");
        let files = format_patches(&src, &[second.clone(), third.clone()], &mbox.to_string_lossy(), true)
            .await
            .unwrap();
        assert_eq!(files, [mbox.to_string_lossy()]);

        let folder = root.join("patches");
        let files = format_patches(&src, &[first.clone(), second.clone()], &folder.to_string_lossy(), false)
            .await
            .unwrap();
        assert_eq!(files.len(), 2);
        assert!(files[0].ends_with("0001-first.patch") && files[1].contains("0002-second"), "{files:?}");

        // Apply the root commit's patch, then the mbox, in a fresh repo.
        repo(&dst).await;
        let info = inspect_patch(&dst, &files[0]).await.unwrap();
        assert_eq!(info.kind, "mbox");
        assert_eq!(info.commits, ["first"]);
        assert_eq!(info.count, 1);
        let r = apply_patch(&dst, &files[0]).await.unwrap();
        assert!(r.success && r.mode == "am", "{r:?}");

        let info = inspect_patch(&dst, &mbox.to_string_lossy()).await.unwrap();
        assert_eq!(info.commits, ["second: latin-1", "third"]);
        assert!(info.stat.contains("a.txt") && info.stat.contains("b.txt"), "{}", info.stat);
        let r = apply_patch(&dst, &mbox.to_string_lossy()).await.unwrap();
        assert!(r.success, "{r:?}");
        assert_eq!(git(&dst, &["log", "--format=%s"]).await, "third\nsecond: latin-1\nfirst");
        assert_eq!(std::fs::read(dst.join("a.txt")).unwrap(), b"one\ncaf\xe9\n");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn working_patch_applies_and_conflicts_fall_back_to_three_way() {
        let root = base("working");
        let dir = root.join("repo");
        repo(&dir).await;
        let patch = root.join("wip.patch");
        let patch_s = patch.to_string_lossy().into_owned();
        assert!(save_working_patch(&dir, &patch_s).await.is_err(), "no HEAD yet");
        commit(&dir, "a.txt", b"1\n2\n3\n", "base").await;
        assert!(save_working_patch(&dir, &patch_s).await.is_err(), "nothing to save");

        std::fs::write(dir.join("a.txt"), b"1\nTWO\n3\n").unwrap();
        save_working_patch(&dir, &patch_s).await.unwrap();
        git(&dir, &["checkout", "--", "a.txt"]).await;

        let info = inspect_patch(&dir, &patch_s).await.unwrap();
        assert_eq!(info.kind, "diff");
        assert_eq!(info.applies_cleanly, Some(true));
        let r = apply_patch(&dir, &patch_s).await.unwrap();
        assert!(r.success && r.mode == "apply" && !r.conflicted, "{r:?}");
        assert_eq!(std::fs::read(dir.join("a.txt")).unwrap(), b"1\nTWO\n3\n");

        // The same line changed differently in a commit: plain apply fails,
        // the three-way fallback leaves a conflict to resolve.
        git(&dir, &["checkout", "--", "a.txt"]).await;
        commit(&dir, "a.txt", b"1\nzwei\n3\n", "other change").await;
        let info = inspect_patch(&dir, &patch_s).await.unwrap();
        assert_eq!(info.applies_cleanly, Some(false));
        assert!(info.check_error.is_some());
        let r = apply_patch(&dir, &patch_s).await.unwrap();
        assert!(!r.success && r.conflicted && r.mode == "apply-3way", "{r:?}");

        git(&dir, &["checkout", "-q", "-f", "HEAD"]).await;
        git(&dir, &["reset", "-q", "--hard"]).await;

        // An mbox that conflicts leaves `git am` in progress.
        let first = git(&dir, &["rev-list", "--max-parents=0", "HEAD"]).await;
        git(&dir, &["checkout", "-q", "-b", "side", &first]).await;
        let side = commit(&dir, "a.txt", b"1
DOS
3
", "side change").await;
        git(&dir, &["checkout", "-q", "main"]).await;
        let mbox = root.join("side.mbox").to_string_lossy().into_owned();
        format_patches(&dir, &[side], &mbox, true).await.unwrap();
        let r = apply_patch(&dir, &mbox).await.unwrap();
        assert!(!r.success && r.conflicted && r.stopped && r.mode == "am", "{r:?}");
        assert!(am_in_progress(&dir).await.unwrap());
        git(&dir, &["am", "--abort"]).await;

        assert!(apply_patch(&dir, "relative.patch").await.is_err());
        std::fs::write(root.join("empty.patch"), "\n").unwrap();
        assert!(inspect_patch(&dir, &root.join("empty.patch").to_string_lossy()).await.is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn refuses_merges_and_bad_ids() {
        let root = base("refuse");
        let dir = root.join("repo");
        repo(&dir).await;
        commit(&dir, "a.txt", b"a\n", "a").await;
        git(&dir, &["checkout", "-q", "-b", "side"]).await;
        commit(&dir, "b.txt", b"b\n", "b").await;
        git(&dir, &["checkout", "-q", "main"]).await;
        commit(&dir, "c.txt", b"c\n", "c").await;
        git(&dir, &["merge", "-q", "--no-ff", "--no-edit", "side"]).await;
        let merge = git(&dir, &["rev-parse", "HEAD"]).await;
        let out = root.join("x.mbox").to_string_lossy().into_owned();
        let err = format_patches(&dir, &[merge], &out, true).await.unwrap_err().to_string();
        assert!(err.contains("merge"), "{err}");
        assert!(format_patches(&dir, &["HEAD".into()], &out, true).await.is_err());
        assert!(format_patches(&dir, &[], &out, true).await.is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn reads_mail_files_like_git_am() {
        let root = base("mail");
        let dir = root.join("repo");
        repo(&dir).await;
        // A saved mail (no mbox separator), folded and encoded subject, and a
        // body line that only looks like a header.
        let mail = root.join("fix.eml");
        std::fs::write(
            &mail,
            "From: Ada <ada@x>\nSubject: [PATCH] =?UTF-8?q?caf=C3=A9?= is\n spelled right\nDate: Mon, 1 Jan 2024 00:00:00 +0000\n\n\
             Body text first.\nSubject: not a header\n---\n a.txt | 1 +\n\ndiff --git a/a.txt b/a.txt\nnew file mode 100644\n\
             --- /dev/null\n+++ b/a.txt\n@@ -0,0 +1 @@\n+a\n",
        )
        .unwrap();
        assert!(is_mbox(&std::fs::read(&mail).unwrap()));
        assert!(!is_mbox(b"diff --git a/x b/x\n"));
        assert!(!is_mbox(b"Note: something\nSubject: x\n\n"), "needs From too");
        let info = inspect_patch(&dir, &mail.to_string_lossy()).await.unwrap();
        assert_eq!(info.kind, "mbox");
        assert_eq!(info.count, 1);
        assert_eq!(info.commits, ["café is spelled right"]);

        // git am needs a clean index: refuse up front instead of leaving a
        // half-started session.
        commit(&dir, "base.txt", b"x\n", "base").await;
        std::fs::write(dir.join("base.txt"), b"y\n").unwrap();
        git(&dir, &["add", "base.txt"]).await;
        let err = apply_patch(&dir, &mail.to_string_lossy()).await.unwrap_err().to_string();
        assert!(err.contains("clean index"), "{err}");
        assert!(!am_in_progress(&dir).await.unwrap());
        git(&dir, &["reset", "-q", "--hard"]).await;
        let r = apply_patch(&dir, &mail.to_string_lossy()).await.unwrap();
        assert!(r.success, "{r:?}");
        assert_eq!(git(&dir, &["log", "-1", "--format=%an %s"]).await, "Ada café is spelled right");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn working_patch_ignores_diff_config() {
        let root = base("diffcfg");
        let dir = root.join("repo");
        repo(&dir).await;
        commit(&dir, "a.txt", b"1\n", "base").await;
        git(&dir, &["config", "diff.noprefix", "true"]).await;
        git(&dir, &["config", "diff.mnemonicPrefix", "true"]).await;
        git(&dir, &["config", "format.noprefix", "true"]).await;
        std::fs::write(dir.join(".gitattributes"), "*.txt diff=upper\n").unwrap();
        git(&dir, &["config", "diff.upper.textconv", "tr a-z A-Z <"]).await;
        std::fs::write(dir.join("a.txt"), b"1\nadded\n").unwrap();
        let patch = root.join("w.patch");
        save_working_patch(&dir, &patch.to_string_lossy()).await.unwrap();
        let text = std::fs::read_to_string(&patch).unwrap();
        assert!(text.contains("--- a/a.txt") && text.contains("+added"), "{text}");
        git(&dir, &["checkout", "--", "a.txt"]).await;
        let info = inspect_patch(&dir, &patch.to_string_lossy()).await.unwrap();
        assert_eq!(info.applies_cleanly, Some(true), "{info:?}");

        let oid = commit(&dir, "a.txt", b"1\nmore\n", "more").await;
        let mbox = root.join("c.mbox");
        format_patches(&dir, &[oid], &mbox.to_string_lossy(), true).await.unwrap();
        let text = std::fs::read_to_string(&mbox).unwrap();
        assert!(text.contains("--- a/a.txt") && text.contains("+more"), "{text}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
