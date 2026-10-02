import { beforeEach, describe, expect, it, vi } from "vitest";
import { get, writable } from "svelte/store";

const result = { matches: [], truncated: false, scanned: 0, tips: "t" };
const tauri = vi.hoisted(() => ({ searchCommits: vi.fn(), searchChanges: vi.fn(), getCommitGraph: vi.fn() }));
vi.mock("../tauri", () => tauri);
vi.mock("./settings", () => ({
  settings: writable({ max_commits: 100, graph_hide_remotes: false, graph_current_branch_only: false }),
}));

const s = await import("./graphSearch");
const { activeRepoPath } = await import("./repos");

describe("graph search kinds", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    tauri.searchCommits.mockResolvedValue(result);
    tauri.searchChanges.mockResolvedValue(result);
    activeRepoPath.set("/r");
    s.searchKind.set("commits");
    s.changeOptions.set({ regex: false, matchCase: false, paths: "" });
  });

  it("searches commits as you type", async () => {
    s.searchQuery.set("fix");
    s.scheduleSearch();
    await vi.runAllTimersAsync();
    expect(tauri.searchCommits).toHaveBeenCalledWith("/r", "fix", expect.anything(), s.SEARCH_MAX_RESULTS);
    expect(tauri.searchChanges).not.toHaveBeenCalled();
  });

  it("waits for Enter in code mode, then sends the options", async () => {
    s.searchKind.set("changes");
    s.changeOptions.set({ regex: true, matchCase: false, paths: " src/, *.rs " });
    s.searchQuery.set("fn helper");
    s.scheduleSearch();
    await vi.runAllTimersAsync();
    expect(tauri.searchChanges).not.toHaveBeenCalled();
    expect(get(s.searchResult)).toBeNull();

    await s.runSearch();
    expect(tauri.searchChanges).toHaveBeenCalledWith(
      "/r",
      "fn helper",
      { regex: true, ignoreCase: true, paths: ["src/", "*.rs"] },
      expect.anything(),
      s.SEARCH_MAX_RESULTS,
    );
    expect(get(s.searchResult)).toEqual(result);

    // Typing again drops the stale results; a forced re-run (history moved) searches.
    s.scheduleSearch();
    expect(get(s.searchResult)).toBeNull();
    s.scheduleSearch(0, true);
    await vi.runAllTimersAsync();
    expect(tauri.searchChanges).toHaveBeenCalledTimes(2);
  });
});
