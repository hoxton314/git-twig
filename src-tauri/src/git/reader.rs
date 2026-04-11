//! git2-based read operations — commit graph, branches, diffs, file contents.

use git2::{BranchType, Delta, Diff, DiffOptions, Oid, Repository, Sort};
use serde::Serialize;
use std::collections::HashMap;

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
    /// Index of the lane this commit occupies.
    pub lane: usize,
    /// Edges from this row to parent rows: (from_lane, to_lane, parent_oid).
    pub edges: Vec<(usize, usize, String)>,
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

/// Simple lane assignment: tracks which OID is expected in each lane.
fn compute_lanes(commits: Vec<CommitInfo>) -> CommitGraph {
    // Map oid -> row index for quick lookup
    let _oid_to_row: HashMap<&str, usize> = commits
        .iter()
        .enumerate()
        .map(|(i, c)| (c.oid.as_str(), i))
        .collect();

    // Active lanes: each slot is either Some(oid we're expecting) or None (free)
    let mut lanes: Vec<Option<String>> = Vec::new();
    let mut entries: Vec<GraphEntry> = Vec::new();
    let mut max_lanes: usize = 0;

    for commit in &commits {
        // Find which lane this commit occupies (it was reserved by a child, or we allocate new)
        let my_lane = lanes
            .iter()
            .position(|slot| slot.as_deref() == Some(&commit.oid))
            .unwrap_or_else(|| {
                // Allocate a new lane
                let free = lanes.iter().position(|s| s.is_none());
                match free {
                    Some(idx) => {
                        lanes[idx] = Some(commit.oid.clone());
                        idx
                    }
                    None => {
                        lanes.push(Some(commit.oid.clone()));
                        lanes.len() - 1
                    }
                }
            });

        // This lane is now consumed by this commit
        lanes[my_lane] = None;

        let mut edges: Vec<(usize, usize, String)> = Vec::new();

        // Assign parents to lanes
        for (pi, parent_oid) in commit.parent_oids.iter().enumerate() {
            // Check if parent already has a lane reserved
            let parent_lane =
                if let Some(existing) = lanes.iter().position(|s| s.as_deref() == Some(parent_oid))
                {
                    existing
                } else if pi == 0 {
                    // First parent takes our lane (straight line)
                    lanes[my_lane] = Some(parent_oid.clone());
                    my_lane
                } else {
                    // Merge parent — find a free lane
                    let free = lanes.iter().position(|s| s.is_none());
                    match free {
                        Some(idx) => {
                            lanes[idx] = Some(parent_oid.clone());
                            idx
                        }
                        None => {
                            lanes.push(Some(parent_oid.clone()));
                            lanes.len() - 1
                        }
                    }
                };

            edges.push((my_lane, parent_lane, parent_oid.clone()));
        }

        if lanes.len() > max_lanes {
            max_lanes = lanes.len();
        }

        // Trim trailing empty lanes
        while lanes.last() == Some(&None) {
            lanes.pop();
        }

        entries.push(GraphEntry {
            commit: commit.clone(),
            lane: my_lane,
            edges,
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

/// Check if diff content is an LFS pointer and extract the size if so.
fn check_lfs_pointer(content: &str) -> Option<String> {
    if content.contains("version https://git-lfs.github.com/spec/") {
        for line in content.lines() {
            if let Some(size_str) = line.strip_prefix("size ") {
                let bytes: u64 = size_str.trim().parse().unwrap_or(0);
                return Some(format_bytes(bytes));
            }
        }
        Some("unknown size".to_string())
    } else {
        None
    }
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

    parse_diff(&diff, repo)
}

/// Get the diff of the working directory against HEAD.
pub fn read_working_diff(repo: &Repository) -> Result<Vec<DiffFile>, TwigError> {
    let head_tree = repo
        .head()
        .ok()
        .and_then(|h| h.peel_to_tree().ok());

    let mut opts = DiffOptions::new();
    opts.context_lines(3);
    opts.include_untracked(true);

    // Staged changes
    let staged = repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))?;
    // Unstaged changes
    let mut unstaged_opts = DiffOptions::new();
    unstaged_opts.context_lines(3);
    let unstaged = repo.diff_index_to_workdir(None, Some(&mut unstaged_opts))?;

    let mut files = parse_diff(&staged, repo)?;
    files.extend(parse_diff(&unstaged, repo)?);

    Ok(files)
}

fn parse_diff(diff: &Diff, _repo: &Repository) -> Result<Vec<DiffFile>, TwigError> {
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
