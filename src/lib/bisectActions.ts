/** Bisect actions for the graph menu, the operation banner and the palette. */
import { get } from "svelte/store";
import * as tauri from "./tauri";
import { activeRepoPath } from "./stores/repos";
import { refreshAll } from "./stores/graph";
import { bisectState, operationBusy, refreshOperation } from "./stores/operation";
import { toast, toastError } from "./stores/toasts";
import type { BisectInfo, CommandResult } from "./types/git";

/** Commit → its bisect mark, for the graph badges. */
export function bisectMarks(info: BisectInfo | null): Map<string, "good" | "bad" | "skip"> {
  const marks = new Map<string, "good" | "bad" | "skip">();
  if (!info) return marks;
  for (const o of info.good) marks.set(o, "good");
  for (const o of info.skipped) marks.set(o, "skip");
  if (info.bad) marks.set(info.bad, "bad");
  return marks;
}

/** One-line progress for the banner. */
export function bisectProgress(info: BisectInfo): string {
  if (info.first_bad) return "Found the first bad commit";
  if (!info.bad && info.good.length === 0) return `Mark a ${info.term_bad} and a ${info.term_good} commit to begin`;
  if (!info.bad) return `Mark a ${info.term_bad} commit to begin`;
  if (info.good.length === 0) return `Mark a ${info.term_good} commit to begin`;
  const left = info.remaining ?? 0;
  const steps = info.steps ?? 0;
  return `${left} candidate${left === 1 ? "" : "s"} left, about ${steps} step${steps === 1 ? "" : "s"}`;
}

async function run(label: string, op: (path: string) => Promise<CommandResult>) {
  const path = get(activeRepoPath);
  if (!path || get(operationBusy)) return;
  operationBusy.set(label);
  try {
    const res = await op(path);
    await refreshAll(path);
    await refreshOperation(path);
    if (!res.success) {
      toastError(`${label} failed`, res.message.trim() || "git bisect failed");
      return;
    }
    const info = get(bisectState);
    if (info?.first_bad) {
      toast("success", `First bad commit: ${info.first_bad.slice(0, 7)} ${info.first_bad_subject ?? ""}`, {
        title: "Bisect",
        duration: 0,
      });
    }
  } catch (err) {
    toastError(`${label} failed`, err);
  } finally {
    operationBusy.set(null);
  }
}

export const startBisect = (bad: string | null, good: string | null) =>
  run("Start bisect", (p) => tauri.bisectStart(p, bad, good));

export const markBisect = (verdict: "good" | "bad" | "skip", rev?: string) =>
  run(`Mark ${verdict}`, (p) => tauri.bisectMark(p, verdict, rev));
