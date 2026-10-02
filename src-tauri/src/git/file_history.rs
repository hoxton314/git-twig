//! git2-based reads for per-file views: file history (following renames),
//! the diff of one file in one commit, line blame, and tracked-file listing.

use std::collections::HashMap;
use std::path::{Component, Path};

use git2::{BlameOptions, Commit, DiffOptions, ObjectType, Oid, Repository, Sort, Tree};
use serde::Serialize;

use crate::error::TwigError;
use crate::git::graph;
use crate::git::reader::{self, CommitInfo, DiffFile};

/// Only plain repo-relative paths are accepted (no `..`, no absolute paths).
pub(crate) fn validate_rel_path(path: &str) -> Result<(), TwigError> {
    let p = Path::new(path);
    if path.is_empty()
        || !p
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(TwigError::InvalidArgument(format!(
            "'{path}' is not a repository-relative path"
        )));
    }
    Ok(())
}

fn short(oid: Oid) -> String {
    let mut s = oid.to_string();
    s.truncate(7);
    s
}

pub(crate) fn commit_info(commit: &Commit) -> CommitInfo {
    graph::commit_info(commit)
}

/// Resolve any revision expression (`HEAD`, a branch, `abc123^`) to a commit.
fn resolve_rev<'r>(repo: &'r Repository, rev: &str) -> Result<Commit<'r>, TwigError> {
    if rev.is_empty() || rev.starts_with('-') {
        return Err(TwigError::InvalidArgument(format!("invalid revision '{rev}'")));
    }
    Ok(repo.revparse_single(rev)?.peel_to_commit()?)
}

/// Blob id of `path` in `tree`, or `None` if absent / not a file.
fn entry_id(tree: &Tree, path: &str) -> Option<Oid> {
    match tree.get_path(Path::new(path)) {
        Ok(e) if matches!(e.kind(), Some(ObjectType::Blob)) => Some(e.id()),
        _ => None,
    }
}

// ── File history ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct FileHistoryEntry {
    pub commit: CommitInfo,
    /// Path of the file in this commit.
    pub path: String,
    /// Previous path when this commit renamed the file.
    pub old_path: Option<String>,
    /// "added" | "modified" | "deleted" | "renamed"
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileHistoryPage {
    pub entries: Vec<FileHistoryEntry>,
    pub has_more: bool,
}

/// If `commit` added `path`, find the path it was renamed from (if any).
fn find_rename_source(
    repo: &Repository,
    parent_tree: &Tree,
    tree: &Tree,
    path: &str,
) -> Result<Option<String>, TwigError> {
    let mut opts = DiffOptions::new();
    let mut diff = repo.diff_tree_to_tree(Some(parent_tree), Some(tree), Some(&mut opts))?;
    reader::detect_renames(&mut diff)?;
    for delta in diff.deltas() {
        if delta.status() != git2::Delta::Renamed {
            continue;
        }
        let new = delta.new_file().path().map(|p| p.to_string_lossy().to_string());
        if new.as_deref() == Some(path) {
            return Ok(delta
                .old_file()
                .path()
                .map(|p| p.to_string_lossy().to_string()));
        }
    }
    Ok(None)
}

