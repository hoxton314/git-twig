//! Partial staging: stage / unstage / discard individual hunks or lines.
//!
//! The diff for one file is re-read with git2 (always with full whitespace
//! and fixed context, regardless of the display options), the user's
//! selection is mapped onto it by line number, and a patch containing only
//! the selected changes is built and fed to `git apply` (CLI, like all
//! writes):
//!
//! | action  | diff read          | apply                          |
//! |---------|--------------------|--------------------------------|
//! | stage   | index -> workdir   | `git apply --cached`           |
//! | unstage | HEAD -> index      | `git apply --cached --reverse` |
//! | discard | index -> workdir   | `git apply --reverse`          |
//!
//! For a forward patch, unselected removals become context and unselected
//! additions are dropped (the old side must match the target). For a reverse
//! patch it is the other way round: the new side must match what is on disk
//! or in the index, so unselected additions become context and unselected
//! removals are dropped.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use git2::{DiffOptions, Repository};
use serde::Deserialize;

use crate::error::TwigError;
use crate::git::writer::{run_git, GitOutput};

/// Which working-tree diff the selection was made in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffArea {
    Staged,
    Unstaged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HunkAction {
    Stage,
    Unstage,
    Discard,
}

/// A changed line the user selected, exactly as it was displayed. `content`
/// (including its line terminator) is compared against the freshly read diff
/// so a stale view can never stage or discard a different line.
#[derive(Debug, Clone, Deserialize)]
pub struct SelectedLine {
    pub origin: String,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub content: String,
}

/// A displayed hunk's line ranges. Every change inside them is selected, so
/// "stage hunk" also covers whitespace-only changes hidden by the display
/// options.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct HunkRange {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct PatchLine {
    pub origin: char,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub content: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct PatchHunk {
    pub old_start: u32,
    pub new_start: u32,
    pub lines: Vec<PatchLine>,
}

/// One file's full-fidelity diff, as needed to build a partial patch.
#[derive(Debug, Clone)]
pub(crate) struct FilePatchSource {
    pub old_path: String,
    pub new_path: String,
    pub old_exists: bool,
    pub new_exists: bool,
    pub old_mode: u32,
    pub new_mode: u32,
    pub hunks: Vec<PatchHunk>,
}

const STALE_DIFF: &str = "The diff has changed since it was displayed. Refresh and try again.";

/// Read the diff of `file_path` in `area` with fixed context and no
/// whitespace options, keeping raw line bytes (CRLF, non-UTF-8 intact).
pub(crate) fn read_patch_source(
    repo: &Repository,
    area: DiffArea,
    file_path: &str,
) -> Result<FilePatchSource, TwigError> {
    if file_path.is_empty() {
        return Err(TwigError::InvalidArgument("empty file path".to_string()));
    }
    let mut opts = DiffOptions::new();
    opts.context_lines(3);
    opts.pathspec(file_path);
    opts.disable_pathspec_match(true);

    let diff = match area {
        DiffArea::Staged => {
            let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
            repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))?
        }
        DiffArea::Unstaged => {
            opts.include_untracked(true);
            opts.recurse_untracked_dirs(true);
            opts.show_untracked_content(true);
            repo.diff_index_to_workdir(None, Some(&mut opts))?
        }
    };

    for idx in 0..diff.deltas().len() {
        let Some(patch) = git2::Patch::from_diff(&diff, idx)? else {
            continue;
        };
        let delta = patch.delta();
        let old_path = delta.old_file().path().map(|p| p.to_string_lossy().into_owned());
        let new_path = delta.new_file().path().map(|p| p.to_string_lossy().into_owned());
        if new_path.as_deref() != Some(file_path) && old_path.as_deref() != Some(file_path) {
            continue;
        }
        if delta.status() == git2::Delta::Typechange {
            return Err(TwigError::InvalidArgument(
                "File type changes can only be staged or discarded as a whole".to_string(),
            ));
        }
        if delta.old_file().is_binary() || delta.new_file().is_binary() {
            return Err(TwigError::InvalidArgument(
                "Binary files can only be staged or discarded as a whole".to_string(),
            ));
        }
        let old_mode = u32::from(delta.old_file().mode());
        let new_mode = u32::from(delta.new_file().mode());
        if old_mode == u32::from(git2::FileMode::Commit) || new_mode == u32::from(git2::FileMode::Commit) {
            return Err(TwigError::InvalidArgument(
                "Submodule changes cannot be staged line by line".to_string(),
            ));
        }

        let mut hunks = Vec::with_capacity(patch.num_hunks());
        for h in 0..patch.num_hunks() {
            let (hunk, n) = patch.hunk(h)?;
            let mut lines = Vec::with_capacity(n);
            for l in 0..n {
                let line = patch.line_in_hunk(h, l)?;
                let origin = line.origin();
                // EOF-newline pseudo lines are re-derived from the content.
                if !matches!(origin, ' ' | '+' | '-') {
                    continue;
                }
                lines.push(PatchLine {
                    origin,
                    old_lineno: line.old_lineno(),
                    new_lineno: line.new_lineno(),
                    content: line.content().to_vec(),
                });
            }
            hunks.push(PatchHunk {
                old_start: hunk.old_start(),
                new_start: hunk.new_start(),
                lines,
            });
        }

        let path = new_path.clone().or(old_path.clone()).unwrap_or_default();
        return Ok(FilePatchSource {
            old_path: old_path.unwrap_or_else(|| path.clone()),
            new_path: new_path.unwrap_or_else(|| path.clone()),
            old_exists: !delta.old_file().id().is_zero(),
            new_exists: !delta.new_file().id().is_zero()
                || (area == DiffArea::Unstaged && delta.status() != git2::Delta::Deleted),
            old_mode,
            new_mode,
            hunks,
        });
    }
    Err(TwigError::InvalidArgument(STALE_DIFF.to_string()))
}

