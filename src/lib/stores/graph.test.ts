import { beforeEach, describe, expect, it, vi } from "vitest";
import { get, writable } from "svelte/store";
import type { CommitGraph, GraphEntry } from "../types/git";

const tauri = vi.hoisted(() => ({ getCommitGraph: vi.fn() }));
vi.mock("../tauri", () => tauri);
vi.mock("./settings", () => ({
  settings: writable({ max_commits: 3, graph_hide_remotes: false, graph_current_branch_only: false }),
}));

const { commitGraph, loadMoreCommits, ensureGraphLoaded } = await import("./graph");
const { activeRepoPath } = await import("./repos");

function entry(n: number): GraphEntry {
  return {
    commit: {
      oid: `oid${n}`,
      short_oid: `o${n}`,
      summary: `c${n}`,
      body: "",
      author_name: "t",
      author_email: "t@t",
      author_gravatar: "",
      timestamp: n,
      parent_oids: [],
    },
    lane: 0,
    has_incoming: false,
    rails: [],
    parent_lanes: [],
    merge_ins: [],
  } as GraphEntry;
}

/** A fake history of `total` commits; `getCommitGraph(path, limit, skip)` pages it. */
function serve(total: number, tips = "t1", lanes = 1) {
  tauri.getCommitGraph.mockImplementation(async (_p: string, limit: number, skip: number) => {
    const end = Math.min(total, skip + limit);
    const entries = Array.from({ length: Math.max(0, end - skip) }, (_, i) => entry(skip + i));
    return {
      entries,
      total_lanes: lanes,
      refs: {},
      unpushed_oids: [],
      offset: skip,
      has_more: end < total,
      tips,
    } satisfies CommitGraph;
  });
}

const oids = () => get(commitGraph)?.entries.map((e) => e.commit.oid) ?? [];

beforeEach(async () => {
  tauri.getCommitGraph.mockReset();
  activeRepoPath.set("/repo");
  serve(10);
  commitGraph.set(await tauri.getCommitGraph("/repo", 3, 0, {}));
});

describe("loadMoreCommits", () => {
  it("appends the next page while the tips are unchanged", async () => {
    await loadMoreCommits();
    expect(oids()).toEqual(["oid0", "oid1", "oid2", "oid3", "oid4", "oid5"]);
    expect(get(commitGraph)?.offset).toBe(0);
    expect(tauri.getCommitGraph).toHaveBeenLastCalledWith("/repo", 3, 3, expect.anything());
  });

  it("reloads from the start instead of mixing histories when the tips moved", async () => {
    serve(12, "t2", 2);
    await loadMoreCommits();
    expect(tauri.getCommitGraph).toHaveBeenLastCalledWith("/repo", 6, 0, expect.anything());
    const g = get(commitGraph);
    expect(g?.tips).toBe("t2");
    expect(g?.entries.map((e) => e.commit.oid)).toEqual(["oid0", "oid1", "oid2", "oid3", "oid4", "oid5"]);
    expect(g?.total_lanes).toBe(2);
  });

  it("shares one request between concurrent callers", async () => {
    const calls = tauri.getCommitGraph.mock.calls.length;
    await Promise.all([loadMoreCommits(), loadMoreCommits()]);
    expect(tauri.getCommitGraph.mock.calls.length).toBe(calls + 1);
    expect(oids()).toHaveLength(6);
  });

  it("drops a page that arrives after the user switched repos", async () => {
    let release!: () => void;
    tauri.getCommitGraph.mockImplementationOnce(
      () => new Promise((r) => (release = () => r({ ...get(commitGraph)!, entries: [entry(99)], offset: 3 }))),
    );
    const p = loadMoreCommits();
    activeRepoPath.set("/other");
    release();
    await p;
    // (Switching repos resets the graph on its own; the stale page must not land.)
    expect(oids()).not.toContain("oid99");
  });

  it("does nothing at the end of history", async () => {
    serve(3);
    commitGraph.set(await tauri.getCommitGraph("/repo", 3, 0, {}));
    const calls = tauri.getCommitGraph.mock.calls.length;
    await loadMoreCommits();
    expect(tauri.getCommitGraph.mock.calls.length).toBe(calls);
  });
});

describe("ensureGraphLoaded", () => {
  it("pages until enough rows are loaded or history ends", async () => {
    await ensureGraphLoaded(8);
    expect(oids().length).toBeGreaterThanOrEqual(8);
    await ensureGraphLoaded(Number.MAX_SAFE_INTEGER);
    expect(oids()).toHaveLength(10);
    expect(get(commitGraph)?.has_more).toBe(false);
  });

  it("gives up instead of spinning when loading fails", async () => {
    tauri.getCommitGraph.mockRejectedValue(new Error("boom"));
    await ensureGraphLoaded(100);
    expect(oids()).toHaveLength(3);
  });
});
