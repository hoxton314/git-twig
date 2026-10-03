/**
 * Custom colour themes: JSON documents of the app's CSS colour tokens on top
 * of a built-in base (dark / light). Pure helpers (validation, contrast,
 * import parsing); applying them to the page is in `stores/settings.ts`.
 */

/** Every colour token a theme can set (kept in sync with app.css by a test). */
import { tr } from "./i18n";
export const THEME_TOKENS = [
  "--color-bg",
  "--color-surface",
  "--color-surface-elevated",
  "--color-border",
  "--color-accent-secondary",
  "--color-on-accent",
  "--color-text-primary",
  "--color-text-muted",
  "--color-lane-0",
  "--color-lane-1",
  "--color-lane-2",
  "--color-lane-3",
  "--color-lane-4",
  "--color-lane-5",
  "--color-diff-add-bg",
  "--color-diff-add-text",
  "--color-diff-add-word-bg",
  "--color-diff-del-bg",
  "--color-diff-del-text",
  "--color-diff-del-word-bg",
  "--color-diff-hunk-bg",
  "--color-syntax-keyword",
  "--color-syntax-string",
  "--color-syntax-number",
  "--color-syntax-comment",
  "--color-syntax-function",
  "--color-syntax-type",
  "--color-syntax-variable",
  "--color-syntax-meta",
  "--color-search-hit-bg",
  "--color-search-hit-text",
  "--color-search-active-bg",
  "--color-search-active-outline",
] as const;

export type ThemeToken = (typeof THEME_TOKENS)[number];

export interface CustomTheme {
  id: string;
  name: string;
  /** Built-in theme underneath (unset tokens, form controls, color-scheme). */
  base: "dark" | "light";
  colors: Partial<Record<ThemeToken, string>>;
}

/** Groups for the editor. */
export const TOKEN_GROUPS: { title: string; tokens: ThemeToken[] }[] = [
  { title: "Surfaces & text", tokens: ["--color-bg", "--color-surface", "--color-surface-elevated", "--color-border", "--color-text-primary", "--color-text-muted", "--color-accent-secondary", "--color-on-accent"] },
  { title: "Graph lanes", tokens: ["--color-lane-0", "--color-lane-1", "--color-lane-2", "--color-lane-3", "--color-lane-4", "--color-lane-5"] },
  { title: "Diff", tokens: ["--color-diff-add-bg", "--color-diff-add-text", "--color-diff-add-word-bg", "--color-diff-del-bg", "--color-diff-del-text", "--color-diff-del-word-bg", "--color-diff-hunk-bg"] },
  { title: "Syntax", tokens: ["--color-syntax-keyword", "--color-syntax-string", "--color-syntax-number", "--color-syntax-comment", "--color-syntax-function", "--color-syntax-type", "--color-syntax-variable", "--color-syntax-meta"] },
  { title: "Search", tokens: ["--color-search-hit-bg", "--color-search-hit-text", "--color-search-active-bg", "--color-search-active-outline"] },
];

/** Human label for a token ("--color-diff-add-bg" → "diff add bg"). */
export function tokenLabel(token: string): string {
  return token.replace(/^--color-/, "").replace(/-/g, " ");
}

export const CUSTOM_PREFIX = "custom:";

const HEX = /^#(?:[0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i;
const FUNC = /^(?:rgb|rgba|hsl|hsla)\(\s*[0-9.%+-]+\s*(?:[ ,]\s*[0-9.%+-]+\s*){2}(?:[,/]\s*[0-9.%+-]+\s*)?\)$/i;

/** A colour value a theme may use: hex, rgb()/rgba(), hsl()/hsla() or `inherit`. */
export function isColor(value: string): boolean {
  const v = value.trim();
  return v.length <= 64 && (HEX.test(v) || FUNC.test(v) || v === "inherit" || v === "transparent");
}

