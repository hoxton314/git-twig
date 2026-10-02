import { describe, expect, it, vi } from "vitest";
import type { BisectInfo } from "./types/git";

vi.mock("./tauri", () => ({}));
vi.mock("./stores/graph", () => ({ refreshAll: vi.fn() }));
vi.mock("./stores/operation", async () => {
  const { writable } = await import("svelte/store");
  return { bisectState: writable(null), operationBusy: writable(null), refreshOperation: vi.fn() };
});
vi.mock("./stores/toasts", () => ({ toast: vi.fn(), toastError: vi.fn() }));

const { bisectMarks, bisectProgress } = await import("./bisectActions");

const info = (over: Partial<BisectInfo> = {}): BisectInfo => ({
  term_bad: "bad",
  term_good: "good",
  bad: "b",
  good: ["g1", "g2"],
  skipped: ["s"],
  current: "c",
  current_subject: "testing this",
  remaining: 5,
  steps: 2,
  first_bad: null,
  first_bad_subject: null,
  ...over,
});

describe("bisect helpers", () => {
  it("maps commits to their marks", () => {
    const m = bisectMarks(info());
    expect(m.get("b")).toBe("bad");
    expect(m.get("g2")).toBe("good");
    expect(m.get("s")).toBe("skip");
    expect(m.has("c")).toBe(false);
    expect(bisectMarks(null).size).toBe(0);
  });

  it("describes progress, missing ends and the result", () => {
    expect(bisectProgress(info())).toBe("5 candidates left, about 2 steps");
    expect(bisectProgress(info({ remaining: 2, steps: 1 }))).toBe("2 candidates left, about 1 step");
    expect(bisectProgress(info({ good: [] }))).toBe("Mark a good commit to begin");
    expect(bisectProgress(info({ bad: null, term_bad: "new" }))).toBe("Mark a new commit to begin");
    expect(bisectProgress(info({ bad: null, good: [] }))).toBe("Mark a bad and a good commit to begin");
    expect(bisectProgress(info({ first_bad: "b" }))).toBe("Found the first bad commit");
  });
});
