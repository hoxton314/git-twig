import { beforeEach, describe, expect, it, vi } from "vitest";
import { get } from "svelte/store";

const tauri = vi.hoisted(() => ({
  loadSettings: vi.fn(async () => ({ context_lines: 3, graph_hide_remotes: false })),
  saveSettings: vi.fn(async () => {}),
  loadRepoSettings: vi.fn(async () => ({ "/a": { context_lines: 9, theme: "light" }, "/b": { bogus: 1 } })),
  saveRepoSettings: vi.fn(async () => {}),
  setDiffReadDefaults: vi.fn(),
  emitSync: vi.fn(),
}));
vi.mock("../tauri", () => tauri);
const root = vi.hoisted(() => ({ setAttribute: vi.fn(), style: { setProperty: vi.fn(), removeProperty: vi.fn() } }));
vi.stubGlobal("document", { documentElement: root });

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

  it("flushes pending override changes (e.g. before quitting)", async () => {
    tauri.saveRepoSettings.mockClear();
    s.setRepoOverride("/a", "tab_size", 2);
    await s.flushSettings();
    expect(tauri.saveRepoSettings).toHaveBeenCalledWith({ "/a": { context_lines: 9, tab_size: 2 } });
  });

  it("never saves overrides it couldn't read", async () => {
    tauri.loadRepoSettings.mockRejectedValueOnce(new Error("EACCES"));
    await s.loadSettings();
    tauri.saveRepoSettings.mockClear();
    s.setRepoOverride("/z", "tab_size", 2);
    await s.flushSettings();
    vi.useFakeTimers();
    vi.advanceTimersByTime(1000);
    vi.useRealTimers();
    expect(tauri.saveRepoSettings).not.toHaveBeenCalled();
  });

  it("takes another window's saved settings without saving or echoing them", async () => {
    vi.useFakeTimers();
    tauri.saveSettings.mockClear();
    tauri.emitSync.mockClear();
    s.applyRemoteSettings("settings", { ...get(s.globalSettings), tab_size: 7 });
    s.applyRemoteSettings("repo-settings", { "/r": { context_lines: 1 } });
    vi.advanceTimersByTime(1000);
    vi.useRealTimers();
    expect(get(s.globalSettings).tab_size).toBe(7);
    expect(get(s.repoOverrides)).toEqual({ "/r": { context_lines: 1 } });
    expect(tauri.saveSettings).not.toHaveBeenCalled();
    expect(tauri.emitSync).not.toHaveBeenCalled();
  });

  it("keeps this window's unsaved change when another window's save arrives", () => {
    vi.useFakeTimers();
    s.updateGlobalSettings({ tab_size: 3 }); // not saved yet (debounced)
    s.applyRemoteSettings("settings", { ...get(s.globalSettings), tab_size: 8, context_lines: 11 });
    expect(get(s.globalSettings).tab_size).toBe(3);
    expect(get(s.globalSettings).context_lines).toBe(11);
    s.setRepoOverride("/q", "tab_size", 2);
    s.applyRemoteSettings("repo-settings", { "/other": { tab_size: 6 } });
    expect(get(s.repoOverrides)).toEqual({ "/other": { tab_size: 6 }, "/q": { tab_size: 2 } });
    vi.advanceTimersByTime(1000); // the pending saves write the merged values
    vi.useRealTimers();
    // Once saved, a later remote value applies as is.
    s.applyRemoteSettings("settings", { ...get(s.globalSettings), tab_size: 5 });
    expect(get(s.globalSettings).tab_size).toBe(5);
  });

  it("applies a custom theme's colours over its base and removes them on switch", () => {
    root.setAttribute.mockClear();
    root.style.setProperty.mockClear();
    root.style.removeProperty.mockClear();
    s.updateGlobalSettings({
      custom_themes: [{ id: "x", name: "X", base: "light", colors: { "--color-bg": "#fafafa", "--color-border": "url(evil)" } }],
      theme: "custom:x",
    });
    expect(root.setAttribute).toHaveBeenLastCalledWith("data-theme", "light");
    expect(root.style.setProperty).toHaveBeenCalledWith("--color-bg", "#fafafa");
    expect(root.style.setProperty).not.toHaveBeenCalledWith("--color-border", "url(evil)");
    s.updateGlobalSettings({ theme: "dark" });
    expect(root.setAttribute).toHaveBeenLastCalledWith("data-theme", "dark");
    expect(root.style.removeProperty).toHaveBeenCalledWith("--color-bg");
    // A theme id that no longer exists falls back to dark.
    s.updateGlobalSettings({ theme: "custom:gone" });
    expect(root.setAttribute).toHaveBeenLastCalledWith("data-theme", "dark");
  });

  it("merges list edits made from the latest value over another window's save", () => {
    vi.useFakeTimers();
    const theme = (id: string) => ({ id, name: id, base: "dark" as const, colors: {} });
    s.updateGlobalSettingsWith((cur) => ({ custom_themes: [...(cur.custom_themes ?? []), theme("mine")] }));
    s.applyRemoteSettings("settings", { ...get(s.globalSettings), custom_themes: [theme("theirs")] });
    expect(get(s.globalSettings).custom_themes.map((t) => t.id)).toEqual(["theirs", "mine"]);
    vi.advanceTimersByTime(1000);
    vi.useRealTimers();
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
