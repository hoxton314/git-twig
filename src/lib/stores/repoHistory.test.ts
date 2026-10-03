import { describe, expect, it, vi } from "vitest";
import { get } from "svelte/store";

const tauri = vi.hoisted(() => ({
  loadRepoHistory: vi.fn(async () => ({ recent: [], favorites: [], groups: [] })),
  saveRepoHistory: vi.fn(async () => {}),
  repoPathsExist: vi.fn(async (p: string[]) => p.map(() => true)),
  emitSync: vi.fn(),
}));
vi.mock("../tauri", () => tauri);

const h = await import("./repoHistory");

describe("repo history across windows", () => {
  it("replays this window's unsaved changes over another window's save", async () => {
    await h.loadRepoHistory();
    vi.useFakeTimers();
    await h.setFavoriteRepo("/mine", true, "mine"); // pending save
    h.applyRemoteHistory({ recent: [{ path: "/theirs", name: "theirs", last_opened: 5 }], favorites: ["/theirs"], groups: [] });
    const now = get(h.repoHistory);
    expect(now.favorites).toEqual(["/theirs", "/mine"]);
    expect(now.recent.map((r) => r.path)).toEqual(["/theirs", "/mine"]);
    vi.advanceTimersByTime(1000);
    vi.useRealTimers();
    expect(tauri.saveRepoHistory).toHaveBeenLastCalledWith(now);
  });
});
