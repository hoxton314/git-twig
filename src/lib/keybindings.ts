/**
 * Global keybinding registry.
 *
 * Each action has a default shortcut and can be overridden via settings.
 * Shortcuts are stored as strings like "Ctrl+Enter", "Ctrl+Shift+B".
 * Modifiers: Ctrl, Shift, Alt. On macOS, Ctrl also matches Cmd; on other
 * platforms the Super/Meta key never matches.
 */

// ── Action definitions ───────────────────────────────────────────────

export interface KeybindingAction {
  id: string;
  label: string;
  category: string;
  defaultShortcut: string;
}

export const ACTIONS: KeybindingAction[] = [
  // Navigation
  { id: "open_repo",      label: "Open repository",      category: "Navigation", defaultShortcut: "Ctrl+O" },
  { id: "close_tab",      label: "Close tab",            category: "Navigation", defaultShortcut: "Ctrl+W" },
  { id: "next_tab",       label: "Next tab",             category: "Navigation", defaultShortcut: "Ctrl+Tab" },
  { id: "prev_tab",       label: "Previous tab",         category: "Navigation", defaultShortcut: "Ctrl+Shift+Tab" },
  { id: "go_home",        label: "Go to home screen",    category: "Navigation", defaultShortcut: "Ctrl+H" },
  { id: "go_settings",    label: "Open settings",        category: "Navigation", defaultShortcut: "Ctrl+," },

  // Sidebar & panels
  { id: "toggle_sidebar", label: "Toggle sidebar",       category: "Panels",     defaultShortcut: "Ctrl+B" },

  // Git operations
  { id: "commit",         label: "Commit",               category: "Git",        defaultShortcut: "Ctrl+Enter" },
  { id: "push",           label: "Push",                 category: "Git",        defaultShortcut: "Ctrl+Shift+P" },
  { id: "pull",           label: "Pull",                 category: "Git",        defaultShortcut: "Ctrl+Shift+L" },
  { id: "fetch",          label: "Fetch all",            category: "Git",        defaultShortcut: "Ctrl+Shift+F" },

  // Hosting integrations (GitHub / GitLab / Gitea)
  { id: "open_pull_requests",  label: "Show pull requests",   category: "Hosting", defaultShortcut: "" },
  { id: "create_pull_request", label: "Create pull request",  category: "Hosting", defaultShortcut: "" },
  { id: "refresh_ci_status",   label: "Refresh CI status",    category: "Hosting", defaultShortcut: "" },
];

// ── Shortcut parsing & matching ──────────────────────────────────────

interface ParsedShortcut {
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  key: string;  // lowercase key name
}

const IS_MAC =
  typeof navigator !== "undefined" && /Mac|iPhone|iPad/i.test(navigator.platform || navigator.userAgent);

function parseShortcut(shortcut: string): ParsedShortcut {
  // A trailing "+" is the plus key itself (e.g. "Ctrl++"), not a separator.
  let keyPart = "";
  let rest = shortcut;
  if (shortcut.endsWith("++") || shortcut === "+") {
    keyPart = "+";
    rest = shortcut.slice(0, -1);
  }
  const parts = rest.split("+").map((p) => p.trim()).filter(Boolean);
  const mods = parts.map((p) => p.toLowerCase());
  if (!keyPart) keyPart = parts.length > 0 ? parts[parts.length - 1] : "";
  let key = keyPart.toLowerCase();
  if (key === "escape") key = "esc";
  if (key === " ") key = "space";
  return {
    ctrl: mods.includes("ctrl") || mods.includes("cmd") || mods.includes("meta"),
    shift: mods.includes("shift"),
    alt: mods.includes("alt") || mods.includes("option"),
    key,
  };
}

/** Physical-key fallback for letters/digits (non-Latin layouts, macOS Option). */
function codeToKey(code: string | undefined): string | null {
  if (!code) return null;
  if (/^Key[A-Z]$/.test(code)) return code.slice(3).toLowerCase();
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  return null;
}

function matchesEvent(parsed: ParsedShortcut, e: KeyboardEvent): boolean {
  // Some synthetic events (e.g. autofill) have no `key`.
  if (!e.key || !parsed.key) return false;
  // "Ctrl" means Cmd on macOS; elsewhere the Super/Meta key is not Ctrl.
  const ctrl = IS_MAC ? e.ctrlKey || e.metaKey : e.ctrlKey;
  if (!IS_MAC && e.metaKey) return false;
  if (parsed.ctrl !== ctrl) return false;
  if (parsed.shift !== e.shiftKey) return false;
  if (parsed.alt !== e.altKey) return false;

  // Normalize key names
  let eventKey = e.key.toLowerCase();
  if (eventKey === " ") eventKey = "space";
  if (eventKey === "escape") eventKey = "esc";

  if (eventKey === parsed.key) return true;
  // Fall back to the physical key for letters/digits so shortcuts keep
  // working on non-Latin layouts and with Option-modified characters.
  const physical = codeToKey(e.code);
  return physical !== null && physical === parsed.key && eventKey.length === 1 && !/[a-z0-9]/.test(eventKey);
}

