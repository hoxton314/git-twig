//! Large-repo timings (#39). Opt-in, release mode recommended:
//!
//! ```sh
//! TWIG_BENCH_REPO=/path/to/big/repo cargo test --release bench_large_repo -- --ignored --nocapture
//! ```
//!
//! Prints one line per operation; nothing is asserted beyond success.

use std::time::Instant;

use git2::Repository;

use crate::git::graph::{self, GraphOptions};
use crate::git::reader;

fn time<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    println!("{label:<48} {:>9.1} ms", t.elapsed().as_secs_f64() * 1000.0);
    out
}

#[test]
#[ignore = "needs TWIG_BENCH_REPO"]
fn bench_large_repo() {
    let Ok(path) = std::env::var("TWIG_BENCH_REPO") else {
        return;
    };
    let repo = Repository::open(&path).unwrap();
    let opts = GraphOptions::default();
    let page = 5000;

    let first = time("graph: first page (5000)", || graph::read_commit_graph_page(&repo, 0, page, &opts).unwrap());
    println!("{:<48} {:>9}", "  total lanes", first.total_lanes);
    time("graph: first page again (cached order)", || graph::read_commit_graph_page(&repo, 0, page, &opts).unwrap());
    time("graph: page at 50k", || graph::read_commit_graph_page(&repo, 50_000, page, &opts).unwrap());
    time("graph: page at 250k", || graph::read_commit_graph_page(&repo, 250_000, page, &opts).unwrap());
    time("graph: page at 280k (cold checkpoints)", || graph::read_commit_graph_page(&repo, 280_000, page, &opts).unwrap());
    time("graph: next page 285k (scrolling)", || graph::read_commit_graph_page(&repo, 285_000, page, &opts).unwrap());
    let cached = graph::read_commit_graph_page(&repo, 250_000, 200, &opts).unwrap();
    let uncached = time("graph: page at 250k, uncached (old code)", || {
        graph::read_commit_graph_page_uncached(&repo, 250_000, 200, &opts).unwrap()
    });
    assert!(
        cached.entries.iter().zip(&uncached.entries).all(|(a, b)| a.commit.oid == b.commit.oid
            && a.lane == b.lane
            && a.rails == b.rails
            && a.parent_lanes == b.parent_lanes),
        "cached rows differ from the uncached walk"
    );
    time("search: no match (whole history)", || graph::search_commits(&repo, "zzz-no-such-text", &opts, 500).unwrap());
    time("search: no match, again", || graph::search_commits(&repo, "zzz-no-such-text", &opts, 500).unwrap());
    time("search: common word (500 results)", || graph::search_commits(&repo, "module", &opts, 500).unwrap());
    let oldest = time("locate: root commit", || {
        let mut walk = repo.revwalk().unwrap();
        walk.push_head().unwrap();
        let root = walk.last().unwrap().unwrap().to_string();
        graph::locate_commit(&repo, &root, &opts).unwrap()
    });
    println!("{:<48} {:>9?}", "  root row", oldest.index);
    let root = oldest.oid.clone();
    time("locate: root commit again (index built)", || graph::locate_commit(&repo, &root, &opts).unwrap());
    time("status: working tree", || reader::read_working_status(&repo).unwrap());
    time("branches: list", || reader::read_branches(&repo).unwrap());
    time("refs map", || reader::build_refs_map(&repo).unwrap());
    time("unpushed oids", || reader::compute_unpushed_oids(&repo));
}
