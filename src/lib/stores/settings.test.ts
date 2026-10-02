import { beforeEach, describe, expect, it, vi } from "vitest";
import { get } from "svelte/store";

const tauri = vi.hoisted(() => ({
  loadSettings: vi.fn(async () => ({ context_lines: 3, graph_hide_remotes: false })),
  saveSettings: vi.fn(async () => {}),
  loadRepoSettings: vi.fn(async () => ({ "/a": { context_lines: 9, theme: "light" }, "/b": { bogus: 1 } })),
  saveRepoSettings: vi.fn(async () => {}),
  setDiffReadDefaults: vi.fn(),
}));
vi.mock("../tauri", () => tauri);
vi.stubGlobal("document", {
  documentElement: { setAttribute: vi.fn(), style: { setProperty: vi.fn(), removeProperty: vi.fn() } },
});

const s = await import("./settings");
const { activeRepoPath } = await import("./repos");
const { dueRepos } = await import("./autofetch");

describe("per-repository settings store", () => {
  beforeEach(async () => {
    activeRepoPath.set(null);
    await s.loadSettings();
  });

  it("loads overrides, dropping keys that can't be overridden", () => {
    expect(get(s.repoOverrides)).toEqual({ "/a": { context_lines: 9 } });
  });

  it("serves the active repository's effective settings", () => {
    expect(get(s.settings).context_lines).toBe(3);
    activeRepoPath.set("/a");
    expect(get(s.settings).context_lines).toBe(9);
    expect(get(s.globalSettings).context_lines).toBe(3);
  });

  it("writes overridden keys to the override and the rest globally", () => {
    activeRepoPath.set("/a");
    s.updateSettings({ context_lines: 20, graph_hide_remotes: true });
    expect(get(s.repoOverrides)["/a"]).toEqual({ context_lines: 20 });
    expect(get(s.globalSettings).context_lines).toBe(3);
    expect(get(s.globalSettings).graph_hide_remotes).toBe(true);
    // The Settings screen always edits global values.
    s.updateGlobalSettings({ context_lines: 4 });
    expect(get(s.globalSettings).context_lines).toBe(4);
    expect(get(s.settings).context_lines).toBe(20);
  });

  it("clears overrides back to the global value", () => {
    s.setRepoOverride("/b", "tab_size", 8);
    s.clearRepoOverride("/a", "context_lines");
    expect(get(s.repoOverrides)).toEqual({ "/b": { tab_size: 8 } });
    s.clearRepoOverrides("/b");
    expect(get(s.repoOverrides)).toEqual({});
  });
});

describe("auto-fetch schedule", () => {
  it("fetches each repository on its own interval", () => {
    const last = new Map<string, number>();
    const interval = (p: string) => ({ "/fast": 60, "/slow": 600, "/off": 0 })[p] ?? 0;
    const paths = ["/fast", "/slow", "/off"];
    expect(dueRepos(paths, 0, interval, last)).toEqual([]); // first sight waits a full interval
    expect(dueRepos(paths, 59_000, interval, last)).toEqual([]);
    expect(dueRepos(paths, 60_000, interval, last)).toEqual(["/fast"]);
    last.set("/fast", 60_000);
    expect(dueRepos(paths, 600_000, interval, last)).toEqual(["/fast", "/slow"]);
  });
});
