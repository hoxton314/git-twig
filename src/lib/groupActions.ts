/** Actions on repository groups that open tabs (the list logic is in repoGroups.ts). */
import { get } from "svelte/store";
import * as tauri from "./tauri";
import { openRepos, addRepo, activeRepoPath } from "./stores/repos";
import { currentView } from "./stores/ui";
import { findGroup, refreshMissingPaths } from "./stores/repoHistory";
import { toast } from "./stores/toasts";

/**
 * Open every repository of a group as a tab (already open ones are kept)
 * and switch to the group's first repository. Missing folders are skipped
 * and reported.
 */
export async function openRepoGroup(id: string): Promise<void> {
  const group = findGroup(id);
  if (!group) return;
  if (group.paths.length === 0) {
    toast("info", `“${group.name}” has no repositories yet.`);
    return;
  }
  const failed: string[] = [];
  let first: string | null = null;
  for (const path of group.paths) {
    if (get(openRepos).has(path)) {
      first ??= path;
      continue;
    }
    try {
      addRepo(await tauri.openRepo(path));
      first ??= path;
    } catch {
      failed.push(path);
    }
  }
  if (first) {
    activeRepoPath.set(first);
    currentView.set("repos");
  }
  if (failed.length > 0) {
    refreshMissingPaths();
    toast("warning", `Couldn't open ${failed.length} of ${group.paths.length} repositories in “${group.name}”:\n${failed.join("\n")}`, {
      title: "Open group",
      duration: 0,
    });
  }
}
