//! Commit graph reads: paginated history walk with lane assignment, commit
//! search over the whole history, and locating a commit/ref in graph order.
//!
//! Every function here walks history with the same [`graph_walk`] setup, so
//! the `index` of a commit reported by search/locate is its row in the graph.

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

use git2::{Commit, Oid, Repository, Revwalk, Sort};
use serde::{Deserialize, Serialize};

use crate::error::TwigError;
use crate::git::reader::{
    build_refs_map, compute_unpushed_oids, short_oid, CommitGraph, CommitInfo, GraphEntry,
};

/// Which refs the graph walks.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct GraphOptions {
    /// Walk only local branches (and HEAD); remote-tracking refs are skipped.
    pub hide_remotes: bool,
    /// Walk only HEAD's history.
    pub current_branch_only: bool,
}

/// Start a revwalk in graph order (topological, newest first) and return a
/// signature of the tips it was seeded with.
fn graph_walk<'r>(
    repo: &'r Repository,
    opts: &GraphOptions,
) -> Result<(Revwalk<'r>, String), TwigError> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;

    let mut hasher = DefaultHasher::new();
    opts.hide_remotes.hash(&mut hasher);
    opts.current_branch_only.hash(&mut hasher);

    if !opts.current_branch_only {
        let mut prefixes = vec!["refs/heads/"];
        walk.push_glob("refs/heads/*")?;
        if !opts.hide_remotes {
            walk.push_glob("refs/remotes/*")?;
            prefixes.push("refs/remotes/");
        }
        let mut tips: Vec<(String, String)> = Vec::new();
        for reference in repo.references()? {
            let Ok(reference) = reference else { continue };
            let Some(name) = reference.name() else { continue };
            if !prefixes.iter().any(|p| name.starts_with(p)) {
                continue;
            }
            let target = reference
                .resolve()
                .ok()
                .and_then(|r| r.target())
                .map(|o| o.to_string())
                .unwrap_or_default();
            tips.push((name.to_string(), target));
        }
        tips.sort();
        tips.hash(&mut hasher);
    }

    // History can change without any ref moving: `fetch --deepen` /
    // `--unshallow` rewrites `shallow`, and grafts rewrite parents.
    for file in ["shallow", "info/grafts"] {
        std::fs::read(repo.path().join(file)).ok().hash(&mut hasher);
    }

    // HEAD too, for a detached HEAD (and as the only tip in current-branch mode).
    if let Ok(head) = repo.head() {
        if let Some(target) = head.target() {
            walk.push(target)?;
            target.as_bytes().hash(&mut hasher);
        }
    }

    Ok((walk, format!("{:016x}", hasher.finish())))
}

/// Decode commit text: UTF-8 when valid, Latin-1 when the commit says so
/// (every byte is one code point), otherwise lossily. git2's `&str`
/// accessors return `None` for non-UTF-8 text, which showed up as empty
/// summaries and "Unknown" authors.
pub(crate) fn decode_text(bytes: &[u8], encoding: Option<&str>) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    let latin1 = encoding.is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "iso-8859-1" | "iso8859-1" | "latin1" | "latin-1" | "l1"
        )
    });
    if latin1 {
        bytes.iter().map(|&b| char::from(b)).collect()
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// Commit summary (first paragraph, unwrapped), decoded like `decode_text`.
pub(crate) fn commit_summary(commit: &Commit) -> String {
    commit
        .summary_bytes()
        .map(|b| decode_text(b, commit.message_encoding()))
        .unwrap_or_default()
}

/// Full commit message, decoded like `decode_text`.
pub(crate) fn commit_message(commit: &Commit) -> String {
    decode_text(commit.message_bytes(), commit.message_encoding())
}

/// Author name, decoded with the commit's encoding (may be empty).
pub(crate) fn commit_author_name(commit: &Commit) -> String {
    decode_text(commit.author().name_bytes(), commit.message_encoding())
}

/// Build the IPC commit description.
pub(crate) fn commit_info(commit: &Commit) -> CommitInfo {
    let oid = commit.id();
    let author = commit.author();
    let enc = commit.message_encoding();
    // git re-encodes the whole commit (idents included) by its header.
    let email = decode_text(author.email_bytes(), enc);
    let gravatar_hash = format!("{:x}", md5::compute(email.trim().to_lowercase().as_bytes()));
    let name = decode_text(author.name_bytes(), enc);
    CommitInfo {
        oid: oid.to_string(),
        short_oid: short_oid(oid),
        summary: commit.summary_bytes().map(|b| decode_text(b, enc)).unwrap_or_default(),
        body: commit.body_bytes().map(|b| decode_text(b, enc)).unwrap_or_default(),
        author_name: if name.is_empty() { "Unknown".to_string() } else { name },
        author_email: email,
        author_gravatar: gravatar_hash,
        timestamp: commit.time().seconds(),
        parent_oids: commit.parent_ids().map(|id| id.to_string()).collect(),
    }
}

// ── Lane assignment ──────────────────────────────────────────────────

/// Drawing data for one graph row (see `GraphEntry`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LaneRow {
    pub lane: usize,
    pub has_incoming: bool,
    pub rails: Vec<usize>,
    pub parent_lanes: Vec<usize>,
    pub merge_ins: Vec<usize>,
}

