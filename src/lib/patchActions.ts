/**
 * Save commits / working changes as patch files and apply patch files, with
 * the native file dialogs and a confirmation preview before applying.
 */
import { open, save, ask } from "@tauri-apps/plugin-dialog";
import * as tauri from "./tauri";
import type { ApplyPatchResult, PatchInfo } from "./types/git";
import { refreshAll } from "./stores/graph";
import { refreshOperation } from "./stores/operation";
import { toast, toastError } from "./stores/toasts";
import { tr } from "./i18n";

const patchFilters = () => [
  { name: tr("patches.filterPatches"), extensions: ["patch", "diff", "mbox", "eml"] },
  { name: tr("patches.filterAll"), extensions: ["*"] },
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
    ? await open({ directory: true, multiple: false, title: tr("patches.saveToFolderTitle", { count: n }) })
    : await save({
        title: n === 1 ? tr("patches.saveOneTitle") : tr("patches.saveManyTitle", { count: n }),
        defaultPath: opts.defaultName ?? (n === 1 ? `${ordered[0].slice(0, 7)}.patch` : `${n}-commits.mbox`),
        filters: patchFilters(),
      });
  if (!target || Array.isArray(target)) return;
  try {
    const files = await tauri.formatPatches(path, ordered, target, !opts.folder);
    toast(
      "success",
      opts.folder
        ? tr("patches.savedFiles", { count: files.length, target })
        : n === 1
          ? tr("patches.savedPatch", { target })
          : tr("patches.savedCommits", { count: n, target }),
    );
  } catch (err) {
    toastError(tr("patches.saveFailed"), err);
  }
}

export async function saveWorkingPatchAction(path: string) {
  const target = await save({
    title: tr("patches.saveWorkingTitle"),
    defaultPath: "changes.patch",
    filters: patchFilters(),
  });
  if (!target) return;
  try {
    await tauri.saveWorkingPatch(path, target);
    toast("success", tr("patches.savedWorking", { target }));
  } catch (err) {
    toastError(tr("patches.saveFailed"), err);
  }
}

/** Confirmation text for applying `info` (exported for tests). */
export function applySummary(file: string, info: PatchInfo): string {
  const name = file.split(/[\\/]/).pop() ?? file;
  const lines: string[] = [];
  if (info.kind === "mbox") {
    const n = info.count;
    lines.push(tr("patches.applyMboxConfirm", { count: n, name }), "");
    const shown = info.commits.slice(0, 10);
    lines.push(...shown.map((s) => `• ${s}`));
    if (n > shown.length) lines.push(tr("patches.andMore", { count: n - shown.length }));
  } else if (info.applies_cleanly) {
    lines.push(tr("patches.applyConfirm", { name }));
  } else {
    lines.push(
      info.check_error
        ? tr("patches.notCleanError", { name, error: info.check_error.split("\n")[0] })
        : tr("patches.notClean", { name }),
      "",
      tr("patches.threeWayConfirm"),
    );
  }
  if (info.stat) lines.push("", info.stat);
  return lines.join("\n");
}

/** What to tell the user when applying didn't finish (exported for tests). */
export function failureText(r: ApplyPatchResult): string {
  const why = r.message.trim().split("\n").filter(Boolean).slice(-3).join("\n");
  const detail = why ? `\n\n${why}` : "";
  if (r.stopped) {
    return tr(r.conflicted ? "patches.seriesConflicts" : "patches.seriesStopped") + detail;
  }
  if (r.conflicted) return tr("patches.appliedWithConflicts") + detail;
  return tr("patches.gitCouldNotApply") + detail;
}

export async function applyPatchAction(path: string) {
  const file = await open({
    title: tr("patches.applyFileTitle"),
    multiple: false,
    directory: false,
    filters: patchFilters(),
  });
  if (!file || Array.isArray(file)) return;
  let info: PatchInfo;
  try {
    info = await tauri.inspectPatch(path, file);
  } catch (err) {
    toastError(tr("patches.cannotApply"), err);
    return;
  }
  const ok = await ask(applySummary(file, info), {
    title: tr("patches.applyTitle"),
    kind: info.applies_cleanly === false ? "warning" : "info",
    okLabel: info.applies_cleanly === false ? tr("patches.apply3way") : tr("common.apply"),
    cancelLabel: tr("common.cancel"),
  });
  if (!ok) return;
  try {
    const r = await tauri.applyPatch(path, file);
    await refreshAll(path);
    if (r.mode === "am") await refreshOperation(path);
    if (r.success) {
      toast(
        "success",
        r.mode === "am"
          ? tr("patches.appliedCommits", { count: info.count })
          : tr("patches.appliedWorking"),
      );
    } else {
      toast("warning", failureText(r), {
        title: r.conflicted ? tr("commits.conflicts") : tr("patches.applyStopped"),
        duration: 0,
      });
    }
  } catch (err) {
    await refreshAll(path);
    toastError(tr("patches.applyFailed"), err);
  }
}