/// Commits (newest first) that changed `path`, following renames backwards.
///
/// Walks every commit reachable from `rev` in topological order, tracking
/// the file's path per commit (renames are propagated to parents). A commit
/// is listed when the file differs from its parent (for merges: from every
/// parent, matching git's default history simplification).
pub fn read_file_history(
    repo: &Repository,
    path: &str,
    rev: Option<&str>,
    skip: usize,
    limit: usize,
) -> Result<FileHistoryPage, TwigError> {
    validate_rel_path(path)?;
    let start = match rev {
        Some(r) => resolve_rev(repo, r)?,
        None => match repo.head() {
            Ok(h) => h.peel_to_commit()?,
            // Unborn branch: no history yet.
            Err(_) => {
                return Ok(FileHistoryPage {
                    entries: vec![],
                    has_more: false,
                })
            }
        },
    };

    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
    walk.push(start.id())?;

    let mut paths: HashMap<Oid, String> = HashMap::new();
    paths.insert(start.id(), path.to_string());

    let mut matched = 0usize;
    let mut entries = Vec::new();

    for oid in walk {
        let oid = oid?;
        let Some(cur_path) = paths.remove(&oid) else {
            continue;
        };
        let commit = repo.find_commit(oid)?;
        let tree = commit.tree()?;
        let cur = entry_id(&tree, &cur_path);

        let parents: Vec<Commit> = commit.parents().collect();
        let mut parent_path = cur_path.clone();
        let mut listed: Option<(String, Option<String>)> = None;

        if parents.is_empty() {
            if cur.is_some() {
                listed = Some(("added".to_string(), None));
            }
        } else {
            let parent_trees: Vec<Tree> =
                parents.iter().map(|p| p.tree()).collect::<Result<_, _>>()?;
            let parent_ids: Vec<Option<Oid>> =
                parent_trees.iter().map(|t| entry_id(t, &cur_path)).collect();
            let treesame = parent_ids.contains(&cur);
            if !treesame {
                let first = parent_ids[0];
                let status = match (first, cur) {
                    (None, Some(_)) => {
                        if parents.len() == 1 {
                            if let Some(old) =
                                find_rename_source(repo, &parent_trees[0], &tree, &cur_path)?
                            {
                                parent_path = old.clone();
                                listed = Some(("renamed".to_string(), Some(old)));
                            }
                        }
                        "added"
                    }
                    (Some(_), None) => "deleted",
                    _ => "modified",
                };
                if listed.is_none() {
                    listed = Some((status.to_string(), None));
                }
            }
        }

        for p in &parents {
            paths.entry(p.id()).or_insert_with(|| parent_path.clone());
        }

        if let Some((status, old_path)) = listed {
            if matched >= skip {
                if entries.len() >= limit {
                    return Ok(FileHistoryPage {
                        entries,
                        has_more: true,
                    });
                }
                entries.push(FileHistoryEntry {
                    commit: commit_info(&commit),
                    path: cur_path,
                    old_path,
                    status,
                });
            }
            matched += 1;
        }
    }

    Ok(FileHistoryPage {
        entries,
        has_more: false,
    })
}

/// Diff of a single file in one commit (against its first parent). When the
/// commit renamed the file, pass `old_path` so both sides are paired.
pub fn read_file_diff_at(
    repo: &Repository,
    oid: &str,
    path: &str,
    old_path: Option<&str>,
) -> Result<Vec<DiffFile>, TwigError> {
    validate_rel_path(path)?;
    if let Some(op) = old_path {
        validate_rel_path(op)?;
    }
    let commit = reader::resolve_commit(repo, oid)?;
    let tree = commit.tree()?;
    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };

    let mut opts = DiffOptions::new();
    opts.context_lines(3);
    opts.disable_pathspec_match(true);
    opts.pathspec(path);
    if let Some(op) = old_path {
        opts.pathspec(op);
    }
    let mut diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut opts))?;
    reader::detect_renames(&mut diff)?;
    let files = reader::parse_diff(&diff)?;
    Ok(files
        .into_iter()
        .filter(|f| {
            f.new_path.as_deref() == Some(path)
                || f.old_path.as_deref() == Some(path)
        })
        .collect())
}

// ── Blame ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct BlameHunk {
    pub oid: String,
    pub short_oid: String,
    pub author_name: String,
    pub author_email: String,
    pub timestamp: i64,
    pub summary: String,
    /// 1-based first line in the blamed file.
    pub start_line: usize,
    pub line_count: usize,
    /// Path of the file in the commit that introduced these lines.
    pub orig_path: String,
    pub orig_start_line: usize,
    /// True when blame stopped at a boundary (root commit / shallow edge).
    pub is_boundary: bool,
    /// Whether that commit has a parent (so "blame previous" is possible).
    pub has_parent: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlameResult {
    pub path: String,
    /// Full OID of the commit that was blamed.
    pub rev_oid: String,
    pub rev_short: String,
    pub lines: Vec<String>,
    pub hunks: Vec<BlameHunk>,
}

