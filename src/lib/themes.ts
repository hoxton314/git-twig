/**
 * Custom colour themes: JSON documents of the app's CSS colour tokens on top
 * of a built-in base (dark / light). Pure helpers (validation, contrast,
 * import parsing); applying them to the page is in `stores/settings.ts`.
 */

/** Every colour token a theme can set (kept in sync with app.css by a test). */
export const THEME_TOKENS = [
  "--color-bg",
  "--color-surface",
  "--color-surface-elevated",
  "--color-border",
  "--color-accent-secondary",
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
  { title: "Surfaces & text", tokens: ["--color-bg", "--color-surface", "--color-surface-elevated", "--color-border", "--color-text-primary", "--color-text-muted", "--color-accent-secondary"] },
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

/** Parse a hex or rgb(a) colour to [r, g, b] (0–255); null for anything else. */
export function toRgb(value: string): [number, number, number] | null {
  const v = value.trim();
  if (HEX.test(v)) {
    let h = v.slice(1);
    if (h.length <= 4) h = [...h].map((c) => c + c).join("");
    return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16)) as [number, number, number];
  }
  const m = v.match(/^rgba?\(\s*([0-9.]+)\s*[ ,]\s*([0-9.]+)\s*[ ,]\s*([0-9.]+)/i);
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null;
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
    if (c !== null && c < min) out.push(`${tokenLabel(fg)} on ${tokenLabel(bg)}: contrast ${c.toFixed(1)}:1 (aim for ${min}:1)`);
  }
  return out;
}

/**
 * Validate an imported theme document. Unknown tokens are dropped; an
 * invalid colour, name or base rejects the file with a reason.
 */
export function parseTheme(json: unknown, id: string): { theme: CustomTheme } | { error: string } {
  if (!json || typeof json !== "object") return { error: "The file isn't a theme (expected a JSON object)." };
  const o = json as Record<string, unknown>;
  const name = typeof o.name === "string" ? o.name.trim().slice(0, 60) : "";
  if (!name) return { error: "The theme has no name." };
  const base = o.base === "light" ? "light" : o.base === "dark" || o.base === undefined ? "dark" : null;
  if (!base) return { error: `Unknown base theme "${String(o.base)}" (use "dark" or "light").` };
  if (!o.colors || typeof o.colors !== "object") return { error: "The theme has no colors." };
  const colors: Partial<Record<ThemeToken, string>> = {};
  const known = new Set<string>(THEME_TOKENS);
  for (const [k, v] of Object.entries(o.colors as Record<string, unknown>)) {
    if (!known.has(k)) continue;
    if (typeof v !== "string" || !isColor(v)) return { error: `${k} is not a valid colour: ${JSON.stringify(v)}` };
    colors[k as ThemeToken] = v.trim();
  }
  if (Object.keys(colors).length === 0) return { error: "The theme sets none of Twig's colour tokens." };
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
