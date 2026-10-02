import type { PrState, ReviewState } from "../../lib/types/hosting";

/** "3 days ago" style relative time for ISO timestamps. */
export function relativeTime(iso: string): string {
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return iso;
  const secs = Math.round((Date.now() - t) / 1000);
  const abs = Math.abs(secs);
  const units: [number, string][] = [
    [60, "second"],
    [60, "minute"],
    [24, "hour"],
    [30, "day"],
    [12, "month"],
    [Number.POSITIVE_INFINITY, "year"],
  ];
  let value = abs;
  let unit = "second";
  for (const [size, name] of units) {
    unit = name;
    if (value < size) break;
    value = Math.floor(value / size);
  }
  if (unit === "second" && value < 45) return "just now";
  const label = `${value} ${unit}${value === 1 ? "" : "s"}`;
  return secs >= 0 ? `${label} ago` : `in ${label}`;
}

export function reviewLabel(state: ReviewState | null): string {
  switch (state) {
    case "approved":
      return "Approved";
    case "changes_requested":
      return "Changes requested";
    case "review_required":
      return "Review required";
    default:
      return "";
  }
}

export function stateLabel(state: PrState): string {
  return state === "open" ? "Open" : state === "merged" ? "Merged" : "Closed";
}
