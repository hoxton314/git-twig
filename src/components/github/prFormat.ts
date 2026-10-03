import { get } from "svelte/store";
import { locale, tr } from "../../lib/i18n";
import type { PrState, ReviewState } from "../../lib/types/hosting";

/** "3 days ago" style relative time for ISO timestamps. */
export function relativeTime(iso: string): string {
  const time = Date.parse(iso);
  if (Number.isNaN(time)) return iso;
  const secs = Math.round((Date.now() - time) / 1000);
  const abs = Math.abs(secs);
  const units: [number, Intl.RelativeTimeFormatUnit][] = [
    [60, "second"],
    [60, "minute"],
    [24, "hour"],
    [30, "day"],
    [12, "month"],
    [Number.POSITIVE_INFINITY, "year"],
  ];
  let value = abs;
  let unit: Intl.RelativeTimeFormatUnit = "second";
  for (const [size, name] of units) {
    unit = name;
    if (value < size) break;
    value = Math.floor(value / size);
  }
  if (unit === "second" && value < 45) return tr("prs.justNow");
  return new Intl.RelativeTimeFormat(get(locale), { numeric: "always" }).format(secs >= 0 ? -value : value, unit);
}

export function reviewLabel(state: ReviewState | null): string {
  switch (state) {
    case "approved":
      return tr("prs.reviewApproved");
    case "changes_requested":
      return tr("prs.reviewChangesRequested");
    case "review_required":
      return tr("prs.reviewRequired");
    default:
      return "";
  }
}

export function stateLabel(state: PrState): string {
  return state === "open" ? tr("prs.stateOpen") : state === "merged" ? tr("prs.stateMerged") : tr("prs.stateClosed");
}

const MARK = "\u0000";

/**
 * Params that stand in for values a component wraps in markup: translate
 * with `markers("branch")`, then render `segments(text)`, putting the value
 * where a segment's `param` is set.
 */
export function markers(...names: string[]): Record<string, string> {
  return Object.fromEntries(names.map((n) => [n, `${MARK}${n}${MARK}`]));
}

export function segments(text: string): { text: string; param?: string }[] {
  return text
    .split(MARK)
    .map((s, i) => (i % 2 ? { text: "", param: s } : { text: s }))
    .filter((s) => s.param || s.text);
}
