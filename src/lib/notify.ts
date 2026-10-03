/**
 * Notifications for new upstream commits after an auto-fetch: system
 * notifications while the window is in the background, a toast when it's
 * focused. Desktop notifications can't report clicks (plugin limitation),
 * so the message says which repository and branch to look at.
 */
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import type { BranchInfo } from "./types/git";
import { toast } from "./stores/toasts";

export type NotifyMode = "off" | "current" | "all";

export interface NewCommits {
  branch: string;
  upstream: string;
  count: number;
}

/**
 * Branches whose upstream gained commits between two snapshots (behind
 * count went up). `current` limits it to the checked-out branch.
 */
export function newUpstreamCommits(before: BranchInfo[], after: BranchInfo[], mode: NotifyMode): NewCommits[] {
  if (mode === "off") return [];
  const prev = new Map(before.filter((b) => !b.is_remote).map((b) => [b.name, b]));
  const out: NewCommits[] = [];
  for (const b of after) {
    if (b.is_remote || !b.upstream) continue;
    if (mode === "current" && !b.is_head) continue;
    const old = prev.get(b.name);
    if (!old || old.upstream !== b.upstream) continue;
    const count = b.behind - old.behind;
    if (count > 0) out.push({ branch: b.name, upstream: b.upstream, count });
  }
  return out;
}

/** One line per branch: "main: 3 new commits on origin/main". */
export function describe(events: NewCommits[]): string {
  return events
    .map((e) => `${e.branch}: ${e.count} new commit${e.count === 1 ? "" : "s"} on ${e.upstream}`)
    .join("\n");
}

let permission: Promise<boolean> | null = null;

async function allowed(): Promise<boolean> {
  permission ??= (async () => {
    try {
      if (await isPermissionGranted()) return true;
      return (await requestPermission()) === "granted";
    } catch {
      return false;
    }
  })();
  return permission;
}

/** Tell the user about new upstream commits in `repoName`. */
export async function announceNewCommits(repoName: string, events: NewCommits[]): Promise<void> {
  if (events.length === 0) return;
  const body = describe(events);
  if (typeof document !== "undefined" && document.hasFocus()) {
    toast("info", body, { title: `New commits in ${repoName}` });
    return;
  }
  if (await allowed()) {
    try {
      sendNotification({ title: `New commits in ${repoName}`, body });
      return;
    } catch {
      // Fall through to a toast if the system refused.
    }
  }
  toast("info", body, { title: `New commits in ${repoName}` });
}
