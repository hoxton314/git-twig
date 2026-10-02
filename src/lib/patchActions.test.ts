import { beforeEach, describe, expect, it, vi } from "vitest";

const tauri = vi.hoisted(() => ({ formatPatches: vi.fn(async () => ["/out/x.mbox"]) }));
const dialog = vi.hoisted(() => ({ open: vi.fn(), save: vi.fn(), ask: vi.fn() }));
const toasts = vi.hoisted(() => ({ toast: vi.fn(), toastError: vi.fn() }));
vi.mock("./tauri", () => tauri);
vi.mock("@tauri-apps/plugin-dialog", () => dialog);
vi.mock("./stores/toasts", () => toasts);
vi.mock("./stores/graph", () => ({ refreshAll: vi.fn() }));
vi.mock("./stores/operation", () => ({ refreshOperation: vi.fn() }));

const { applySummary, patchFileName, savePatchesAction } = await import("./patchActions");

describe("patch actions", () => {
  beforeEach(() => vi.clearAllMocks());

  it("saves graph-ordered commits oldest first into one file", async () => {
    dialog.save.mockResolvedValue("/out/x.mbox");
    await savePatchesAction("/r", ["c3", "c2", "c1"]);
    expect(dialog.save.mock.calls[0][0].defaultPath).toBe("3-commits.mbox");
    expect(tauri.formatPatches).toHaveBeenCalledWith("/r", ["c1", "c2", "c3"], "/out/x.mbox", true);
    expect(toasts.toast.mock.calls[0][1]).toBe("Saved 3 commits to /out/x.mbox");
  });

  it("writes one file per commit into a chosen folder, and does nothing on cancel", async () => {
    dialog.open.mockResolvedValue("/out");
    await savePatchesAction("/r", ["b", "a"], { folder: true });
    expect(tauri.formatPatches).toHaveBeenCalledWith("/r", ["a", "b"], "/out", false);
    dialog.save.mockResolvedValue(null);
    await savePatchesAction("/r", ["a"]);
    expect(tauri.formatPatches).toHaveBeenCalledTimes(1);
  });

  it("names single-commit patches like format-patch", () => {
    expect(patchFileName("abc1234", "Fix: crash on start (#12)")).toBe("abc1234-fix-crash-on-start-12.patch");
    expect(patchFileName("abc1234", "!!!")).toBe("abc1234.patch");
  });

  it("summarises what applying will do", () => {
    const mbox = applySummary("/p/series.mbox", {
      kind: "mbox", commits: ["one", "two"], stat: " a | 1 +", applies_cleanly: null, check_error: null,
    });
    expect(mbox).toContain("Apply 2 commits from series.mbox");
    expect(mbox).toContain("• two");
    const conflict = applySummary("C:\\p\\x.diff", {
      kind: "diff", commits: [], stat: "", applies_cleanly: false, check_error: "error: patch failed: a:2\nmore",
    });
    expect(conflict).toContain("x.diff doesn't apply cleanly:\nerror: patch failed: a:2");
    expect(conflict).not.toContain("more");
    expect(conflict).toContain("three-way merge");
  });
});