const MAX_BLAME_BYTES: usize = 8 * 1024 * 1024;

/// Blame `path` as of `rev` (default `HEAD`).
pub fn read_blame(repo: &Repository, path: &str, rev: Option<&str>) -> Result<BlameResult, TwigError> {
    validate_rel_path(path)?;
    let commit = resolve_rev(repo, rev.unwrap_or("HEAD"))?;
    let tree = commit.tree()?;
    let entry = tree.get_path(Path::new(path)).map_err(|_| {
        TwigError::InvalidArgument(format!(
            "'{path}' does not exist in commit {}",
            short(commit.id())
        ))
    })?;
    if entry.kind() != Some(ObjectType::Blob) {
        return Err(TwigError::InvalidArgument(format!("'{path}' is not a file")));
    }
    let blob = repo.find_blob(entry.id())?;
    if blob.is_binary() {
        return Err(TwigError::InvalidArgument(format!(
            "'{path}' is a binary file and cannot be blamed"
        )));
    }
    if blob.size() > MAX_BLAME_BYTES {
        return Err(TwigError::InvalidArgument(format!(
            "'{path}' is too large to blame"
        )));
    }
    let text = String::from_utf8_lossy(blob.content());
    let lines: Vec<String> = text
        .split_inclusive('\n')
        .map(|l| l.trim_end_matches('\n').trim_end_matches('\r').to_string())
        .collect();

    let mut opts = BlameOptions::new();
    opts.newest_commit(commit.id());
    let blame = repo.blame_file(Path::new(path), Some(&mut opts))?;

    let mut cache: HashMap<Oid, (String, i64, String, String, bool)> = HashMap::new();
    let mut hunks = Vec::with_capacity(blame.len());
    for h in blame.iter() {
        let oid = h.final_commit_id();
        let info = match cache.get(&oid) {
            Some(i) => i.clone(),
            None => {
                let i = match repo.find_commit(oid) {
                    Ok(c) => {
                        let a = c.author();
                        let name = graph::commit_author_name(&c);
                        (
                            if name.is_empty() { "Unknown".to_string() } else { name },
                            c.time().seconds(),
                            graph::commit_summary(&c),
                            graph::decode_text(a.email_bytes(), c.message_encoding()),
                            c.parent_count() > 0,
                        )
                    }
                    Err(_) => {
                        let sig = h.final_signature();
                        let name = graph::decode_text(sig.name_bytes(), None);
                        (
                            if name.is_empty() { "Unknown".to_string() } else { name },
                            sig.when().seconds(),
                            String::new(),
                            graph::decode_text(sig.email_bytes(), None),
                            false,
                        )
                    }
                };
                cache.insert(oid, i.clone());
                i
            }
        };
        hunks.push(BlameHunk {
            oid: oid.to_string(),
            short_oid: short(oid),
            author_name: info.0,
            author_email: info.3,
            timestamp: info.1,
            summary: info.2,
            start_line: h.final_start_line(),
            line_count: h.lines_in_hunk(),
            orig_path: h
                .path()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string()),
            orig_start_line: h.orig_start_line(),
            is_boundary: h.is_boundary(),
            has_parent: info.4,
        });
    }

    Ok(BlameResult {
        path: path.to_string(),
        rev_oid: commit.id().to_string(),
        rev_short: short(commit.id()),
        lines,
        hunks,
    })
}

// ── Tracked files ─────────────────────────────────────────────────────

