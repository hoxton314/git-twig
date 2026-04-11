//! git2-based read operations — commit graph, branches, diffs, file contents.

use git2::{BranchType, Delta, Diff, DiffOptions, Oid, Repository, Sort};
use serde::Serialize;

use crate::error::TwigError;

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
pub struct CommitGraph {
    pub entries: Vec<GraphEntry>,
    pub total_lanes: usize,
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
            short_oid: oid.to_string()[..7.min(oid.to_string().len())].to_string(),
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
    let graph = compute_lanes(commits);
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

    for commit in &commits {
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
            commit: commit.clone(),
            lane: my_lane,
            has_incoming,
            rails,
            parent_lanes,
        });
    }

    CommitGraph {
        entries,
        total_lanes: max_lanes,
    }
}

// ── Branches ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_remote: bool,
    pub is_head: bool,
    pub upstream: Option<String>,
    pub oid: String,
    pub short_oid: String,
    pub last_commit_summary: String,
    pub last_commit_timestamp: i64,
}

pub fn read_branches(repo: &Repository) -> Result<Vec<BranchInfo>, TwigError> {
    let mut branches = Vec::new();

    let head_oid = repo.head().ok().and_then(|h| h.target());

    for branch_type in &[BranchType::Local, BranchType::Remote] {
        for branch_result in repo.branches(Some(*branch_type))? {
            let (branch, btype) = branch_result?;
            let name = branch.name()?.unwrap_or("").to_string();
            let is_remote = btype == BranchType::Remote;

            let reference = branch.into_reference();
            let oid = match reference.target() {
                Some(o) => o,
                None => continue,
            };

            let commit = repo.find_commit(oid)?;
            let is_head = head_oid == Some(oid);

            let upstream = if !is_remote {
                repo.find_branch(&name, BranchType::Local)
                    .ok()
                    .and_then(|b| b.upstream().ok())
                    .and_then(|u| u.name().ok().flatten().map(String::from))
            } else {
                None
            };

            branches.push(BranchInfo {
                name,
                is_remote,
                is_head,
                upstream,
                oid: oid.to_string(),
                short_oid: oid.to_string()[..7.min(oid.to_string().len())].to_string(),
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

    let statuses = repo.statuses(Some(
        git2::StatusOptions::new()
            .include_untracked(true)
            .renames_head_to_index(true)
            .renames_index_to_workdir(true),
    ))?;

    for entry in statuses.iter() {
        let path = entry.path().unwrap_or("").to_string();
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
        opts.pathspec(p);
    }

    let diff = repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))?;
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
    opts.show_untracked_content(true);
    if let Some(p) = file_path {
        opts.pathspec(p);
    }

    let diff = repo.diff_index_to_workdir(None, Some(&mut opts))?;
    parse_diff(&diff)
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
        Delta::Added => "added",
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
fn check_lfs_pointer(content: &str) -> Option<String> {
    // Real LFS pointers are under 256 bytes; anything larger is just a
    // file that happens to reference the spec URL.
    if content.len() > 512 {
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
    let oid = Oid::from_str(oid_str)?;
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;

    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };

    let mut opts = DiffOptions::new();
    opts.context_lines(3);

    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut opts))?;

    parse_diff(&diff)
}

/// Get the combined diff of the working directory against HEAD (staged + unstaged).
pub fn read_working_diff(repo: &Repository) -> Result<Vec<DiffFile>, TwigError> {
    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());

    let mut opts = DiffOptions::new();
    opts.context_lines(3);
    opts.include_untracked(true);

    let staged = repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))?;
    let mut unstaged_opts = DiffOptions::new();
    unstaged_opts.context_lines(3);
    unstaged_opts.include_untracked(true);
    unstaged_opts.show_untracked_content(true);
    let unstaged = repo.diff_index_to_workdir(None, Some(&mut unstaged_opts))?;

    let mut files = parse_diff(&staged)?;
    files.extend(parse_diff(&unstaged)?);

    Ok(files)
}

fn parse_diff(diff: &Diff) -> Result<Vec<DiffFile>, TwigError> {
    let mut files: Vec<DiffFile> = Vec::new();

    let num_deltas = diff.deltas().len();

    for delta_idx in 0..num_deltas {
        let delta = diff.get_delta(delta_idx).expect("delta in range");
        let old_path = delta.old_file().path().map(|p| p.to_string_lossy().to_string());
        let new_path = delta.new_file().path().map(|p| p.to_string_lossy().to_string());
        let is_binary = delta.old_file().is_binary() || delta.new_file().is_binary();
        let status = delta_status_str(delta.status()).to_string();

        let mut hunks: Vec<DiffHunk> = Vec::new();
        let mut is_lfs = false;
        let mut lfs_size: Option<String> = None;
        let mut full_content = String::new();

        // Use print to iterate hunks/lines for this specific delta
        let patch = git2::Patch::from_diff(diff, delta_idx)?;
        if let Some(patch) = patch {
            let num_hunks = patch.num_hunks();
            for hunk_idx in 0..num_hunks {
                let (hunk, num_lines) = patch.hunk(hunk_idx)?;
                let header = std::str::from_utf8(hunk.header()).unwrap_or("").to_string();
                let mut lines: Vec<DiffLine> = Vec::new();

                for line_idx in 0..num_lines {
                    let line = patch.line_in_hunk(hunk_idx, line_idx)?;
                    let content = std::str::from_utf8(line.content()).unwrap_or("").to_string();
                    full_content.push_str(&content);

                    let origin = match line.origin() {
                        '+' => "+".to_string(),
                        '-' => "-".to_string(),
                        ' ' => " ".to_string(),
                        c => c.to_string(),
                    };

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

        // Check for LFS
        if let Some(size) = check_lfs_pointer(&full_content) {
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
