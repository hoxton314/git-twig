//! git2-based read operations — commit graph, branches, diffs, file contents.

use std::collections::HashMap;

use std::path::{Component, Path};

use git2::{
    BranchType, Commit, Delta, Diff, DiffFindOptions, DiffOptions, ObjectType, Oid, Repository,
    Sort,
};
use serde::Serialize;

use crate::error::TwigError;

/// First 7 hex chars of an OID (OID strings are ASCII, so slicing is safe).
fn short_oid(oid: Oid) -> String {
    let mut s = oid.to_string();
    s.truncate(7);
    s
}

/// Resolve a commit from a full or abbreviated hex OID.
///
/// `Oid::from_str` zero-pads short input instead of expanding it, so an
/// abbreviated hash would look up a non-existent object.
pub(crate) fn resolve_commit<'r>(repo: &'r Repository, oid_str: &str) -> Result<Commit<'r>, TwigError> {
    if !oid_str.is_empty() && oid_str.len() <= 64 && oid_str.bytes().all(|b| b.is_ascii_hexdigit())
    {
        if oid_str.len() >= 40 {
            return Ok(repo.find_commit(Oid::from_str(oid_str)?)?);
        }
        let obj = repo.find_object_by_prefix(oid_str, None)?;
        return Ok(obj.peel_to_commit()?);
    }
    Err(TwigError::InvalidArgument(format!(
        "'{oid_str}' is not a commit hash"
    )))
}

