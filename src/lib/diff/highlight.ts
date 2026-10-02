/**
 * Lazy syntax highlighting for diff lines.
 *
 * Uses highlight.js *core* (~22 KB min) plus only the grammars actually
 * needed, each loaded on demand through a dynamic import so nothing is in the
 * startup bundle. hljs's HTML output is never injected: it is parsed into
 * plain `{ text, cls }` tokens that Svelte renders as text nodes, so file
 * contents can't become markup.
 */
import type { HLJSApi } from "highlight.js";

export interface SyntaxToken {
  text: string;
  /** hljs scope classes of the innermost span ("" for plain text). */
  cls: string;
}

type Loader = () => Promise<{ default: unknown }>;

// Curated set of common languages; Vite emits one small chunk per entry.
const LOADERS: Record<string, Loader> = {
  bash: () => import("highlight.js/lib/languages/bash"),
  c: () => import("highlight.js/lib/languages/c"),
  cpp: () => import("highlight.js/lib/languages/cpp"),
  csharp: () => import("highlight.js/lib/languages/csharp"),
  css: () => import("highlight.js/lib/languages/css"),
  dart: () => import("highlight.js/lib/languages/dart"),
  dockerfile: () => import("highlight.js/lib/languages/dockerfile"),
  go: () => import("highlight.js/lib/languages/go"),
  ini: () => import("highlight.js/lib/languages/ini"),
  java: () => import("highlight.js/lib/languages/java"),
  javascript: () => import("highlight.js/lib/languages/javascript"),
  json: () => import("highlight.js/lib/languages/json"),
  kotlin: () => import("highlight.js/lib/languages/kotlin"),
  less: () => import("highlight.js/lib/languages/less"),
  lua: () => import("highlight.js/lib/languages/lua"),
  makefile: () => import("highlight.js/lib/languages/makefile"),
  markdown: () => import("highlight.js/lib/languages/markdown"),
  nix: () => import("highlight.js/lib/languages/nix"),
  php: () => import("highlight.js/lib/languages/php"),
  python: () => import("highlight.js/lib/languages/python"),
  ruby: () => import("highlight.js/lib/languages/ruby"),
  rust: () => import("highlight.js/lib/languages/rust"),
  scss: () => import("highlight.js/lib/languages/scss"),
  sql: () => import("highlight.js/lib/languages/sql"),
  swift: () => import("highlight.js/lib/languages/swift"),
  typescript: () => import("highlight.js/lib/languages/typescript"),
  xml: () => import("highlight.js/lib/languages/xml"),
  yaml: () => import("highlight.js/lib/languages/yaml"),
};

const EXT_TO_LANG: Record<string, string> = {
  sh: "bash", bash: "bash", zsh: "bash", fish: "bash",
  c: "c", h: "c",
  cc: "cpp", cpp: "cpp", cxx: "cpp", hpp: "cpp", hh: "cpp", hxx: "cpp",
  cs: "csharp",
  css: "css",
  dart: "dart",
  go: "go",
  ini: "ini", toml: "ini", cfg: "ini", conf: "ini", editorconfig: "ini",
  java: "java",
  js: "javascript", mjs: "javascript", cjs: "javascript", jsx: "javascript",
  json: "json", jsonc: "json", json5: "json",
  kt: "kotlin", kts: "kotlin",
  less: "less",
  lua: "lua",
  mk: "makefile",
  md: "markdown", markdown: "markdown",
  nix: "nix",
  php: "php",
  py: "python", pyi: "python",
  rb: "ruby",
  rs: "rust",
  scss: "scss", sass: "scss",
  sql: "sql",
  swift: "swift",
  ts: "typescript", tsx: "typescript", mts: "typescript", cts: "typescript",
  html: "xml", htm: "xml", xml: "xml", svg: "xml", svelte: "xml", vue: "xml",
  xhtml: "xml", plist: "xml",
  yml: "yaml", yaml: "yaml",
};

const NAME_TO_LANG: Record<string, string> = {
  dockerfile: "dockerfile",
  containerfile: "dockerfile",
  makefile: "makefile",
  gnumakefile: "makefile",
  "cargo.lock": "ini",
  ".gitconfig": "ini",
  ".bashrc": "bash",
  ".zshrc": "bash",
  ".profile": "bash",
};

