/**
 * Global keybinding registry.
 *
 * Each action has a default shortcut and can be overridden via settings.
 * Shortcuts are stored as strings like "Ctrl+Enter", "Ctrl+Shift+B".
 * Modifiers: Ctrl, Shift, Alt. On macOS, Ctrl also matches Cmd; on other
 * platforms the Super/Meta key never matches.
 */

import { writable } from "svelte/store";
import { tr, type MessageKey } from "./i18n";

// ── Action definitions ───────────────────────────────────────────────

interface ActionDef {
  id: string;
  /** Catalog key of the action's name (palette, keybinding settings). */
  labelKey: MessageKey;
  /** Catalog key of the group it is listed under. */
  categoryKey: MessageKey;
  /** Empty string = no default shortcut (still listed, rebindable, and in the palette). */
  defaultShortcut: string;
  /** Also fire while focus is in an input/textarea (e.g. commit, command palette). */
  allowInInputs?: boolean;
  /** Hide from the command palette (e.g. actions that only make sense as a key press). */
  hideInPalette?: boolean;
}

export interface KeybindingAction extends ActionDef {
  /** Name in the current language (not reactive; use `$t(labelKey)` in markup). */
  readonly label: string;
  /** Category in the current language (not reactive; use `$t(categoryKey)` in markup). */
  readonly category: string;
}

