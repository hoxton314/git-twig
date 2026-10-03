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
  import { t, tr } from "../../lib/i18n";
  import type { WorktreeInfo } from "../../lib/types/git";

  const list = $derived($worktrees);
  const repoPath = $derived($activeRepoPath);
  let expanded = $state(true);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  let busy = $state(false);

  function label(w: WorktreeInfo): string {
    if (w.branch) return w.branch;
    if (w.head_short) return $t("worktrees.detached", { sha: w.head_short });
    return w.name ?? $t("worktrees.worktree");
  }

  function tail(p: string): string {
    return p.split(/[\\/]/).filter(Boolean).pop() ?? p;
  }

  function open(w: WorktreeInfo) {
    if (w.is_current) return;
    if (w.is_prunable) {
      toast("warning", tr("worktrees.missingDir"));
      return;
    }
    openAsTab(w.path);
  }

  async function remove(w: WorktreeInfo) {
    if (!repoPath || busy) return;
    if ($settings.confirm_destructive_ops) {
      const ok = await ask(tr("worktrees.removeConfirm", { name: label(w), path: w.path }), {
        title: tr("worktrees.removeTitle"),
        kind: "warning",
      });
      if (!ok) return;
    }
    busy = true;
    try {
      let res = await tauri.worktreeRemove(repoPath, w.path, false);
      if (!res.success && /modified or untracked|is dirty|use --force/i.test(res.message)) {
        const force = await ask(
          tr("worktrees.discardConfirm", { details: res.message.trim() }),
          { title: tr("worktrees.discardTitle"), kind: "warning" },
        );
        if (!force) return;
        res = await tauri.worktreeRemove(repoPath, w.path, true);
      }
      if (res.success) {
        toast("success", tr("worktrees.removed", { name: tail(w.path) }));
        // Close its tab if it was open.
        if ($openRepos.has(w.path)) toast("info", tr("worktrees.stillOpen"));
      } else {
        toast("error", res.message.trim(), { title: tr("worktrees.removeFailed") });
      }
    } catch (err) {
      toastError(tr("worktrees.removeFailed"), err);
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
      if (res.success) toast("success", tr("worktrees.pruned"));
      else toast("error", res.message.trim(), { title: tr("worktrees.pruneFailed") });
    } catch (err) {
      toastError(tr("worktrees.pruneFailed"), err);
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
        { label: tr("worktrees.openInNewTab"), action: () => open(w), disabled: w.is_current || w.is_prunable },
        {
          label: tr("worktrees.copyPath"),
          action: async () => {
            try {
              await navigator.clipboard.writeText(w.path);
              toast("success", tr("worktrees.pathCopied"));
            } catch (err) {
              toastError(tr("worktrees.copyFailed"), err);
            }
          },
        },
        { separator: true },
        { label: tr("worktrees.prune"), action: prune, disabled: !list.some((x) => x.is_prunable) },
        {
          label: tr("worktrees.removeMenu"),
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
        <span class="section-title">{$t("worktrees.title")}</span>
        <span class="count">{list.length}</span>
      </button>
      <button class="icon-btn" onclick={() => ($addWorktreeOpen = true)} title={$t("worktrees.addTitle")} aria-label={$t("worktrees.addTitle")}>
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
          title={w.is_current ? $t("worktrees.itemTitleCurrent", { path: w.path }) : $t("worktrees.itemTitle", { path: w.path })}
          onclick={() => open(w)}
          onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), open(w))}
          oncontextmenu={(e) => openMenu(e, w)}
        >
          <FolderGit2 size={13} />
          <span class="name">{label(w)}</span>
          {#if w.is_main}<span class="tag">{$t("worktrees.mainTag")}</span>{/if}
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
