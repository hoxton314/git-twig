import { beforeEach, describe, expect, it, vi } from "vitest";
import { get, writable } from "svelte/store";

const tauri = vi.hoisted(() => ({ listReposInDir: vi.fn() }));
vi.mock("../tauri", () => tauri);
const settingsStore = vi.hoisted(() => ({ value: null as unknown }));
vi.mock("./settings", async () => {
  const { writable } = await import("svelte/store");
  const s = writable({ default_repo_dir: "/code" as string | null });
  settingsStore.value = s;
  return { settings: s };
});

const { scannedRepos, refreshScannedRepos } = await import("./scannedRepos");
const settings = settingsStore.value as ReturnType<typeof writable<{ default_repo_dir: string | null }>>;
const repo = (name: string) => ({ path: `/code/${name}`, name, head_name: "main" });

beforeEach(() => {
  tauri.listReposInDir.mockReset();
  settings.set({ default_repo_dir: "/code" });
});

describe("refreshScannedRepos", () => {
  it("re-uses a fresh scan unless forced", async () => {
    tauri.listReposInDir.mockResolvedValue([repo("a")]);
    await refreshScannedRepos(true);
    await refreshScannedRepos();
    expect(tauri.listReposInDir).toHaveBeenCalledTimes(1);
    await refreshScannedRepos(true);
    expect(tauri.listReposInDir).toHaveBeenCalledTimes(2);
    expect(get(scannedRepos).map((r) => r.name)).toEqual(["a"]);
  });

  it("caches a failed scan instead of retrying on every call", async () => {
    tauri.listReposInDir.mockRejectedValue(new Error("gone"));
    await refreshScannedRepos(true);
    await refreshScannedRepos();
    await refreshScannedRepos();
    expect(tauri.listReposInDir).toHaveBeenCalledTimes(1);
    expect(get(scannedRepos)).toEqual([]);
  });

  it("drops results of a scan whose folder was changed meanwhile", async () => {
    let release!: (v: unknown) => void;
    tauri.listReposInDir.mockReturnValueOnce(new Promise((r) => (release = r)));
    const pending = refreshScannedRepos(true);
    settings.set({ default_repo_dir: null });
    await refreshScannedRepos(true); // clears for "no folder"
    release([repo("stale")]);
    await pending;
    expect(get(scannedRepos)).toEqual([]);
  });
});