/** Normalised form of a shortcut string, for equality/conflict checks. */
export function normalizeShortcut(shortcut: string): string {
  const p = parseShortcut(shortcut);
  return [p.ctrl && "ctrl", p.shift && "shift", p.alt && "alt", p.key].filter(Boolean).join("+");
}

// ── Shortcut display helpers ─────────────────────────────────────────

/** Convert a KeyboardEvent to a shortcut string (for capture UI). */
export function eventToShortcut(e: KeyboardEvent): string | null {
  // Ignore bare modifier presses (and keyless synthetic events)
  if (!e.key || ["Control", "Shift", "Alt", "Meta", "AltGraph", "OS", "Dead", "Unidentified"].includes(e.key)) return null;

  const parts: string[] = [];
  if (e.ctrlKey || (IS_MAC && e.metaKey)) parts.push("Ctrl");
  if (e.shiftKey) parts.push("Shift");
  if (e.altKey) parts.push("Alt");

  let key = e.key;
  if (key === " ") key = "Space";
  else if (key === "Escape") key = "Esc";
  else if (key.length === 1) {
    // With Alt (macOS Option) or non-Latin layouts, prefer the physical letter.
    const physical = codeToKey(e.code);
    if (physical && !/[a-z0-9]/i.test(key)) key = physical;
    key = key.toUpperCase();
  }
  // Named keys like Enter, Tab, etc. are already capitalized

  parts.push(key);
  return parts.join("+");
}

// ── Registry ─────────────────────────────────────────────────────────

type ActionHandler = () => void;

const handlers = new Map<string, ActionHandler>();
let overrides: Record<string, string> = {};
let parsedBindings: Map<string, ParsedShortcut> = new Map();
let installed = false;

function rebuildParsedBindings() {
  parsedBindings = new Map();
  for (const action of ACTIONS) {
    const shortcut = overrides[action.id] || action.defaultShortcut;
    if (shortcut) {
      parsedBindings.set(action.id, parseShortcut(shortcut));
    }
  }
}

function handleKeydown(e: KeyboardEvent) {
  // Ignore IME composition and events another handler already consumed.
  if (e.isComposing || e.defaultPrevented) return;
  // While a modal dialog is open, global shortcuts must not act behind it.
  if (document.querySelector('[aria-modal="true"]')) return;

  // Don't intercept when focused on an input/textarea/select/contenteditable
  const target = e.target as HTMLElement | null;
  const tag = target?.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || target?.isContentEditable) {
    // Exception: allow Ctrl+Enter for commit even in textarea
    const commitBinding = parsedBindings.get("commit");
    if (commitBinding && matchesEvent(commitBinding, e)) {
      const handler = handlers.get("commit");
      if (handler) {
        e.preventDefault();
        handler();
      }
    }
    return;
  }

  for (const [actionId, parsed] of parsedBindings) {
    if (matchesEvent(parsed, e)) {
      const handler = handlers.get(actionId);
      if (handler) {
        e.preventDefault();
        handler();
      }
      return;
    }
  }
}

/** Set keybinding overrides (from settings). Call whenever overrides change. */
export function setOverrides(newOverrides: Record<string, string>) {
  overrides = newOverrides ?? {};
  rebuildParsedBindings();
}

/** Register a handler for an action. Returns an unsubscribe function. */
export function onAction(actionId: string, handler: ActionHandler): () => void {
  handlers.set(actionId, handler);
  return () => {
    // Don't remove a handler that a newer registration has since replaced.
    if (handlers.get(actionId) === handler) handlers.delete(actionId);
  };
}

/** Get the current effective shortcut for an action. */
export function getShortcut(actionId: string): string {
  return overrides[actionId] || ACTIONS.find((a) => a.id === actionId)?.defaultShortcut || "";
}

/** Install the global keydown listener. Call once at startup. */
export function installKeybindings() {
  if (installed) return;
  rebuildParsedBindings();
  window.addEventListener("keydown", handleKeydown);
  installed = true;
}