/// All paths in the index (stage 0), sorted. Used by the file picker.
pub fn list_tracked_files(repo: &Repository) -> Result<Vec<String>, TwigError> {
    let mut index = repo.index()?;
    index.read(false)?;
    let mut out: Vec<String> = index
        .iter()
        .filter(|e| (e.flags >> 12) & 0x3 == 0)
        .map(|e| String::from_utf8_lossy(&e.path).to_string())
        .collect();
    out.sort();
    out.dedup();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(dir)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(ok, "git {args:?} failed");
    }

    fn temp_repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("twig-history-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        dir
    }

    #[test]
    fn history_follows_renames_and_pages() {
        let dir = temp_repo("follow");
        let body: String = (0..40).map(|i| format!("line {i}\n")).collect();
        std::fs::write(dir.join("a.txt"), &body).unwrap();
        std::fs::write(dir.join("other.txt"), "x\n").unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", "add a"]);
        std::fs::write(dir.join("a.txt"), format!("{body}more\n")).unwrap();
        git(&dir, &["commit", "-qam", "edit a"]);
        std::fs::write(dir.join("other.txt"), "y\n").unwrap();
        git(&dir, &["commit", "-qam", "edit other"]);
        git(&dir, &["mv", "a.txt", "b.txt"]);
        git(&dir, &["commit", "-qm", "rename"]);
        std::fs::write(dir.join("b.txt"), format!("{body}more\nagain\n")).unwrap();
        git(&dir, &["commit", "-qam", "edit b"]);

        let repo = Repository::open(&dir).unwrap();
        let page = read_file_history(&repo, "b.txt", None, 0, 100).unwrap();
        let summaries: Vec<&str> = page.entries.iter().map(|e| e.commit.summary.as_str()).collect();
        assert_eq!(summaries, vec!["edit b", "rename", "edit a", "add a"]);
        assert_eq!(page.entries[1].status, "renamed");
        assert_eq!(page.entries[1].old_path.as_deref(), Some("a.txt"));
        assert_eq!(page.entries[2].path, "a.txt");
        assert_eq!(page.entries[3].status, "added");
        assert!(!page.has_more);

        let p1 = read_file_history(&repo, "b.txt", None, 0, 2).unwrap();
        assert_eq!(p1.entries.len(), 2);
        assert!(p1.has_more);
        let p2 = read_file_history(&repo, "b.txt", None, 2, 2).unwrap();
        assert_eq!(p2.entries[0].commit.summary, "edit a");

        let rename_oid = page.entries[1].commit.oid.clone();
        let d = read_file_diff_at(&repo, &rename_oid, "b.txt", Some("a.txt")).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].status, "renamed");

        assert!(read_file_history(&repo, "../x", None, 0, 10).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn blame_attributes_lines_and_lists_files() {
        let dir = temp_repo("blame");
        std::fs::write(dir.join("f.txt"), "one\ntwo\n").unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", "first"]);
        std::fs::write(dir.join("f.txt"), "one\nTWO\nthree\n").unwrap();
        git(&dir, &["commit", "-qam", "second"]);

        let repo = Repository::open(&dir).unwrap();
        let b = read_blame(&repo, "f.txt", None).unwrap();
        assert_eq!(b.lines, vec!["one", "TWO", "three"]);
        let total: usize = b.hunks.iter().map(|h| h.line_count).sum();
        assert_eq!(total, 3);
        assert_eq!(b.hunks[0].summary, "first");
        assert_eq!(b.hunks[1].summary, "second");
        assert!(b.hunks[1].has_parent);

        let prev = read_blame(&repo, "f.txt", Some(&format!("{}^", b.hunks[1].oid))).unwrap();
        assert_eq!(prev.lines, vec!["one", "two"]);
        assert!(read_blame(&repo, "missing.txt", None).is_err());
        assert!(read_blame(&repo, "f.txt", Some("--evil")).is_err());

        assert_eq!(list_tracked_files(&repo).unwrap(), vec!["f.txt".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
