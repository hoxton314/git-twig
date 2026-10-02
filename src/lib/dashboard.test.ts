import { describe, expect, it } from "vitest";
import { dashboardPaths, fetchAge, runPool } from "./dashboard";
import type { RepoHistory } from "./types/git";

const history: RepoHistory = {
  recent: [
    { path: "/r1", name: "r1", last_opened: 5 },
    { path: "/named-only", name: "n", last_opened: 0 },
  ],
  favorites: ["/f1", "/r1"],
  groups: [{ id: "g", name: "G", paths: ["/g1", "/f1"] }],
};

describe("dashboard", () => {
  it("covers a group, or every known repository once", () => {
    expect(dashboardPaths("g", history, [])).toEqual(["/g1", "/f1"]);
    expect(dashboardPaths("missing", history, [])).toEqual([]);
    expect(dashboardPaths(null, history, ["/open", "/g1"])).toEqual(["/open", "/g1", "/f1", "/r1"]);
  });

  it("runs at most `limit` at a time and stops starting when cancelled", async () => {
    let running = 0;
    let peak = 0;
    const done: number[] = [];
    await runPool([1, 2, 3, 4, 5, 6], 2, async (n) => {
      running++;
      peak = Math.max(peak, running);
      await new Promise((r) => setTimeout(r, 5));
      done.push(n);
      running--;
    });
    expect(peak).toBe(2);
    expect(done.sort()).toEqual([1, 2, 3, 4, 5, 6]);

    let stop = false;
    const started: number[] = [];
    await runPool([1, 2, 3, 4], 1, async (n) => {
      started.push(n);
      if (n === 2) stop = true;
    }, () => stop);
    expect(started).toEqual([1, 2]);
    await runPool([], 3, async () => {});
  });

  it("formats fetch age", () => {
    expect(fetchAge(null)).toBe("never");
    expect(fetchAge(1000, 1000_000 + 30_000)).toBe("just now");
    expect(fetchAge(0, 7200_000)).toBe("2 h ago");
  });
});
