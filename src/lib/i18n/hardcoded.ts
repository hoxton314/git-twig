/**
 * Finds user-visible strings written directly into Svelte components
 * instead of going through the message catalog (`$t` / `tr`). Used by
 * `hardcoded.test.ts`, which fails CI on new ones.
 *
 * Heuristics, not a parser:
 *  - markup text between tags,
 *  - literal `title`, `aria-label`, `placeholder`, `alt`, `label` attributes,
 *  - sentence-like string literals inside markup `{…}` expressions,
 *  - string literals passed to `toast` / `toastError` / `ask` / `message` /
 *    `confirm` in the script, and `title:` / `label:` / `hint:` /
 *    `description:` / `placeholder:` / `okLabel:` / `cancelLabel:` /
 *    `message:` object properties.
 *
 * Text made only of allowed words (product names, git terms), symbols and
 * numbers passes. A line containing `i18n-ignore` is skipped.
 */

export interface Hardcoded {
  line: number;
  text: string;
}

/** Words that stay as they are in every language. */
const ALLOWED_WORDS = new Set([
  "twig",
  "git",
  "github",
  "gitlab",
  "gitea",
  "head",
  "lfs",
  "sha",
  "ssh",
  "gpg",
  "https",
  "http",
  "url",
  "ci",
  "ctrl",
  "alt",
  "shift",
  "cmd",
  "esc",
  "enter",
  "tab",
  "fetch_head",
  "orig_head",
  "merge_head",
  "ok",
  "id",
  "oid",
  "px",
  "ms",
  "kb",
  "mb",
  "gb",
  "vs",
]);

/** True when `text` contains words a translator would need to see. */
export function isTranslatable(text: string): boolean {
  const words = text.replace(/\{[^}]*\}/g, " ").match(/[\p{L}][\p{L}'’-]*/gu) ?? [];
  return words.some((w) => w.length >= 2 && !ALLOWED_WORDS.has(w.toLowerCase()));
}

/** Replace `{…}` expressions (balanced, string-aware) with `fill(expr)`. */
function mapExpressions(src: string, fill: (expr: string, start: number) => string): string {
  let out = "";
  let i = 0;
  while (i < src.length) {
    if (src[i] !== "{") {
      out += src[i++];
      continue;
    }
    const start = i;
    let depth = 0;
    let quote: string | null = null;
    for (; i < src.length; i++) {
      const c = src[i];
      if (quote) {
        if (c === "\\") i++;
        else if (c === quote) quote = null;
        continue;
      }
      if (c === '"' || c === "'" || c === "`") quote = c;
      else if (c === "{") depth++;
      else if (c === "}" && --depth === 0) break;
    }
    i++;
    out += fill(src.slice(start, i), start);
  }
  return out;
}

/** Blank out a range while keeping newlines, so offsets and lines stay put. */
function blank(s: string): string {
  return s.replace(/[^\n]/g, " ");
}

function lineAt(src: string, offset: number): number {
  let n = 1;
  for (let i = 0; i < offset && i < src.length; i++) if (src[i] === "\n") n++;
  return n;
}

const STRING_RE = /"((?:[^"\\\n]|\\.)*)"|'((?:[^'\\\n]|\\.)*)'|`((?:[^`\\]|\\.)*)`/g;

const CALL_RE = /\b(?:toast|toastError|ask|message|confirm)\(\s*("(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*'|`(?:[^`\\]|\\.)*`)/g;
const TOAST_RE = /\btoast\(\s*["'](?:info|success|error|warning)["']\s*,\s*("(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*'|`(?:[^`\\]|\\.)*`)/g;
const PROP_RE =
  /\b(?:title|label|hint|description|placeholder|okLabel|cancelLabel|message)\s*:\s*("(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*'|`(?:[^`\\]|\\.)*`)/g;

const TOAST_KINDS = new Set(["info", "success", "error", "warning"]);

function unquote(lit: string): string {
  return lit.slice(1, -1).replace(/\$\{[^}]*\}/g, " ");
}

/** Script-side findings (toasts, dialogs, label/title properties). */
export function findInScript(src: string, offset = 0, whole = src): Hardcoded[] {
  const found: Hardcoded[] = [];
  for (const re of [CALL_RE, TOAST_RE, PROP_RE]) {
    for (const m of src.matchAll(re)) {
      const text = unquote(m[1]);
      if (TOAST_KINDS.has(text)) continue;
      if (isTranslatable(text)) found.push({ line: lineAt(whole, offset + m.index! + m[0].indexOf(m[1])), text });
    }
  }
  return found;
}

/** Sentence-like literals inside a markup expression: `{ok ? "Saved" : "Failed"}`. */
function looksLikeUiString(text: string): boolean {
  return isTranslatable(text) && /^\s*\p{Lu}\p{Ll}/u.test(text);
}

export function findHardcoded(source: string): Hardcoded[] {
  const found: Hardcoded[] = [];
  // Scripts: check toasts/dialogs, then blank them out of the markup.
  let markup = source.replace(/<!--[\s\S]*?-->/g, blank).replace(/<style[\s\S]*?<\/style>/g, blank);
  markup = markup.replace(/<script[\s\S]*?<\/script>/g, (block, offset: number) => {
    found.push(...findInScript(block, offset, source));
    return blank(block);
  });

  // Expressions: look for UI-ish literals and toast calls, then blank them.
  markup = mapExpressions(markup, (expr, start) => {
    found.push(...findInScript(expr, start, source));
    for (const m of expr.matchAll(STRING_RE)) {
      const text = (m[1] ?? m[2] ?? m[3] ?? "").replace(/\$\{[^}]*\}/g, " ");
      if (looksLikeUiString(text)) found.push({ line: lineAt(source, start + m.index!), text });
    }
    return blank(expr);
  });

  // Literal UI attributes.
  const attrRe = /\s(?:title|aria-label|placeholder|alt|label|aria-description)="([^"]*)"/g;
  for (const m of markup.matchAll(attrRe)) {
    if (isTranslatable(m[1])) found.push({ line: lineAt(source, m.index!), text: m[1].trim() });
  }

  // Text between tags (tags themselves, with their attributes, removed).
  const tagRe = /<\/?[A-Za-z][^>]*>/g;
  let last = 0;
  const pushText = (text: string, at: number) => {
    const trimmed = text.trim();
    if (trimmed && isTranslatable(trimmed)) found.push({ line: lineAt(source, at + text.indexOf(trimmed)), text: trimmed });
  };
  for (const m of markup.matchAll(tagRe)) {
    pushText(markup.slice(last, m.index), last);
    last = m.index! + m[0].length;
  }
  pushText(markup.slice(last), last);

  const lines = source.split("\n");
  const seen = new Set<string>();
  return found
    .filter((f) => !lines[f.line - 1]?.includes("i18n-ignore"))
    .filter((f) => {
      const k = `${f.line}:${f.text}`;
      if (seen.has(k)) return false;
      seen.add(k);
      return true;
    })
    .sort((a, b) => a.line - b.line);
}