/** Parse a hex, rgb(a) or hsl(a) colour to [r, g, b] (0–255); null otherwise. */
export function toRgb(value: string): [number, number, number] | null {
  const v = value.trim();
  if (HEX.test(v)) {
    let h = v.slice(1);
    if (h.length <= 4) h = [...h].map((c) => c + c).join("");
    return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16)) as [number, number, number];
  }
  const m = v.match(/^rgba?\(\s*([0-9.]+)\s*[ ,]\s*([0-9.]+)\s*[ ,]\s*([0-9.]+)/i);
  if (m) return [Number(m[1]), Number(m[2]), Number(m[3])];
  const hsl = v.match(/^hsla?\(\s*([0-9.]+)\s*[ ,]\s*([0-9.]+)%\s*[ ,]\s*([0-9.]+)%/i);
  if (hsl) {
    const h = Number(hsl[1]) % 360;
    const s = Number(hsl[2]) / 100;
    const l = Number(hsl[3]) / 100;
    const k = (n: number) => (n + h / 30) % 12;
    const a = s * Math.min(l, 1 - l);
    const f = (n: number) => l - a * Math.max(-1, Math.min(k(n) - 3, 9 - k(n), 1));
    return [f(0), f(8), f(4)].map((x) => Math.round(x * 255)) as [number, number, number];
  }
  return null;
}

/** Alpha (0–1) of a colour value; 1 when opaque or unknown. */
export function alphaOf(value: string): number {
  const v = value.trim();
  if (HEX.test(v) && (v.length === 5 || v.length === 9)) {
    const a = v.length === 5 ? v[4] + v[4] : v.slice(7, 9);
    return parseInt(a, 16) / 255;
  }
  const inner = v.match(/^(?:rgba?|hsla?)\((.*)\)$/i)?.[1];
  if (!inner) return 1;
  const parts = inner.split(/[\s,/]+/).filter(Boolean);
  if (parts.length < 4) return 1;
  const last = parts[3];
  const n = parseFloat(last);
  if (Number.isNaN(n)) return 1;
  return last.endsWith("%") ? n / 100 : n;
}

/**
 * The value to store when the colour picker (which only knows opaque
 * #rrggbb) sets `hex` on a token that had `previous`: keeps its alpha.
 */
export function pickedColor(hex: string, previous: string): string {
  const a = previous ? alphaOf(previous) : 1;
  if (a >= 1) return hex;
  const [r, g, b] = toRgb(hex) ?? [0, 0, 0];
  return `rgba(${r}, ${g}, ${b}, ${Math.round(a * 1000) / 1000})`;
}

/** WCAG contrast ratio of two colours (null if either can't be parsed). */
export function contrast(a: string, b: string): number | null {
  const ca = toRgb(a);
  const cb = toRgb(b);
  if (!ca || !cb) return null;
  const lum = ([r, g, bl]: [number, number, number]) => {
    const f = (c: number) => {
      const s = c / 255;
      return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
    };
    return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(bl);
  };
  const [hi, lo] = [lum(ca), lum(cb)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

/** Readability warnings for text on the theme's backgrounds. */
export function contrastWarnings(colors: Partial<Record<ThemeToken, string>>): string[] {
  const out: string[] = [];
  const pairs: [ThemeToken, ThemeToken, number][] = [
    ["--color-text-primary", "--color-bg", 4.5],
    ["--color-text-primary", "--color-surface", 4.5],
    ["--color-text-muted", "--color-bg", 3],
  ];
  for (const [fg, bg, min] of pairs) {
    const c = colors[fg] && colors[bg] ? contrast(colors[fg]!, colors[bg]!) : null;
    if (c !== null && c < min) out.push(tr("settings.themes.lowContrast", { fg: tokenLabel(fg), bg: tokenLabel(bg), ratio: c.toFixed(1), min }));
  }
  return out;
}

/**
 * Validate an imported theme document. Unknown tokens are dropped; an
 * invalid colour, name or base rejects the file with a reason.
 */
export function parseTheme(json: unknown, id: string): { theme: CustomTheme } | { error: string } {
  if (!json || typeof json !== "object") return { error: tr("settings.themes.errNotTheme") };
  const o = json as Record<string, unknown>;
  const name = typeof o.name === "string" ? o.name.trim().slice(0, 60) : "";
  if (!name) return { error: tr("settings.themes.errNoName") };
  const base = o.base === "light" ? "light" : o.base === "dark" || o.base === undefined ? "dark" : null;
  if (!base) return { error: tr("settings.themes.errBase", { base: String(o.base) }) };
  if (!o.colors || typeof o.colors !== "object") return { error: tr("settings.themes.errNoColors") };
  const colors: Partial<Record<ThemeToken, string>> = {};
  const known = new Set<string>(THEME_TOKENS);
  for (const [k, v] of Object.entries(o.colors as Record<string, unknown>)) {
    if (!known.has(k)) continue;
    if (typeof v !== "string" || !isColor(v)) return { error: tr("settings.themes.errColor", { token: k, value: JSON.stringify(v) }) };
    colors[k as ThemeToken] = v.trim();
  }
  if (Object.keys(colors).length === 0) return { error: tr("settings.themes.errNoTokens") };
  return { theme: { id, name, base, colors } };
}

/** The document written by Export. */
export function exportTheme(t: CustomTheme): string {
  return JSON.stringify({ name: t.name, base: t.base, colors: t.colors }, null, 2) + "\n";
}

/** Normalise a computed colour for an `<input type="color">` (#rrggbb). */
export function toHexInput(value: string): string {
  const rgb = toRgb(value);
  if (!rgb) return "#000000";
  return "#" + rgb.map((c) => Math.round(Math.min(255, Math.max(0, c))).toString(16).padStart(2, "0")).join("");
}