const ACTION_DEFS: ActionDef[] = [
  // Navigation
  { id: "open_repo",      labelKey: "actions.openRepo",      categoryKey: "actions.category.navigation", defaultShortcut: "Ctrl+O" },
  { id: "open_repo_menu", labelKey: "actions.openRepoMenu", categoryKey: "actions.category.repository", defaultShortcut: "" },
  { id: "clone_from_url", labelKey: "actions.cloneFromUrl", categoryKey: "actions.category.repository", defaultShortcut: "" },
  { id: "init_repository", labelKey: "actions.initRepository", categoryKey: "actions.category.repository", defaultShortcut: "" },
  { id: "close_tab",      labelKey: "actions.closeTab",            categoryKey: "actions.category.navigation", defaultShortcut: "Ctrl+W" },
  { id: "next_tab",       labelKey: "actions.nextTab",             categoryKey: "actions.category.navigation", defaultShortcut: "Ctrl+Tab" },
  { id: "prev_tab",       labelKey: "actions.prevTab",         categoryKey: "actions.category.navigation", defaultShortcut: "Ctrl+Shift+Tab" },
  { id: "go_home",        labelKey: "actions.goHome",    categoryKey: "actions.category.navigation", defaultShortcut: "Ctrl+H" },
  { id: "go_settings",    labelKey: "actions.goSettings",        categoryKey: "actions.category.navigation", defaultShortcut: "Ctrl+," },

  // Sidebar & panels
  { id: "toggle_sidebar", labelKey: "actions.toggleSidebar",       categoryKey: "actions.category.panels",     defaultShortcut: "Ctrl+B" },

  // Git operations
  { id: "commit",         labelKey: "actions.commit",               categoryKey: "actions.category.git",        defaultShortcut: "Ctrl+Enter" },
  { id: "push",           labelKey: "actions.push",                 categoryKey: "actions.category.git",        defaultShortcut: "Ctrl+Shift+P" },
  { id: "pull",           labelKey: "actions.pull",                 categoryKey: "actions.category.git",        defaultShortcut: "Ctrl+Shift+L" },
  { id: "fetch",          labelKey: "actions.fetch",            categoryKey: "actions.category.git",        defaultShortcut: "Ctrl+Shift+F" },

  // Commit context menu, undo history, tags
  { id: "commit_context_menu",  labelKey: "actions.commitContextMenu",          categoryKey: "actions.category.commit", defaultShortcut: "Shift+F10" },
  { id: "branch_from_selected", labelKey: "actions.branchFromSelected", categoryKey: "actions.category.commit", defaultShortcut: "" },
  { id: "create_tag",           labelKey: "actions.createTag",    categoryKey: "actions.category.commit", defaultShortcut: "" },
  { id: "save_patch_selected",  labelKey: "actions.savePatchSelected",  categoryKey: "actions.category.commit", defaultShortcut: "" },
  { id: "save_working_patch",   labelKey: "actions.saveWorkingPatch",    categoryKey: "actions.category.commit", defaultShortcut: "" },
  { id: "apply_patch",          labelKey: "actions.applyPatch",                 categoryKey: "actions.category.commit", defaultShortcut: "" },
  { id: "search_code",          labelKey: "actions.searchCode",                      categoryKey: "actions.category.navigation", defaultShortcut: "Ctrl+Shift+G" },
  { id: "bisect_start",         labelKey: "actions.bisectStart", categoryKey: "actions.category.commit", defaultShortcut: "" },
  { id: "bisect_good",          labelKey: "actions.bisectGood",  categoryKey: "actions.category.commit", defaultShortcut: "" },
  { id: "bisect_bad",           labelKey: "actions.bisectBad",   categoryKey: "actions.category.commit", defaultShortcut: "" },
  { id: "bisect_skip",          labelKey: "actions.bisectSkip",       categoryKey: "actions.category.commit", defaultShortcut: "" },
  { id: "bisect_reset",         labelKey: "actions.bisectReset",                     categoryKey: "actions.category.commit", defaultShortcut: "" },
  { id: "squash_selected",      labelKey: "actions.squashSelected",          categoryKey: "actions.category.commit", defaultShortcut: "" },
  { id: "cherry_pick_selected", labelKey: "actions.cherryPickSelected",    categoryKey: "actions.category.commit", defaultShortcut: "" },
  { id: "revert_selected",      labelKey: "actions.revertSelected",            categoryKey: "actions.category.commit", defaultShortcut: "" },
  { id: "copy_commit_sha",      labelKey: "actions.copyCommitSha",          categoryKey: "actions.category.commit", defaultShortcut: "" },
  { id: "show_undo_history",    labelKey: "actions.showUndoHistory",            categoryKey: "actions.category.git",    defaultShortcut: "Ctrl+Shift+H" },
  { id: "push_all_tags",        labelKey: "actions.pushAllTags",                     categoryKey: "actions.category.git",    defaultShortcut: "" },
  // Staging panel: file lists & commit helpers
  { id: "staging_stage_all",      labelKey: "actions.stagingStageAll",              categoryKey: "actions.category.staging", defaultShortcut: "" },
  { id: "staging_unstage_all",    labelKey: "actions.stagingUnstageAll",            categoryKey: "actions.category.staging", defaultShortcut: "" },
  { id: "staging_toggle_tree",    labelKey: "actions.stagingToggleTree",   categoryKey: "actions.category.staging", defaultShortcut: "" },
  { id: "staging_filter_files",   labelKey: "actions.stagingFilterFiles",           categoryKey: "actions.category.staging", defaultShortcut: "" },
  { id: "commit_toggle_amend",    labelKey: "actions.commitToggleAmend",       categoryKey: "actions.category.staging", defaultShortcut: "" },
  { id: "commit_toggle_signoff",  labelKey: "actions.commitToggleSignoff",   categoryKey: "actions.category.staging", defaultShortcut: "" },
  { id: "commit_add_coauthor",    labelKey: "actions.commitAddCoauthor",        categoryKey: "actions.category.staging", defaultShortcut: "" },
  { id: "commit_insert_template", labelKey: "actions.commitInsertTemplate", categoryKey: "actions.category.staging", defaultShortcut: "" },
  { id: "commit_message_history", labelKey: "actions.commitMessageHistory",         categoryKey: "actions.category.staging", defaultShortcut: "" },
  // Branch list / remotes
  { id: "remotes_manage",        labelKey: "actions.remotesManage",        categoryKey: "actions.category.git",        defaultShortcut: "" },
  { id: "branch_create",         labelKey: "actions.branchCreate",         categoryKey: "actions.category.git",        defaultShortcut: "" },
  { id: "branch_rename_current", labelKey: "actions.branchRenameCurrent", categoryKey: "actions.category.git",        defaultShortcut: "" },
  { id: "branch_filter",         labelKey: "actions.branchFilter",        categoryKey: "actions.category.navigation", defaultShortcut: "" },
  // Conflict resolution & history rewriting (rebase, force push)
  { id: "force_push",         labelKey: "actions.forcePush",          categoryKey: "actions.category.git", defaultShortcut: "" },
  { id: "rebase_onto",        labelKey: "actions.rebaseOnto",      categoryKey: "actions.category.git", defaultShortcut: "" },
  { id: "interactive_rebase", labelKey: "actions.interactiveRebase",              categoryKey: "actions.category.git", defaultShortcut: "" },
  { id: "operation_continue", labelKey: "actions.operationContinue", categoryKey: "actions.category.git", defaultShortcut: "" },
  { id: "operation_abort",    labelKey: "actions.operationAbort",    categoryKey: "actions.category.git", defaultShortcut: "" },
  { id: "operation_skip",     labelKey: "actions.operationSkip", categoryKey: "actions.category.git", defaultShortcut: "" },
  { id: "resolve_conflicts",  labelKey: "actions.resolveConflicts",                categoryKey: "actions.category.git", defaultShortcut: "" },
  // Commit graph (Ctrl+F / "/" also open search while the commit list is focused)
  { id: "graph_search",                labelKey: "actions.graphSearch",                         categoryKey: "actions.category.graph", defaultShortcut: "" },
  { id: "graph_filter",                labelKey: "actions.graphFilter",                         categoryKey: "actions.category.graph", defaultShortcut: "" },
  { id: "graph_jump_head",             labelKey: "actions.graphJumpHead",                           categoryKey: "actions.category.graph", defaultShortcut: "" },
  { id: "graph_goto_ref",              labelKey: "actions.graphGotoRef",           categoryKey: "actions.category.graph", defaultShortcut: "" },
  { id: "graph_view_options",          labelKey: "actions.graphViewOptions",                     categoryKey: "actions.category.graph", defaultShortcut: "" },
  { id: "graph_load_all",              labelKey: "actions.graphLoadAll",                    categoryKey: "actions.category.graph", defaultShortcut: "" },
  { id: "graph_toggle_remotes",        labelKey: "actions.graphToggleRemotes",        categoryKey: "actions.category.graph", defaultShortcut: "" },
  { id: "graph_toggle_current_branch", labelKey: "actions.graphToggleCurrentBranch",    categoryKey: "actions.category.graph", defaultShortcut: "" },
  // File history & blame, stash extras, submodules, worktrees
  { id: "file_history",      labelKey: "actions.fileHistory",            categoryKey: "actions.category.history",    defaultShortcut: "" },
  { id: "blame_file",        labelKey: "actions.blameFile",                   categoryKey: "actions.category.history",    defaultShortcut: "" },
  { id: "stash_files",       labelKey: "actions.stashFiles",    categoryKey: "actions.category.git",        defaultShortcut: "" },
  { id: "submodules_update", labelKey: "actions.submodulesUpdate", categoryKey: "actions.category.git", defaultShortcut: "" },
  { id: "submodules_sync",   labelKey: "actions.submodulesSync",      categoryKey: "actions.category.git",        defaultShortcut: "" },
  { id: "new_window",        labelKey: "actions.newWindow",               categoryKey: "actions.category.application", defaultShortcut: "Ctrl+Shift+N" },
  { id: "repo_dashboard",    labelKey: "actions.repoDashboard", categoryKey: "actions.category.repository", defaultShortcut: "" },
  { id: "lfs_manage",        labelKey: "actions.lfsManage",                 categoryKey: "actions.category.git",        defaultShortcut: "" },
  { id: "worktree_add",      labelKey: "actions.worktreeAdd",            categoryKey: "actions.category.git",        defaultShortcut: "" },
  // App shell: command palette, tabs, settings, updater
  { id: "command_palette",      labelKey: "actions.commandPalette",                 categoryKey: "actions.category.navigation",  defaultShortcut: "Ctrl+K", allowInInputs: true, hideInPalette: true },
  { id: "move_tab_left",        labelKey: "actions.moveTabLeft",                   categoryKey: "actions.category.navigation",  defaultShortcut: "Ctrl+Shift+PageUp" },
  { id: "move_tab_right",       labelKey: "actions.moveTabRight",                  categoryKey: "actions.category.navigation",  defaultShortcut: "Ctrl+Shift+PageDown" },
  { id: "toggle_favorite_repo", labelKey: "actions.toggleFavoriteRepo", categoryKey: "actions.category.repository", defaultShortcut: "" },
  { id: "reveal_repo",          labelKey: "actions.revealRepo",          categoryKey: "actions.category.repository", defaultShortcut: "" },
  { id: "open_terminal",        labelKey: "actions.openTerminal",     categoryKey: "actions.category.repository", defaultShortcut: "" },
  { id: "open_editor",          labelKey: "actions.openEditor",       categoryKey: "actions.category.repository", defaultShortcut: "" },
  { id: "copy_repo_path",       labelKey: "actions.copyRepoPath",            categoryKey: "actions.category.repository", defaultShortcut: "" },
  { id: "check_for_updates",    labelKey: "actions.checkForUpdates",               categoryKey: "actions.category.application", defaultShortcut: "" },
  { id: "open_settings_folder", labelKey: "actions.openSettingsFolder",            categoryKey: "actions.category.application", defaultShortcut: "" },
  { id: "export_settings",      labelKey: "actions.exportSettings",                categoryKey: "actions.category.application", defaultShortcut: "" },
  { id: "import_settings",      labelKey: "actions.importSettings",                categoryKey: "actions.category.application", defaultShortcut: "" },
  // Hosting integrations (GitHub / GitLab / Gitea)
  { id: "open_pull_requests",  labelKey: "actions.openPullRequests",   categoryKey: "actions.category.hosting", defaultShortcut: "" },
  { id: "create_pull_request", labelKey: "actions.createPullRequest",  categoryKey: "actions.category.hosting", defaultShortcut: "" },
  { id: "refresh_ci_status",   labelKey: "actions.refreshCiStatus",    categoryKey: "actions.category.hosting", defaultShortcut: "" },
  // Diff viewer
  { id: "diff_next_hunk", labelKey: "actions.diffNextHunk",  categoryKey: "actions.category.diff",       defaultShortcut: "Alt+ArrowDown" },
  { id: "diff_prev_hunk", labelKey: "actions.diffPrevHunk", categoryKey: "actions.category.diff",    defaultShortcut: "Alt+ArrowUp" },
  // Ctrl+F is handled by the diff panel itself while it has focus.
  { id: "diff_find",      labelKey: "actions.diffFind",         categoryKey: "actions.category.diff",       defaultShortcut: "" },
  { id: "diff_toggle_whitespace", labelKey: "actions.diffToggleWhitespace", categoryKey: "actions.category.diff", defaultShortcut: "" },
];

export const ACTIONS: KeybindingAction[] = ACTION_DEFS.map((def) => ({
  ...def,
  get label() {
    return tr(def.labelKey);
  },
  get category() {
    return tr(def.categoryKey);
  },
}));

/** Name of an action in the current language, or its id when unknown. */
export function actionLabel(actionId: string): string {
  return ACTIONS.find((a) => a.id === actionId)?.label ?? actionId;
}

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
