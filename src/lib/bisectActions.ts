/** Bisect actions for the graph menu, the operation banner and the palette. */
import { get } from "svelte/store";
import * as tauri from "./tauri";
import { activeRepoPath } from "./stores/repos";
import { refreshAll } from "./stores/graph";
import { bisectState, operationBusy, refreshOperation } from "./stores/operation";
import { toast, toastError } from "./stores/toasts";
import { tr } from "./i18n";
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
  if (info.first_bad) return tr("bisect.foundFirstBad");
  if (!info.bad && info.good.length === 0) return tr("bisect.markBoth", { bad: info.term_bad, good: info.term_good });
  if (!info.bad) return tr("bisect.markOne", { term: info.term_bad });
  if (info.good.length === 0) return tr("bisect.markOne", { term: info.term_good });
  const left = info.remaining ?? 0;
  const steps = tr("bisect.steps", { count: info.steps ?? 0 });
  return tr("bisect.candidatesLeft", { count: left, steps });
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
      toastError(tr("operation.failed", { label }), res.message.trim() || tr("bisect.gitFailed"));
      return;
    }
    const info = get(bisectState);
    if (info?.first_bad) {
      toast("success", tr("bisect.firstBad", { oid: info.first_bad.slice(0, 7), subject: info.first_bad_subject ?? "" }), {
        title: tr("bisect.title"),
        duration: 0,
      });
    }
  } catch (err) {
    toastError(tr("operation.failed", { label }), err);
  } finally {
    operationBusy.set(null);
  }
}

const VERDICT_LABELS = { good: "bisect.markGood", bad: "bisect.markBad", skip: "bisect.markSkip" } as const;

export const startBisect = (bad: string | null, good: string | null) =>
  run(tr("bisect.start"), (p) => tauri.bisectStart(p, bad, good));

export const markBisect = (verdict: "good" | "bad" | "skip", rev?: string) =>
  run(tr(VERDICT_LABELS[verdict]), (p) => tauri.bisectMark(p, verdict, rev));
