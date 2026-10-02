import { describe, expect, it, vi } from "vitest";

vi.mock("../tauri", () => ({ getLfsStatus: vi.fn(async () => ({ version: null, patterns: [] })) }));

const { pruneSummary } = await import("./lfs");

describe("lfs prune summary", () => {
  it("reads git lfs prune --dry-run output", () => {
    expect(pruneSummary("2 local objects, 1 retained, done.\n1 file would be pruned (2.0 KB), done.\n")).toEqual({
      count: 1,
      size: "2.0 KB",
    });
    expect(pruneSummary("40 local objects, 3 retained\n37 files would be pruned (1.2 GB)")).toEqual({
      count: 37,
      size: "1.2 GB",
    });
    expect(pruneSummary("2 local objects, 2 retained, done.\n")).toEqual({ count: 0, size: null });
  });
});
