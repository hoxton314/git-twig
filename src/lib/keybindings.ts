/**
 * Global keybinding registry.
 *
 * Each action has a default shortcut and can be overridden via settings.
 * Shortcuts are stored as strings like "Ctrl+Enter", "Ctrl+Shift+B".
 * Modifiers: Ctrl, Shift, Alt. On macOS, Ctrl maps to Cmd.
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
];

// ── Shortcut parsing & matching ──────────────────────────────────────

interface ParsedShortcut {
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  key: string;  // lowercase key name
}

function parseShortcut(shortcut: string): ParsedShortcut {
  const parts = shortcut.split("+").map((p) => p.trim());
  return {
    ctrl: parts.includes("Ctrl"),
    shift: parts.includes("Shift"),
    alt: parts.includes("Alt"),
    key: parts[parts.length - 1].toLowerCase(),
  };
}

function matchesEvent(parsed: ParsedShortcut, e: KeyboardEvent): boolean {
  const ctrl = e.ctrlKey || e.metaKey;
  if (parsed.ctrl !== ctrl) return false;
  if (parsed.shift !== e.shiftKey) return false;
  if (parsed.alt !== e.altKey) return false;

  // Normalize key names
  let eventKey = e.key.toLowerCase();
  if (eventKey === " ") eventKey = "space";
  if (eventKey === "escape") eventKey = "esc";

  return eventKey === parsed.key;
}

// ── Shortcut display helpers ─────────────────────────────────────────

/** Convert a KeyboardEvent to a shortcut string (for capture UI). */
export function eventToShortcut(e: KeyboardEvent): string | null {
  // Ignore bare modifier presses
  if (["Control", "Shift", "Alt", "Meta"].includes(e.key)) return null;

  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push("Ctrl");
  if (e.shiftKey) parts.push("Shift");
  if (e.altKey) parts.push("Alt");

  let key = e.key;
  if (key === " ") key = "Space";
  else if (key === "Escape") key = "Esc";
  else if (key.length === 1) key = key.toUpperCase();
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
  // Don't intercept when focused on an input/textarea/select
  const tag = (e.target as HTMLElement)?.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") {
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
  overrides = newOverrides;
  rebuildParsedBindings();
}

/** Register a handler for an action. Returns an unsubscribe function. */
export function onAction(actionId: string, handler: ActionHandler): () => void {
  handlers.set(actionId, handler);
  return () => { handlers.delete(actionId); };
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
