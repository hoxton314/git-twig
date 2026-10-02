//! CLI-based write operations — commit, push, pull, merge, rebase, stash, and all LFS ops.
//! Uses system `git` via tokio::process::Command for safety and LFS compatibility.
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

use crate::error::TwigError;

#[derive(Debug)]
pub struct GitOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

pub(crate) async fn run_git(repo_path: &Path, args: &[&str]) -> Result<GitOutput, TwigError> {
    let mut cmd = Command::new("git");
    // HTTPS auth env, when running inside `hosting::net_auth::with_network_auth`.
    crate::hosting::net_auth::apply_env(&mut cmd);
    let output = cmd
        .args(args)
        .current_dir(repo_path)
        // There is no terminal to answer prompts from a GUI. Without these,
        // a credential prompt or an editor (merge/pull commit message) would
        // block on a TTY inherited from the launching shell and hang forever.
        .stdin(Stdio::null())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_MERGE_AUTOEDIT", "no")
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| TwigError::GitCli(format!("Failed to execute git: {e}")))?;

    Ok(GitOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// Reject arguments that git would otherwise interpret as command-line flags.
/// Branch names, refs and remote names never legitimately begin with `-`
/// (git itself forbids it), so a leading dash means a crafted/invalid value.
pub(crate) fn safe_ref(value: &str) -> Result<(), TwigError> {
    if value.is_empty() {
        return Err(TwigError::GitCli("empty git ref/name argument".to_string()));
    }
    if value.starts_with('-') {
        return Err(TwigError::GitCli(format!(
            "invalid name '{value}': must not start with '-'"
        )));
    }
    Ok(())
}

/// Run a git command that takes user-supplied file paths after `--`.
/// `--literal-pathspecs` makes git treat each path verbatim, so file names
/// containing glob characters (`*`, `?`, `[`) or pathspec magic (`:(...)`)
/// cannot expand to other files — critical for destructive `restore`/`clean`.
pub(crate) async fn run_git_paths(
    repo_path: &Path,
    args: &[&str],
    paths: &[&str],
) -> Result<GitOutput, TwigError> {
    let mut full = Vec::with_capacity(args.len() + paths.len() + 2);
    full.push("--literal-pathspecs");
    full.extend_from_slice(args);
    full.push("--");
    full.extend_from_slice(paths);
    run_git(repo_path, &full).await
}

// ── Branch operations ─────────────────────────────────────────────────

pub async fn checkout_branch(repo_path: &Path, branch_name: &str) -> Result<GitOutput, TwigError> {
    safe_ref(branch_name)?;
    // Trailing `--` forces `branch_name` to be read as a revision. Without it,
    // a name that is not a ref but matches a file would make git *restore that
    // file* from the index, silently discarding its working-tree changes.
    run_git(repo_path, &["checkout", branch_name, "--"]).await
}

/// Checkout a remote branch (e.g. "origin/feature") as a local tracking branch.
/// If a local branch with the derived name already exists, checkout that instead
/// of failing — it is almost always the branch the user wants.
///
/// The remote is the configured remote whose name prefixes (git forbids
/// overlapping remote names) `remote_branch`, so names with `/` work.
pub async fn checkout_remote_branch(
    repo_path: &Path,
    remote_branch: &str,
) -> Result<GitOutput, TwigError> {
    safe_ref(remote_branch)?;
    let remotes = run_git(repo_path, &["remote"]).await?;
    let remote = remotes
        .stdout
        .lines()
        .map(str::trim)
        .filter(|r| {
            !r.is_empty()
                && remote_branch.len() > r.len() + 1
                && remote_branch.starts_with(r)
                && remote_branch.as_bytes()[r.len()] == b'/'
        })
        .max_by_key(|r| r.len());
    let Some(remote) = remote else {
        return Err(TwigError::GitCli(format!(
            "'{remote_branch}' is not a remote branch name"
        )));
    };
    let branch = &remote_branch[remote.len() + 1..];
    super::branch_ops::checkout_remote_tracking(repo_path, remote, branch).await
}

pub async fn create_branch(
    repo_path: &Path,
    branch_name: &str,
    start_point: Option<&str>,
) -> Result<GitOutput, TwigError> {
    safe_ref(branch_name)?;
    match start_point {
        Some(sp) => {
            safe_ref(sp)?;
            run_git(repo_path, &["checkout", "-b", branch_name, sp, "--"]).await
        }
        None => run_git(repo_path, &["checkout", "-b", branch_name]).await,
    }
}

pub async fn rename_branch(
    repo_path: &Path,
    old_name: &str,
    new_name: &str,
) -> Result<GitOutput, TwigError> {
    safe_ref(old_name)?;
    safe_ref(new_name)?;
    run_git(repo_path, &["branch", "-m", old_name, new_name]).await
}

pub async fn delete_branch(
    repo_path: &Path,
    branch_name: &str,
    force: bool,
) -> Result<GitOutput, TwigError> {
    safe_ref(branch_name)?;
    let flag = if force { "-D" } else { "-d" };
    run_git(repo_path, &["branch", flag, branch_name]).await
}

/// Delete a branch on a remote (`git push <remote> --delete <branch>`).
/// `branch_name` is the branch name without the remote prefix.
pub async fn delete_remote_branch(
    repo_path: &Path,
    remote: &str,
    branch_name: &str,
) -> Result<GitOutput, TwigError> {
    safe_ref(remote)?;
    safe_ref(branch_name)?;
    run_git(repo_path, &["push", remote, "--delete", branch_name]).await
}

pub async fn push_branch(
    repo_path: &Path,
    remote: &str,
    branch_name: &str,
    set_upstream: bool,
) -> Result<GitOutput, TwigError> {
    safe_ref(remote)?;
    safe_ref(branch_name)?;
    // Qualified source so a same-named tag can't make it ambiguous; no
    // destination, so `remote.<name>.push` mappings (e.g. Gerrit
    // `refs/for/*`) still apply.
    let refspec = format!("refs/heads/{branch_name}");
    let mut args = vec!["push"];
    if set_upstream {
        args.push("-u");
    }
    args.extend(["--", remote, &refspec]);
    run_git(repo_path, &args).await
}

pub async fn pull(repo_path: &Path, remote: &str, branch: Option<&str>) -> Result<GitOutput, TwigError> {
    safe_ref(remote)?;
    match branch {
        Some(b) => {
            safe_ref(b)?;
            run_git(repo_path, &["pull", remote, b]).await
        }
        None => run_git(repo_path, &["pull", remote]).await,
    }
}

pub async fn has_uncommitted_changes(repo_path: &Path) -> Result<bool, TwigError> {
    // `--no-optional-locks` keeps this read-only probe from taking index.lock,
    // which could otherwise make a concurrent write fail.
    let output = run_git(repo_path, &["--no-optional-locks", "status", "--porcelain"]).await?;
    if !output.success {
        return Err(TwigError::GitCli(output.stderr));
    }
    Ok(!output.stdout.trim().is_empty())
}

pub async fn fetch_all(repo_path: &Path) -> Result<GitOutput, TwigError> {
    run_git(repo_path, &["fetch", "--all", "--prune"]).await
}

pub async fn merge_branch(repo_path: &Path, branch_name: &str) -> Result<GitOutput, TwigError> {
    safe_ref(branch_name)?;
    run_git(repo_path, &["merge", "--no-edit", branch_name]).await
}

// ── Undo operations ──────────────────────────────────────────────────

/// Whether `rev` resolves to a commit (e.g. `HEAD` is born, `HEAD~1` exists).
pub(crate) async fn rev_exists(repo_path: &Path, rev: &str) -> Result<bool, TwigError> {
    let spec = format!("{rev}^{{commit}}");
    let out = run_git(repo_path, &["rev-parse", "--verify", "--quiet", &spec]).await?;
    Ok(out.success)
}

pub async fn undo_last_commit(repo_path: &Path) -> Result<GitOutput, TwigError> {
    if !rev_exists(repo_path, "HEAD").await? {
        return Err(TwigError::GitCli("there is no commit to undo".to_string()));
    }
    if rev_exists(repo_path, "HEAD~1").await? {
        run_git(repo_path, &["reset", "--soft", "HEAD~1"]).await
    } else {
        // Root commit: `HEAD~1` does not exist. Deleting the branch ref that
        // HEAD points to returns the repo to an unborn branch while keeping
        // the index — the equivalent of a soft reset past the first commit.
        run_git(repo_path, &["update-ref", "-d", "HEAD"]).await
    }
}

// ── Discard operations ────────────────────────────────────────────────

pub async fn restore_files(repo_path: &Path, paths: &[&str]) -> Result<GitOutput, TwigError> {
    run_git_paths(repo_path, &["restore"], paths).await
}

pub async fn clean_files(repo_path: &Path, paths: &[&str]) -> Result<GitOutput, TwigError> {
    // `-d` is required to remove untracked directories; without it `git clean`
    // silently leaves directories behind while still reporting success.
    run_git_paths(repo_path, &["clean", "-fd"], paths).await
}

// ── Commit operations ─────────────────────────────────────────────────

pub async fn stage_files(repo_path: &Path, paths: &[&str]) -> Result<GitOutput, TwigError> {
    run_git_paths(repo_path, &["add"], paths).await
}

pub async fn unstage_files(repo_path: &Path, paths: &[&str]) -> Result<GitOutput, TwigError> {
    if rev_exists(repo_path, "HEAD").await? {
        run_git_paths(repo_path, &["restore", "--staged"], paths).await
    } else {
        // Unborn branch (no commits yet): `restore --staged` needs HEAD to
        // exist, so drop the entries from the index instead. `-f` only skips
        // the "staged content differs" safety check; with `--cached` the
        // working tree is never touched.
        run_git_paths(
            repo_path,
            &["rm", "--cached", "-r", "-f", "--quiet", "--ignore-unmatch"],
            paths,
        )
        .await
    }
}

pub async fn commit(repo_path: &Path, message: &str) -> Result<GitOutput, TwigError> {
    run_git(repo_path, &["commit", "-m", message]).await
}

// ── Stash ─────────────────────────────────────────────────────────────

pub async fn stash_push(
    repo_path: &Path,
    message: Option<&str>,
) -> Result<GitOutput, TwigError> {
    match message {
        Some(msg) => run_git(repo_path, &["stash", "push", "--include-untracked", "-m", msg]).await,
        None => run_git(repo_path, &["stash", "push", "--include-untracked"]).await,
    }
}

pub async fn stash_pop(repo_path: &Path, index: u32) -> Result<GitOutput, TwigError> {
    let stash_ref = format!("stash@{{{index}}}");
    run_git(repo_path, &["stash", "pop", &stash_ref]).await
}

pub async fn stash_apply(repo_path: &Path, index: u32) -> Result<GitOutput, TwigError> {
    let stash_ref = format!("stash@{{{index}}}");
    run_git(repo_path, &["stash", "apply", &stash_ref]).await
}

pub async fn stash_drop(repo_path: &Path, index: u32) -> Result<GitOutput, TwigError> {
    let stash_ref = format!("stash@{{{index}}}");
    run_git(repo_path, &["stash", "drop", &stash_ref]).await
}

pub async fn stash_list(repo_path: &Path) -> Result<GitOutput, TwigError> {
    run_git(
        repo_path,
        &["stash", "list", "--format=%gd%x00%s%x00%aI"],
    )
    .await
}

/// Resolve the commit SHA of the stash entry at the top of the stack
/// (`stash@{0}`), or `None` if there is no stash.
pub async fn stash_top_sha(repo_path: &Path) -> Result<Option<String>, TwigError> {
    let out = run_git(repo_path, &["rev-parse", "--verify", "--quiet", "stash@{0}"]).await?;
    let sha = out.stdout.trim().to_string();
    Ok(if sha.is_empty() { None } else { Some(sha) })
}

/// Find the current stack index of the stash entry whose commit matches `sha`.
/// Stash indices shift as entries are added/removed, so a previously-captured
/// SHA must be re-resolved to an index before popping it.
pub async fn stash_index_for_sha(repo_path: &Path, sha: &str) -> Result<Option<u32>, TwigError> {
    let out = run_git(repo_path, &["stash", "list", "--format=%gd %H"]).await?;
    for line in out.stdout.lines() {
        if let Some((reference, line_sha)) = line.split_once(' ') {
            if line_sha.trim() == sha {
                return Ok(reference
                    .trim_start_matches("stash@{")
                    .trim_end_matches('}')
                    .parse::<u32>()
                    .ok());
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    async fn git_ok(dir: &Path, args: &[&str]) {
        let mut full = vec!["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"];
        full.extend_from_slice(args);
        let out = run_git(dir, &full).await.unwrap();
        assert!(out.success, "git {args:?}: {}", out.stderr);
    }

    async fn temp_repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("twig-writer-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git_ok(&dir, &["init", "-q", "-b", "main"]).await;
        dir
    }

    #[tokio::test]
    async fn checkout_remote_branch_with_slashed_remote() {
        let dir = temp_repo("remote-co").await;
        git_ok(&dir, &["commit", "-q", "--allow-empty", "-m", "c1"]).await;
        let head = run_git(&dir, &["rev-parse", "HEAD"]).await.unwrap().stdout.trim().to_string();
        git_ok(&dir, &["remote", "add", "origin", "https://example.invalid/a.git"]).await;
        git_ok(&dir, &["remote", "add", "my/fork", "https://example.invalid/b.git"]).await;
        git_ok(&dir, &["update-ref", "refs/remotes/my/fork/feature", &head]).await;
        git_ok(&dir, &["update-ref", "refs/remotes/origin/topic", &head]).await;

        let out = checkout_remote_branch(&dir, "my/fork/feature").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        let cur = run_git(&dir, &["symbolic-ref", "--short", "HEAD"]).await.unwrap();
        assert_eq!(cur.stdout.trim(), "feature");
        let up = run_git(&dir, &["rev-parse", "--abbrev-ref", "feature@{upstream}"]).await.unwrap();
        assert_eq!(up.stdout.trim(), "my/fork/feature");

        let out = checkout_remote_branch(&dir, "origin/topic").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        // Existing local branch is checked out instead of failing.
        let out = checkout_remote_branch(&dir, "my/fork/feature").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert!(checkout_remote_branch(&dir, "nope/x").await.is_err());
        assert!(checkout_remote_branch(&dir, "origin/HEAD").await.map(|o| !o.success).unwrap_or(true));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn push_branch_with_same_named_tag() {
        let dir = temp_repo("push-tag").await;
        git_ok(&dir, &["commit", "-q", "--allow-empty", "-m", "c1"]).await;
        let bare = dir.with_extension("bare.git");
        let _ = std::fs::remove_dir_all(&bare);
        let bare_s = bare.to_string_lossy().to_string();
        git_ok(&dir, &["init", "-q", "--bare", &bare_s]).await;
        git_ok(&dir, &["remote", "add", "origin", &bare_s]).await;
        git_ok(&dir, &["tag", "release"]).await;
        git_ok(&dir, &["branch", "release"]).await;
        let out = push_branch(&dir, "origin", "release", true).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        let up = run_git(&dir, &["rev-parse", "--abbrev-ref", "release@{upstream}"]).await.unwrap();
        assert_eq!(up.stdout.trim(), "origin/release");
        let out = push_branch(&dir, "origin", "main", false).await.unwrap();
        assert!(out.success, "{}", out.stderr);

        // A configured push mapping is honoured.
        git_ok(&dir, &["config", "remote.origin.push", "refs/heads/*:refs/for/*"]).await;
        let out = push_branch(&dir, "origin", "main", false).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        let mapped = run_git(&bare, &["rev-parse", "--verify", "--quiet", "refs/for/main"]).await.unwrap();
        assert!(mapped.success, "push mapping ignored");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&bare);
    }

    #[tokio::test]
    async fn unborn_unstage_and_root_undo() {
        let dir = temp_repo("unborn").await;
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
        stage_files(&dir, &["a.txt"]).await.unwrap();
        std::fs::write(dir.join("a.txt"), "a2\n").unwrap();
        let out = unstage_files(&dir, &["a.txt"]).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert!(dir.join("a.txt").exists());

        stage_files(&dir, &["a.txt"]).await.unwrap();
        git_ok(&dir, &["commit", "-q", "-m", "root"]).await;
        let out = undo_last_commit(&dir).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert!(!rev_exists(&dir, "HEAD").await.unwrap());
        assert!(undo_last_commit(&dir).await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn checkout_does_not_restore_files_and_globs_are_literal() {
        let dir = temp_repo("checkout").await;
        std::fs::write(dir.join("README"), "v1\n").unwrap();
        stage_files(&dir, &["README"]).await.unwrap();
        git_ok(&dir, &["commit", "-q", "-m", "c1"]).await;
        std::fs::write(dir.join("README"), "dirty\n").unwrap();

        // Not a branch: must fail rather than discard README's changes.
        let out = checkout_branch(&dir, "README").await.unwrap();
        assert!(!out.success);
        assert_eq!(std::fs::read_to_string(dir.join("README")).unwrap(), "dirty\n");

        // A glob-looking name must only clean that exact file.
        std::fs::write(dir.join("*.tmp"), "").unwrap();
        std::fs::write(dir.join("keep.tmp"), "").unwrap();
        let out = clean_files(&dir, &["*.tmp"]).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert!(!dir.join("*.tmp").exists());
        assert!(dir.join("keep.tmp").exists());

        git_ok(&dir, &["branch", "feature"]).await;
        let out = checkout_branch(&dir, "feature").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        let out = create_branch(&dir, "other", Some("main")).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
