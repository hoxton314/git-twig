/**
 * Command palette registry.
 *
 * Every entry in `ACTIONS` (keybindings.ts) with a registered handler shows
 * up automatically. Other areas can contribute dynamic items with
 * `registerPaletteProvider()`:
 *
 * ```ts
 * onMount(() => registerPaletteProvider({
 *   id: "stash",
 *   getItems: () => get(stashEntries).map((s) => ({
 *     id: `stash:${s.index}`, label: `Apply ${s.message}`, category: "Stash",
 *     run: () => applyStash(s.index),
 *   })),
 * }));
 * ```
 */
import { writable, get } from "svelte/store";

export interface PaletteItem {
  /** Stable id; used to rank recently used items first. */
  id: string;
  label: string;
  /** Group shown on the right, e.g. "Git", "Branch", "Tab". */
  category: string;
  /** Secondary text (path, upstream, ...). Searched too. */
  detail?: string;
  /** Shortcut to display, e.g. "Ctrl+Shift+P". */
  shortcut?: string;
  /** Extra search terms not shown in the UI. */
  keywords?: string;
  /** Items that cannot run right now are shown dimmed. */
  disabled?: boolean;
  run: () => void | Promise<void>;
}

export interface PaletteProvider {
  id: string;
  /** Called each time the palette opens and on every query change. */
  getItems: (query: string) => PaletteItem[] | Promise<PaletteItem[]>;
  /** Higher sorts earlier among equal scores (actions use 0). */
  priority?: number;
}

const providers = new Map<string, PaletteProvider>();

/** Register (or replace) a provider. Returns an unregister function. */
export function registerPaletteProvider(provider: PaletteProvider): () => void {
  providers.set(provider.id, provider);
  return () => {
    if (providers.get(provider.id) === provider) providers.delete(provider.id);
  };
}

export function getPaletteProviders(): PaletteProvider[] {
  return [...providers.values()];
}

// ── Open state ───────────────────────────────────────────────────────

export const paletteState = writable<{ open: boolean; query: string }>({ open: false, query: "" });

export function openPalette(query = "") {
  paletteState.set({ open: true, query });
}

export function closePalette() {
  paletteState.update((s) => ({ ...s, open: false }));
}

export function togglePalette() {
  if (get(paletteState).open) closePalette();
  else openPalette();
}

// ── Recently used ────────────────────────────────────────────────────

const RECENT_KEY = "twig.palette.recent";
const MAX_RECENT = 50;

function readRecent(): string[] {
  try {
    const raw = localStorage.getItem(RECENT_KEY);
    const parsed = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? parsed.filter((x) => typeof x === "string") : [];
  } catch {
    return [];
  }
}

let recentIds: string[] = readRecent();

export function recentPaletteIds(): string[] {
  return recentIds;
}

export function markPaletteItemUsed(id: string) {
  recentIds = [id, ...recentIds.filter((x) => x !== id)].slice(0, MAX_RECENT);
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(recentIds));
  } catch {
    // Storage unavailable: recency just isn't remembered across restarts.
  }
}

// ── Fuzzy matching ───────────────────────────────────────────────────

export interface FuzzyMatch {
  score: number;
  /** Indices into the matched text, for highlighting. */
  indices: number[];
}

function isBoundary(text: string, i: number): boolean {
  if (i === 0) return true;
  const prev = text[i - 1];
  const cur = text[i];
  return /[\s/_\-.:]/.test(prev) || (prev === prev.toLowerCase() && cur !== cur.toLowerCase());
}

/**
 * Subsequence fuzzy match. Rewards consecutive characters, word starts and
 * early matches; returns null when `query` is not a subsequence of `text`.
 */
export function fuzzyMatch(query: string, text: string): FuzzyMatch | null {
  const q = query.toLowerCase().replace(/\s+/g, "");
  if (!q) return { score: 0, indices: [] };
  const t = text.toLowerCase();

  // Exact substring is the strongest signal.
  const sub = t.indexOf(q);
  if (sub !== -1) {
    const indices = Array.from({ length: q.length }, (_, k) => sub + k);
    const boundary = isBoundary(text, sub) ? 30 : 0;
    return { score: 100 + boundary - sub * 0.5 - (t.length - q.length) * 0.05, indices };
  }

  const indices: number[] = [];
  let score = 0;
  let ti = 0;
  let prev = -2;
  for (const ch of q) {
    let found = -1;
    // Prefer a word-boundary occurrence ahead, else the next occurrence.
    for (let j = ti; j < t.length; j++) {
      if (t[j] !== ch) continue;
      if (found === -1) found = j;
      if (j === prev + 1 || isBoundary(text, j)) {
        found = j;
        break;
      }
    }
    if (found === -1) return null;
    if (found === prev + 1) score += 8;
    if (isBoundary(text, found)) score += 10;
    score -= Math.min(found - ti, 10) * 0.5;
    indices.push(found);
    prev = found;
    ti = found + 1;
  }
  return { score, indices };
}
