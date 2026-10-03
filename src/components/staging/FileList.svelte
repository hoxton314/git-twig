<script module lang="ts">
  import type { Plus } from "lucide-svelte";
  import type { FileStatus } from "../../lib/types/git";

  /** A per-row action button (also shown on folder rows in tree view). */
  export interface RowAction {
    /** Any lucide-svelte icon component. */
    icon: typeof Plus;
    /** Tooltip for a single file. */
    title: string;
    /** Tooltip for a folder row. */
    folderTitle: string;
    /** CSS modifier: "stage" (green hover) or "danger" (red hover). */
    tone: "stage" | "danger";
    run: (files: FileStatus[]) => void;
  }

  /** What a context menu was requested for. */
  export type MenuTarget =
    | { kind: "file"; file: FileStatus }
    | { kind: "folder"; path: string; files: FileStatus[] };
</script>

<script lang="ts">
  import {
    ChevronDown,
    ChevronRight,
    FileText,
    FilePlus,
    FileX,
    FilePen,
    Folder,
    FolderOpen,
    AlertTriangle,
  } from "lucide-svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { buildTree, visibleRows } from "./fileTree";
  import { t, type MessageKey } from "../../lib/i18n";

  const STATUS_LABELS: Record<string, MessageKey> = {
    added: "staging.statusAdded",
    modified: "staging.statusModified",
    deleted: "staging.statusDeleted",
    renamed: "staging.statusRenamed",
    copied: "staging.statusCopied",
    typechange: "staging.statusTypechange",
    untracked: "staging.statusUntracked",
    conflicted: "staging.statusConflicted",
  };

  function statusLabel(status: string): string {
    const key = STATUS_LABELS[status];
    return key ? $t(key) : status;
  }

  interface Props {
    files: FileStatus[];
    tree: boolean;
    /** Path of the selected file in this list, if any. */
    selectedPath: string | null;
    emptyText: string;
    actions: RowAction[];
    onselect: (file: FileStatus) => void;
    onmenu: (target: MenuTarget, x: number, y: number) => void;
  }

  let { files, tree, selectedPath, emptyText, actions, onselect, onmenu }: Props = $props();

  const collapsed = new SvelteSet<string>();
  const rows = $derived(tree ? visibleRows(buildTree(files), collapsed) : []);

  let listEl = $state<HTMLDivElement | null>(null);

  function statusIcon(status: string) {
    switch (status) {
      case "added":
      case "untracked":
        return FilePlus;
      case "deleted":
        return FileX;
      case "modified":
        return FilePen;
      case "conflicted":
        return AlertTriangle;
      default:
        return FileText;
    }
  }

  function statusColor(status: string): string {
    switch (status) {
      case "added":
      case "untracked":
        return "var(--color-diff-add-text)";
      case "deleted":
        return "var(--color-diff-del-text)";
      case "modified":
      case "renamed":
        return "var(--color-lane-2)";
      case "conflicted":
        return "var(--color-diff-del-text)";
      default:
        return "var(--color-text-muted)";
    }
  }

  function toggleDir(path: string) {
    if (collapsed.has(path)) collapsed.delete(path);
    else collapsed.add(path);
  }

  function openMenuFromKeyboard(el: HTMLElement, target: MenuTarget) {
    const rect = el.getBoundingClientRect();
    onmenu(target, rect.left + 24, rect.bottom);
  }

  function moveFocus(from: HTMLElement, delta: number) {
    if (!listEl) return;
    const items = Array.from(listEl.querySelectorAll<HTMLElement>("[data-row]"));
    const i = items.indexOf(from);
    const next = items[i + delta];
    if (next) next.focus();
  }

  // Keyboard handling for rows: Enter/Space activates, the ContextMenu key
  // (or Shift+F10) opens the context menu, arrows move between rows and
  // Left/Right collapse/expand folders. Keys from nested buttons are ignored.
  function onRowKey(e: KeyboardEvent, target: MenuTarget, activate: () => void) {
    if (e.target !== e.currentTarget) return;
    const el = e.currentTarget as HTMLElement;
    if (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) {
      e.preventDefault();
      openMenuFromKeyboard(el, target);
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      activate();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      moveFocus(el, 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      moveFocus(el, -1);
    } else if (target.kind === "folder" && e.key === "ArrowLeft" && !collapsed.has(target.path)) {
      e.preventDefault();
      collapsed.add(target.path);
    } else if (target.kind === "folder" && e.key === "ArrowRight" && collapsed.has(target.path)) {
      e.preventDefault();
      collapsed.delete(target.path);
    }
  }

  function onRowMenu(e: MouseEvent, target: MenuTarget) {
    e.preventDefault();
    e.stopPropagation();
    onmenu(target, e.clientX, e.clientY);
  }
</script>

{#snippet fileRow(file: FileStatus, label: string, depth: number)}
  {@const Icon = statusIcon(file.status)}
  {@const target = { kind: "file", file } as const}
  <div
    class="file-item"
    class:selected={selectedPath === file.path}
    style="padding-left: {16 + depth * 12}px"
    onclick={() => onselect(file)}
    oncontextmenu={(e) => onRowMenu(e, target)}
    onkeydown={(e) => onRowKey(e, target, () => onselect(file))}
    role="button"
    tabindex="0"
    data-row
    aria-haspopup="menu"
  >
    <Icon size={13} color={statusColor(file.status)} />
    <span class="file-name" title="{file.path} ({statusLabel(file.status)})">{label}</span>
    {#each actions as action (action.title)}
      {@const ActionIcon = action.icon}
      <button
        class="action-btn {action.tone}"
        onclick={(e) => { e.stopPropagation(); action.run([file]); }}
        title={action.title}
        aria-label={action.title}
      >
        <ActionIcon size={12} />
      </button>
    {/each}
  </div>
{/snippet}

<div class="file-list" bind:this={listEl}>
  {#if files.length === 0}
    <div class="empty-msg">{emptyText}</div>
  {:else if tree}
    {#each rows as row (row.node.kind === "dir" ? `d:${row.node.path}` : `f:${row.node.file.path}`)}
      {#if row.node.kind === "dir"}
        {@const dir = row.node}
        {@const open = !collapsed.has(dir.path)}
        {@const target = { kind: "folder", path: dir.path, files: dir.files } as const}
        <div
          class="file-item folder-item"
          style="padding-left: {8 + row.depth * 12}px"
          onclick={() => toggleDir(dir.path)}
          oncontextmenu={(e) => onRowMenu(e, target)}
          onkeydown={(e) => onRowKey(e, target, () => toggleDir(dir.path))}
          role="button"
          tabindex="0"
          data-row
          aria-expanded={open}
          aria-haspopup="menu"
        >
          {#if open}
            <ChevronDown size={12} />
            <FolderOpen size={13} />
          {:else}
            <ChevronRight size={12} />
            <Folder size={13} />
          {/if}
          <span class="file-name" title={dir.path}>{dir.name}</span>
          <span class="folder-count">{dir.files.length}</span>
          {#each actions as action (action.title)}
            {@const ActionIcon = action.icon}
            <button
              class="action-btn {action.tone}"
              onclick={(e) => { e.stopPropagation(); action.run(dir.files); }}
              title={action.folderTitle}
              aria-label={action.folderTitle}
            >
              <ActionIcon size={12} />
            </button>
          {/each}
        </div>
      {:else}
        {@render fileRow(row.node.file, row.node.name, row.depth)}
      {/if}
    {/each}
  {:else}
    {#each files as file (file.path)}
      {@render fileRow(file, file.path, 0)}
    {/each}
  {/if}
</div>

<style>
  .file-list {
    max-height: 200px;
    overflow-y: auto;
  }

  .file-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 8px 3px 16px;
    cursor: pointer;
    transition: background 0.1s;
    color: var(--color-text-primary);
    border-left: 2px solid transparent;
  }

  .file-item:hover {
    background: var(--color-surface);
  }

  .file-item.selected {
    background: var(--color-surface-elevated);
    border-left-color: var(--color-accent);
  }

  .folder-item {
    color: var(--color-text-muted);
  }

  .file-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .folder-item .file-name {
    color: var(--color-text-primary);
  }

  .folder-count {
    font-size: 10px;
    opacity: 0.7;
  }

  .action-btn {
    display: none;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border: none;
    border-radius: 3px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 0;
    flex-shrink: 0;
  }

  .file-item:hover .action-btn,
  .file-item:focus-within .action-btn,
  .file-item.selected .action-btn {
    display: flex;
  }

  .file-item:focus-visible {
    outline: 1px solid var(--color-accent);
    outline-offset: -1px;
  }

  .action-btn.stage:hover {
    background: var(--color-diff-add-bg);
    color: var(--color-diff-add-text);
  }

  .action-btn.danger:hover {
    background: var(--color-diff-del-bg);
    color: var(--color-diff-del-text);
  }

  .empty-msg {
    padding: 8px 16px;
    color: var(--color-text-muted);
    font-style: italic;
    font-size: 11px;
  }
</style>