// ── Commit graph ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct CommitInfo {
    pub oid: String,
    pub short_oid: String,
    pub summary: String,
    pub body: String,
    pub author_name: String,
    pub author_email: String,
    pub author_gravatar: String,
    pub timestamp: i64,
    pub parent_oids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphEntry {
    pub commit: CommitInfo,
    /// Lane this commit sits on.
    pub lane: usize,
    /// True if a child reserved this lane (line enters from above).
    pub has_incoming: bool,
    /// Lanes with active pass-through lines (straight vertical, full row height).
    /// Does NOT include the commit's own lane.
    pub rails: Vec<usize>,
    /// Lane index for each parent. First parent usually continues on `lane`;
    /// merge parents branch to different lanes. Used to draw lines from the
    /// commit node downward.
    pub parent_lanes: Vec<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RefLabel {
    pub name: String,
    pub ref_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommitGraph {
    pub entries: Vec<GraphEntry>,
    pub total_lanes: usize,
    pub refs: HashMap<String, Vec<RefLabel>>,
    pub unpushed_oids: Vec<String>,
}

/// Read the full commit graph for the repo, computing lane assignments.
/// The commits are returned in topological order (newest first).
pub fn read_commit_graph(repo: &Repository, max_commits: usize) -> Result<CommitGraph, TwigError> {
    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;

    // Push all references so we walk all branches
    revwalk.push_glob("refs/heads/*")?;
    revwalk.push_glob("refs/remotes/*")?;
    // Also push HEAD in case of detached HEAD
    if let Ok(head) = repo.head() {
        if let Some(target) = head.target() {
            revwalk.push(target)?;
        }
    }

    let mut commits: Vec<CommitInfo> = Vec::new();

    for oid_result in revwalk {
        if commits.len() >= max_commits {
            break;
        }
        let oid = oid_result?;
        let commit = repo.find_commit(oid)?;

        let author = commit.author();
        let email = author.email().unwrap_or("").to_string();
        let gravatar_hash = format!("{:x}", md5::compute(email.trim().to_lowercase().as_bytes()));

        let parent_oids = commit
            .parent_ids()
            .map(|id| id.to_string())
            .collect::<Vec<_>>();

        commits.push(CommitInfo {
            oid: oid.to_string(),
            short_oid: short_oid(oid),
            summary: commit.summary().unwrap_or("").to_string(),
            body: commit.body().unwrap_or("").to_string(),
            author_name: author.name().unwrap_or("Unknown").to_string(),
            author_email: email,
            author_gravatar: gravatar_hash,
            timestamp: commit.time().seconds(),
            parent_oids,
        });
    }

    // Lane assignment algorithm
    let mut graph = compute_lanes(commits);
    graph.refs = build_refs_map(repo)?;
    graph.unpushed_oids = compute_unpushed_oids(repo);
    Ok(graph)
}

/// Lane assignment algorithm.
///
/// Walks commits in topological order, assigning each to a lane (column).
/// For every row it records:
///   - which lane the commit sits on
///   - whether there was a line entering from above (child reserved the lane)
///   - which other lanes carry pass-through lines (rails)
///   - which lanes the parents are assigned to (for drawing outgoing lines)
fn compute_lanes(commits: Vec<CommitInfo>) -> CommitGraph {
    // Each slot is Some(oid) when a child has reserved that lane for a future
    // commit, or None when the lane is free.
    let mut lanes: Vec<Option<String>> = Vec::new();
    let mut entries: Vec<GraphEntry> = Vec::with_capacity(commits.len());
    let mut max_lanes: usize = 0;

    for commit in commits {
        // ── 1. Find or allocate lane for this commit ──────────────────
        let reserved = lanes
            .iter()
            .position(|slot| slot.as_deref() == Some(&commit.oid));

        let (my_lane, has_incoming) = match reserved {
            Some(lane) => (lane, true),
            None => {
                let lane = lanes
                    .iter()
                    .position(|s| s.is_none())
                    .unwrap_or_else(|| {
                        lanes.push(None);
                        lanes.len() - 1
                    });
                (lane, false)
            }
        };

        // ── 2. Consume the lane ───────────────────────────────────────
        lanes[my_lane] = None;

        // ── 3. Snapshot pass-through rails ────────────────────────────
        // These are lanes still occupied by other branches — they draw
        // straight vertical lines through this row.
        let rails: Vec<usize> = lanes
            .iter()
            .enumerate()
            .filter_map(|(i, s)| if s.is_some() { Some(i) } else { None })
            .collect();

        // ── 4. Assign parents to lanes ────────────────────────────────
        let mut parent_lanes: Vec<usize> = Vec::with_capacity(commit.parent_oids.len());

        for (pi, parent_oid) in commit.parent_oids.iter().enumerate() {
            // Parent may already have a lane (reserved by another child's merge)
            if let Some(existing) = lanes.iter().position(|s| s.as_deref() == Some(parent_oid)) {
                parent_lanes.push(existing);
            } else if pi == 0 {
                // First parent inherits our lane (straight continuation)
                lanes[my_lane] = Some(parent_oid.clone());
                parent_lanes.push(my_lane);
            } else {
                // Merge parent — allocate a lane
                let lane = lanes
                    .iter()
                    .position(|s| s.is_none())
                    .unwrap_or_else(|| {
                        lanes.push(None);
                        lanes.len() - 1
                    });
                lanes[lane] = Some(parent_oid.clone());
                parent_lanes.push(lane);
            }
        }

        if lanes.len() > max_lanes {
            max_lanes = lanes.len();
        }

        // Trim trailing empty lanes to keep the graph compact
        while lanes.last() == Some(&None) {
            lanes.pop();
        }

        entries.push(GraphEntry {
            commit,
            lane: my_lane,
            has_incoming,
            rails,
            parent_lanes,
        });
    }

    CommitGraph {
        entries,
        total_lanes: max_lanes,
        refs: HashMap::new(),
        unpushed_oids: vec![],
    }
}

/// Build a map from commit OID -> list of branch/tag labels pointing at it.
fn build_refs_map(repo: &Repository) -> Result<HashMap<String, Vec<RefLabel>>, TwigError> {
    let mut map: HashMap<String, Vec<RefLabel>> = HashMap::new();

    for reference_result in repo.references()? {
        let reference = reference_result?;

        if reference.name() == Some("HEAD") {
            continue;
        }

        let ref_type = if reference.is_tag() {
            "tag"
        } else if reference.is_remote() {
            "remote"
        } else if reference.is_branch() {
            "local"
        } else {
            continue;
        };

        let target_oid = match reference.peel(git2::ObjectType::Commit) {
            Ok(obj) => obj.id().to_string(),
            Err(_) => continue,
        };

        let name = reference.shorthand().unwrap_or("").to_string();

        map.entry(target_oid)
            .or_default()
            .push(RefLabel {
                name,
                ref_type: ref_type.to_string(),
            });
    }

    Ok(map)
}

/// Compute the set of commit OIDs on HEAD that are not yet on its upstream.
fn compute_unpushed_oids(repo: &Repository) -> Vec<String> {
    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return vec![],
    };

    let head_oid = match head.target() {
        Some(oid) => oid,
        None => return vec![],
    };

    // Detached HEAD has no upstream.
    if !head.is_branch() {
        return vec![];
    }
    let branch = git2::Branch::wrap(head);

    let upstream = match branch.upstream() {
        Ok(u) => u,
        Err(_) => return vec![],
    };

    let upstream_oid = match upstream.get().target() {
        Some(oid) => oid,
        None => return vec![],
    };

    let mut revwalk = match repo.revwalk() {
        Ok(rw) => rw,
        Err(_) => return vec![],
    };

    if revwalk.push(head_oid).is_err() || revwalk.hide(upstream_oid).is_err() {
        return vec![];
    }

    revwalk
        .filter_map(|oid| oid.ok().map(|o| o.to_string()))
        .collect()
}

// ── Branches ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_remote: bool,
    pub is_head: bool,
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub oid: String,
    pub short_oid: String,
    pub last_commit_summary: String,
    pub last_commit_timestamp: i64,
}

