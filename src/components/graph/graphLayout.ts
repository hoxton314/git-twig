import type { GraphDateFormat, GraphRowDensity } from "../../lib/types/git";
import { tr } from "../../lib/i18n";

export const GRAPH_LEFT_PAD = 12;
export const GRAPH_LANE_WIDTH = 20;

/** Pixel width of the graph (lanes) column for `totalLanes` lanes. */
export function graphColumnWidth(totalLanes: number): number {
  return Math.max((totalLanes + 1) * GRAPH_LANE_WIDTH + GRAPH_LEFT_PAD, 48 + GRAPH_LEFT_PAD);
}

export const ROW_HEIGHTS: Record<GraphRowDensity, number> = {
  compact: 26,
  normal: 34,
  comfortable: 42,
};

export const AVATAR_SIZES: Record<GraphRowDensity, number> = {
  compact: 16,
  normal: 20,
  comfortable: 24,
};

export function rowHeight(density: GraphRowDensity | string): number {
  return ROW_HEIGHTS[density as GraphRowDensity] ?? ROW_HEIGHTS.normal;
}

export function avatarSize(density: GraphRowDensity | string): number {
  return AVATAR_SIZES[density as GraphRowDensity] ?? AVATAR_SIZES.normal;
}

/** Resizable, hideable columns on the right of each row. */
export type GraphColumn = "author" | "sha" | "date";

export const DEFAULT_COL_WIDTHS: Record<GraphColumn, number> = {
  author: 120,
  sha: 64,
  date: 90,
};

/** Column width bounds for the resizable author/SHA/date columns. */
export const MIN_COL_WIDTH = 40;
export const MAX_COL_WIDTH = 480;

export function clampColWidth(w: number): number {
  return Math.round(Math.min(MAX_COL_WIDTH, Math.max(MIN_COL_WIDTH, w)));
}

function pad(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

/** Relative age like "5m ago", "3d ago", "2y ago". */
function relative(ts: number, now: number): string {
  // Clamp so clock skew (commit slightly in the future) never goes negative.
  const diff = Math.max(0, now - ts * 1000);
  const minutes = Math.floor(diff / 60000);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  if (minutes < 1) return tr("time.justNow");
  if (minutes < 60) return tr("graph.ageMinutes", { count: minutes });
  if (hours < 24) return tr("graph.ageHours", { count: hours });
  if (days < 30) return tr("graph.ageDays", { count: days });
  const months = Math.floor(days / 30.44);
  if (months < 12) return tr("graph.ageMonths", { count: months });
  return tr("graph.ageYears", { count: Math.floor(days / 365.25) });
}

/** Format a commit timestamp (seconds) for the date column. */
export function formatCommitDate(ts: number, format: GraphDateFormat | string, now: number): string {
  const d = new Date(ts * 1000);
  switch (format) {
    case "iso":
      return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
    case "locale":
      return d.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
    default:
      return relative(ts, now);
  }
}
