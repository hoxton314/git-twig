import { tr } from "../../lib/i18n";

/** Short relative time for a unix timestamp (seconds). */
export function relativeTime(ts: number): string {
  const date = new Date(ts * 1000);
  const diff = Math.max(0, Date.now() - date.getTime());
  const minutes = Math.floor(diff / 60000);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  if (minutes < 1) return tr("history.time.justNow");
  if (minutes < 60) return tr("history.time.minutesAgo", { count: minutes });
  if (hours < 24) return tr("history.time.hoursAgo", { count: hours });
  if (days < 30) return tr("history.time.daysAgo", { count: days });
  if (days < 365) return tr("history.time.monthsAgo", { count: Math.floor(days / 30) });
  return date.toLocaleDateString();
}

export function fullDate(ts: number): string {
  return new Date(ts * 1000).toLocaleString();
}

export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}
