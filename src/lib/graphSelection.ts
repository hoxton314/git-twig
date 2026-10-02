/**
 * Multi-selection in the commit graph (Ctrl/Cmd-click toggles, Shift-click
 * and Shift+↑/↓ extend a range from the anchor). The primary selection
 * stays `selectedCommitOid`; the extra set is only meaningful while it
 * contains the primary, so anything else that selects a commit collapses
 * it back to one without having to know about it.
 *
 * All helpers are pure; `order` is the graph's row order (newest first).
 */

export interface Selection {
  /** Selected oids in graph order. */
  oids: string[];
  /** Range anchor (the last plainly or Ctrl-clicked commit). */
  anchor: string | null;
}

export const EMPTY_SELECTION: Selection = { oids: [], anchor: null };

/** The commits actually selected, given the primary oid. */
export function effectiveSelection(sel: Selection, primary: string | null): string[] {
  if (!primary || primary === "__wip__") return [];
  return sel.oids.includes(primary) ? sel.oids : [primary];
}

function inOrder(oids: Iterable<string>, order: string[]): string[] {
  const set = new Set(oids);
  return order.filter((o) => set.has(o));
}

/** Ctrl/Cmd-click: toggle `oid` in the selection that `primary` implies. */
export function toggle(sel: Selection, primary: string | null, oid: string, order: string[]): Selection {
  const cur = new Set(effectiveSelection(sel, primary));
  if (cur.has(oid)) cur.delete(oid);
  else cur.add(oid);
  return { oids: inOrder(cur, order), anchor: oid };
}

/** Shift-click / Shift+arrow: everything between the anchor and `oid`. */
export function extendTo(sel: Selection, primary: string | null, oid: string, order: string[]): Selection {
  const anchor = sel.anchor && order.includes(sel.anchor) ? sel.anchor : primary;
  const a = anchor ? order.indexOf(anchor) : -1;
  const b = order.indexOf(oid);
  if (a < 0 || b < 0) return { oids: [oid], anchor: oid };
  const [lo, hi] = a <= b ? [a, b] : [b, a];
  return { oids: order.slice(lo, hi + 1), anchor };
}

/**
 * The primary oid after a toggle: the toggled commit when it was added,
 * otherwise the nearest remaining one (or null when nothing is left).
 */
export function primaryAfterToggle(next: Selection, oid: string): string | null {
  if (next.oids.includes(oid)) return oid;
  return next.oids[0] ?? null;
}

/**
 * With exactly two commits selected: `from` is the older (lower in the
 * graph), `to` the newer, so the diff reads as "what changed since".
 */
export function comparePair(selected: string[]): { from: string; to: string } | null {
  if (selected.length !== 2) return null;
  return { from: selected[1], to: selected[0] };
}
