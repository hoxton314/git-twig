<script lang="ts">
  import {
    GitBranch,
    GitMerge,
    Globe,
    Check,
    Plus,
    RefreshCw,
    Trash2,
    ChevronDown,
    ChevronRight,
    ArrowUp,
    ArrowDown,
  } from "lucide-svelte";
  import { activeRepoPath, updateRepo } from "../../lib/stores/repos";
  import { branches, commitGraph, graphLoading, refreshAll } from "../../lib/stores/graph";
  import * as tauri from "../../lib/tauri";
  import type { BranchInfo } from "../../lib/types/git";
  import { message } from "@tauri-apps/plugin-dialog";
  import Modal from "../shared/Modal.svelte";

  const repoPath = $derived($activeRepoPath);
  const allBranches = $derived($branches);

  const localBranches = $derived(allBranches.filter((b) => !b.is_remote));
  const remoteBranches = $derived(allBranches.filter((b) => b.is_remote));

  const currentBranch = $derived(localBranches.find((b) => b.is_head) ?? null);
  const defaultBranch = $derived(
    localBranches.find((b) => b.name === "main")?.name ??
    localBranches.find((b) => b.name === "master")?.name ??
    null
  );

  let localExpanded = $state(true);
  let remoteExpanded = $state(true);
  let newBranchName = $state("");
  let showCreateInput = $state(false);
  let loading = $state(false);

  // Drag-and-drop state
  let dragSource = $state<string | null>(null);
  let dragTarget = $state<string | null>(null);

  // Merge dialog state
  let mergeDialog = $state<{ source: string; target: string; targetIsHead: boolean } | null>(null);
  let merging = $state(false);

  // Load branches when repo changes
  let lastLoadedPath: string | null = null;
  $effect(() => {
    const path = repoPath;
    if (path && path !== lastLoadedPath) {
      lastLoadedPath = path;
      loadBranches(path);
    }
  });

  async function loadBranches(path: string) {
    try {
      const result = await tauri.getBranches(path);
      $branches = result;
    } catch (err) {
      console.error("Failed to load branches:", err);
    }
  }

  async function handleCheckout(branch: BranchInfo) {
    if (!repoPath || branch.is_head) return;
    loading = true;
    try {
      const result = await tauri.checkoutBranch(repoPath, branch.name);
      if (result.success) {
        // Refresh state
        const info = await tauri.getRepoInfo(repoPath);
        updateRepo(info);
        await loadBranches(repoPath);
        // Reload the graph
        $graphLoading = true;
        const graph = await tauri.getCommitGraph(repoPath, 5000);
        $commitGraph = graph;
        $graphLoading = false;
      } else {
        await message(result.message, { title: "Checkout Failed", kind: "error" });
      }
    } catch (err) {
      console.error("Checkout error:", err);
    } finally {
      loading = false;
    }
  }

  async function handleCreateBranch() {
    if (!repoPath || !newBranchName.trim()) return;
    loading = true;
    try {
      const result = await tauri.createBranch(repoPath, newBranchName.trim());
      if (result.success) {
        newBranchName = "";
        showCreateInput = false;
        const info = await tauri.getRepoInfo(repoPath);
        updateRepo(info);
        await loadBranches(repoPath);
      } else {
        console.error("Create branch failed:", result.message);
      }
    } catch (err) {
      console.error("Create branch error:", err);
    } finally {
      loading = false;
    }
  }

  async function handleDeleteBranch(e: MouseEvent, branch: BranchInfo) {
    e.stopPropagation();
    if (!repoPath || branch.is_head) return;
    loading = true;
    try {
      const result = await tauri.deleteBranch(repoPath, branch.name, false);
      if (result.success) {
        await loadBranches(repoPath);
      } else {
        console.error("Delete branch failed:", result.message);
      }
    } catch (err) {
      console.error("Delete branch error:", err);
    } finally {
      loading = false;
    }
  }

  async function handleFetch() {
    if (!repoPath) return;
    loading = true;
    try {
      await tauri.fetchAll(repoPath);
      await loadBranches(repoPath);
    } catch (err) {
      console.error("Fetch error:", err);
    } finally {
      loading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") handleCreateBranch();
    if (e.key === "Escape") {
      showCreateInput = false;
      newBranchName = "";
    }
  }

  // ── Merge default branch into current ─────────────────────────────────

  function handleMergeDefault(e: MouseEvent) {
    e.stopPropagation();
    if (!defaultBranch || !currentBranch) return;
    mergeDialog = {
      source: defaultBranch,
      target: currentBranch.name,
      targetIsHead: true,
    };
  }

  // ── Drag and drop ─────────────────────────────────────────────────────

  function handleDragStart(e: DragEvent, branch: BranchInfo) {
    if (!e.dataTransfer) return;
    dragSource = branch.name;
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", branch.name);
  }

  function handleDragOver(e: DragEvent, branch: BranchInfo) {
    if (!dragSource || dragSource === branch.name || branch.is_remote) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    dragTarget = branch.name;
  }

  function handleDragLeave() {
    dragTarget = null;
  }

  function handleDrop(e: DragEvent, branch: BranchInfo) {
    e.preventDefault();
    if (!dragSource || dragSource === branch.name || branch.is_remote) return;

    mergeDialog = {
      source: dragSource,
      target: branch.name,
      targetIsHead: branch.is_head,
    };

    dragSource = null;
    dragTarget = null;
  }

  function handleDragEnd() {
    dragSource = null;
    dragTarget = null;
  }

  // ── Execute merge ─────────────────────────────────────────────────────

  async function executeMerge() {
    if (!repoPath || !mergeDialog) return;
    const { source, target, targetIsHead } = mergeDialog;
    merging = true;

    try {
      // Checkout target first if it's not the current HEAD
      if (!targetIsHead) {
        const checkout = await tauri.checkoutBranch(repoPath, target);
        if (!checkout.success) {
          console.error("Checkout failed:", checkout.message);
          merging = false;
          return;
        }
      }

      const result = await tauri.mergeBranch(repoPath, source);
      mergeDialog = null;

      if (!result.success) {
        console.error("Merge failed:", result.message);
      }

      await refreshAll(repoPath);
    } catch (err) {
      console.error("Merge error:", err);
    } finally {
      merging = false;
    }
  }
</script>

<div class="branch-list">
  <div class="section-header">
    <span class="section-title">Branches</span>
    <div class="header-actions">
      <button class="icon-btn" onclick={() => (showCreateInput = !showCreateInput)} title="New branch">
        <Plus size={14} />
      </button>
      <button class="icon-btn" onclick={handleFetch} title="Fetch all" disabled={loading}>
        <RefreshCw size={14} />
      </button>
    </div>
  </div>

  {#if showCreateInput}
    <div class="create-input">
      <input
        type="text"
        placeholder="New branch name..."
        bind:value={newBranchName}
        onkeydown={handleKeydown}
      />
    </div>
  {/if}

  <!-- Local branches -->
  <button class="group-header" onclick={() => (localExpanded = !localExpanded)}>
    {#if localExpanded}
      <ChevronDown size={14} />
    {:else}
      <ChevronRight size={14} />
    {/if}
    <GitBranch size={14} />
    <span>Local</span>
    <span class="count">{localBranches.length}</span>
  </button>

  {#if localExpanded}
    {#each localBranches as branch (branch.name)}
      <div
        class="branch-item"
        class:active={branch.is_head}
        class:drag-over={dragTarget === branch.name}
        class:dragging={dragSource === branch.name}
        draggable="true"
        ondragstart={(e) => handleDragStart(e, branch)}
        ondragover={(e) => handleDragOver(e, branch)}
        ondragleave={() => handleDragLeave()}
        ondrop={(e) => handleDrop(e, branch)}
        ondragend={() => handleDragEnd()}
        onclick={() => handleCheckout(branch)}
        onkeydown={(e) => e.key === "Enter" && handleCheckout(branch)}
        role="button"
        tabindex="0"
        title="{branch.name} — {branch.last_commit_summary}"
      >
        {#if branch.is_head}
          <Check size={12} class="head-icon" />
        {:else}
          <span class="dot"></span>
        {/if}
        <span class="branch-name">{branch.name}</span>
        {#if branch.ahead > 0 || branch.behind > 0}
          <span class="ahead-behind">
            {#if branch.ahead > 0}
              <span class="ab-badge ab-ahead" title="{branch.ahead} ahead of remote">
                <ArrowUp size={10} />{branch.ahead}
              </span>
            {/if}
            {#if branch.behind > 0}
              <span class="ab-badge ab-behind" title="{branch.behind} behind remote">
                <ArrowDown size={10} />{branch.behind}
              </span>
            {/if}
          </span>
        {/if}
        {#if branch.is_head && defaultBranch && branch.name !== defaultBranch}
          <button
            class="merge-btn"
            onclick={(e) => handleMergeDefault(e)}
            title="Merge {defaultBranch} into {branch.name}"
          >
            <GitMerge size={12} />
          </button>
        {/if}
        {#if !branch.is_head}
          <button
            class="delete-btn"
            onclick={(e) => handleDeleteBranch(e, branch)}
            title="Delete branch"
          >
            <Trash2 size={12} />
          </button>
        {/if}
      </div>
    {/each}
  {/if}

  <!-- Remote branches -->
  <button class="group-header" onclick={() => (remoteExpanded = !remoteExpanded)}>
    {#if remoteExpanded}
      <ChevronDown size={14} />
    {:else}
      <ChevronRight size={14} />
    {/if}
    <Globe size={14} />
    <span>Remote</span>
    <span class="count">{remoteBranches.length}</span>
  </button>

  {#if remoteExpanded}
    {#each remoteBranches as branch (branch.name)}
      <button
        class="branch-item remote"
        draggable="true"
        ondragstart={(e) => handleDragStart(e, branch)}
        ondragend={() => handleDragEnd()}
        onclick={() => handleCheckout(branch)}
        title="{branch.name} — {branch.last_commit_summary}"
      >
        <span class="dot"></span>
        <span class="branch-name">{branch.name}</span>
      </button>
    {/each}
  {/if}
</div>

<!-- Merge confirmation modal -->
<Modal open={!!mergeDialog} title="Merge Branch" onclose={() => (mergeDialog = null)} width="360px">
  {#if mergeDialog}
    <div class="merge-dialog">
      <p class="merge-desc">
        Merge <strong>{mergeDialog.source}</strong> into <strong>{mergeDialog.target}</strong>?
      </p>
      {#if !mergeDialog.targetIsHead}
        <p class="merge-warning">This will checkout <strong>{mergeDialog.target}</strong> first.</p>
      {/if}
      <div class="merge-actions">
        <button class="btn-merge" onclick={executeMerge} disabled={merging}>
          {#if merging}Merging...{:else}Merge{/if}
        </button>
        <button class="btn-cancel" onclick={() => (mergeDialog = null)} disabled={merging}>
          Cancel
        </button>
      </div>
    </div>
  {/if}
</Modal>

<style>
  .branch-list {
    display: flex;
    flex-direction: column;
    padding: 0;
    font-size: 12px;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px 6px;
  }

  .section-title {
    font-weight: 600;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--color-text-muted);
  }

  .header-actions {
    display: flex;
    gap: 2px;
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

  .create-input {
    padding: 4px 12px 8px;
  }

  .create-input input {
    width: 100%;
    padding: 4px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 12px;
    outline: none;
    box-sizing: border-box;
  }

  .create-input input:focus {
    border-color: var(--color-accent);
  }

  .group-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    color: var(--color-text-muted);
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    border: none;
    background: none;
    text-align: left;
    width: 100%;
  }

  .group-header:hover {
    color: var(--color-text-primary);
    background: var(--color-surface-elevated);
  }

  .count {
    margin-left: auto;
    opacity: 0.5;
  }

  .branch-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px 5px 24px;
    color: var(--color-text-primary);
    cursor: pointer;
    border: none;
    background: none;
    text-align: left;
    width: 100%;
    transition: background 0.1s;
  }

  .branch-item:hover {
    background: var(--color-surface-elevated);
  }

  .branch-item.active {
    color: var(--color-accent);
    font-weight: 500;
  }

  .branch-item.remote {
    color: var(--color-text-muted);
  }

  .branch-item.drag-over {
    background: rgba(122, 162, 247, 0.15);
    outline: 1px dashed var(--color-accent);
    outline-offset: -1px;
  }

  .branch-item.dragging {
    opacity: 0.4;
  }

  .branch-item :global(.head-icon) {
    color: var(--color-accent);
    flex-shrink: 0;
  }

  .branch-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-text-muted);
    flex-shrink: 0;
    opacity: 0.4;
  }

  .ahead-behind {
    display: flex;
    gap: 3px;
    flex-shrink: 0;
    margin-left: auto;
  }

  .ab-badge {
    display: inline-flex;
    align-items: center;
    gap: 1px;
    font-size: 10px;
    font-weight: 500;
    padding: 0 3px;
    border-radius: 3px;
    line-height: 16px;
  }

  .ab-ahead {
    color: #e0af68;
    background: rgba(224, 175, 104, 0.15);
  }

  .ab-behind {
    color: #7aa2f7;
    background: rgba(122, 162, 247, 0.15);
  }

  .delete-btn,
  .merge-btn {
    display: none;
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
    flex-shrink: 0;
  }

  .branch-item:hover .delete-btn,
  .branch-item:hover .merge-btn {
    display: flex;
  }

  .delete-btn:hover {
    background: rgba(247, 118, 142, 0.2);
    color: #f7768e;
  }

  .merge-btn:hover {
    background: rgba(122, 162, 247, 0.2);
    color: var(--color-accent);
  }

  /* ── Merge dialog ─────────────────────────────────────────────────── */

  .merge-dialog {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .merge-desc {
    color: var(--color-text-primary);
    font-size: 13px;
    margin: 0;
    line-height: 1.5;
  }

  .merge-desc strong {
    color: var(--color-accent);
  }

  .merge-warning {
    color: #e0af68;
    font-size: 12px;
    margin: 0;
    line-height: 1.5;
  }

  .merge-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    margin-top: 4px;
  }

  .btn-merge {
    background: var(--color-accent);
    color: #1a1b26;
    border: none;
    border-radius: 4px;
    padding: 6px 16px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }

  .btn-merge:hover:not(:disabled) {
    filter: brightness(1.1);
  }

  .btn-merge:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .btn-cancel {
    background: transparent;
    color: var(--color-text-muted);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    padding: 6px 16px;
    font-size: 13px;
    cursor: pointer;
  }

  .btn-cancel:hover:not(:disabled) {
    color: var(--color-text-primary);
  }

  .btn-cancel:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