pub fn read_branches(repo: &Repository) -> Result<Vec<BranchInfo>, TwigError> {
    let mut branches = Vec::new();

    for branch_type in &[BranchType::Local, BranchType::Remote] {
        for branch_result in repo.branches(Some(*branch_type))? {
            let (branch, btype) = branch_result?;
            // Skip (rather than fail the whole list on) non-UTF-8 names.
            let Ok(Some(name)) = branch.name().map(|n| n.map(String::from)) else {
                continue;
            };
            let is_remote = btype == BranchType::Remote;
            let is_head = branch.is_head();

            // Symbolic refs (e.g. `origin/HEAD`) have no direct target.
            let Some(oid) = branch.get().target() else {
                continue;
            };
            // A ref pointing at a non-commit (or a missing object) should not
            // break the whole branch list.
            let Ok(commit) = repo.find_commit(oid) else {
                continue;
            };

            let (upstream, ahead, behind) = if is_remote {
                (None, 0, 0)
            } else {
                match branch.upstream() {
                    Ok(upstream_branch) => {
                        let upstream_name =
                            upstream_branch.name().ok().flatten().map(String::from);
                        let (a, b) = upstream_branch
                            .get()
                            .target()
                            .and_then(|uoid| repo.graph_ahead_behind(oid, uoid).ok())
                            .unwrap_or((0, 0));
                        (upstream_name, a, b)
                    }
                    Err(_) => (None, 0, 0),
                }
            };

            branches.push(BranchInfo {
                name,
                is_remote,
                is_head,
                upstream,
                ahead,
                behind,
                oid: oid.to_string(),
                short_oid: short_oid(oid),
                last_commit_summary: commit.summary().unwrap_or("").to_string(),
                last_commit_timestamp: commit.time().seconds(),
            });
        }
    }

    // Sort: HEAD first, then local alphabetically, then remote alphabetically
    branches.sort_by(|a, b| {
        b.is_head
            .cmp(&a.is_head)
            .then(a.is_remote.cmp(&b.is_remote))
            .then(a.name.cmp(&b.name))
    });

    Ok(branches)
}

// ── Working directory status ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct FileStatus {
    pub path: String,
    pub status: String,
    pub is_new: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkingStatus {
    pub staged: Vec<FileStatus>,
    pub unstaged: Vec<FileStatus>,
}

