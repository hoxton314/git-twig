<script lang="ts">
  /** Sidebar section listing linked worktrees (hidden with only the main one). */
  import { ask } from "@tauri-apps/plugin-dialog";
  import { FolderGit2, Plus, ChevronDown, ChevronRight, Lock, TriangleAlert } from "lucide-svelte";
  import ContextMenu, { type MenuItem } from "../shared/ContextMenu.svelte";
  import { activeRepoPath, openRepos } from "../../lib/stores/repos";
  import { refreshAll } from "../../lib/stores/graph";
  import { settings } from "../../lib/stores/settings";
  import { worktrees, refreshRepoTools, openAsTab, addWorktreeOpen } from "../../lib/stores/repotools";
  import { toast, toastError } from "../../lib/stores/toasts";
  import * as tauri from "../../lib/tauri";
  import type { WorktreeInfo } from "../../lib/types/git";

  const list = $derived($worktrees);
  const repoPath = $derived($activeRepoPath);
  let expanded = $state(true);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  let busy = $state(false);

  function label(w: WorktreeInfo): string {
    if (w.branch) return w.branch;
    if (w.head_short) return `detached @ ${w.head_short}`;
    return w.name ?? "worktree";
  }

  function tail(p: string): string {
    return p.split(/[\\/]/).filter(Boolean).pop() ?? p;
  }

  function open(w: WorktreeInfo) {
    if (w.is_current) return;
    if (w.is_prunable) {
      toast("warning", "This worktree's directory is missing. Prune it to clean up.");
      return;
    }
    openAsTab(w.path);
  }

  async function remove(w: WorktreeInfo) {
    if (!repoPath || busy) return;
    if ($settings.confirm_destructive_ops) {
      const ok = await ask(`Remove worktree "${label(w)}" at\n${w.path}?\n\nThe branch is kept.`, {
        title: "Remove Worktree",
        kind: "warning",
      });
      if (!ok) return;
    }
    busy = true;
    try {
      let res = await tauri.worktreeRemove(repoPath, w.path, false);
      if (!res.success && /modified or untracked|is dirty|use --force/i.test(res.message)) {
        const force = await ask(
          `The worktree has uncommitted changes:\n\n${res.message.trim()}\n\nRemove it anyway and discard them?`,
          { title: "Discard Worktree Changes", kind: "warning" },
        );
        if (!force) return;
        res = await tauri.worktreeRemove(repoPath, w.path, true);
      }
      if (res.success) {
        toast("success", `Removed worktree ${tail(w.path)}`);
        // Close its tab if it was open.
        if ($openRepos.has(w.path)) toast("info", "The removed worktree is still open in a tab; close it.");
      } else {
        toast("error", res.message.trim(), { title: "Remove worktree failed" });
      }
    } catch (err) {
      toastError("Remove worktree failed", err);
    } finally {
      busy = false;
      await refreshRepoTools(repoPath);
      refreshAll(repoPath);
    }
  }

  async function prune() {
    if (!repoPath) return;
    try {
      const res = await tauri.worktreePrune(repoPath);
      if (res.success) toast("success", "Pruned stale worktrees");
      else toast("error", res.message.trim(), { title: "Prune failed" });
    } catch (err) {
      toastError("Prune failed", err);
    } finally {
      refreshRepoTools(repoPath);
    }
  }

  function openMenu(e: MouseEvent, w: WorktreeInfo) {
    e.preventDefault();
    menu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: "Open in new tab", action: () => open(w), disabled: w.is_current || w.is_prunable },
        {
          label: "Copy path",
          action: async () => {
            try {
              await navigator.clipboard.writeText(w.path);
              toast("success", "Path copied");
            } catch (err) {
              toastError("Copy failed", err);
            }
          },
        },
        { separator: true },
        { label: "Prune stale worktrees", action: prune, disabled: !list.some((x) => x.is_prunable) },
        {
          label: "Remove worktree…",
          action: () => remove(w),
          danger: true,
          disabled: w.is_main || w.is_current || w.is_locked,
        },
      ],
    };
  }
</script>

{#if list.length > 1}
  <div class="section">
    <div class="section-header">
      <button class="title-btn" onclick={() => (expanded = !expanded)} aria-expanded={expanded}>
        {#if expanded}<ChevronDown size={12} />{:else}<ChevronRight size={12} />{/if}
        <span class="section-title">Worktrees</span>
        <span class="count">{list.length}</span>
      </button>
      <button class="icon-btn" onclick={() => ($addWorktreeOpen = true)} title="Add worktree" aria-label="Add worktree">
        <Plus size={14} />
      </button>
    </div>

    {#if expanded}
      {#each list as w (w.path)}
        <div
          class="item"
          class:current={w.is_current}
          class:stale={w.is_prunable}
          role="button"
          tabindex="0"
          title="{w.path}{w.is_current ? '\n(this tab)' : '\nClick to open in a new tab'}"
          onclick={() => open(w)}
          onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), open(w))}
          oncontextmenu={(e) => openMenu(e, w)}
        >
          <FolderGit2 size={13} />
          <span class="name">{label(w)}</span>
          {#if w.is_main}<span class="tag">main</span>{/if}
          {#if w.is_locked}<Lock size={11} />{/if}
          {#if w.is_prunable}<TriangleAlert size={11} />{/if}
          <span class="dir">{tail(w.path)}</span>
        </div>
      {/each}
    {/if}
  </div>
{/if}

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<style>
  .section {
    display: flex;
    flex-direction: column;
    font-size: 12px;
    border-top: 1px solid var(--color-border);
    padding-bottom: 6px;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px 4px 8px;
  }

  .title-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    border: none;
    background: none;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 0;
  }

  .section-title {
    font-weight: 600;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .count {
    opacity: 0.6;
    margin-left: 4px;
  }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 0;
  }

  .icon-btn:hover {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px 5px 24px;
    color: var(--color-text-primary);
    cursor: pointer;
    min-width: 0;
  }

  .item:hover {
    background: var(--color-surface-elevated);
  }

  .item.current {
    color: var(--color-accent);
    cursor: default;
  }

  .item.stale {
    color: var(--color-text-muted);
    font-style: italic;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tag {
    font-size: 10px;
    padding: 0 4px;
    border-radius: 3px;
    background: var(--color-surface-elevated);
    color: var(--color-text-muted);
  }

  .dir {
    margin-left: auto;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    font-size: 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 45%;
  }
</style>