/// Incremental lane assignment over commits fed in topological order.
///
/// Each slot holds the commit a child line is waiting for. A commit's first
/// parent always continues straight down the commit's own lane, even when
/// another child already reserved that parent elsewhere. When the parent is
/// reached it sits on the *lowest* lane waiting for it and the other lanes
/// merge into its node (`merge_ins`), so the mainline never drifts right
/// after a fork.
#[derive(Debug, Default, Clone)]
pub(crate) struct LaneState {
    lanes: Vec<Option<Oid>>,
    max_lanes: usize,
}

impl LaneState {
    fn alloc(&mut self) -> usize {
        match self.lanes.iter().position(Option::is_none) {
            Some(i) => i,
            None => {
                self.lanes.push(None);
                self.lanes.len() - 1
            }
        }
    }

    /// Widest the graph has been so far.
    pub fn max_lanes(&self) -> usize {
        self.max_lanes
    }

    pub fn step(&mut self, oid: Oid, parents: &[Oid]) -> LaneRow {
        // ── 1. Lanes waiting for this commit; it takes the lowest ─────
        let waiting: Vec<usize> = self
            .lanes
            .iter()
            .enumerate()
            .filter_map(|(i, s)| (*s == Some(oid)).then_some(i))
            .collect();
        let (lane, has_incoming) = match waiting.first() {
            Some(&l) => (l, true),
            None => (self.alloc(), false),
        };
        let merge_ins: Vec<usize> = waiting.iter().skip(1).copied().collect();
        for &l in &waiting {
            self.lanes[l] = None;
        }

        // ── 2. Lanes still occupied by other lines pass through ──────
        let rails: Vec<usize> = self
            .lanes
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.is_some().then_some(i))
            .collect();

        // ── 3. Reserve lanes for the parents ─────────────────────────
        let mut parent_lanes: Vec<usize> = Vec::with_capacity(parents.len());
        for (pi, parent) in parents.iter().enumerate() {
            if let Some(dup) = parents[..pi].iter().position(|p| p == parent) {
                // Same parent listed twice: reuse its lane.
                parent_lanes.push(parent_lanes[dup]);
            } else if pi == 0 {
                self.lanes[lane] = Some(*parent);
                parent_lanes.push(lane);
            } else if let Some(existing) =
                self.lanes.iter().position(|s| *s == Some(*parent))
            {
                parent_lanes.push(existing);
            } else {
                let l = self.alloc();
                self.lanes[l] = Some(*parent);
                parent_lanes.push(l);
            }
        }

        self.max_lanes = self.max_lanes.max(self.lanes.len()).max(lane + 1);

        // Trim trailing empty lanes to keep the graph compact.
        while self.lanes.last() == Some(&None) {
            self.lanes.pop();
        }

        LaneRow {
            lane,
            has_incoming,
            rails,
            parent_lanes,
            merge_ins,
        }
    }
}

// ── Order cache ──────────────────────────────────────────────────────
//
// Topological order needs the whole history walked before the first row, so
// walking per call made every page, search and locate cost O(history).
// The order is computed once per (repository, tips signature) and kept with
// lane-state checkpoints, so a page resumes lane assignment from the nearest
// checkpoint instead of from row 0. Any ref change alters the tips signature
// and therefore the key, as do changes to `shallow` and `info/grafts`, so a
// cached order never describes stale history.

/// A lane-state snapshot is kept every this many rows (small in tests so
/// the resume-from-checkpoint path is exercised).
const CHECKPOINT_EVERY: usize = if cfg!(test) { 5 } else { 2000 };
/// Repositories whose order is kept (a kernel-sized order is ~26 MB).
const MAX_CACHED_REPOS: usize = 4;

struct OrderCache {
    key: String,
    order: Arc<Vec<Oid>>,
    /// oid → row, built on first use (locate).
    index: Arc<OnceLock<HashMap<Oid, usize>>>,
    /// Lane state *before* row `k`, for `k` multiples of CHECKPOINT_EVERY.
    checkpoints: BTreeMap<usize, LaneState>,
}

static CACHES: Mutex<Vec<OrderCache>> = Mutex::new(Vec::new());