/// Resolve the user's selection to a predicate over the source diff's
/// changed lines, verifying every explicitly selected line still exists with
/// the same content.
pub(crate) fn resolve_selection(
    src: &FilePatchSource,
    lines: &[SelectedLine],
    ranges: &[HunkRange],
) -> Result<Vec<Vec<bool>>, TwigError> {
    let mut wanted: HashMap<(char, u32), &str> = HashMap::new();
    for l in lines {
        let key = match l.origin.as_str() {
            "+" => ('+', l.new_lineno),
            "-" => ('-', l.old_lineno),
            _ => continue, // context lines carry no change
        };
        let Some(n) = key.1 else {
            return Err(TwigError::InvalidArgument("selected line has no line number".to_string()));
        };
        wanted.insert((key.0, n), l.content.as_str());
    }

    let in_ranges = |line: &PatchLine| {
        ranges.iter().any(|r| match line.origin {
            '-' => line
                .old_lineno
                .is_some_and(|n| n >= r.old_start && n < r.old_start.saturating_add(r.old_lines)),
            '+' => line
                .new_lineno
                .is_some_and(|n| n >= r.new_start && n < r.new_start.saturating_add(r.new_lines)),
            _ => false,
        })
    };

    let mut found = 0usize;
    let mut out = Vec::with_capacity(src.hunks.len());
    for hunk in &src.hunks {
        let mut sel = Vec::with_capacity(hunk.lines.len());
        for line in &hunk.lines {
            let lineno = match line.origin {
                '+' => line.new_lineno,
                '-' => line.old_lineno,
                _ => None,
            };
            let explicit = lineno.and_then(|n| wanted.get(&(line.origin, n)));
            if let Some(expected) = explicit {
                if String::from_utf8_lossy(&line.content) != *expected {
                    return Err(TwigError::InvalidArgument(STALE_DIFF.to_string()));
                }
                found += 1;
            }
            sel.push(line.origin != ' ' && (explicit.is_some() || in_ranges(line)));
        }
        out.push(sel);
    }
    if found != wanted.len() {
        return Err(TwigError::InvalidArgument(STALE_DIFF.to_string()));
    }
    Ok(out)
}

