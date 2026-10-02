/**
 * Intra-line (word-level) diffing and render-segment merging for diff lines.
 */
import type { SyntaxToken } from "./highlight";

/** Half-open character range [start, end). */
export type Range = [number, number];

const TOKEN_RE = /\w+|\s+|[^\w\s]/gu;

function tokenize(s: string): { text: string; start: number }[] {
  const out: { text: string; start: number }[] = [];
  for (const m of s.matchAll(TOKEN_RE)) {
    out.push({ text: m[0], start: m.index ?? 0 });
  }
  return out;
}

/** Above this many LCS cells a pair is too big to be worth diffing. */
const MAX_CELLS = 40_000;

/**
 * Changed character ranges of a removed line `a` and its paired added line
 * `b`. Returns null when the lines are too different (highlighting nearly
 * everything is just noise) or too long.
 */
export function wordDiff(a: string, b: string): { old: Range[]; new: Range[] } | null {
  if (a === b) return null;
  const ta = tokenize(a);
  const tb = tokenize(b);
  const n = ta.length;
  const m = tb.length;
  if (n === 0 || m === 0 || n * m > MAX_CELLS) return null;

  // Strip common prefix/suffix first; usually leaves a tiny middle.
  let pre = 0;
  while (pre < n && pre < m && ta[pre].text === tb[pre].text) pre++;
  let suf = 0;
  while (suf < n - pre && suf < m - pre && ta[n - 1 - suf].text === tb[m - 1 - suf].text) suf++;

  const ai = ta.slice(pre, n - suf);
  const bi = tb.slice(pre, m - suf);
  const keepA = new Array<boolean>(ai.length).fill(false);
  const keepB = new Array<boolean>(bi.length).fill(false);

  if (ai.length > 0 && bi.length > 0) {
    const w = bi.length + 1;
    const dp = new Uint32Array((ai.length + 1) * w);
    for (let i = ai.length - 1; i >= 0; i--) {
      for (let j = bi.length - 1; j >= 0; j--) {
        dp[i * w + j] =
          ai[i].text === bi[j].text
            ? dp[(i + 1) * w + j + 1] + 1
            : Math.max(dp[(i + 1) * w + j], dp[i * w + j + 1]);
      }
    }
    let i = 0;
    let j = 0;
    while (i < ai.length && j < bi.length) {
      if (ai[i].text === bi[j].text) {
        keepA[i++] = true;
        keepB[j++] = true;
      } else if (dp[(i + 1) * w + j] >= dp[i * w + j + 1]) {
        i++;
      } else {
        j++;
      }
    }
  }

  // Too little in common -> the whole line changed; don't highlight words.
  let commonChars = 0;
  for (let k = 0; k < pre; k++) commonChars += ta[k].text.length;
  for (let k = 0; k < suf; k++) commonChars += ta[n - 1 - k].text.length;
  ai.forEach((t, k) => {
    if (keepA[k] && t.text.trim()) commonChars += t.text.length;
  });
  if (commonChars < Math.max(a.trim().length, b.trim().length) * 0.4) return null;

  const ranges = (toks: { text: string; start: number }[], keep: boolean[]): Range[] => {
    const out: Range[] = [];
    toks.forEach((t, k) => {
      if (keep[k]) return;
      const s = t.start;
      const e = t.start + t.text.length;
      const last = out[out.length - 1];
      if (last && last[1] === s) last[1] = e;
      else out.push([s, e]);
    });
    return out;
  };
  return { old: ranges(ai, keepA), new: ranges(bi, keepB) };
}

/** Case-insensitive occurrences of `query` in `text`. */
export function findMatches(text: string, query: string): Range[] {
  if (!query) return [];
  const hay = text.toLowerCase();
  const out: Range[] = [];
  let i = hay.indexOf(query);
  while (i >= 0) {
    out.push([i, i + query.length]);
    i = hay.indexOf(query, i + query.length);
  }
  return out;
}

export interface Segment {
  text: string;
  /** Syntax classes. */
  cls: string;
  /** Inside a word-diff change. */
  changed: boolean;
  /** Index into the cell's search matches, or -1. */
  match: number;
  /** First segment of its match (used for counting / navigation). */
  matchStart: boolean;
}

/**
 * Split a line into render segments so syntax tokens, word-diff ranges and
 * search matches can all be styled independently.
 */
export function buildSegments(
  text: string,
  syntax: SyntaxToken[] | null,
  changed: Range[] | null,
  matches: Range[],
): Segment[] {
  if (!syntax && !changed?.length && matches.length === 0) {
    return [{ text, cls: "", changed: false, match: -1, matchStart: false }];
  }
  const base = syntax ?? [{ text, cls: "" }];
  const cuts = new Set<number>();
  for (const [s, e] of changed ?? []) {
    cuts.add(s);
    cuts.add(e);
  }
  for (const [s, e] of matches) {
    cuts.add(s);
    cuts.add(e);
  }
  const inRange = (pos: number, rs: Range[]) => rs.findIndex(([s, e]) => pos >= s && pos < e);

  const out: Segment[] = [];
  let pos = 0;
  for (const tok of base) {
    const end = pos + tok.text.length;
    let start = pos;
    const points = [...cuts].filter((c) => c > start && c < end).sort((x, y) => x - y);
    points.push(end);
    for (const p of points) {
      if (p <= start) continue;
      const mi = inRange(start, matches);
      out.push({
        text: tok.text.slice(start - pos, p - pos),
        cls: tok.cls,
        changed: changed ? inRange(start, changed) >= 0 : false,
        match: mi,
        matchStart: mi >= 0 && matches[mi][0] === start,
      });
      start = p;
    }
    pos = end;
  }
  return out;
}