/// Read the working directory status, split into staged and unstaged files.
pub fn read_working_status(repo: &Repository) -> Result<WorkingStatus, TwigError> {
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();

    // Only staged renames are detected (like `git status`). Workdir rename
    // detection would pair an untracked file with a deleted tracked one into a
    // single entry, which the stage/discard actions cannot handle: the new
    // path is untracked (so `git restore` fails) and the deletion is hidden.
    let statuses = repo.statuses(Some(
        git2::StatusOptions::new()
            .include_untracked(true)
            .renames_head_to_index(true),
    ))?;

    for entry in statuses.iter() {
        // For renames `entry.path()` is the *old* path; report the new one,
        // which is what exists in the index/workdir and what stage/unstage/
        // diff operations need to act on.
        let path = entry
            .head_to_index()
            .and_then(|d| d.new_file().path().map(|p| p.to_string_lossy().into_owned()))
            .or_else(|| entry.path().map(String::from))
            .unwrap_or_default();
        let s = entry.status();

        // Staged (index) changes
        if s.intersects(
            git2::Status::INDEX_NEW
                | git2::Status::INDEX_MODIFIED
                | git2::Status::INDEX_DELETED
                | git2::Status::INDEX_RENAMED
                | git2::Status::INDEX_TYPECHANGE,
        ) {
            let status = if s.contains(git2::Status::INDEX_NEW) {
                "added"
            } else if s.contains(git2::Status::INDEX_DELETED) {
                "deleted"
            } else if s.contains(git2::Status::INDEX_RENAMED) {
                "renamed"
            } else {
                "modified"
            };
            staged.push(FileStatus {
                path: path.clone(),
                status: status.to_string(),
                is_new: s.contains(git2::Status::INDEX_NEW),
            });
        }

        // Conflicted files (unresolved merge conflicts)
        if s.contains(git2::Status::CONFLICTED) {
            unstaged.push(FileStatus {
                path,
                status: "conflicted".to_string(),
                is_new: false,
            });
            continue;
        }

        // Unstaged (workdir) changes
        if s.intersects(
            git2::Status::WT_NEW
                | git2::Status::WT_MODIFIED
                | git2::Status::WT_DELETED
                | git2::Status::WT_RENAMED
                | git2::Status::WT_TYPECHANGE,
        ) {
            let status = if s.contains(git2::Status::WT_NEW) {
                "untracked"
            } else if s.contains(git2::Status::WT_DELETED) {
                "deleted"
            } else if s.contains(git2::Status::WT_RENAMED) {
                "renamed"
            } else {
                "modified"
            };
            unstaged.push(FileStatus {
                path,
                status: status.to_string(),
                is_new: s.contains(git2::Status::WT_NEW),
            });
        }
    }

    Ok(WorkingStatus { staged, unstaged })
}

/// Get the staged diff (index vs HEAD) for a single file or all files.
pub fn read_staged_diff(
    repo: &Repository,
    file_path: Option<&str>,
) -> Result<Vec<DiffFile>, TwigError> {
    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());

    let mut opts = DiffOptions::new();
    opts.context_lines(3);
    if let Some(p) = file_path {
        literal_pathspec(&mut opts, p);
    }

    let mut diff = repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))?;
    if file_path.is_none() {
        detect_renames(&mut diff)?;
    }
    parse_diff(&diff)
}

/// Get the unstaged diff (workdir vs index) for a single file or all files.
pub fn read_unstaged_diff(
    repo: &Repository,
    file_path: Option<&str>,
) -> Result<Vec<DiffFile>, TwigError> {
    let mut opts = DiffOptions::new();
    opts.context_lines(3);
    opts.include_untracked(true);
    opts.recurse_untracked_dirs(true);
    opts.show_untracked_content(true);
    if let Some(p) = file_path {
        literal_pathspec(&mut opts, p);
    }

    let diff = repo.diff_index_to_workdir(None, Some(&mut opts))?;
    parse_diff(&diff)
}

/// Restrict a diff to exactly one path. Pathspecs are globs by default, so a
/// file literally named e.g. `[ab].txt` or `*.rs` would otherwise match other
/// files. Directory paths (untracked dirs are reported as `dir/`) still match
/// everything beneath them.
fn literal_pathspec(opts: &mut DiffOptions, path: &str) {
    opts.pathspec(path.trim_end_matches('/'));
    opts.disable_pathspec_match(true);
}

/// Pair up deleted/added files into renames (like `git diff -M`).
pub(crate) fn detect_renames(diff: &mut Diff) -> Result<(), TwigError> {
    let mut find = DiffFindOptions::new();
    find.renames(true);
    diff.find_similar(Some(&mut find))?;
    Ok(())
}

