/**
 * Global keybinding registry.
 *
 * Each action has a default shortcut and can be overridden via settings.
 * Shortcuts are stored as strings like "Ctrl+Enter", "Ctrl+Shift+B".
 * Modifiers: Ctrl, Shift, Alt. On macOS, Ctrl also matches Cmd; on other
 * platforms the Super/Meta key never matches.
 */

import { writable } from "svelte/store";

// ── Action definitions ───────────────────────────────────────────────

export interface KeybindingAction {
  id: string;
  label: string;
  category: string;
  /** Empty string = no default shortcut (still listed, rebindable, and in the palette). */
  defaultShortcut: string;
  /** Also fire while focus is in an input/textarea (e.g. commit, command palette). */
  allowInInputs?: boolean;
  /** Hide from the command palette (e.g. actions that only make sense as a key press). */
  hideInPalette?: boolean;
}

export const ACTIONS: KeybindingAction[] = [
  // Navigation
  { id: "open_repo",      label: "Open repository",      category: "Navigation", defaultShortcut: "Ctrl+O" },
  { id: "open_repo_menu", label: "Open repository menu (search repositories)", category: "Repository", defaultShortcut: "" },
  { id: "clone_from_url", label: "Clone repository from URL…", category: "Repository", defaultShortcut: "" },
  { id: "init_repository", label: "Initialize new repository…", category: "Repository", defaultShortcut: "" },
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

  // Commit context menu, undo history, tags
  { id: "commit_context_menu",  label: "Open commit context menu",          category: "Commit", defaultShortcut: "Shift+F10" },
  { id: "branch_from_selected", label: "Create branch at selected commit…", category: "Commit", defaultShortcut: "" },
  { id: "create_tag",           label: "Create tag at selected commit…",    category: "Commit", defaultShortcut: "" },
  { id: "cherry_pick_selected", label: "Cherry-pick selected commit(s)",    category: "Commit", defaultShortcut: "" },
  { id: "revert_selected",      label: "Revert selected commit",            category: "Commit", defaultShortcut: "" },
  { id: "copy_commit_sha",      label: "Copy selected commit SHA",          category: "Commit", defaultShortcut: "" },
  { id: "show_undo_history",    label: "Undo history (reflog)…",            category: "Git",    defaultShortcut: "Ctrl+Shift+H" },
  { id: "push_all_tags",        label: "Push all tags",                     category: "Git",    defaultShortcut: "" },
  // Staging panel: file lists & commit helpers
  { id: "staging_stage_all",      label: "Stage all changes",              category: "Staging", defaultShortcut: "" },
  { id: "staging_unstage_all",    label: "Unstage all changes",            category: "Staging", defaultShortcut: "" },
  { id: "staging_toggle_tree",    label: "Toggle tree / flat file list",   category: "Staging", defaultShortcut: "" },
  { id: "staging_filter_files",   label: "Filter changed files",           category: "Staging", defaultShortcut: "" },
  { id: "commit_toggle_amend",    label: "Toggle amend last commit",       category: "Staging", defaultShortcut: "" },
  { id: "commit_toggle_signoff",  label: "Toggle Signed-off-by trailer",   category: "Staging", defaultShortcut: "" },
  { id: "commit_add_coauthor",    label: "Add co-author to commit",        category: "Staging", defaultShortcut: "" },
  { id: "commit_insert_template", label: "Insert commit message template", category: "Staging", defaultShortcut: "" },
  { id: "commit_message_history", label: "Recent commit messages",         category: "Staging", defaultShortcut: "" },
  // Branch list / remotes
  { id: "remotes_manage",        label: "Manage remotes…",        category: "Git",        defaultShortcut: "" },
  { id: "branch_create",         label: "Create branch…",         category: "Git",        defaultShortcut: "" },
  { id: "branch_rename_current", label: "Rename current branch…", category: "Git",        defaultShortcut: "" },
  { id: "branch_filter",         label: "Filter branches",        category: "Navigation", defaultShortcut: "" },
  // Conflict resolution & history rewriting (rebase, force push)
  { id: "force_push",         label: "Force push (with lease)",          category: "Git", defaultShortcut: "" },
  { id: "rebase_onto",        label: "Rebase current branch onto…",      category: "Git", defaultShortcut: "" },
  { id: "interactive_rebase", label: "Interactive rebase…",              category: "Git", defaultShortcut: "" },
  { id: "operation_continue", label: "Continue merge / rebase / cherry-pick", category: "Git", defaultShortcut: "" },
  { id: "operation_abort",    label: "Abort merge / rebase / cherry-pick",    category: "Git", defaultShortcut: "" },
  { id: "operation_skip",     label: "Skip current commit (rebase / cherry-pick)", category: "Git", defaultShortcut: "" },
  { id: "resolve_conflicts",  label: "Resolve conflicts",                category: "Git", defaultShortcut: "" },
  // Commit graph (Ctrl+F / "/" also open search while the commit list is focused)
  { id: "graph_search",                label: "Search commits",                         category: "Graph", defaultShortcut: "" },
  { id: "graph_filter",                label: "Filter commits",                         category: "Graph", defaultShortcut: "" },
  { id: "graph_jump_head",             label: "Jump to HEAD",                           category: "Graph", defaultShortcut: "" },
  { id: "graph_goto_ref",              label: "Go to branch, tag or commit…",           category: "Graph", defaultShortcut: "" },
  { id: "graph_view_options",          label: "Graph view options",                     category: "Graph", defaultShortcut: "" },
  { id: "graph_load_all",              label: "Load entire history",                    category: "Graph", defaultShortcut: "" },
  { id: "graph_toggle_remotes",        label: "Toggle remote branches in graph",        category: "Graph", defaultShortcut: "" },
  { id: "graph_toggle_current_branch", label: "Toggle current branch only in graph",    category: "Graph", defaultShortcut: "" },
  // File history & blame, stash extras, submodules, worktrees
  { id: "file_history",      label: "File history…",            category: "History",    defaultShortcut: "" },
  { id: "blame_file",        label: "Blame…",                   category: "History",    defaultShortcut: "" },
  { id: "stash_files",       label: "Stash selected files…",    category: "Git",        defaultShortcut: "" },
  { id: "submodules_update", label: "Update submodules (init, recursive)", category: "Git", defaultShortcut: "" },
  { id: "submodules_sync",   label: "Sync submodule URLs",      category: "Git",        defaultShortcut: "" },
  { id: "worktree_add",      label: "Add worktree…",            category: "Git",        defaultShortcut: "" },
  // App shell: command palette, tabs, settings, updater
  { id: "command_palette",      label: "Command palette",                 category: "Navigation",  defaultShortcut: "Ctrl+K", allowInInputs: true, hideInPalette: true },
  { id: "move_tab_left",        label: "Move tab left",                   category: "Navigation",  defaultShortcut: "Ctrl+Shift+PageUp" },
  { id: "move_tab_right",       label: "Move tab right",                  category: "Navigation",  defaultShortcut: "Ctrl+Shift+PageDown" },
  { id: "toggle_favorite_repo", label: "Pin/unpin repository to favorites", category: "Repository", defaultShortcut: "" },
  { id: "reveal_repo",          label: "Open repository folder",          category: "Repository", defaultShortcut: "" },
  { id: "open_terminal",        label: "Open repository in terminal",     category: "Repository", defaultShortcut: "" },
  { id: "open_editor",          label: "Open repository in editor",       category: "Repository", defaultShortcut: "" },
  { id: "copy_repo_path",       label: "Copy repository path",            category: "Repository", defaultShortcut: "" },
  { id: "check_for_updates",    label: "Check for updates",               category: "Application", defaultShortcut: "" },
  { id: "open_settings_folder", label: "Open settings folder",            category: "Application", defaultShortcut: "" },
  { id: "export_settings",      label: "Export settings…",                category: "Application", defaultShortcut: "" },
  { id: "import_settings",      label: "Import settings…",                category: "Application", defaultShortcut: "" },
  // Hosting integrations (GitHub / GitLab / Gitea)
  { id: "open_pull_requests",  label: "Show pull requests",   category: "Hosting", defaultShortcut: "" },
  { id: "create_pull_request", label: "Create pull request",  category: "Hosting", defaultShortcut: "" },
  { id: "refresh_ci_status",   label: "Refresh CI status",    category: "Hosting", defaultShortcut: "" },
  // Diff viewer
  { id: "diff_next_hunk", label: "Next change in diff",  category: "Diff",       defaultShortcut: "Alt+ArrowDown" },
  { id: "diff_prev_hunk", label: "Previous change in diff", category: "Diff",    defaultShortcut: "Alt+ArrowUp" },
  // Ctrl+F is handled by the diff panel itself while it has focus.
  { id: "diff_find",      label: "Find in diff",         category: "Diff",       defaultShortcut: "" },
  { id: "diff_toggle_whitespace", label: "Toggle whitespace changes in diffs", category: "Diff", defaultShortcut: "" },
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

/**
 * Effective shortcut of an action: its override when one is stored (an empty
 * override means "unbound"), else its default.
 */
export function resolveShortcut(overrideMap: Record<string, string>, actionId: string): string {
  if (Object.prototype.hasOwnProperty.call(overrideMap, actionId)) return overrideMap[actionId] ?? "";
  return ACTIONS.find((a) => a.id === actionId)?.defaultShortcut ?? "";
}

function rebuildParsedBindings() {
  parsedBindings = new Map();
  for (const action of ACTIONS) {
    const shortcut = resolveShortcut(overrides, action.id);
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
    // Exceptions: commit (Ctrl+Enter in the message box) and actions flagged
    // `allowInInputs` (e.g. the command palette) still fire.
    for (const action of ACTIONS) {
      if (action.id !== "commit" && !action.allowInInputs) continue;
      const binding = parsedBindings.get(action.id);
      const handler = handlers.get(action.id);
      if (binding && handler && matchesEvent(binding, e)) {
        e.preventDefault();
        handler();
        return;
      }
    }
    return;
  }

  // First bound action that currently has a handler wins: a conflicting
  // binding whose component isn't mounted must not swallow the key.
  for (const [actionId, parsed] of parsedBindings) {
    const handler = handlers.get(actionId);
    if (handler && matchesEvent(parsed, e)) {
      e.preventDefault();
      handler();
      return;
    }
  }
}

/** Set keybinding overrides (from settings). Call whenever overrides change. */
export function setOverrides(newOverrides: Record<string, string>) {
  overrides = newOverrides ?? {};
  rebuildParsedBindings();
  shortcutLabels.set(currentShortcutMap());
}

function currentShortcutMap(): Record<string, string> {
  const map: Record<string, string> = {};
  for (const action of ACTIONS) {
    const sc = resolveShortcut(overrides, action.id);
    if (sc) map[action.id] = sc;
  }
  return map;
}

/**
 * Reactive map of action id -> effective shortcut (only bound actions).
 * Use `$shortcutLabels[id]` in templates so tooltips follow rebinding.
 */
export const shortcutLabels = writable<Record<string, string>>(currentShortcutMap());

/** Current shortcut for an action, or "" when unbound (non-reactive). */
export function shortcutFor(actionId: string): string {
  return getShortcut(actionId);
}

/** "Push (Ctrl+Shift+P)" — a tooltip label with the action's shortcut, if any. */
export function withShortcut(label: string, shortcut: string | undefined): string {
  return shortcut ? `${label} (${shortcut})` : label;
}

/** Whether a component currently handles this action. */
export function hasActionHandler(actionId: string): boolean {
  return handlers.has(actionId);
}

/** Run an action's handler as if its shortcut were pressed. Returns false if none is registered. */
export function runAction(actionId: string): boolean {
  const handler = handlers.get(actionId);
  if (!handler) return false;
  handler();
  return true;
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
  return resolveShortcut(overrides, actionId);
}

/** Install the global keydown listener. Call once at startup. */
export function installKeybindings() {
  if (installed) return;
  rebuildParsedBindings();
  window.addEventListener("keydown", handleKeydown);
  installed = true;
}
