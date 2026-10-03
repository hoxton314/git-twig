import { describe, expect, it, vi } from "vitest";
import type { BranchInfo } from "./types/git";

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn(async () => true),
  requestPermission: vi.fn(),
  sendNotification: vi.fn(),
}));
vi.mock("./stores/toasts", () => ({ toast: vi.fn() }));

const { newUpstreamCommits, describe: describeEvents } = await import("./notify");

const b = (name: string, behind: number, over: Partial<BranchInfo> = {}) =>
  ({ name, is_remote: false, is_head: false, upstream: `origin/${name}`, ahead: 0, behind, ...over }) as BranchInfo;

describe("new upstream commits", () => {
  const before = [b("main", 0, { is_head: true }), b("dev", 2), b("origin/main", 0, { is_remote: true, upstream: null })];

  it("reports branches whose upstream gained commits", () => {
    const after = [b("main", 3, { is_head: true }), b("dev", 4), b("local", 5, { upstream: null })];
    expect(newUpstreamCommits(before, after, "current")).toEqual([{ branch: "main", upstream: "origin/main", count: 3 }]);
    expect(newUpstreamCommits(before, after, "all").map((e) => [e.branch, e.count])).toEqual([["main", 3], ["dev", 2]]);
    expect(newUpstreamCommits(before, after, "off")).toEqual([]);
  });

  it("ignores branches that are new, retargeted or no further behind", () => {
    const after = [
      b("main", 0, { is_head: true }),
      b("dev", 9, { upstream: "fork/dev" }),
      b("fresh", 7),
    ];
    expect(newUpstreamCommits(before, after, "all")).toEqual([]);
  });

  it("describes events one per line", () => {
    expect(describeEvents([{ branch: "main", upstream: "origin/main", count: 1 }, { branch: "dev", upstream: "origin/dev", count: 2 }])).toBe(
      "main: 1 new commit on origin/main\ndev: 2 new commits on origin/dev",
    );
  });
});
