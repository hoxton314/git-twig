/**
 * Save commits / working changes as patch files and apply patch files, with
 * the native file dialogs and a confirmation preview before applying.
 */
import { open, save, ask } from "@tauri-apps/plugin-dialog";
import * as tauri from "./tauri";
import type { PatchInfo } from "./types/git";
import { refreshAll } from "./stores/graph";
import { refreshOperation } from "./stores/operation";
import { toast, toastError } from "./stores/toasts";

const PATCH_FILTERS = [
  { name: "Patches", extensions: ["patch", "diff", "mbox", "eml"] },
  { name: "All files", extensions: ["*"] },
];

/** File-name-safe slug of a commit subject, like `git format-patch`. */
export function patchFileName(short: string, subject: string): string {
  const slug = subject
    .toLowerCase()
    .replace(/[^a-z0-9._]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 52);
  return `${short}${slug ? `-${slug}` : ""}.patch`;
}

/**
 * Save commits as patches. `oids` are in graph order (newest first) and are
 * written oldest first. `folder`: one file per commit in a chosen folder;
 * otherwise a single file (an mbox when several).
 */
export async function savePatchesAction(
  path: string,
  oids: string[],
  opts: { folder?: boolean; defaultName?: string } = {},
) {
  if (oids.length === 0) return;
  const ordered = [...oids].reverse();
  const n = ordered.length;
  const target = opts.folder
    ? await open({ directory: true, multiple: false, title: `Save ${n} patch files to folder` })
    : await save({
        title: n === 1 ? "Save Commit as Patch" : `Save ${n} Commits as One Patch File`,
        defaultPath: opts.defaultName ?? (n === 1 ? `${ordered[0].slice(0, 7)}.patch` : `${n}-commits.mbox`),
        filters: PATCH_FILTERS,
      });
  if (!target || Array.isArray(target)) return;
  try {
    const files = await tauri.formatPatches(path, ordered, target, !opts.folder);
    toast(
      "success",
      opts.folder ? `Saved ${files.length} patch file${files.length === 1 ? "" : "s"} to ${target}` : `Saved ${n === 1 ? "patch" : `${n} commits`} to ${target}`,
    );
  } catch (err) {
    toastError("Save Patch Failed", err);
  }
}

export async function saveWorkingPatchAction(path: string) {
  const target = await save({ title: "Save Working Changes as Patch", defaultPath: "changes.patch", filters: PATCH_FILTERS });
  if (!target) return;
  try {
    await tauri.saveWorkingPatch(path, target);
    toast("success", `Saved working changes to ${target}`);
  } catch (err) {
    toastError("Save Patch Failed", err);
  }
}

/** Confirmation text for applying `info` (exported for tests). */
export function applySummary(file: string, info: PatchInfo): string {
  const name = file.split(/[\\/]/).pop() ?? file;
  const lines: string[] = [];
  if (info.kind === "mbox") {
    const n = info.commits.length;
    lines.push(`Apply ${n} commit${n === 1 ? "" : "s"} from ${name} onto the current branch (git am)?`, "");
    const shown = info.commits.slice(0, 10);
    lines.push(...shown.map((s) => `• ${s}`));
    if (n > shown.length) lines.push(`… and ${n - shown.length} more`);
  } else if (info.applies_cleanly) {
    lines.push(`Apply ${name} to the working tree?`);
  } else {
    lines.push(
      `${name} doesn't apply cleanly${info.check_error ? `:\n${info.check_error.split("\n")[0]}` : "."}`,
      "",
      "Apply it with a three-way merge? Conflicting files are left with conflict markers to resolve.",
    );
  }
  if (info.stat) lines.push("", info.stat);
  return lines.join("\n");
}

export async function applyPatchAction(path: string) {
  const file = await open({ title: "Apply Patch File", multiple: false, directory: false, filters: PATCH_FILTERS });
  if (!file || Array.isArray(file)) return;
  let info: PatchInfo;
  try {
    info = await tauri.inspectPatch(path, file);
  } catch (err) {
    toastError("Cannot Apply Patch", err);
    return;
  }
  const ok = await ask(applySummary(file, info), {
    title: "Apply Patch",
    kind: info.applies_cleanly === false ? "warning" : "info",
    okLabel: info.applies_cleanly === false ? "Apply (3-way)" : "Apply",
    cancelLabel: "Cancel",
  });
  if (!ok) return;
  try {
    const r = await tauri.applyPatch(path, file);
    await refreshAll(path);
    if (r.mode === "am") await refreshOperation(path);
    if (r.success) {
      toast("success", r.mode === "am" ? `Applied ${info.commits.length} commit(s) from the patch` : "Applied the patch to the working tree");
    } else if (r.conflicted) {
      toast(
        "warning",
        r.mode === "am"
          ? "Applying the patch stopped with conflicts. Resolve them, then continue, skip or abort from the banner above the graph."
          : "The patch was applied with conflicts. Resolve the conflicted files in the changes list.",
        { title: "Conflicts", duration: 0 },
      );
    } else {
      toast("error", r.message.trim() || "git could not apply the patch.", { title: "Apply Patch Failed", duration: 0 });
    }
  } catch (err) {
    await refreshAll(path);
    toastError("Apply Patch Failed", err);
  }
}