fn cache_key(repo: &Repository, tips: &str) -> String {
    format!("{}\u{0}{tips}", repo.path().display())
}

/// Graph order (and its tips signature) for `opts`, from the cache when the
/// tips are unchanged.
fn graph_order(repo: &Repository, opts: &GraphOptions) -> Result<(Arc<Vec<Oid>>, String, String), TwigError> {
    let (walk, tips) = graph_walk(repo, opts)?;
    let key = cache_key(repo, &tips);
    if let Ok(mut caches) = CACHES.lock() {
        if let Some(pos) = caches.iter().position(|c| c.key == key) {
            // Most recently used first.
            let hit = caches.remove(pos);
            let order = hit.order.clone();
            caches.insert(0, hit);
            return Ok((order, tips, key));
        }
    }
    let order: Vec<Oid> = walk.collect::<Result<_, _>>()?;
    let order = Arc::new(order);
    if let Ok(mut caches) = CACHES.lock() {
        // Drop older orders of the same repository and the least recent ones.
        let prefix = format!("{}\u{0}", repo.path().display());
        caches.retain(|c| !c.key.starts_with(&prefix));
        caches.insert(
            0,
            OrderCache {
                key: key.clone(),
                order: order.clone(),
                index: Arc::new(OnceLock::new()),
                checkpoints: BTreeMap::new(),
            },
        );
        caches.truncate(MAX_CACHED_REPOS);
    }
    Ok((order, tips, key))
}

/// The nearest checkpoint at or before `row` (row 0 with a fresh state).
fn checkpoint_before(key: &str, row: usize) -> (usize, LaneState) {
    let Ok(caches) = CACHES.lock() else {
        return (0, LaneState::default());
    };
    caches
        .iter()
        .find(|c| c.key == key)
        .and_then(|c| c.checkpoints.range(..=row).next_back().map(|(k, s)| (*k, s.clone())))
        .unwrap_or((0, LaneState::default()))
}

fn store_checkpoints(key: &str, new: Vec<(usize, LaneState)>) {
    if new.is_empty() {
        return;
    }
    if let Ok(mut caches) = CACHES.lock() {
        if let Some(c) = caches.iter_mut().find(|c| c.key == key) {
            c.checkpoints.extend(new);
        }
    }
}

/// Row of `oid` in the cached order (builds the index on first use).
fn row_of(key: &str, order: &[Oid], oid: Oid) -> Option<usize> {
    let index = CACHES.lock().ok().and_then(|c| c.iter().find(|c| c.key == key).map(|c| c.index.clone()));
    match index {
        Some(index) => index
            .get_or_init(|| order.iter().enumerate().map(|(i, o)| (*o, i)).collect())
            .get(&oid)
            .copied(),
        None => order.iter().position(|o| *o == oid),
    }
}

// ── Paginated graph ──────────────────────────────────────────────────

/// Read rows `[skip, skip + limit)` of the commit graph.
///
/// Lanes are computed in graph order from the nearest cached checkpoint, so
/// rows from different pages line up as long as `tips` in the responses
/// match.
pub fn read_commit_graph_page(
    repo: &Repository,
    skip: usize,
    limit: usize,
    opts: &GraphOptions,
) -> Result<CommitGraph, TwigError> {
    let limit = limit.max(1);
    let (order, tips, key) = graph_order(repo, opts)?;
    let end = skip.saturating_add(limit).min(order.len());
    let (from, mut state) = checkpoint_before(&key, skip);
    let mut entries: Vec<GraphEntry> = Vec::with_capacity(end.saturating_sub(skip).min(10_000));
    let mut new_checkpoints = Vec::new();

    for (i, &oid) in order.iter().enumerate().take(end).skip(from) {
        if i % CHECKPOINT_EVERY == 0 && i > from {
            new_checkpoints.push((i, state.clone()));
        }
        let commit = repo.find_commit(oid)?;
        let parents: Vec<Oid> = commit.parent_ids().collect();
        let row = state.step(oid, &parents);
        if i >= skip {
            entries.push(GraphEntry {
                commit: commit_info(&commit),
                lane: row.lane,
                has_incoming: row.has_incoming,
                rails: row.rails,
                parent_lanes: row.parent_lanes,
                merge_ins: row.merge_ins,
            });
        }
    }
    store_checkpoints(&key, new_checkpoints);
    let has_more = end < order.len();

    let mut refs = build_refs_map(repo)?;
    if opts.hide_remotes {
        refs.retain(|_, labels| {
            labels.retain(|l| l.ref_type != "remote");
            !labels.is_empty()
        });
    }

    Ok(CommitGraph {
        entries,
        total_lanes: state.max_lanes(),
        refs,
        unpushed_oids: compute_unpushed_oids(repo),
        offset: skip,
        has_more,
        tips,
    })
}

