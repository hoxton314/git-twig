import { beforeEach, describe, expect, it, vi } from "vitest";

const tauri = vi.hoisted(() => ({ cherryPickCommits: vi.fn(), cherryPickCommit: vi.fn() }));
const toasts = vi.hoisted(() => ({ toast: vi.fn(), toastError: vi.fn() }));
vi.mock("./tauri", () => tauri);
vi.mock("./stores/toasts", () => toasts);
vi.mock("./stores/graph", () => ({ refreshAll: vi.fn(async () => {}) }));
vi.mock("./stores/settings", async () => ({ settings: (await import("svelte/store")).writable({ confirm_destructive_ops: false }) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ ask: vi.fn(async () => true) }));

const { cherryPickRangeAction } = await import("./commitActions");

const result = (over: object) => ({
  success: true,
  message: "",
  conflicted: false,
  previous_head: "base",
  previous_branch: "main",
  stash_oid: null,
  picked: 0,
  skipped: [],
  empty: false,
  ...over,
});

describe("cherryPickRangeAction", () => {
  beforeEach(() => vi.clearAllMocks());

  it("applies graph-ordered (newest first) commits oldest first", async () => {
    tauri.cherryPickCommits.mockResolvedValue(result({ picked: 2, skipped: ["c"] }));
    await cherryPickRangeAction("/r", ["a", "b", "c"]);
    expect(tauri.cherryPickCommits).toHaveBeenCalledWith("/r", ["c", "b", "a"]);
    const [kind, text, opts] = toasts.toast.mock.calls[0];
    expect(kind).toBe("success");
    expect(text).toBe("Cherry-picked 2 commits (1 already on HEAD, skipped)");
    expect(opts.action.label).toBe("Undo");
  });

  it("reports how far it got on conflicts", async () => {
    tauri.cherryPickCommits.mockResolvedValue(result({ success: false, conflicted: true, picked: 1 }));
    await cherryPickRangeAction("/r", ["a", "b", "c"]);
    const [kind, text] = toasts.toast.mock.calls[0];
    expect(kind).toBe("warning");
    expect(text).toContain("after 1 of 3 commits");
  });

  it("explains a stop on an already-applied commit", async () => {
    tauri.cherryPickCommits.mockResolvedValue(result({ success: false, empty: true, picked: 1 }));
    await cherryPickRangeAction("/r", ["a", "b", "c"]);
    const [kind, text, opts] = toasts.toast.mock.calls[0];
    expect(kind).toBe("warning");
    expect(opts.title).toBe("Nothing to apply");
    expect(text).toContain("already on HEAD");
  });

  it("reports a failure part-way with what was applied and an Undo", async () => {
    tauri.cherryPickCommits.mockResolvedValue(
      result({ success: false, picked: 2, message: "error: your local changes would be overwritten\n" }),
    );
    await cherryPickRangeAction("/r", ["a", "b", "c"]);
    expect(toasts.toast).toHaveBeenCalledTimes(1);
    const [kind, text, opts] = toasts.toast.mock.calls[0];
    expect(kind).toBe("error");
    expect(text).toBe("error: your local changes would be overwritten 2 of 3 commits were applied.");
    expect(opts.action.label).toBe("Undo");
  });

  it("uses the single-commit path for one commit", async () => {
    tauri.cherryPickCommit.mockResolvedValue(result({}));
    await cherryPickRangeAction("/r", ["a"]);
    expect(tauri.cherryPickCommit).toHaveBeenCalledWith("/r", "a");
    expect(tauri.cherryPickCommits).not.toHaveBeenCalled();
  });
});
