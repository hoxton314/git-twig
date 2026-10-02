<script module lang="ts">
  export type StashAction = "apply" | "pop" | "drop" | "rename" | "branch";
</script>

<script lang="ts">
  /** Modal showing a stash entry's changes (tracked + untracked part). */
  import Modal from "../shared/Modal.svelte";
  import DiffFileList from "../history/DiffFileList.svelte";
  import * as tauri from "../../lib/tauri";
  import type { StashDetail, StashDiff } from "../../lib/types/git";
  import { Loader2, ArchiveRestore, Copy, GitBranchPlus, Pencil, Trash2 } from "lucide-svelte";

  interface Props {
    stash: StashDetail | null;
    repoPath: string;
    busy: boolean;
    onclose: () => void;
    onaction: (action: StashAction, stash: StashDetail) => void;
  }

  let { stash, repoPath, busy, onclose, onaction }: Props = $props();

  let diff = $state<StashDiff | null>(null);
  let error = $state<string | null>(null);
  let tab = $state<"tracked" | "untracked">("tracked");

  let req = 0;
  $effect(() => {
    const s = stash;
    const r = repoPath;
    if (!s) return;
    const id = ++req;
    diff = null;
    error = null;
    tab = "tracked";
    tauri
      .stashShow(r, s.oid)
      .then((d) => {
        if (id !== req) return;
        diff = d;
        if (d.tracked.length === 0 && d.untracked.length > 0) tab = "untracked";
      })
      .catch((err) => {
        if (id === req) error = String(err);
      });
  });

  const shown = $derived(diff ? (tab === "tracked" ? diff.tracked : diff.untracked) : []);
</script>

<Modal open={stash !== null} title={stash ? `Stash — ${stash.message}` : "Stash"} {onclose} width="min(1100px, 94vw)">
  {#if stash}
    <div class="viewer">
      <div class="bar">
        <div class="tabs" role="tablist">
          <button role="tab" class="tab" class:active={tab === "tracked"} aria-selected={tab === "tracked"} onclick={() => (tab = "tracked")}>
            Changes <span class="n">{diff?.tracked.length ?? "…"}</span>
          </button>
          {#if stash.has_untracked}
            <button role="tab" class="tab" class:active={tab === "untracked"} aria-selected={tab === "untracked"} onclick={() => (tab = "untracked")}>
              Untracked <span class="n">{diff?.untracked.length ?? "…"}</span>
            </button>
          {/if}
        </div>
        <span class="ref" title={stash.oid}>{stash.reference} · {stash.oid.slice(0, 7)}</span>
        <div class="actions">
          <button class="btn" disabled={busy} onclick={() => onaction("pop", stash)} title="Apply and remove">
            <ArchiveRestore size={13} /> Pop
          </button>
          <button class="btn" disabled={busy} onclick={() => onaction("apply", stash)} title="Apply and keep">
            <Copy size={13} /> Apply
          </button>
          <button class="btn" disabled={busy} onclick={() => onaction("branch", stash)} title="Create a branch from this stash">
            <GitBranchPlus size={13} /> Branch…
          </button>
          <button class="btn" disabled={busy} onclick={() => onaction("rename", stash)} title="Rename">
            <Pencil size={13} />
          </button>
          <button class="btn danger" disabled={busy} onclick={() => onaction("drop", stash)} title="Drop">
            <Trash2 size={13} />
          </button>
        </div>
      </div>
      <div class="body">
        {#if error}
          <div class="empty error">{error}</div>
        {:else if !diff}
          <div class="empty"><Loader2 size={14} class="spinner" /> Loading stash…</div>
        {:else}
          <DiffFileList files={shown} expandAll={shown.length <= 8} emptyText={tab === "tracked" ? "No tracked changes" : "No untracked files"} />
        {/if}
      </div>
    </div>
  {/if}
</Modal>

<style>
  .viewer {
    display: flex;
    flex-direction: column;
    height: calc(100vh - 150px);
    margin: -20px;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 10px 0;
    border-bottom: 1px solid var(--color-border);
  }

  .tabs {
    display: flex;
    gap: 2px;
  }

  .tab {
    padding: 6px 12px;
    border: none;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 12px;
    cursor: pointer;
  }

  .tab.active {
    color: var(--color-text-primary);
    border-bottom-color: var(--color-accent);
  }

  .n {
    opacity: 0.6;
    margin-left: 4px;
  }

  .ref {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-muted);
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .actions {
    display: flex;
    gap: 4px;
    padding-bottom: 5px;
  }

  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 9px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
  }

  .btn:hover:not(:disabled) {
    border-color: var(--color-accent);
  }

  .btn.danger:hover:not(:disabled) {
    border-color: var(--color-diff-del-text);
    color: var(--color-diff-del-text);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  .empty {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 14px;
    font-size: 12px;
    color: var(--color-text-muted);
  }

  .empty.error {
    color: var(--color-diff-del-text);
  }

  .viewer :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
