<script lang="ts">
  import {
    ChevronDown,
    ChevronRight,
    Archive,
    ArchiveRestore,
    Copy,
    Trash2,
    Loader2,
  } from "lucide-svelte";
  import { activeRepoPath } from "../../lib/stores/repos";
  import {
    stashEntries,
    refreshStash,
    refreshAll,
  } from "../../lib/stores/graph";
  import * as tauri from "../../lib/tauri";

  const repoPath = $derived($activeRepoPath);
  const entries = $derived($stashEntries);

  let expanded = $state(false);
  let stashMessage = $state("");
  let loading = $state(false);
  let actionLoading = $state<number | null>(null);

  // Load stash list when repo changes
  let lastLoadedPath: string | null = null;
  $effect(() => {
    const path = repoPath;
    if (path && path !== lastLoadedPath) {
      lastLoadedPath = path;
      refreshStash(path);
    }
  });

  async function handleStashPush() {
    if (!repoPath) return;
    loading = true;
    try {
      const result = await tauri.stashPush(
        repoPath,
        stashMessage.trim() || undefined,
      );
      if (result.success) {
        stashMessage = "";
        await refreshAll();
      }
    } catch (err) {
      console.error("Stash push error:", err);
    } finally {
      loading = false;
    }
  }

  async function handlePop(index: number) {
    if (!repoPath) return;
    actionLoading = index;
    try {
      const result = await tauri.stashPop(repoPath, index);
      if (result.success) {
        await refreshAll();
      }
    } catch (err) {
      console.error("Stash pop error:", err);
    } finally {
      actionLoading = null;
    }
  }

  async function handleApply(index: number) {
    if (!repoPath) return;
    actionLoading = index;
    try {
      const result = await tauri.stashApply(repoPath, index);
      if (result.success) {
        await refreshAll();
      }
    } catch (err) {
      console.error("Stash apply error:", err);
    } finally {
      actionLoading = null;
    }
  }

  async function handleDrop(index: number) {
    if (!repoPath) return;
    actionLoading = index;
    try {
      const result = await tauri.stashDrop(repoPath, index);
      if (result.success) {
        await refreshAll();
      }
    } catch (err) {
      console.error("Stash drop error:", err);
    } finally {
      actionLoading = null;
    }
  }

  function formatTime(timestamp: string): string {
    try {
      const date = new Date(timestamp);
      const now = new Date();
      const diffMs = now.getTime() - date.getTime();
      const diffMins = Math.floor(diffMs / 60000);
      if (diffMins < 1) return "just now";
      if (diffMins < 60) return `${diffMins}m ago`;
      const diffHours = Math.floor(diffMins / 60);
      if (diffHours < 24) return `${diffHours}h ago`;
      const diffDays = Math.floor(diffHours / 24);
      if (diffDays < 30) return `${diffDays}d ago`;
      return date.toLocaleDateString();
    } catch {
      return "";
    }
  }
</script>

<div class="stash-section">
  <div
    class="section-header"
    onclick={() => (expanded = !expanded)}
    onkeydown={(e) => e.key === "Enter" && (expanded = !expanded)}
    role="button"
    tabindex="0"
  >
    {#if expanded}
      <ChevronDown size={14} />
    {:else}
      <ChevronRight size={14} />
    {/if}
    <Archive size={12} />
    <span class="section-title">Stashes</span>
    <span class="section-count">{entries.length}</span>
  </div>

  {#if expanded}
    <div class="stash-content">
      <!-- Stash push input -->
      <div class="stash-push">
        <input
          class="stash-input"
          type="text"
          placeholder="Stash message (optional)..."
          bind:value={stashMessage}
          onkeydown={(e) => e.key === "Enter" && handleStashPush()}
        />
        <button
          class="stash-push-btn"
          onclick={handleStashPush}
          disabled={loading}
          title="Stash changes"
        >
          {#if loading}
            <Loader2 size={12} class="spinner" />
          {:else}
            <Archive size={12} />
          {/if}
        </button>
      </div>

      <!-- Stash entries -->
      <div class="stash-list">
        {#each entries as entry (entry.index)}
          <div class="stash-item">
            <div class="stash-info">
              <span class="stash-msg" title={entry.message}>{entry.message}</span>
              <span class="stash-time">{formatTime(entry.timestamp)}</span>
            </div>
            <div class="stash-actions">
              {#if actionLoading === entry.index}
                <Loader2 size={12} class="spinner" />
              {:else}
                <button
                  class="stash-action-btn"
                  onclick={() => handlePop(entry.index)}
                  title="Pop (apply & remove)"
                >
                  <ArchiveRestore size={12} />
                </button>
                <button
                  class="stash-action-btn"
                  onclick={() => handleApply(entry.index)}
                  title="Apply (keep stash)"
                >
                  <Copy size={12} />
                </button>
                <button
                  class="stash-action-btn delete"
                  onclick={() => handleDrop(entry.index)}
                  title="Drop"
                >
                  <Trash2 size={12} />
                </button>
              {/if}
            </div>
          </div>
        {:else}
          <div class="empty-msg">No stashes</div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .stash-section {
    flex-shrink: 0;
  }

  .section-header {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 6px 8px;
    border: none;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-surface);
    color: var(--color-text-muted);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.3px;
    cursor: pointer;
    text-align: left;
  }

  .section-header:hover {
    background: var(--color-surface-elevated);
  }

  .section-count {
    margin-left: auto;
    opacity: 0.6;
    font-weight: 400;
  }

  .stash-content {
    border-bottom: 1px solid var(--color-border);
  }

  .stash-push {
    display: flex;
    gap: 4px;
    padding: 6px 8px;
  }

  .stash-input {
    flex: 1;
    padding: 3px 6px;
    border: 1px solid var(--color-border);
    border-radius: 3px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 11px;
    font-family: inherit;
    outline: none;
    min-width: 0;
  }

  .stash-input:focus {
    border-color: var(--color-accent);
  }

  .stash-input::placeholder {
    color: var(--color-text-muted);
  }

  .stash-push-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border: 1px solid var(--color-border);
    border-radius: 3px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    cursor: pointer;
    padding: 0;
    flex-shrink: 0;
  }

  .stash-push-btn:hover {
    background: var(--color-surface-elevated);
  }

  .stash-push-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .stash-list {
    max-height: 150px;
    overflow-y: auto;
  }

  .stash-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px 4px 16px;
    transition: background 0.1s;
  }

  .stash-item:hover {
    background: var(--color-surface);
  }

  .stash-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .stash-msg {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .stash-time {
    font-size: 10px;
    color: var(--color-text-muted);
  }

  .stash-actions {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
    align-items: center;
  }

  .stash-action-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border: none;
    border-radius: 3px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 0;
  }

  .stash-action-btn:hover {
    background: rgba(122, 162, 247, 0.2);
    color: var(--color-accent);
  }

  .stash-action-btn.delete:hover {
    background: rgba(247, 118, 142, 0.2);
    color: var(--color-diff-del-text);
  }

  .stash-actions :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  .stash-push-btn :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .empty-msg {
    padding: 8px 16px;
    color: var(--color-text-muted);
    font-style: italic;
    font-size: 11px;
  }
</style>