/// The pre-cache implementation (walk + lanes from row 0), kept as the
/// ground truth for tests.
#[cfg(test)]
pub(crate) fn read_commit_graph_page_uncached(
    repo: &Repository,
    skip: usize,
    limit: usize,
    opts: &GraphOptions,
) -> Result<CommitGraph, TwigError> {
    let limit = limit.max(1);
    let end = skip.saturating_add(limit);
    let (walk, tips) = graph_walk(repo, opts)?;
    let mut state = LaneState::default();
    let mut entries: Vec<GraphEntry> = Vec::with_capacity(limit.min(10_000));
    let mut has_more = false;

    for (i, oid) in walk.enumerate() {
        let oid = oid?;
        if i >= end {
            has_more = true;
            break;
        }
        let commit = repo.find_commit(oid)?;
        let parents: Vec<Oid> = commit.parent_ids().collect();
        let row = state.step(oid, &parents);
        if i >= skip {
            entries.push(GraphEntry {
                commit: commit_info(&commit),
                lane: row.lane,
                has_incoming: row.has_incoming,
                rails: row.rails,
                parent_lanes: row.parent_lanes,
                merge_ins: row.merge_ins,
            });
        }
    }

    let mut refs = build_refs_map(repo)?;
    if opts.hide_remotes {
        refs.retain(|_, labels| {
            labels.retain(|l| l.ref_type != "remote");
            !labels.is_empty()
        });
    }

    Ok(CommitGraph {
        entries,
        total_lanes: state.max_lanes(),
        refs,
        unpushed_oids: compute_unpushed_oids(repo),
        offset: skip,
        has_more,
        tips,
    })
}

// ── Search ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct SearchMatch {
    /// Row of this commit in the (unfiltered) graph.
    pub index: usize,
    pub commit: CommitInfo,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommitSearchResult {
    pub matches: Vec<SearchMatch>,
    /// True when the result was cut off at `max_results`.
    pub truncated: bool,
    /// Number of commits examined.
    pub scanned: usize,
    pub tips: String,
}

/// Case-insensitive matcher over message, author name/email and SHA prefix.
struct Matcher {
    needle: String,
    hex_prefix: Option<String>,
}

impl Matcher {
    fn new(query: &str) -> Self {
        let needle = query.trim().to_lowercase();
        let hex_prefix = (needle.len() >= 4 && needle.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| needle.clone());
        Self { needle, hex_prefix }
    }

    fn matches(&self, commit: &Commit) -> bool {
        if let Some(prefix) = &self.hex_prefix {
            if commit.id().to_string().starts_with(prefix) {
                return true;
            }
        }
        let author = commit.author();
        let contains = |s: String| s.to_lowercase().contains(&self.needle);
        let enc = commit.message_encoding();
        contains(decode_text(author.name_bytes(), enc))
            || contains(decode_text(author.email_bytes(), enc))
            || contains(decode_text(commit.message_bytes(), enc))
    }
}

/// Search the whole graph history (not just loaded rows) for commits whose
/// message, author or SHA matches `query`.
pub fn search_commits(
    repo: &Repository,
    query: &str,
    opts: &GraphOptions,
    max_results: usize,
) -> Result<CommitSearchResult, TwigError> {
    let matcher = Matcher::new(query);
    let mut matches = Vec::new();
    let mut truncated = false;
    let mut scanned = 0;
    if matcher.needle.is_empty() {
        let (_, tips) = graph_walk(repo, opts)?;
        return Ok(CommitSearchResult {
            matches,
            truncated,
            scanned,
            tips,
        });
    }
    let (order, tips, _) = graph_order(repo, opts)?;

    for (index, &oid) in order.iter().enumerate() {
        scanned = index + 1;
        let commit = repo.find_commit(oid)?;
        if matcher.matches(&commit) {
            if matches.len() >= max_results {
                truncated = true;
                break;
            }
            matches.push(SearchMatch {
                index,
                commit: commit_info(&commit),
            });
        }
    }

    Ok(CommitSearchResult {
        matches,
        truncated,
        scanned,
        tips,
    })
}

// ── Locate ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct LocatedCommit {
    pub oid: String,
    /// Row of the commit in the graph, or `None` when the commit exists but
    /// is not part of the graph (e.g. hidden by the view options).
    pub index: Option<usize>,
    pub tips: String,
}

