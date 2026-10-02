//! In-progress operation state (merge / rebase / cherry-pick / revert) and
//! conflict resolution. Reads use git2; writes go through the git CLI.
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;

use git2::{Repository, RepositoryState};
use serde::Serialize;
use tokio::process::Command;

use crate::error::TwigError;
use crate::git::writer::{run_git, run_git_paths, GitOutput};

// ── CLI helper ───────────────────────────────────────────────────────

/// Like `run_git`, but with `GIT_EDITOR=true` (so `--continue` and friends
/// accept the prepared message instead of blocking on an editor) plus any
/// extra environment variables.
pub(crate) async fn run_git_noedit(
    repo_path: &Path,
    args: &[&str],
    envs: &[(&str, &str)],
) -> Result<GitOutput, TwigError> {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .current_dir(repo_path)
        .stdin(Stdio::null())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_MERGE_AUTOEDIT", "no")
        .env("GIT_EDITOR", "true")
        .kill_on_drop(true);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let output = cmd
        .output()
        .await
        .map_err(|e| TwigError::GitCli(format!("Failed to execute git: {e}")))?;
    Ok(GitOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// Quote a value for a POSIX shell (git runs editors through `sh`, also on
/// Windows via Git for Windows).
pub(crate) fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// Path as a forward-slash string, which both POSIX sh and Git for Windows'
/// sh accept.
pub(crate) fn shell_path(path: &Path) -> String {
    let s = path.to_string_lossy().to_string();
    if cfg!(windows) {
        s.replace('\\', "/")
    } else {
        s
    }
}

/// Reject anything that is not a plain repository-relative path.
pub(crate) fn validate_rel_path(file_path: &str) -> Result<(), TwigError> {
    let rel = Path::new(file_path);
    if file_path.is_empty()
        || !rel
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(TwigError::InvalidArgument(format!(
            "'{file_path}' is not a repository-relative path"
        )));
    }
    Ok(())
}

// ── Types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ConflictFile {
    pub path: String,
    /// both_modified | both_added | deleted_by_us | deleted_by_them |
    /// added_by_us | added_by_them | both_deleted
    pub kind: String,
    pub has_base: bool,
    pub has_ours: bool,
    pub has_theirs: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepoOperationState {
    /// none | merge | rebase | cherry_pick | revert | am | bisect
    pub kind: String,
    pub interactive: bool,
    pub conflicts: Vec<ConflictFile>,
    /// Prepared commit message (comment lines stripped), if git wrote one.
    pub message: Option<String>,
    /// Branch being rebased (short name), when known.
    pub head_name: Option<String>,
    /// Short OID the branch is being rebased onto.
    pub onto: Option<String>,
    pub step: Option<u32>,
    pub total: Option<u32>,
    /// Commit currently being applied/merged (full OID).
    pub current_commit: Option<String>,
    pub current_subject: Option<String>,
    /// Rebase stopped at an `edit` (or `break`) rather than a conflict.
    pub stopped_for_edit: bool,
    pub can_skip: bool,
    /// Label for the "ours" side in this operation (e.g. "HEAD", "upstream").
    pub ours_label: String,
    /// Label for the "theirs" side.
    pub theirs_label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConflictVersions {
    pub path: String,
    pub base: Option<String>,
    pub ours: Option<String>,
    pub theirs: Option<String>,
    /// Working-tree file (usually with conflict markers).
    pub merged: Option<String>,
    pub is_binary: bool,
    pub too_large: bool,
    /// Some version is not valid UTF-8. Text is withheld: editing a lossy
    /// decode and saving it would corrupt every non-UTF-8 byte.
    pub not_utf8: bool,
    /// The path is a symbolic link on some side; only taking a side makes
    /// sense.
    pub is_symlink: bool,
}

// ── Reads ────────────────────────────────────────────────────────────

/// Strip `#` comment lines (as `git commit --cleanup=strip` does) and
/// surrounding blank lines.
pub(crate) fn strip_comments(msg: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for line in msg.lines() {
        // Everything below a scissors line is discarded by git.
        if line.starts_with("# ------------------------ >8") {
            break;
        }
        if line.starts_with('#') {
            continue;
        }
        out.push(line.trim_end());
    }
    // Collapse consecutive blank lines.
    let mut collapsed: Vec<&str> = Vec::new();
    for line in out {
        if line.is_empty() && collapsed.last().is_some_and(|l| l.is_empty()) {
            continue;
        }
        collapsed.push(line);
    }
    collapsed.join("\n").trim().to_string()
}

fn read_trimmed(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn short(oid: &str) -> String {
    oid.chars().take(7).collect()
}

fn commit_subject(repo: &Repository, spec: &str) -> Option<(String, String)> {
    let obj = repo.revparse_single(spec).ok()?;
    let commit = obj.peel_to_commit().ok()?;
    Some((
        commit.id().to_string(),
        commit.summary().unwrap_or("").to_string(),
    ))
}

/// List conflicted index entries.
pub fn read_conflicts(repo: &Repository) -> Result<Vec<ConflictFile>, TwigError> {
    let mut index = repo.index()?;
    index.read(false)?;
    let mut out = Vec::new();
    if !index.has_conflicts() {
        return Ok(out);
    }
    for conflict in index.conflicts()? {
        let conflict = conflict?;
        let entry = conflict
            .our
            .as_ref()
            .or(conflict.their.as_ref())
            .or(conflict.ancestor.as_ref());
        let Some(entry) = entry else { continue };
        let path = String::from_utf8_lossy(&entry.path).to_string();
        let (b, o, t) = (
            conflict.ancestor.is_some(),
            conflict.our.is_some(),
            conflict.their.is_some(),
        );
        let kind = match (b, o, t) {
            (_, true, true) if b => "both_modified",
            (_, true, true) => "both_added",
            (true, false, true) => "deleted_by_us",
            (true, true, false) => "deleted_by_them",
            (false, true, false) => "added_by_us",
            (false, false, true) => "added_by_them",
            _ => "both_deleted",
        };
        out.push(ConflictFile {
            path,
            kind: kind.to_string(),
            has_base: b,
            has_ours: o,
            has_theirs: t,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// Determine which multi-step operation (if any) is in progress.
pub fn read_operation_state(repo: &Repository) -> Result<RepoOperationState, TwigError> {
    let gitdir = repo.path().to_path_buf();
    let state = repo.state();
    let conflicts = read_conflicts(repo)?;

    let (kind, interactive) = match state {
        RepositoryState::Clean => ("none", false),
        RepositoryState::Merge => ("merge", false),
        RepositoryState::Revert | RepositoryState::RevertSequence => ("revert", false),
        RepositoryState::CherryPick | RepositoryState::CherryPickSequence => {
            ("cherry_pick", false)
        }
        RepositoryState::Bisect => ("bisect", false),
        RepositoryState::RebaseInteractive => ("rebase", true),
        RepositoryState::Rebase
        | RepositoryState::RebaseMerge
        | RepositoryState::ApplyMailboxOrRebase => ("rebase", false),
        RepositoryState::ApplyMailbox => ("am", false),
    };

    let mut st = RepoOperationState {
        kind: kind.to_string(),
        interactive,
        conflicts,
        message: None,
        head_name: None,
        onto: None,
        step: None,
        total: None,
        current_commit: None,
        current_subject: None,
        stopped_for_edit: false,
        can_skip: matches!(kind, "rebase" | "cherry_pick" | "revert" | "am"),
        ours_label: "Ours (HEAD)".to_string(),
        theirs_label: "Theirs".to_string(),
    };

    let merge_msg = std::fs::read_to_string(gitdir.join("MERGE_MSG"))
        .ok()
        .map(|m| strip_comments(&m))
        .filter(|m| !m.is_empty());

    match kind {
        "merge" => {
            st.message = merge_msg;
            if let Some((oid, subject)) = commit_subject(repo, "MERGE_HEAD") {
                st.current_commit = Some(oid);
                st.current_subject = Some(subject);
            }
            st.theirs_label = "Theirs (incoming)".to_string();
        }
        "cherry_pick" => {
            st.message = merge_msg;
            if let Some((oid, subject)) = commit_subject(repo, "CHERRY_PICK_HEAD") {
                st.current_commit = Some(oid);
                st.current_subject = Some(subject);
            }
            st.theirs_label = "Theirs (picked commit)".to_string();
        }
        "revert" => {
            st.message = merge_msg;
            if let Some((oid, subject)) = commit_subject(repo, "REVERT_HEAD") {
                st.current_commit = Some(oid);
                st.current_subject = Some(subject);
            }
            st.theirs_label = "Theirs (reverted)".to_string();
        }
        "rebase" | "am" => {
            let merge_dir = gitdir.join("rebase-merge");
            let apply_dir = gitdir.join("rebase-apply");
            let dir = if merge_dir.is_dir() { merge_dir } else { apply_dir };
            st.head_name = read_trimmed(&dir.join("head-name"))
                .map(|h| h.trim_start_matches("refs/heads/").to_string());
            st.onto = read_trimmed(&dir.join("onto")).map(|o| short(&o));
            let (step, total) = if dir.join("msgnum").exists() {
                (dir.join("msgnum"), dir.join("end"))
            } else {
                (dir.join("next"), dir.join("last"))
            };
            st.step = read_trimmed(&step).and_then(|s| s.parse().ok());
            st.total = read_trimmed(&total).and_then(|s| s.parse().ok());

            let current = read_trimmed(&dir.join("stopped-sha"))
                .and_then(|sha| commit_subject(repo, &sha))
                .or_else(|| commit_subject(repo, "REBASE_HEAD"));
            if let Some((oid, subject)) = current {
                st.current_commit = Some(oid);
                st.current_subject = Some(subject);
            }
            // `amend` marks an edit stop; with no conflicts and nothing
            // pending it is a stop for the user to amend/inspect.
            st.stopped_for_edit = st.conflicts.is_empty() && dir.join("amend").exists();
            st.message = std::fs::read_to_string(dir.join("message"))
                .ok()
                .map(|m| strip_comments(&m))
                .filter(|m| !m.is_empty())
                .or(merge_msg);
            // During a rebase the sides are swapped from the user's view:
            // "ours" is the branch being rebased onto, "theirs" the commit
            // being replayed.
            st.ours_label = "Ours (upstream / onto)".to_string();
            st.theirs_label = "Theirs (your commit)".to_string();
        }
        _ => {}
    }

    Ok(st)
}

const MAX_VERSION_BYTES: usize = 2 * 1024 * 1024;

#[derive(Default)]
struct DecodeFlags {
    binary: bool,
    too_large: bool,
    not_utf8: bool,
}

fn decode(bytes: Vec<u8>, flags: &mut DecodeFlags) -> Option<String> {
    if bytes.len() > MAX_VERSION_BYTES {
        flags.too_large = true;
        return None;
    }
    if bytes.iter().take(8000).any(|&b| b == 0) {
        flags.binary = true;
        return None;
    }
    match String::from_utf8(bytes) {
        Ok(s) => Some(s),
        Err(_) => {
            flags.not_utf8 = true;
            None
        }
    }
}

/// Read base/ours/theirs from index stages 1-3 plus the working-tree file.
pub fn read_conflict_versions(
    repo: &Repository,
    file_path: &str,
) -> Result<ConflictVersions, TwigError> {
    validate_rel_path(file_path)?;
    let rel = Path::new(file_path);
    let mut index = repo.index()?;
    index.read(false)?;

    let mut flags = DecodeFlags::default();
    let mut is_symlink = false;
    let mut stage = |n: i32| -> Result<Option<String>, TwigError> {
        match index.get_path(rel, n) {
            Some(entry) => {
                is_symlink |= entry.mode == u32::from(git2::FileMode::Link);
                let blob = repo.find_blob(entry.id)?;
                Ok(decode(blob.content().to_vec(), &mut flags))
            }
            None => Ok(None),
        }
    };
    let mut base = stage(1)?;
    let mut ours = stage(2)?;
    let mut theirs = stage(3)?;

    let workdir = repo
        .workdir()
        .ok_or_else(|| TwigError::Git(git2::Error::from_str("bare repository")))?;
    let full = workdir.join(rel);
    is_symlink |= std::fs::symlink_metadata(&full).is_ok_and(|m| m.file_type().is_symlink());
    let mut merged = if is_symlink {
        None
    } else {
        match std::fs::read(&full) {
            Ok(data) => decode(data, &mut flags),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(TwigError::Io(e)),
        }
    };
    if flags.not_utf8 || is_symlink {
        // Never hand out a partial set: the editor would treat a missing
        // side as "deleted".
        (base, ours, theirs, merged) = (None, None, None, None);
    }

    Ok(ConflictVersions {
        path: file_path.to_string(),
        base,
        ours,
        theirs,
        merged,
        is_binary: flags.binary,
        too_large: flags.too_large,
        not_utf8: flags.not_utf8,
        is_symlink,
    })
}

// ── Writes ───────────────────────────────────────────────────────────

/// `git <op> --abort` for the in-progress operation.
pub async fn abort_operation(repo_path: &Path, kind: &str) -> Result<GitOutput, TwigError> {
    match kind {
        "merge" => run_git(repo_path, &["merge", "--abort"]).await,
        "rebase" => run_git_noedit(repo_path, &["rebase", "--abort"], &[]).await,
        "cherry_pick" => run_git(repo_path, &["cherry-pick", "--abort"]).await,
        "revert" => run_git(repo_path, &["revert", "--abort"]).await,
        "am" => run_git(repo_path, &["am", "--abort"]).await,
        "bisect" => run_git(repo_path, &["bisect", "reset"]).await,
        other => Err(TwigError::InvalidArgument(format!(
            "no abortable operation in progress ({other})"
        ))),
    }
}

/// `git <op> --skip` (drop the current commit and move on).
pub async fn skip_operation(repo_path: &Path, kind: &str) -> Result<GitOutput, TwigError> {
    let op = match kind {
        "rebase" => "rebase",
        "cherry_pick" => "cherry-pick",
        "revert" => "revert",
        "am" => "am",
        other => {
            return Err(TwigError::InvalidArgument(format!(
                "'{other}' cannot be skipped"
            )))
        }
    };
    run_git_noedit(repo_path, &[op, "--skip"], &[]).await
}

/// Continue the in-progress operation. `message` (if given) replaces the
/// prepared commit message for the commit git is about to create.
pub async fn continue_operation(
    repo_path: &Path,
    gitdir: &Path,
    kind: &str,
    message: Option<&str>,
) -> Result<GitOutput, TwigError> {
    let message = message.map(strip_comments).filter(|m| !m.is_empty());
    match kind {
        "merge" => {
            let msg = match message {
                Some(m) => m,
                None => std::fs::read_to_string(gitdir.join("MERGE_MSG"))
                    .map(|m| strip_comments(&m))
                    .unwrap_or_default(),
            };
            if msg.is_empty() {
                return Err(TwigError::InvalidArgument(
                    "a commit message is required to conclude the merge".to_string(),
                ));
            }
            run_git_noedit(repo_path, &["commit", "--cleanup=strip", "-m", &msg], &[]).await
        }
        "cherry_pick" | "revert" => {
            if let Some(m) = &message {
                std::fs::write(gitdir.join("MERGE_MSG"), format!("{m}\n"))?;
            }
            let op = if kind == "revert" { "revert" } else { "cherry-pick" };
            run_git_noedit(repo_path, &[op, "--continue"], &[]).await
        }
        "rebase" => {
            if let Some(m) = &message {
                let merge_dir = gitdir.join("rebase-merge");
                if merge_dir.join("message").exists() {
                    std::fs::write(merge_dir.join("message"), format!("{m}\n"))?;
                }
                if gitdir.join("MERGE_MSG").exists() {
                    std::fs::write(gitdir.join("MERGE_MSG"), format!("{m}\n"))?;
                }
            }
            run_git_noedit(repo_path, &["rebase", "--continue"], &[]).await
        }
        "am" => run_git_noedit(repo_path, &["am", "--continue"], &[]).await,
        other => Err(TwigError::InvalidArgument(format!(
            "no continuable operation in progress ({other})"
        ))),
    }
}

/// Resolve conflicted paths by taking one side wholesale. If that side
/// deleted the file, the file is removed instead.
pub async fn take_side(
    repo_path: &Path,
    paths_with_side: &[(String, bool)],
    side: &str,
) -> Result<GitOutput, TwigError> {
    let flag = match side {
        "ours" => "--ours",
        "theirs" => "--theirs",
        other => {
            return Err(TwigError::InvalidArgument(format!(
                "unknown conflict side '{other}'"
            )))
        }
    };
    let present: Vec<&str> = paths_with_side
        .iter()
        .filter(|(_, has)| *has)
        .map(|(p, _)| p.as_str())
        .collect();
    let deleted: Vec<&str> = paths_with_side
        .iter()
        .filter(|(_, has)| !*has)
        .map(|(p, _)| p.as_str())
        .collect();

    let mut last = GitOutput {
        success: true,
        stdout: String::new(),
        stderr: String::new(),
    };
    if !present.is_empty() {
        let out = run_git_paths(repo_path, &["checkout", flag], &present).await?;
        if !out.success {
            return Ok(out);
        }
        last = run_git_paths(repo_path, &["add"], &present).await?;
        if !last.success {
            return Ok(last);
        }
    }
    if !deleted.is_empty() {
        last = run_git_paths(repo_path, &["rm", "--quiet", "-f"], &deleted).await?;
    }
    Ok(last)
}

/// Mark paths as resolved (stage them, including deletions).
pub async fn mark_resolved(repo_path: &Path, paths: &[&str]) -> Result<GitOutput, TwigError> {
    run_git_paths(repo_path, &["add", "-A"], paths).await
}

/// Write resolved content to a working-tree file.
pub fn write_worktree_file(
    repo_path: &Path,
    file_path: &str,
    content: &str,
) -> Result<(), TwigError> {
    validate_rel_path(file_path)?;
    // Never write through a symlink (the file itself or a parent directory
    // inside the repo): it could point outside the working tree.
    let mut cur = repo_path.to_path_buf();
    for comp in Path::new(file_path).components() {
        cur.push(comp);
        if std::fs::symlink_metadata(&cur).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(TwigError::InvalidArgument(format!(
                "'{file_path}' goes through a symbolic link; resolve it by taking a side"
            )));
        }
    }
    let full: PathBuf = repo_path.join(file_path);
    std::fs::write(full, content)?;
    Ok(())
}

/// Launch a merge tool for one conflicted path and wait for it to exit.
/// `tool` is the user's configured tool: either a command template using
/// `$BASE`/`$LOCAL`/`$REMOTE`/`$MERGED`, or a path/name of a tool git knows
/// (meld, kdiff3, ...). Without one, git's `merge.tool` config is used.
pub async fn open_merge_tool(
    repo_path: &Path,
    file_path: &str,
    tool: Option<&str>,
) -> Result<GitOutput, TwigError> {
    validate_rel_path(file_path)?;
    let base: Vec<String> = vec![
        "-c".into(),
        "mergetool.keepBackup=false".into(),
        "-c".into(),
        "mergetool.prompt=false".into(),
    ];

    let tool = tool.map(str::trim).filter(|t| !t.is_empty());
    let Some(tool) = tool else {
        // No Twig setting: only defer to git if a tool is configured there,
        // otherwise git may fall back to a terminal tool (vimdiff) that has
        // no TTY to run in.
        let cfg = run_git(repo_path, &["config", "--get", "merge.tool"]).await?;
        if !cfg.success || cfg.stdout.trim().is_empty() {
            return Err(TwigError::Config(
                "No merge tool configured. Set one in Settings > Editor & Diff, \
                 or configure git's merge.tool."
                    .to_string(),
            ));
        }
        let mut args: Vec<&str> = base.iter().map(String::as_str).collect();
        args.extend_from_slice(&["mergetool", "--no-prompt", "--", file_path]);
        return run_git(repo_path, &args).await;
    };

    let custom = |cmd: String| -> Vec<String> {
        let mut a = base.clone();
        a.extend([
            "-c".to_string(),
            format!("mergetool.twig.cmd={cmd}"),
            "-c".to_string(),
            "mergetool.twig.trustExitCode=true".to_string(),
            "mergetool".to_string(),
            "--no-prompt".to_string(),
            "--tool=twig".to_string(),
            "--".to_string(),
            file_path.to_string(),
        ]);
        a
    };

    if tool.contains("$MERGED") {
        let args = custom(tool.to_string());
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        return run_git(repo_path, &refs).await;
    }

    // A path or name of a tool git has built-in support for.
    let name = Path::new(tool)
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        let mut a = base.clone();
        a.extend([
            "-c".to_string(),
            format!("mergetool.{name}.path={tool}"),
            "mergetool".to_string(),
            "--no-prompt".to_string(),
            format!("--tool={name}"),
            "--".to_string(),
            file_path.to_string(),
        ]);
        let refs: Vec<&str> = a.iter().map(String::as_str).collect();
        let out = run_git(repo_path, &refs).await?;
        let unknown = !out.success
            && (out.stderr.contains("Unknown merge tool") || out.stdout.contains("Unknown merge tool"));
        if !unknown {
            return Ok(out);
        }
    }

    // Unknown tool: hand it the merged file with conflict markers.
    let args = custom(format!("{} \"$MERGED\"", shell_quote(tool)));
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_git(repo_path, &refs).await
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn git_ok(dir: &Path, args: &[&str]) {
        let mut full = vec!["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"];
        full.extend_from_slice(args);
        let out = run_git(dir, &full).await.unwrap();
        assert!(out.success, "git {args:?}: {}", out.stderr);
    }

    async fn conflicted_repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("twig-conflicts-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git_ok(&dir, &["init", "-q", "-b", "main"]).await;
        git_ok(&dir, &["config", "user.name", "t"]).await;
        git_ok(&dir, &["config", "user.email", "t@t"]).await;
        git_ok(&dir, &["config", "commit.gpgsign", "false"]).await;
        std::fs::write(dir.join("f.txt"), "base\n").unwrap();
        std::fs::write(dir.join("gone.txt"), "x\n").unwrap();
        git_ok(&dir, &["add", "."]).await;
        git_ok(&dir, &["commit", "-q", "-m", "base"]).await;
        git_ok(&dir, &["checkout", "-q", "-b", "feature"]).await;
        std::fs::write(dir.join("f.txt"), "theirs\n").unwrap();
        std::fs::write(dir.join("gone.txt"), "changed\n").unwrap();
        git_ok(&dir, &["commit", "-q", "-am", "feature change"]).await;
        git_ok(&dir, &["checkout", "-q", "main"]).await;
        std::fs::write(dir.join("f.txt"), "ours\n").unwrap();
        git_ok(&dir, &["rm", "-q", "gone.txt"]).await;
        git_ok(&dir, &["commit", "-q", "-am", "main change"]).await;
        dir
    }

    /// Conflict between two Latin-1 edits of one file.
    async fn latin1_conflict(name: &str) -> PathBuf {
        let dir = conflicted_repo(name).await;
        std::fs::write(dir.join("l1.txt"), b"caf\xe9\n").unwrap();
        git_ok(&dir, &["add", "."]).await;
        git_ok(&dir, &["commit", "-q", "-m", "latin1"]).await;
        git_ok(&dir, &["checkout", "-q", "-b", "l1side"]).await;
        std::fs::write(dir.join("l1.txt"), b"caf\xe9 side\n").unwrap();
        git_ok(&dir, &["commit", "-q", "-am", "side"]).await;
        git_ok(&dir, &["checkout", "-q", "main"]).await;
        std::fs::write(dir.join("l1.txt"), b"caf\xe9 main\n").unwrap();
        git_ok(&dir, &["commit", "-q", "-am", "main"]).await;
        let out = run_git(&dir, &["merge", "--no-edit", "l1side"]).await.unwrap();
        assert!(!out.success);
        dir
    }

    #[tokio::test]
    async fn non_utf8_versions_are_not_offered_as_text() {
        let dir = latin1_conflict("latin1").await;
        let repo = Repository::open(&dir).unwrap();
        let v = read_conflict_versions(&repo, "l1.txt").unwrap();
        assert!(v.not_utf8, "lossy text would corrupt the file when saved");
        assert!(v.merged.is_none() && v.ours.is_none() && v.theirs.is_none());
        // Taking a side keeps the bytes intact.
        let out = take_side(&dir, &[("l1.txt".into(), true)], "theirs").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert_eq!(std::fs::read(dir.join("l1.txt")).unwrap(), b"caf\xe9 side\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn writes_never_follow_symlinks() {
        let dir = conflicted_repo("symlink").await;
        let outside = dir.with_extension("outside");
        let _ = std::fs::remove_dir_all(&outside);
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("target.txt"), "keep\n").unwrap();
        std::os::unix::fs::symlink(outside.join("target.txt"), dir.join("link.txt")).unwrap();
        std::os::unix::fs::symlink(&outside, dir.join("linkdir")).unwrap();

        assert!(write_worktree_file(&dir, "link.txt", "pwned\n").is_err());
        assert!(write_worktree_file(&dir, "linkdir/target.txt", "pwned\n").is_err());
        assert_eq!(std::fs::read_to_string(outside.join("target.txt")).unwrap(), "keep\n");
        // Regular files (including new ones in real subdirectories) still work.
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        write_worktree_file(&dir, "sub/new.txt", "ok\n").unwrap();
        write_worktree_file(&dir, "f.txt", "ok\n").unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap(), "ok\n");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn symlink_conflict_is_flagged() {
        let dir = conflicted_repo("symconf").await;
        std::os::unix::fs::symlink("a", dir.join("ln")).unwrap();
        git_ok(&dir, &["add", "."]).await;
        git_ok(&dir, &["commit", "-q", "-m", "ln"]).await;
        git_ok(&dir, &["checkout", "-q", "-b", "lnside"]).await;
        std::fs::remove_file(dir.join("ln")).unwrap();
        std::os::unix::fs::symlink("b", dir.join("ln")).unwrap();
        git_ok(&dir, &["commit", "-q", "-am", "b"]).await;
        git_ok(&dir, &["checkout", "-q", "main"]).await;
        std::fs::remove_file(dir.join("ln")).unwrap();
        std::os::unix::fs::symlink("c", dir.join("ln")).unwrap();
        git_ok(&dir, &["commit", "-q", "-am", "c"]).await;
        let out = run_git(&dir, &["merge", "--no-edit", "lnside"]).await.unwrap();
        assert!(!out.success);
        let repo = Repository::open(&dir).unwrap();
        let v = read_conflict_versions(&repo, "ln").unwrap();
        assert!(v.is_symlink);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn strip_comments_removes_hash_lines() {
        let msg = "Merge branch 'x'\n\n# Conflicts:\n#\tf.txt\n\nbody\n";
        assert_eq!(strip_comments(msg), "Merge branch 'x'\n\nbody");
    }

    #[tokio::test]
    async fn merge_conflict_detect_resolve_continue() {
        let dir = conflicted_repo("merge").await;
        let out = run_git(&dir, &["merge", "--no-edit", "feature"]).await.unwrap();
        assert!(!out.success);

        let repo = Repository::open(&dir).unwrap();
        let st = read_operation_state(&repo).unwrap();
        assert_eq!(st.kind, "merge");
        assert_eq!(st.conflicts.len(), 2);
        let f = st.conflicts.iter().find(|c| c.path == "f.txt").unwrap();
        assert_eq!(f.kind, "both_modified");
        let g = st.conflicts.iter().find(|c| c.path == "gone.txt").unwrap();
        assert_eq!(g.kind, "deleted_by_us");
        assert!(st.message.as_deref().unwrap_or("").starts_with("Merge branch 'feature'"));

        let v = read_conflict_versions(&repo, "f.txt").unwrap();
        assert_eq!(v.base.as_deref(), Some("base\n"));
        assert_eq!(v.ours.as_deref(), Some("ours\n"));
        assert_eq!(v.theirs.as_deref(), Some("theirs\n"));
        assert!(v.merged.unwrap().contains("<<<<<<<"));

        // Take theirs for f.txt, ours (deletion) for gone.txt.
        let out = take_side(&dir, &[("f.txt".into(), true)], "theirs").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        let out = take_side(&dir, &[("gone.txt".into(), false)], "ours").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap(), "theirs\n");
        assert!(!dir.join("gone.txt").exists());

        let st = read_operation_state(&repo).unwrap();
        assert!(st.conflicts.is_empty());

        let gitdir = repo.path().to_path_buf();
        let out = continue_operation(&dir, &gitdir, "merge", Some("Custom merge\n# comment"))
            .await
            .unwrap();
        assert!(out.success, "{}", out.stderr);
        let repo = Repository::open(&dir).unwrap();
        assert_eq!(read_operation_state(&repo).unwrap().kind, "none");
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.message().unwrap().trim(), "Custom merge");
        assert_eq!(head.parent_count(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn rebase_conflict_mark_resolved_and_abort() {
        let dir = conflicted_repo("rebase").await;
        git_ok(&dir, &["checkout", "-q", "feature"]).await;
        let out = run_git_noedit(&dir, &["rebase", "main"], &[]).await.unwrap();
        assert!(!out.success);
        let repo = Repository::open(&dir).unwrap();
        let st = read_operation_state(&repo).unwrap();
        assert_eq!(st.kind, "rebase");
        assert_eq!(st.head_name.as_deref(), Some("feature"));
        assert_eq!(st.step, Some(1));
        assert_eq!(st.total, Some(1));
        assert!(!st.conflicts.is_empty());
        assert_eq!(st.current_subject.as_deref(), Some("feature change"));

        write_worktree_file(&dir, "f.txt", "resolved\n").unwrap();
        let out = mark_resolved(&dir, &["f.txt"]).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert!(write_worktree_file(&dir, "../escape", "x").is_err());

        let out = abort_operation(&dir, "rebase").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        let repo = Repository::open(&dir).unwrap();
        assert_eq!(read_operation_state(&repo).unwrap().kind, "none");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn cherry_pick_continue_uses_message() {
        let dir = conflicted_repo("pick").await;
        let out = run_git(&dir, &["cherry-pick", "feature"]).await.unwrap();
        assert!(!out.success);
        let repo = Repository::open(&dir).unwrap();
        let st = read_operation_state(&repo).unwrap();
        assert_eq!(st.kind, "cherry_pick");
        assert!(st.can_skip);
        let paths: Vec<(String, bool)> =
            st.conflicts.iter().map(|c| (c.path.clone(), c.has_ours)).collect();
        let out = take_side(&dir, &paths, "ours").await.unwrap();
        assert!(out.success, "{}", out.stderr);
        // Resolution equals HEAD -> empty pick; allow via message path anyway.
        std::fs::write(dir.join("f.txt"), "mixed\n").unwrap();
        mark_resolved(&dir, &["f.txt"]).await.unwrap();
        let out = continue_operation(&dir, repo.path(), "cherry_pick", Some("Picked!"))
            .await
            .unwrap();
        assert!(out.success, "{}", out.stderr);
        let repo = Repository::open(&dir).unwrap();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.message().unwrap().trim(), "Picked!");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