/// Quote a path for a patch header the way git does when it contains
/// characters that would otherwise be ambiguous.
fn quote_path(prefix: &str, path: &str) -> String {
    let needs = path
        .chars()
        .any(|c| c == '"' || c == '\\' || c.is_control());
    if !needs {
        return format!("{prefix}{path}");
    }
    let mut s = String::from("\"");
    s.push_str(prefix);
    for c in path.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\t' => s.push_str("\\t"),
            '\r' => s.push_str("\\r"),
            c if c.is_control() => s.push_str(&format!("\\{:03o}", c as u32)),
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

fn ends_nl(b: &[u8]) -> bool {
    b.last() == Some(&b'\n')
}

/// Build a patch containing only the selected changes. `selected[h][l]`
/// marks lines of `src.hunks[h]`. Returns `None` if nothing is selected.
pub(crate) fn build_patch(
    src: &FilePatchSource,
    selected: &[Vec<bool>],
    reverse: bool,
) -> Option<Vec<u8>> {
    let mut total_changes = 0usize;
    let mut chosen = 0usize;
    for (h, hunk) in src.hunks.iter().enumerate() {
        for (l, line) in hunk.lines.iter().enumerate() {
            if line.origin != ' ' {
                total_changes += 1;
                if selected.get(h).and_then(|s| s.get(l)).copied().unwrap_or(false) {
                    chosen += 1;
                }
            }
        }
    }
    if chosen == 0 {
        return None;
    }
    let all = chosen == total_changes;

    // Whether each side of the *patch* exists. The anchored side (old when
    // forward, new when reverse) is unchanged; the other side disappears only
    // if every change of an add/delete is selected.
    let (old_exists, new_exists) = if reverse {
        (src.old_exists || !all, src.new_exists)
    } else {
        (src.old_exists, src.new_exists || !all)
    };

    let crlf = src
        .hunks
        .iter()
        .flat_map(|h| h.lines.iter())
        .any(|l| l.content.ends_with(b"\r\n"));
    let eol: &[u8] = if crlf { b"\r\n" } else { b"\n" };

    let mut out: Vec<u8> = Vec::new();
    let a = quote_path("a/", &src.old_path);
    let b = quote_path("b/", &src.new_path);
    out.extend_from_slice(format!("diff --git {a} {b}\n").as_bytes());
    if !old_exists {
        out.extend_from_slice(format!("new file mode {:o}\n", src.new_mode).as_bytes());
    } else if !new_exists {
        out.extend_from_slice(format!("deleted file mode {:o}\n", src.old_mode).as_bytes());
    }
    let minus = if old_exists { a.clone() } else { "/dev/null".to_string() };
    let plus = if new_exists { b.clone() } else { "/dev/null".to_string() };
    out.extend_from_slice(format!("--- {minus}\n+++ {plus}\n").as_bytes());

    // Net (new - old) line count change of the hunks emitted so far, used
    // to place the non-anchored side of later hunks.
    let mut offset: i64 = 0;
    for (h, hunk) in src.hunks.iter().enumerate() {
        let sel = selected.get(h);
        let is_sel = |l: usize| sel.and_then(|s| s.get(l)).copied().unwrap_or(false);
        if !hunk.lines.iter().enumerate().any(|(l, line)| line.origin != ' ' && is_sel(l)) {
            continue;
        }

        let mut lines: Vec<(u8, Vec<u8>)> = Vec::with_capacity(hunk.lines.len());
        for (l, line) in hunk.lines.iter().enumerate() {
            match (line.origin, is_sel(l), reverse) {
                (' ', _, _) => lines.push((b' ', line.content.clone())),
                ('-', true, _) => lines.push((b'-', line.content.clone())),
                ('+', true, _) => lines.push((b'+', line.content.clone())),
                ('-', false, false) => lines.push((b' ', line.content.clone())),
                ('+', false, true) => lines.push((b' ', line.content.clone())),
                _ => {} // unselected '+' forward / '-' reverse: dropped
            }
        }

        // A line without a trailing newline must be the last line of its
        // side. Unselected lines turned into context can break that (e.g. an
        // old last line kept while additions are appended after it), so give
        // such lines a newline on the side that continues.
        let mut i = 0;
        while i < lines.len() {
            if !ends_nl(&lines[i].1) {
                let rest = &lines[i + 1..];
                let old_after = rest.iter().any(|(o, _)| *o == b' ' || *o == b'-');
                let new_after = rest.iter().any(|(o, _)| *o == b' ' || *o == b'+');
                let origin = lines[i].0;
                let mut with_nl = lines[i].1.clone();
                with_nl.extend_from_slice(eol);
                match origin {
                    b' ' if old_after && new_after => lines[i].1 = with_nl,
                    b' ' if old_after => {
                        let bare = lines[i].1.clone();
                        lines[i] = (b'-', with_nl);
                        lines.insert(i + 1, (b'+', bare));
                        i += 1;
                    }
                    b' ' if new_after => {
                        let bare = lines[i].1.clone();
                        lines[i] = (b'-', bare);
                        lines.insert(i + 1, (b'+', with_nl));
                        i += 1;
                    }
                    b'-' if old_after => lines[i].1 = with_nl,
                    b'+' if new_after => lines[i].1 = with_nl,
                    _ => {}
                }
            }
            i += 1;
        }

        let old_count = lines.iter().filter(|(o, _)| *o != b'+').count() as i64;
        let new_count = lines.iter().filter(|(o, _)| *o != b'-').count() as i64;
        // A zero-length side's start is the line *before* the hunk.
        let (old_start, new_start) = if reverse {
            let new_start = hunk.new_start as i64;
            let new_first = if new_count == 0 { new_start + 1 } else { new_start };
            let old_first = new_first - offset;
            let old_start = if old_count == 0 { old_first - 1 } else { old_first };
            (old_start, new_start)
        } else {
            let old_start = hunk.old_start as i64;
            let old_first = if old_count == 0 { old_start + 1 } else { old_start };
            let new_first = old_first + offset;
            let new_start = if new_count == 0 { new_first - 1 } else { new_first };
            (old_start, new_start)
        };
        offset += new_count - old_count;

        out.extend_from_slice(
            format!(
                "@@ -{},{} +{},{} @@\n",
                old_start.max(0),
                old_count,
                new_start.max(0),
                new_count
            )
            .as_bytes(),
        );
        for (origin, content) in &lines {
            out.push(*origin);
            out.extend_from_slice(content);
            if !ends_nl(content) {
                out.extend_from_slice(b"\n\\ No newline at end of file\n");
            }
        }
    }
    Some(out)
}

/// Removes the temporary patch file when dropped.
struct TempPatch(PathBuf);

impl Drop for TempPatch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

static PATCH_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Apply a patch with `git apply`. The patch goes through a private temp file
/// because the shared CLI runner closes stdin.
pub(crate) async fn apply_patch(
    repo_path: &Path,
    patch: &[u8],
    cached: bool,
    reverse: bool,
) -> Result<GitOutput, TwigError> {
    let n = PATCH_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file = std::env::temp_dir().join(format!("twig-{}-{n}.patch", std::process::id()));
    std::fs::write(&file, patch)?;
    let guard = TempPatch(file);
    let path_str = guard.0.to_string_lossy().into_owned();

    let mut args = vec!["apply", "--whitespace=nowarn"];
    if cached {
        args.push("--cached");
    }
    if reverse {
        args.push("--reverse");
    }
    args.push(&path_str);
    let out = run_git(repo_path, &args).await;
    drop(guard);
    out
}

/// Validate the action/area combination and return `(cached, reverse)`.
pub(crate) fn apply_mode(area: DiffArea, action: HunkAction) -> Result<(bool, bool), TwigError> {
    match (area, action) {
        (DiffArea::Unstaged, HunkAction::Stage) => Ok((true, false)),
        (DiffArea::Staged, HunkAction::Unstage) => Ok((true, true)),
        (DiffArea::Unstaged, HunkAction::Discard) => Ok((false, true)),
        _ => Err(TwigError::InvalidArgument(format!(
            "cannot {action:?} lines of the {area:?} diff"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::writer::run_git;

    async fn git_ok(dir: &Path, args: &[&str]) -> String {
        let mut full = vec!["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"];
        full.extend_from_slice(args);
        let out = run_git(dir, &full).await.unwrap();
        assert!(out.success, "git {args:?}: {}", out.stderr);
        out.stdout
    }

    async fn temp_repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("twig-hunks-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git_ok(&dir, &["init", "-q", "-b", "main"]).await;
        git_ok(&dir, &["config", "core.autocrlf", "false"]).await;
        dir
    }

    async fn commit_file(dir: &Path, name: &str, content: &[u8]) {
        std::fs::write(dir.join(name), content).unwrap();
        git_ok(dir, &["add", "--", name]).await;
        git_ok(dir, &["commit", "-q", "-m", "c"]).await;
    }

    async fn index_content(dir: &Path, name: &str) -> Option<Vec<u8>> {
        let out = run_git(dir, &["show", &format!(":{name}")]).await.unwrap();
        if out.success {
            let raw = tokio::process::Command::new("git")
                .args(["show", &format!(":{name}")])
                .current_dir(dir)
                .output()
                .await
                .unwrap();
            Some(raw.stdout)
        } else {
            None
        }
    }

    /// Select changed lines by (origin, lineno) and apply.
    async fn run_sel(
        dir: &Path,
        area: DiffArea,
        action: HunkAction,
        file: &str,
        pick: &[(char, u32)],
    ) -> GitOutput {
        let repo = Repository::open(dir).unwrap();
        let src = read_patch_source(&repo, area, file).unwrap();
        let mut lines = Vec::new();
        for hunk in &src.hunks {
            for l in &hunk.lines {
                let n = if l.origin == '+' { l.new_lineno } else { l.old_lineno };
                if l.origin != ' ' && pick.contains(&(l.origin, n.unwrap_or(0))) {
                    lines.push(SelectedLine {
                        origin: l.origin.to_string(),
                        old_lineno: l.old_lineno,
                        new_lineno: l.new_lineno,
                        content: String::from_utf8_lossy(&l.content).into_owned(),
                    });
                }
            }
        }
        assert_eq!(lines.len(), pick.len(), "selection not found in diff");
        let sel = resolve_selection(&src, &lines, &[]).unwrap();
        let (cached, reverse) = apply_mode(area, action).unwrap();
        let patch = build_patch(&src, &sel, reverse).unwrap();
        let out = apply_patch(dir, &patch, cached, reverse).await.unwrap();
        assert!(out.success, "apply failed: {}\n{}", out.stderr, String::from_utf8_lossy(&patch));
        out
    }

    #[tokio::test]
    async fn stage_partial_lines_and_unstage() {
        let dir = temp_repo("partial").await;
        commit_file(&dir, "f.txt", b"1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n").await;
        std::fs::write(dir.join("f.txt"), b"1\nTWO\n3\n4\n5\n6\n7\n8\nNINE\n10\nadded\n").unwrap();

        // Stage only the replacement of line 2 (both its - and +).
        run_sel(&dir, DiffArea::Unstaged, HunkAction::Stage, "f.txt", &[('-', 2), ('+', 2)]).await;
        assert_eq!(
            index_content(&dir, "f.txt").await.unwrap(),
            b"1\nTWO\n3\n4\n5\n6\n7\n8\n9\n10\n"
        );
        // Stage only the addition of "NINE" without removing "9" (removals come first, so it lands after "9").
        run_sel(&dir, DiffArea::Unstaged, HunkAction::Stage, "f.txt", &[('+', 9)]).await;
        assert_eq!(
            index_content(&dir, "f.txt").await.unwrap(),
            b"1\nTWO\n3\n4\n5\n6\n7\n8\n9\nNINE\n10\n"
        );
        // Unstage the "TWO" line removal only: the old "2" comes back, TWO stays.
        run_sel(&dir, DiffArea::Staged, HunkAction::Unstage, "f.txt", &[('-', 2)]).await;
        assert_eq!(
            index_content(&dir, "f.txt").await.unwrap(),
            b"1\n2\nTWO\n3\n4\n5\n6\n7\n8\n9\nNINE\n10\n"
        );
        // Working tree untouched throughout.
        assert_eq!(
            std::fs::read(dir.join("f.txt")).unwrap(),
            b"1\nTWO\n3\n4\n5\n6\n7\n8\nNINE\n10\nadded\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn additions_only_and_deletions_only() {
        let dir = temp_repo("adddel").await;
        commit_file(&dir, "a.txt", b"a\nb\nc\n").await;
        std::fs::write(dir.join("a.txt"), b"a\nx\ny\nb\nc\n").unwrap();
        run_sel(&dir, DiffArea::Unstaged, HunkAction::Stage, "a.txt", &[('+', 3)]).await;
        assert_eq!(index_content(&dir, "a.txt").await.unwrap(), b"a\ny\nb\nc\n");

        commit_file(&dir, "d.txt", b"1\n2\n3\n4\n").await;
        std::fs::write(dir.join("d.txt"), b"1\n4\n").unwrap();
        run_sel(&dir, DiffArea::Unstaged, HunkAction::Stage, "d.txt", &[('-', 3)]).await;
        assert_eq!(index_content(&dir, "d.txt").await.unwrap(), b"1\n2\n4\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn discard_selected_lines_in_workdir() {
        let dir = temp_repo("discard").await;
        commit_file(&dir, "f.txt", b"a\nb\nc\nd\n").await;
        std::fs::write(dir.join("f.txt"), b"a\nB\nc\nD\nextra\n").unwrap();
        // Discard the "D" change (remove D, restore d) but keep B and extra.
        run_sel(&dir, DiffArea::Unstaged, HunkAction::Discard, "f.txt", &[('-', 4), ('+', 4)]).await;
        assert_eq!(std::fs::read(dir.join("f.txt")).unwrap(), b"a\nB\nc\nd\nextra\n");
        // Index unchanged.
        assert_eq!(index_content(&dir, "f.txt").await.unwrap(), b"a\nb\nc\nd\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn new_file_partial_and_full() {
        let dir = temp_repo("newfile").await;
        commit_file(&dir, "keep", b"k\n").await;
        std::fs::write(dir.join("n.txt"), b"one\ntwo\nthree\n").unwrap();
        run_sel(&dir, DiffArea::Unstaged, HunkAction::Stage, "n.txt", &[('+', 1), ('+', 3)]).await;
        assert_eq!(index_content(&dir, "n.txt").await.unwrap(), b"one\nthree\n");
        // Unstage everything that is staged: removes the index entry.
        run_sel(&dir, DiffArea::Staged, HunkAction::Unstage, "n.txt", &[('+', 1), ('+', 2)]).await;
        assert_eq!(index_content(&dir, "n.txt").await, None);
        assert!(dir.join("n.txt").exists());

        // Partially discard an untracked file, then discard the rest.
        run_sel(&dir, DiffArea::Unstaged, HunkAction::Discard, "n.txt", &[('+', 2)]).await;
        assert_eq!(std::fs::read(dir.join("n.txt")).unwrap(), b"one\nthree\n");
        run_sel(&dir, DiffArea::Unstaged, HunkAction::Discard, "n.txt", &[('+', 1), ('+', 2)]).await;
        assert!(!dir.join("n.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn deleted_file_partial_and_full() {
        let dir = temp_repo("deleted").await;
        commit_file(&dir, "g.txt", b"1\n2\n3\n").await;
        std::fs::remove_file(dir.join("g.txt")).unwrap();
        run_sel(&dir, DiffArea::Unstaged, HunkAction::Stage, "g.txt", &[('-', 2)]).await;
        assert_eq!(index_content(&dir, "g.txt").await.unwrap(), b"1\n3\n");
        run_sel(&dir, DiffArea::Unstaged, HunkAction::Stage, "g.txt", &[('-', 1), ('-', 2)]).await;
        assert_eq!(index_content(&dir, "g.txt").await, None);
        // Unstage part of the staged deletion: brings back only line 2.
        run_sel(&dir, DiffArea::Staged, HunkAction::Unstage, "g.txt", &[('-', 2)]).await;
        assert_eq!(index_content(&dir, "g.txt").await.unwrap(), b"2\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn no_newline_at_eof_cases() {
        let dir = temp_repo("eof").await;
        // Old last line lacks a newline; additions appended after it.
        commit_file(&dir, "e.txt", b"a\nb").await;
        std::fs::write(dir.join("e.txt"), b"a\nb\nc\nd").unwrap();
        // Diff: -b(noNL) +b +c +d(noNL). Stage only "+c".
        run_sel(&dir, DiffArea::Unstaged, HunkAction::Stage, "e.txt", &[('+', 3)]).await;
        assert_eq!(index_content(&dir, "e.txt").await.unwrap(), b"a\nb\nc\n");

        // Adding a final newline only.
        commit_file(&dir, "f.txt", b"x\ny").await;
        std::fs::write(dir.join("f.txt"), b"x\ny\n").unwrap();
        run_sel(&dir, DiffArea::Unstaged, HunkAction::Stage, "f.txt", &[('-', 2), ('+', 2)]).await;
        assert_eq!(index_content(&dir, "f.txt").await.unwrap(), b"x\ny\n");

        // Removing the final newline, then unstaging only the removal side.
        commit_file(&dir, "g.txt", b"p\nq\n").await;
        std::fs::write(dir.join("g.txt"), b"p\nr").unwrap();
        run_sel(&dir, DiffArea::Unstaged, HunkAction::Stage, "g.txt", &[('-', 2), ('+', 2)]).await;
        assert_eq!(index_content(&dir, "g.txt").await.unwrap(), b"p\nr");
        run_sel(&dir, DiffArea::Staged, HunkAction::Unstage, "g.txt", &[('-', 2)]).await;
        assert_eq!(index_content(&dir, "g.txt").await.unwrap(), b"p\nq\nr");

        // Discard just the removal of a no-newline last line.
        commit_file(&dir, "h.txt", b"1\n2").await;
        std::fs::write(dir.join("h.txt"), b"1\n3").unwrap();
        run_sel(&dir, DiffArea::Unstaged, HunkAction::Discard, "h.txt", &[('-', 2)]).await;
        assert_eq!(std::fs::read(dir.join("h.txt")).unwrap(), b"1\n2\n3");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn crlf_lines_preserved() {
        let dir = temp_repo("crlf").await;
        commit_file(&dir, "w.txt", b"a\r\nb\r\nc\r\n").await;
        std::fs::write(dir.join("w.txt"), b"a\r\nB\r\nc\r\nd\r\n").unwrap();
        run_sel(&dir, DiffArea::Unstaged, HunkAction::Stage, "w.txt", &[('+', 4)]).await;
        assert_eq!(index_content(&dir, "w.txt").await.unwrap(), b"a\r\nb\r\nc\r\nd\r\n");
        run_sel(&dir, DiffArea::Unstaged, HunkAction::Discard, "w.txt", &[('-', 2), ('+', 2)]).await;
        assert_eq!(std::fs::read(dir.join("w.txt")).unwrap(), b"a\r\nb\r\nc\r\nd\r\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn multiple_hunks_offsets_and_ranges() {
        let dir = temp_repo("multi").await;
        let old: Vec<u8> = (1..=30).map(|i| format!("{i}\n")).collect::<String>().into_bytes();
        commit_file(&dir, "m.txt", &old).await;
        let mut new = String::new();
        for i in 1..=30 {
            if i == 2 {
                new.push_str("new-a\nnew-b\n");
            }
            if i == 25 {
                new.push_str("TWENTY-FIVE\n");
                continue;
            }
            new.push_str(&format!("{i}\n"));
        }
        std::fs::write(dir.join("m.txt"), &new).unwrap();

        // Stage only the second hunk via its range (hunk-level staging).
        let repo = Repository::open(&dir).unwrap();
        let src = read_patch_source(&repo, DiffArea::Unstaged, "m.txt").unwrap();
        assert_eq!(src.hunks.len(), 2);
        let h = &src.hunks[1];
        let range = HunkRange {
            old_start: h.old_start,
            old_lines: h.lines.iter().filter(|l| l.origin != '+').count() as u32,
            new_start: h.new_start,
            new_lines: h.lines.iter().filter(|l| l.origin != '-').count() as u32,
        };
        let sel = resolve_selection(&src, &[], &[range]).unwrap();
        let patch = build_patch(&src, &sel, false).unwrap();
        let out = apply_patch(&dir, &patch, true, false).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        let expected: String = (1..=30)
            .map(|i| if i == 25 { "TWENTY-FIVE\n".to_string() } else { format!("{i}\n") })
            .collect();
        assert_eq!(index_content(&dir, "m.txt").await.unwrap(), expected.into_bytes());

        // Now discard the first hunk from the workdir (reverse, earlier hunk).
        run_sel(&dir, DiffArea::Unstaged, HunkAction::Discard, "m.txt", &[('+', 2), ('+', 3)]).await;
        let wd = String::from_utf8(std::fs::read(dir.join("m.txt")).unwrap()).unwrap();
        assert!(!wd.contains("new-a"));
        assert!(wd.contains("TWENTY-FIVE"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn diff_read_options_context_and_whitespace() {
        use crate::git::reader::{read_unstaged_diff, read_unstaged_diff_with, DiffReadOptions};
        let dir = temp_repo("readopts").await;
        commit_file(&dir, "o.txt", b"1\n2\n3\n4\n5\n6\n7\nfoo bar\n").await;
        std::fs::write(dir.join("o.txt"), b"1\n2\n3\nFOUR\n5\n6\n7\nfoo   bar\n").unwrap();
        let repo = Repository::open(&dir).unwrap();

        let default = read_unstaged_diff(&repo, Some("o.txt")).unwrap();
        assert_eq!(default[0].hunks.len(), 1); // 3 lines of context merge both changes

        let zero = DiffReadOptions { context_lines: Some(0), ignore_whitespace: None };
        let files = read_unstaged_diff_with(&repo, Some("o.txt"), &zero).unwrap();
        assert_eq!(files[0].hunks.len(), 2);
        assert!(files[0].hunks.iter().all(|h| h.lines.iter().all(|l| l.origin != " ")));

        let no_ws = DiffReadOptions { context_lines: Some(0), ignore_whitespace: Some(true) };
        let files = read_unstaged_diff_with(&repo, Some("o.txt"), &no_ws).unwrap();
        assert_eq!(files[0].hunks.len(), 1);

        // Staging the visible hunk by range still works from a -w view, and
        // the hidden whitespace change stays unstaged.
        let h = &files[0].hunks[0];
        let sel: Vec<SelectedLine> = h
            .lines
            .iter()
            .filter(|l| l.origin == "+" || l.origin == "-")
            .map(|l| SelectedLine {
                origin: l.origin.clone(),
                old_lineno: l.old_lineno,
                new_lineno: l.new_lineno,
                content: l.content.clone(),
            })
            .collect();
        let range = HunkRange {
            old_start: h.old_start,
            old_lines: h.old_lines,
            new_start: h.new_start,
            new_lines: h.new_lines,
        };
        let src = read_patch_source(&repo, DiffArea::Unstaged, "o.txt").unwrap();
        let selected = resolve_selection(&src, &sel, &[range]).unwrap();
        let patch = build_patch(&src, &selected, false).unwrap();
        let out = apply_patch(&dir, &patch, true, false).await.unwrap();
        assert!(out.success, "{}", out.stderr);
        assert_eq!(
            index_content(&dir, "o.txt").await.unwrap(),
            b"1\n2\n3\nFOUR\n5\n6\n7\nfoo bar\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn stale_selection_rejected_and_odd_paths() {
        let dir = temp_repo("stale").await;
        commit_file(&dir, "sp ace \"q\".txt", b"a\n").await;
        std::fs::write(dir.join("sp ace \"q\".txt"), b"a\nb\n").unwrap();
        let repo = Repository::open(&dir).unwrap();
        let src = read_patch_source(&repo, DiffArea::Unstaged, "sp ace \"q\".txt").unwrap();
        let bad = SelectedLine {
            origin: "+".into(),
            old_lineno: None,
            new_lineno: Some(2),
            content: "not b\n".into(),
        };
        assert!(resolve_selection(&src, &[bad], &[]).is_err());
        run_sel(&dir, DiffArea::Unstaged, HunkAction::Stage, "sp ace \"q\".txt", &[('+', 2)]).await;
        assert_eq!(index_content(&dir, "sp ace \"q\".txt").await.unwrap(), b"a\nb\n");
        assert!(apply_mode(DiffArea::Staged, HunkAction::Discard).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