// ── Diffs ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct DiffFile {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub status: String,
    pub is_binary: bool,
    pub is_lfs: bool,
    pub lfs_size: Option<String>,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffHunk {
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffLine {
    pub origin: String,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub content: String,
}

fn delta_status_str(delta: Delta) -> &'static str {
    match delta {
        Delta::Added | Delta::Untracked => "added",
        Delta::Deleted => "deleted",
        Delta::Modified => "modified",
        Delta::Renamed => "renamed",
        Delta::Copied => "copied",
        Delta::Typechange => "typechange",
        _ => "unknown",
    }
}

/// Check if diff content is an actual LFS pointer file and extract the size.
///
/// Real LFS pointers are tiny (~130 bytes) with exactly this structure:
///   version https://git-lfs.github.com/spec/v1
///   oid sha256:<hash>
///   size <bytes>
///
/// We require all three markers AND a small total size to avoid false
/// positives on files that merely mention the LFS spec URL.
const LFS_POINTER_MAX: usize = 512;

fn check_lfs_pointer(content: &str) -> Option<String> {
    // Real LFS pointers are under 256 bytes; anything larger is just a
    // file that happens to reference the spec URL.
    if content.is_empty() || content.len() > LFS_POINTER_MAX {
        return None;
    }

    let has_version = content
        .lines()
        .any(|l| l.trim().starts_with("version https://git-lfs.github.com/spec/"));
    let has_oid = content.lines().any(|l| l.trim().starts_with("oid sha256:"));

    if !has_version || !has_oid {
        return None;
    }

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(size_str) = trimmed.strip_prefix("size ") {
            let bytes: u64 = size_str.trim().parse().unwrap_or(0);
            return Some(format_bytes(bytes));
        }
    }

    Some("unknown size".to_string())
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Get the diff for a specific commit (compared to its first parent, or to empty tree for root commits).
pub fn read_commit_diff(repo: &Repository, oid_str: &str) -> Result<Vec<DiffFile>, TwigError> {
    let commit = resolve_commit(repo, oid_str)?;
    let tree = commit.tree()?;

    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };

    let mut opts = DiffOptions::new();
    opts.context_lines(3);

    let mut diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut opts))?;
    detect_renames(&mut diff)?;

    parse_diff(&diff)
}

/// Get the combined diff of the working directory against HEAD (staged + unstaged).
pub fn read_working_diff(repo: &Repository) -> Result<Vec<DiffFile>, TwigError> {
    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());

    let mut opts = DiffOptions::new();
    opts.context_lines(3);

    let mut staged = repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))?;
    detect_renames(&mut staged)?;
    let mut unstaged_opts = DiffOptions::new();
    unstaged_opts.context_lines(3);
    unstaged_opts.include_untracked(true);
    unstaged_opts.recurse_untracked_dirs(true);
    unstaged_opts.show_untracked_content(true);
    let unstaged = repo.diff_index_to_workdir(None, Some(&mut unstaged_opts))?;

    let mut files = parse_diff(&staged)?;
    files.extend(parse_diff(&unstaged)?);

    Ok(files)
}

/// Read raw file content from a given source.
///
/// `source` is one of:
///   - `"workdir"` — read from the working directory
///   - `"index"`   — read from the staging area (index)
///   - `"head"`    — read from the HEAD commit tree
///   - any other string is treated as a commit OID
///
/// Returns `None` if the file doesn't exist in that source.
pub fn read_file_blob(
    repo: &Repository,
    file_path: &str,
    source: &str,
) -> Result<Option<Vec<u8>>, TwigError> {
    let rel_path = Path::new(file_path);
    // Only plain repo-relative paths are valid. `workdir.join` would happily
    // follow `..` or replace the base entirely with an absolute path, letting
    // the IPC caller read arbitrary files.
    if file_path.is_empty()
        || !rel_path
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(TwigError::InvalidArgument(format!(
            "'{file_path}' is not a repository-relative path"
        )));
    }

    match source {
        "workdir" => {
            let workdir = repo
                .workdir()
                .ok_or_else(|| TwigError::Git(git2::Error::from_str("bare repository")))?;
            let full_path = workdir.join(rel_path);
            match std::fs::read(&full_path) {
                Ok(data) => Ok(Some(data)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(TwigError::Io(e)),
            }
        }
        "index" => {
            let mut index = repo.index()?;
            // Pick up changes made by CLI writes since the index was loaded.
            index.read(false)?;
            // Stage 0 is the normal (non-conflict) entry; conflicted paths
            // only have stages 1-3 and have no single "index" version.
            match index.get_path(rel_path, 0) {
                Some(e) => {
                    let blob = repo.find_blob(e.id)?;
                    Ok(Some(blob.content().to_vec()))
                }
                None => Ok(None),
            }
        }
        "head" => {
            let head = match repo.head() {
                Ok(h) => h,
                Err(_) => return Ok(None), // no HEAD (empty repo)
            };
            tree_blob(repo, &head.peel_to_tree()?, rel_path)
        }
        oid_str => {
            let commit = resolve_commit(repo, oid_str)?;
            tree_blob(repo, &commit.tree()?, rel_path)
        }
    }
}

