/**
 * Built-in command palette providers: open tabs, branch checkout and
 * recent/favorite repositories. Installed once by AppShell.
 */
import { get } from "svelte/store";
import { registerPaletteProvider, type PaletteItem } from "./palette";
import { openRepos, activeRepoPath, addRepo } from "./stores/repos";
import { currentView } from "./stores/ui";
import { branches, refreshAll } from "./stores/graph";
import { repoHistory, missingRepoPaths } from "./stores/repoHistory";
import { trackOperation } from "./stores/operations";
import { toast, toastError } from "./stores/toasts";
import * as tauri from "./tauri";
import type { BranchInfo } from "./types/git";

async function checkout(repoPath: string, branch: BranchInfo) {
  try {
    const result = await trackOperation(repoPath, "checkout", `Checking out ${branch.name}…`, () =>
      branch.is_remote
        ? tauri.checkoutRemoteBranch(repoPath, branch.name)
        : tauri.checkoutBranch(repoPath, branch.name),
    );
    await refreshAll(repoPath);
    if (result.success) toast("success", `Switched to ${branch.is_remote ? branch.name.replace(/^[^/]+\//, "") : branch.name}`);
    else toastError("Checkout failed", result.message);
  } catch (err) {
    toastError("Checkout failed", err);
  }
}

export async function openRepoPath(path: string) {
  if (get(openRepos).has(path)) {
    activeRepoPath.set(path);
    currentView.set("repos");
    return;
  }
  try {
    addRepo(await tauri.openRepo(path));
  } catch (err) {
    toastError("Open repository failed", err);
  }
}

export function installBuiltinPaletteProviders(): () => void {
  const unsubs = [
    registerPaletteProvider({
      id: "tabs",
      priority: 2,
      getItems: () => {
        const active = get(activeRepoPath);
        return [...get(openRepos).values()]
          .filter((info) => info.path !== active)
          .map(
            (info): PaletteItem => ({
              id: `tab:${info.path}`,
              label: `Switch to ${info.name}`,
              category: "Tab",
              detail: info.head_name ? `${info.head_name} · ${info.path}` : info.path,
              keywords: "tab repository",
              run: () => {
                activeRepoPath.set(info.path);
                currentView.set("repos");
              },
            }),
          );
      },
    }),

    registerPaletteProvider({
      id: "branches",
      priority: 1,
      getItems: (query) => {
        const repoPath = get(activeRepoPath);
        // Branch lists can be long; only offer them once the user types.
        if (!repoPath || get(currentView) !== "repos" || !query.trim()) return [];
        const list = get(branches);
        const localNames = new Set(list.filter((b) => !b.is_remote).map((b) => b.name));
        return list
          .filter((b) => !b.is_head && !b.name.endsWith("/HEAD"))
          // A remote whose local counterpart exists would just check out the local one.
          .filter((b) => !b.is_remote || !localNames.has(b.name.replace(/^[^/]+\//, "")))
          .map(
            (b): PaletteItem => ({
              id: `branch:${b.name}`,
              label: `Checkout ${b.name}`,
              category: b.is_remote ? "Remote branch" : "Branch",
              detail: b.last_commit_summary,
              keywords: "checkout switch branch",
              run: () => checkout(repoPath, b),
            }),
          );
      },
    }),

    registerPaletteProvider({
      id: "recent-repos",
      priority: 0,
      getItems: () => {
        const h = get(repoHistory);
        const open = get(openRepos);
        const missing = get(missingRepoPaths);
        const seen = new Set<string>();
        const items: PaletteItem[] = [];
        const add = (path: string, name: string, favorite: boolean) => {
          if (seen.has(path) || open.has(path)) return;
          seen.add(path);
          const isMissing = missing.has(path);
          items.push({
            id: `repo:${path}`,
            label: `Open ${name}${isMissing ? " (missing)" : ""}`,
            category: favorite ? "Favorite" : "Recent",
            detail: path,
            keywords: "open recent repository favorite",
            disabled: isMissing,
            run: () => openRepoPath(path),
          });
        };
        for (const path of h.favorites) {
          add(path, h.recent.find((r) => r.path === path)?.name ?? path.split(/[\\/]/).filter(Boolean).pop() ?? path, true);
        }
        for (const r of h.recent) if (r.last_opened > 0) add(r.path, r.name, false);
        return items;
      },
    }),
  ];
  return () => unsubs.forEach((fn) => fn());
}