/// Resolve `rev` (branch, tag, `HEAD`, SHA, …) to a commit and find its row
/// in graph order.
pub fn locate_commit(
    repo: &Repository,
    rev: &str,
    opts: &GraphOptions,
) -> Result<LocatedCommit, TwigError> {
    let rev = rev.trim();
    if rev.is_empty() {
        return Err(TwigError::InvalidArgument("empty revision".into()));
    }
    let target = repo
        .revparse_single(rev)
        .and_then(|o| o.peel_to_commit())
        .map_err(|_| TwigError::InvalidArgument(format!("'{rev}' does not name a commit")))?
        .id();
    let (order, tips, key) = graph_order(repo, opts)?;
    let index = row_of(&key, &order, target);
    Ok(LocatedCommit {
        oid: target.to_string(),
        index,
        tips,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    fn oid(n: u8) -> Oid {
        Oid::from_bytes(&[n; 20]).unwrap()
    }

    /// Run lane assignment over `(id, parents)` rows given in topo order.
    fn lanes(rows: &[(u8, &[u8])]) -> (Vec<LaneRow>, usize) {
        let mut st = LaneState::default();
        let out = rows
            .iter()
            .map(|(id, ps)| {
                let parents: Vec<Oid> = ps.iter().map(|p| oid(*p)).collect();
                st.step(oid(*id), &parents)
            })
            .collect();
        (out, st.max_lanes())
    }

    #[test]
    fn merge_keeps_mainline_on_lane_zero() {
        // 4 merges 3 into 2; both fork from 1.
        let (rows, max) = lanes(&[(4, &[2, 3]), (3, &[1]), (2, &[1]), (1, &[])]);
        let l: Vec<usize> = rows.iter().map(|r| r.lane).collect();
        assert_eq!(l, vec![0, 1, 0, 0]);
        assert_eq!(rows[0].parent_lanes, vec![0, 1]);
        assert_eq!(rows[1].parent_lanes, vec![1]);
        assert_eq!(rows[2].parent_lanes, vec![0]);
        assert_eq!(rows[2].rails, vec![1]);
        // The side lane converges into the fork point.
        assert!(rows[3].has_incoming);
        assert_eq!(rows[3].merge_ins, vec![1]);
        assert!(rows[3].rails.is_empty());
        assert_eq!(max, 2);
    }

    #[test]
    fn unmerged_fork_does_not_drift() {
        // Two branch tips (5 on main, 4 on feature) forked from 2, history
        // continues below the fork on lane 0.
        let (rows, _) = lanes(&[
            (5, &[3]),
            (4, &[2]),
            (3, &[2]),
            (2, &[1]),
            (1, &[]),
        ]);
        let l: Vec<usize> = rows.iter().map(|r| r.lane).collect();
        assert_eq!(l, vec![0, 1, 0, 0, 0]);
        assert_eq!(rows[3].merge_ins, vec![1]);
        assert!(rows[4].rails.is_empty());
        assert!(rows[4].merge_ins.is_empty());
    }

    #[test]
    fn fork_from_lower_lane_child() {
        // Tip 4 is on lane 0 and reaches 1 via 2; tip 3 (lane 1) is a direct
        // child of 1. 1 must land on lane 0 with lane 1 merging in.
        let (rows, _) = lanes(&[(4, &[2]), (3, &[1]), (2, &[1]), (1, &[])]);
        assert_eq!(rows[3].lane, 0);
        assert_eq!(rows[3].merge_ins, vec![1]);
    }

    #[test]
    fn octopus_and_lane_reuse() {
        // 9 merges 8, 7, 6; each comes from 1.
        let (rows, max) = lanes(&[
            (9, &[8, 7, 6]),
            (8, &[1]),
            (7, &[1]),
            (6, &[1]),
            (1, &[]),
        ]);
        assert_eq!(rows[0].parent_lanes, vec![0, 1, 2]);
        assert_eq!(rows[4].lane, 0);
        assert_eq!(rows[4].merge_ins, vec![1, 2]);
        assert_eq!(max, 3);
        // A merge parent already waited on by another lane is shared.
        let (rows, _) = lanes(&[(5, &[3]), (4, &[2, 3]), (3, &[1]), (2, &[1]), (1, &[])]);
        assert_eq!(rows[1].parent_lanes, vec![1, 0]);
    }

    fn git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .args(["-c", "user.name=Tester", "-c", "user.email=t@example.com"])
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(dir)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(ok, "git {args:?} failed");
    }

    fn temp_repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("twig-graph-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        dir
    }

    fn commit(dir: &Path, msg: &str) {
        git(dir, &["commit", "-q", "--allow-empty", "-m", msg]);
    }

    /// Branches with `/` in their name (and remotes under them) are walked,
    /// and moving any of them changes `tips`.
    #[test]
    fn nested_branch_names_are_walked() {
        let dir = temp_repo("nested");
        commit(&dir, "base");
        git(&dir, &["checkout", "-q", "-b", "feature/deep/x"]);
        commit(&dir, "nested only");
        git(&dir, &["checkout", "-q", "main"]);
        git(&dir, &["update-ref", "refs/remotes/origin/team/y", "feature/deep/x"]);
        git(&dir, &["branch", "-q", "-f", "feature/deep/x", "main"]);
        let repo = Repository::open(&dir).unwrap();

        let all = read_commit_graph_page(&repo, 0, 100, &GraphOptions::default()).unwrap();
        let subjects: Vec<&str> = all.entries.iter().map(|e| e.commit.summary.as_str()).collect();
        assert!(subjects.contains(&"nested only"), "{subjects:?}");
        let local = GraphOptions { hide_remotes: true, ..Default::default() };
        let g = read_commit_graph_page(&repo, 0, 100, &local).unwrap();
        assert_eq!(g.entries.len(), 1);

        git(&dir, &["checkout", "-q", "-b", "a/b"]);
        commit(&dir, "on a/b");
        let moved = read_commit_graph_page(&repo, 0, 100, &local).unwrap();
        assert_ne!(moved.tips, g.tips);
        assert_eq!(moved.entries.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Commits recorded in a legacy encoding (message *and* author bytes)
    /// show decoded and are searchable instead of rendering as empty /
    /// "Unknown".
    #[cfg(unix)]
    #[test]
    fn non_utf8_commit_metadata_is_shown_and_searchable() {
        use std::os::unix::ffi::OsStrExt;
        let dir = temp_repo("latin1");
        let name = std::ffi::OsStr::from_bytes(b"Ren\xe9");
        let ok = Command::new("git")
            .args(["-c", "commit.gpgsign=false", "-c", "i18n.commitEncoding=ISO-8859-1"])
            .args(["commit", "-q", "--allow-empty", "-F", "-"])
            .env("GIT_AUTHOR_NAME", name)
            .env("GIT_COMMITTER_NAME", name)
            .env("GIT_AUTHOR_EMAIL", "r@example.com")
            .env("GIT_COMMITTER_EMAIL", "r@example.com")
            .current_dir(&dir)
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut c| {
                use std::io::Write;
                c.stdin.take().unwrap().write_all(b"caf\xe9 latin\n")?;
                c.wait()
            })
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(ok);
        let repo = Repository::open(&dir).unwrap();
        let opts = GraphOptions::default();
        let g = read_commit_graph_page(&repo, 0, 10, &opts).unwrap();
        let c = &g.entries[0].commit;
        assert_eq!(c.summary, "caf\u{e9} latin");
        assert_eq!(c.author_name, "Ren\u{e9}");
        assert_eq!(search_commits(&repo, "latin", &opts, 10).unwrap().matches.len(), 1);
        assert_eq!(search_commits(&repo, "REN\u{c9}", &opts, 10).unwrap().matches.len(), 1);
        // Plain lossy fallback for undeclared bytes.
        assert_eq!(decode_text(b"a\xffb", None), "a\u{fffd}b");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every reader built on the decode helpers shows a Latin-1 commit.
    #[cfg(unix)]
    #[test]
    fn non_utf8_metadata_in_other_views() {
        use std::os::unix::ffi::OsStrExt;
        let dir = temp_repo("latin1-views");
        commit(&dir, "base");
        std::fs::write(dir.join("f.txt"), "x\n").unwrap();
        git(&dir, &["add", "f.txt"]);
        let name = std::ffi::OsStr::from_bytes(b"Ren\xe9");
        let ok = Command::new("git")
            .args(["-c", "commit.gpgsign=false", "-c", "i18n.commitEncoding=ISO-8859-1"])
            .args(["commit", "-q", "-F", "-"])
            .env("GIT_AUTHOR_NAME", name)
            .env("GIT_COMMITTER_NAME", name)
            .env("GIT_AUTHOR_EMAIL", "r@example.com")
            .env("GIT_COMMITTER_EMAIL", "r@example.com")
            .current_dir(&dir)
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut c| {
                use std::io::Write;
                c.stdin.take().unwrap().write_all(b"caf\xe9 latin\n\nbody \xe9\n")?;
                c.wait()
            })
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(ok);
        let repo = Repository::open(&dir).unwrap();
        let (subject, author) = ("caf\u{e9} latin", "Ren\u{e9}");

        let hist = crate::git::file_history::read_file_history(&repo, "f.txt", None, 0, 10).unwrap();
        let c = &hist.entries[0].commit;
        assert_eq!((c.summary.as_str(), c.author_name.as_str()), (subject, author));
        assert_eq!(c.body, "body \u{e9}");

        let blame = crate::git::file_history::read_blame(&repo, "f.txt", None).unwrap();
        let h = &blame.hunks[0];
        assert_eq!((h.summary.as_str(), h.author_name.as_str()), (subject, author));

        let list = crate::git::history::list_rebase_commits(&repo, Some("HEAD~1")).unwrap();
        let r = &list.commits[0];
        assert_eq!((r.summary.as_str(), r.author_name.as_str()), (subject, author));
        assert!(r.message.starts_with(subject), "{:?}", r.message);

        let log = crate::git::reflog::read_head_reflog(&repo, 5).unwrap();
        assert_eq!(log[0].commit_summary.as_deref(), Some(subject));

        // Amend prefill keeps the real text (a lossy prefill would be saved).
        let head = crate::git::commit_tools::read_head_commit(&repo).unwrap();
        assert_eq!(head.message, "caf\u{e9} latin\n\nbody \u{e9}");
        // Recent co-authors include the Latin-1 author.
        let authors = crate::git::commit_tools::read_recent_authors(&repo, 50).unwrap();
        assert!(authors.iter().any(|a| a.name == author), "{authors:?}");
        // Tags show the target commit's summary.
        git(&dir, &["tag", "-a", "-m", "release notes", "v1"]);
        let tags = crate::git::tags::read_tags(&repo).unwrap();
        assert_eq!(tags[0].commit_summary.as_deref(), Some(subject));
        assert_eq!(tags[0].message.as_deref(), Some("release notes"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The cached order + checkpoints give exactly the rows of the original
    /// walk-from-row-0 implementation, for any page size and request order,
    /// and a new commit (tips change) is picked up.
    #[test]
    fn cached_pages_match_uncached_walk() {
        let dir = temp_repo("cache-eq");
        for i in 0..30 {
            commit(&dir, &format!("base {i}"));
        }
        for b in 0..4 {
            git(&dir, &["checkout", "-q", "-b", &format!("f{b}"), &format!("main~{}", 5 * b)]);
            for i in 0..(7 + b) {
                commit(&dir, &format!("f{b} {i}"));
            }
            git(&dir, &["checkout", "-q", "main"]);
            if b % 2 == 0 {
                git(&dir, &["merge", "-q", "--no-ff", "-m", &format!("merge f{b}"), &format!("f{b}")]);
            }
        }
        git(&dir, &["merge", "-q", "--no-ff", "-m", "octopus", "f1", "f3"]);
        let repo = Repository::open(&dir).unwrap();
        let opts = GraphOptions::default();

        let check = |repo: &Repository| {
            let truth = read_commit_graph_page_uncached(repo, 0, 10_000, &opts).unwrap();
            let n = truth.entries.len();
            for page in [1usize, 3, 7, 50] {
                // Request pages back to front so later rows are served before
                // their checkpoints exist, then front to back.
                let starts: Vec<usize> = (0..n).step_by(page).collect();
                for &skip in starts.iter().rev().chain(starts.iter()) {
                    let got = read_commit_graph_page(repo, skip, page, &opts).unwrap();
                    let want = read_commit_graph_page_uncached(repo, skip, page, &opts).unwrap();
                    assert_eq!(got.tips, want.tips);
                    assert_eq!(got.has_more, want.has_more, "skip {skip} page {page}");
                    assert_eq!(got.total_lanes, want.total_lanes, "skip {skip} page {page}");
                    assert_eq!(got.entries.len(), want.entries.len());
                    for (a, b) in got.entries.iter().zip(&want.entries) {
                        assert_eq!(a.commit.oid, b.commit.oid);
                        assert_eq!((a.lane, a.has_incoming), (b.lane, b.has_incoming));
                        assert_eq!(a.rails, b.rails);
                        assert_eq!(a.parent_lanes, b.parent_lanes);
                        assert_eq!(a.merge_ins, b.merge_ins);
                    }
                }
            }
            n
        };
        let before = check(&repo);
        // New commit: tips change, so the cache must not serve the old order.
        commit(&dir, "after cache");
        let after = check(&repo);
        assert_eq!(after, before + 1);
        let top = read_commit_graph_page(&repo, 0, 1, &opts).unwrap();
        assert_eq!(top.entries[0].commit.summary, "after cache");
        // Search and locate use the same cached order.
        let res = search_commits(&repo, "f2 3", &opts, 10).unwrap();
        let full = read_commit_graph_page_uncached(&repo, 0, 10_000, &opts).unwrap();
        for m in &res.matches {
            assert_eq!(full.entries[m.index].commit.oid, m.commit.oid);
        }
        let loc = locate_commit(&repo, "f3", &opts).unwrap();
        assert_eq!(full.entries[loc.index.unwrap()].commit.oid, loc.oid);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Deepening a shallow clone changes history without moving any ref:
    /// the cache must notice.
    #[test]
    fn deepening_a_shallow_clone_invalidates_the_cache() {
        let src = temp_repo("shallow-src");
        for i in 0..10 {
            commit(&src, &format!("c{i}"));
        }
        let clone = src.with_extension("clone");
        let _ = std::fs::remove_dir_all(&clone);
        let url = format!("file://{}", src.display());
        git(&src, &["clone", "-q", "--depth", "3", &url, &clone.to_string_lossy()]);
        let repo = Repository::open(&clone).unwrap();
        let opts = GraphOptions::default();
        let before = read_commit_graph_page(&repo, 0, 100, &opts).unwrap();
        assert_eq!(before.entries.len(), 3);
        git(&clone, &["fetch", "-q", "--unshallow"]);
        // Twig opens a fresh handle per read (libgit2 caches shallow info).
        let repo = Repository::open(&clone).unwrap();
        let after = read_commit_graph_page(&repo, 0, 100, &opts).unwrap();
        assert_eq!(after.entries.len(), 10, "stale cached order after --unshallow");
        assert_ne!(after.tips, before.tips);
        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&clone);
    }

    #[test]
    fn pages_are_consistent_with_full_read() {
        let dir = temp_repo("pages");
        for i in 0..5 {
            commit(&dir, &format!("base {i}"));
        }
        git(&dir, &["checkout", "-q", "-b", "feature"]);
        for i in 0..4 {
            commit(&dir, &format!("feature {i}"));
        }
        git(&dir, &["checkout", "-q", "main"]);
        for i in 0..3 {
            commit(&dir, &format!("main {i}"));
        }
        git(&dir, &["merge", "-q", "--no-ff", "-m", "merge feature", "feature"]);
        let repo = Repository::open(&dir).unwrap();
        let opts = GraphOptions::default();

        let full = read_commit_graph_page(&repo, 0, 100, &opts).unwrap();
        assert_eq!(full.entries.len(), 13);
        assert!(!full.has_more);

        let mut paged: Vec<GraphEntry> = Vec::new();
        let mut skip = 0;
        loop {
            let page = read_commit_graph_page(&repo, skip, 4, &opts).unwrap();
            assert_eq!(page.tips, full.tips);
            assert_eq!(page.offset, skip);
            skip += page.entries.len();
            paged.extend(page.entries);
            if !page.has_more {
                break;
            }
        }
        assert_eq!(paged.len(), full.entries.len());
        for (a, b) in paged.iter().zip(&full.entries) {
            assert_eq!(a.commit.oid, b.commit.oid);
            assert_eq!(a.lane, b.lane);
            assert_eq!(a.rails, b.rails);
            assert_eq!(a.parent_lanes, b.parent_lanes);
            assert_eq!(a.merge_ins, b.merge_ins);
        }
        // Mainline (first-parent chain from HEAD) stays on lane 0.
        let mut want = full.entries[0].commit.oid.clone();
        for e in &full.entries {
            if e.commit.oid == want {
                assert_eq!(e.lane, 0, "{} drifted", e.commit.summary);
                match e.commit.parent_oids.first() {
                    Some(p) => want = p.clone(),
                    None => break,
                }
            }
        }

        // Search: message, author, sha prefix; indices match graph rows.
        let res = search_commits(&repo, "FEATURE", &opts, 100).unwrap();
        assert_eq!(res.matches.len(), 5); // 4 commits + the merge message
        for m in &res.matches {
            assert_eq!(full.entries[m.index].commit.oid, m.commit.oid);
        }
        let res = search_commits(&repo, "example.com", &opts, 3).unwrap();
        assert_eq!(res.matches.len(), 3);
        assert!(res.truncated);
        let sha = &full.entries[7].commit.oid[..8];
        let res = search_commits(&repo, sha, &opts, 100).unwrap();
        assert!(res.matches.iter().any(|m| m.index == 7));
        assert!(search_commits(&repo, "  ", &opts, 100).unwrap().matches.is_empty());

        // Locate refs.
        let head = locate_commit(&repo, "HEAD", &opts).unwrap();
        assert_eq!(head.index, Some(0));
        let feat = locate_commit(&repo, "feature", &opts).unwrap();
        let idx = feat.index.unwrap();
        assert_eq!(full.entries[idx].commit.oid, feat.oid);
        assert!(locate_commit(&repo, "nope", &opts).is_err());

        // Current-branch-only on `feature` hides main-only commits.
        git(&dir, &["checkout", "-q", "feature"]);
        let only = GraphOptions {
            current_branch_only: true,
            ..Default::default()
        };
        let g = read_commit_graph_page(&repo, 0, 100, &only).unwrap();
        assert_eq!(g.entries.len(), 9);
        assert_ne!(g.tips, full.tips);
        assert!(g.entries.iter().all(|e| e.rails.is_empty() && e.lane == 0));
        let main_tip = locate_commit(&repo, "main", &only).unwrap();
        assert_eq!(main_tip.index, None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
