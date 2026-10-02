/** Pure helpers for the commit message box. */

export const CONVENTIONAL_TYPES = [
  "feat",
  "fix",
  "docs",
  "refactor",
  "test",
  "chore",
  "perf",
  "style",
  "build",
  "ci",
  "revert",
] as const;

export interface ConventionalPrefix {
  type: string;
  scope: string;
  breaking: boolean;
  /** Summary text after the prefix. */
  rest: string;
}

const PREFIX_RE = /^([a-z]+)(?:\(([^()]*)\))?(!)?: ?(.*)$/;

/** Split a conventional-commit prefix off the summary line, if present. */
export function parseConventional(message: string): ConventionalPrefix | null {
  const nl = message.indexOf("\n");
  const first = nl === -1 ? message : message.slice(0, nl);
  const m = PREFIX_RE.exec(first);
  if (!m || !(CONVENTIONAL_TYPES as readonly string[]).includes(m[1])) return null;
  return { type: m[1], scope: m[2] ?? "", breaking: m[3] === "!", rest: m[4] };
}

/**
 * Rewrite (or remove, when `type` is empty) the conventional-commit prefix of
 * the summary line, keeping the summary text and body intact.
 */
export function applyConventional(message: string, type: string, scope: string): string {
  const nl = message.indexOf("\n");
  const first = nl === -1 ? message : message.slice(0, nl);
  const body = nl === -1 ? "" : message.slice(nl);
  const parsed = parseConventional(message);
  const rest = parsed ? parsed.rest : first;
  const breaking = parsed?.breaking ?? false;
  if (!type) return rest + body;
  const cleanScope = scope.replace(/[()\n]/g, "").trim();
  const prefix = `${type}${cleanScope ? `(${cleanScope})` : ""}${breaking ? "!" : ""}: `;
  return prefix + rest + body;
}

function isTrailerLine(line: string): boolean {
  return /^[A-Za-z0-9-]+: \S/.test(line);
}

/**
 * Append a trailer line (e.g. "Co-authored-by: A <a@b>") unless present:
 * into the trailing trailer block when there is one, else as a new paragraph.
 * Mirrors `append_trailer` in `git/commit_tools.rs`.
 */
export function appendTrailer(message: string, trailer: string): string {
  const body = message.replace(/\s+$/, "");
  if (body.split("\n").some((l) => l.trimEnd() === trailer)) return body;
  if (!body) return `\n\n${trailer}`;
  const paras = body.split("\n\n");
  const last = paras[paras.length - 1];
  const hasBlock = paras.length > 1 && last.trim() !== "" && last.split("\n").every(isTrailerLine);
  return hasBlock ? `${body}\n${trailer}` : `${body}\n\n${trailer}`;
}

// ── Per-repo persistence (localStorage, best effort) ─────────────────

const HISTORY_MAX = 20;

function read<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch {
    return fallback;
  }
}

function write(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Storage unavailable (private mode / quota) — history is a convenience.
  }
}

/** Recent commit messages for a repo, newest first. */
export function loadHistory(repo: string): string[] {
  const list = read<unknown>(`twig.commitHistory:${repo}`, []);
  return Array.isArray(list) ? list.filter((m): m is string => typeof m === "string") : [];
}

export function pushHistory(repo: string, message: string): string[] {
  const msg = message.trim();
  if (!msg) return loadHistory(repo);
  const next = [msg, ...loadHistory(repo).filter((m) => m !== msg)].slice(0, HISTORY_MAX);
  write(`twig.commitHistory:${repo}`, next);
  return next;
}

export function loadSignoff(repo: string): boolean {
  return read<boolean>(`twig.commitSignoff:${repo}`, false) === true;
}

export function saveSignoff(repo: string, on: boolean) {
  write(`twig.commitSignoff:${repo}`, on);
}