/// Read a blob at `path` inside `tree`. Returns `None` if the path is missing
/// or is not a regular file (a directory or submodule entry).
fn tree_blob(
    repo: &Repository,
    tree: &git2::Tree,
    path: &Path,
) -> Result<Option<Vec<u8>>, TwigError> {
    match tree.get_path(path) {
        Ok(entry) if entry.kind() == Some(ObjectType::Blob) => {
            let blob = repo.find_blob(entry.id())?;
            Ok(Some(blob.content().to_vec()))
        }
        _ => Ok(None),
    }
}

pub(crate) fn parse_diff(diff: &Diff) -> Result<Vec<DiffFile>, TwigError> {
    let mut files: Vec<DiffFile> = Vec::new();

    let num_deltas = diff.deltas().len();

    for delta_idx in 0..num_deltas {
        let Some(delta) = diff.get_delta(delta_idx) else {
            continue;
        };
        let old_path = delta.old_file().path().map(|p| p.to_string_lossy().to_string());
        let new_path = delta.new_file().path().map(|p| p.to_string_lossy().to_string());
        let status = delta_status_str(delta.status()).to_string();

        let mut hunks: Vec<DiffHunk> = Vec::new();
        let mut is_lfs = false;
        let mut lfs_size: Option<String> = None;
        // Old/new side text, capped — only needed to sniff small LFS pointers.
        let mut old_side = String::new();
        let mut new_side = String::new();

        // Generating the patch loads file contents, which is when libgit2
        // decides whether a file is binary — so the binary flags must be read
        // *after* this, not from the delta beforehand.
        let patch = git2::Patch::from_diff(diff, delta_idx)?;
        let is_binary = match &patch {
            Some(p) => p.delta().old_file().is_binary() || p.delta().new_file().is_binary(),
            None => delta.old_file().is_binary() || delta.new_file().is_binary(),
        };
        if let Some(patch) = patch {
            let num_hunks = patch.num_hunks();
            for hunk_idx in 0..num_hunks {
                let (hunk, num_lines) = patch.hunk(hunk_idx)?;
                let header = String::from_utf8_lossy(hunk.header()).into_owned();
                let mut lines: Vec<DiffLine> = Vec::new();

                for line_idx in 0..num_lines {
                    let line = patch.line_in_hunk(hunk_idx, line_idx)?;
                    // Lossy rather than dropping non-UTF-8 (e.g. Latin-1) lines.
                    let content = String::from_utf8_lossy(line.content()).into_owned();
                    let origin_char = line.origin();
                    if origin_char != '+' && old_side.len() <= LFS_POINTER_MAX {
                        old_side.push_str(&content);
                    }
                    if origin_char != '-' && new_side.len() <= LFS_POINTER_MAX {
                        new_side.push_str(&content);
                    }
                    let origin = origin_char.to_string();

                    lines.push(DiffLine {
                        origin,
                        old_lineno: line.old_lineno(),
                        new_lineno: line.new_lineno(),
                        content,
                    });
                }

                hunks.push(DiffHunk {
                    header,
                    old_start: hunk.old_start(),
                    old_lines: hunk.old_lines(),
                    new_start: hunk.new_start(),
                    new_lines: hunk.new_lines(),
                    lines,
                });
            }
        }

        // Check for LFS; prefer the new side so a modified pointer reports the
        // new object's size, falling back to the old side for deletions.
        if let Some(size) = check_lfs_pointer(&new_side).or_else(|| check_lfs_pointer(&old_side)) {
            is_lfs = true;
            lfs_size = Some(size);
        }

        files.push(DiffFile {
            old_path,
            new_path,
            status,
            is_binary,
            is_lfs,
            lfs_size,
            hunks,
        });
    }

    Ok(files)
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
        let dir = std::env::temp_dir().join(format!("twig-reader-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        dir
    }

    #[test]
    fn literal_pathspec_and_untracked_dir() {
        let dir = temp_repo("pathspec");
        std::fs::write(dir.join("[ab].txt"), "x\n").unwrap();
        std::fs::write(dir.join("a.txt"), "y\n").unwrap();
        std::fs::create_dir(dir.join("newdir")).unwrap();
        std::fs::write(dir.join("newdir/f.txt"), "z\n").unwrap();
        let repo = Repository::open(&dir).unwrap();

        let files = read_unstaged_diff(&repo, Some("[ab].txt")).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].new_path.as_deref(), Some("[ab].txt"));

        let files = read_unstaged_diff(&repo, Some("newdir/")).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].new_path.as_deref(), Some("newdir/f.txt"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn staged_rename_reports_new_path_and_binary_flag() {
        let dir = temp_repo("rename");
        let body = "line\n".repeat(50);
        std::fs::write(dir.join("old.txt"), &body).unwrap();
        std::fs::write(dir.join("bin.dat"), [0u8, 1, 2, 0, 3]).unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", "init"]);
        git(&dir, &["mv", "old.txt", "new.txt"]);
        std::fs::write(dir.join("bin.dat"), [0u8, 9, 9, 0, 3]).unwrap();
        let repo = Repository::open(&dir).unwrap();

        let status = read_working_status(&repo).unwrap();
        assert_eq!(status.staged.len(), 1);
        assert_eq!(status.staged[0].path, "new.txt");
        assert_eq!(status.staged[0].status, "renamed");

        let unstaged = read_unstaged_diff(&repo, Some("bin.dat")).unwrap();
        assert_eq!(unstaged.len(), 1);
        assert!(unstaged[0].is_binary);

        let staged = read_staged_diff(&repo, None).unwrap();
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].status, "renamed");

        // Index blob must reflect CLI writes made after the handle was opened.
        let _ = repo.index().unwrap();
        std::fs::write(dir.join("later.txt"), "later\n").unwrap();
        git(&dir, &["add", "later.txt"]);
        let blob = read_file_blob(&repo, "later.txt", "index").unwrap();
        assert_eq!(blob.as_deref(), Some(&b"later\n"[..]));

        assert!(read_file_blob(&repo, "../etc/passwd", "workdir").is_err());
        assert!(read_file_blob(&repo, "/etc/passwd", "workdir").is_err());

        let head = repo.head().unwrap().target().unwrap().to_string();
        assert!(read_commit_diff(&repo, &head[..10]).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lanes_for_merge() {
        let mk = |oid: &str, parents: &[&str]| CommitInfo {
            oid: oid.into(),
            short_oid: oid.into(),
            summary: String::new(),
            body: String::new(),
            author_name: String::new(),
            author_email: String::new(),
            author_gravatar: String::new(),
            timestamp: 0,
            parent_oids: parents.iter().map(|p| p.to_string()).collect(),
        };
        // m merges b into a; both fork from r.
        let g = compute_lanes(vec![
            mk("m", &["a", "b"]),
            mk("b", &["r"]),
            mk("a", &["r"]),
            mk("r", &[]),
        ]);
        let lanes: Vec<usize> = g.entries.iter().map(|e| e.lane).collect();
        // `r` was first reserved by `b` (lane 1), so `a` joins that lane.
        assert_eq!(lanes, vec![0, 1, 0, 1]);
        assert!(g.entries.iter().skip(1).all(|e| e.has_incoming));
        assert_eq!(g.entries[1].parent_lanes, vec![1]);
        assert_eq!(g.entries[2].parent_lanes, vec![1]);
        assert_eq!(g.total_lanes, 2);
    }
}