/** The hljs language for a path, or null if we don't highlight it. */
export function languageForPath(path: string | null | undefined): string | null {
  if (!path) return null;
  const name = (path.split("/").pop() ?? "").toLowerCase();
  if (NAME_TO_LANG[name]) return NAME_TO_LANG[name];
  if (name.startsWith("dockerfile")) return "dockerfile";
  const dot = name.lastIndexOf(".");
  if (dot < 0) return null;
  return EXT_TO_LANG[name.slice(dot + 1)] ?? null;
}

let hljsPromise: Promise<HLJSApi> | null = null;
let hljs: HLJSApi | null = null;
const registered = new Set<string>();
const pending = new Map<string, Promise<boolean>>();

function loadCore(): Promise<HLJSApi> {
  if (!hljsPromise) {
    hljsPromise = import("highlight.js/lib/core").then((m) => {
      hljs = m.default;
      return hljs;
    });
  }
  return hljsPromise;
}

/** Whether `lang` is loaded and `highlightLines` can be used synchronously. */
export function isLanguageReady(lang: string): boolean {
  return hljs !== null && registered.has(lang);
}

/** Load hljs core and the grammar for `lang`. Resolves false on failure. */
export function ensureLanguage(lang: string): Promise<boolean> {
  if (isLanguageReady(lang)) return Promise.resolve(true);
  const loader = LOADERS[lang];
  if (!loader) return Promise.resolve(false);
  let p = pending.get(lang);
  if (!p) {
    p = Promise.all([loadCore(), loader()])
      .then(([core, mod]) => {
        core.registerLanguage(lang, mod.default as Parameters<HLJSApi["registerLanguage"]>[1]);
        registered.add(lang);
        return true;
      })
      .catch((err) => {
        console.error(`Failed to load syntax grammar "${lang}":`, err);
        pending.delete(lang);
        return false;
      });
    pending.set(lang, p);
  }
  return p;
}

const ENTITIES: Record<string, string> = {
  "&amp;": "&",
  "&lt;": "<",
  "&gt;": ">",
  "&quot;": '"',
  "&#x27;": "'",
  "&#39;": "'",
};

function unescapeHtml(s: string): string {
  return s.replace(/&(?:amp|lt|gt|quot|#x27|#39);/g, (m) => ENTITIES[m] ?? m);
}

/** Skip pathological inputs (minified bundles etc.) — they'd only stall the UI. */
const MAX_HIGHLIGHT_CHARS = 200_000;
const MAX_LINE_CHARS = 2_000;

/**
 * Highlight consecutive source lines as one block (so multi-line strings and
 * comments are tracked) and return tokens per line. Returns null when the
 * language isn't loaded or the input is too large.
 */
export function highlightLines(lines: string[], lang: string): SyntaxToken[][] | null {
  if (!hljs || !registered.has(lang) || lines.length === 0) return null;
  let total = 0;
  for (const l of lines) {
    if (l.length > MAX_LINE_CHARS) return null;
    total += l.length + 1;
  }
  if (total > MAX_HIGHLIGHT_CHARS) return null;

  let html: string;
  try {
    html = hljs.highlight(lines.join("\n"), { language: lang, ignoreIllegals: true }).value;
  } catch {
    return null;
  }

  const out: SyntaxToken[][] = [[]];
  const stack: string[] = [];
  // hljs output only contains escaped text, <span class="..."> and </span>.
  const re = /<span class="([^"]*)">|<\/span>|([^<]+)|(<)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(html)) !== null) {
    if (m[1] !== undefined) {
      stack.push(m[1]);
    } else if (m[0] === "</span>") {
      stack.pop();
    } else {
      const text = m[2] !== undefined ? unescapeHtml(m[2]) : "<";
      // Innermost scope wins (e.g. a keyword inside a meta block).
      const cls = stack.length > 0 ? stack[stack.length - 1] : "";
      const parts = text.split("\n");
      for (let i = 0; i < parts.length; i++) {
        if (i > 0) out.push([]);
        if (parts[i]) out[out.length - 1].push({ text: parts[i], cls });
      }
    }
  }
  // Defensive: if hljs ever changed the text, fall back to plain rendering.
  if (out.length !== lines.length) return null;
  for (let i = 0; i < lines.length; i++) {
    let joined = "";
    for (const t of out[i]) joined += t.text;
    if (joined !== lines[i]) return null;
  }
  return out;
}
