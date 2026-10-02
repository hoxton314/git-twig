import { writable } from "svelte/store";

/** Which "new repository" dialog is open (`null` = none). */
export const newRepoDialog = writable<"init" | "clone" | null>(null);

/** Open a dialog unless one is already open (it may be mid-clone). */
export function openNewRepoDialog(mode: "init" | "clone") {
  newRepoDialog.update((current) => current ?? mode);
}

/**
 * Folder name `git clone` would pick for `url`: the last path component
 * without a trailing `.git` (`https://h/o/repo.git` → `repo`,
 * `git@h:o/repo` → `repo`, `/srv/x/repo.git/` → `repo`).
 */
export function cloneFolderName(url: string): string {
  let u = url.trim().replace(/[\\/]+$/, "");
  u = u.replace(/\.git$/i, "").replace(/[\\/]+$/, "");
  const last = u.split(/[\\/:]/).pop() ?? "";
  return last.replace(/[^\w.\-]+/g, "-").replace(/^[.-]+/, "");
}

/** Join a folder and a child name with the folder's own separator. */
export function joinPath(dir: string, name: string): string {
  const sep = dir.includes("\\") && !dir.includes("/") ? "\\" : "/";
  return dir.replace(/[\\/]+$/, "") + sep + name;
}
