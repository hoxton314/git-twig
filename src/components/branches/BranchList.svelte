<script lang="ts">
  import {
    GitBranch,
    Globe,
    Check,
    Plus,
    RefreshCw,
    Trash2,
    ChevronDown,
    ChevronRight,
  } from "lucide-svelte";
  import { activeRepoPath, updateRepo } from "../../lib/stores/repos";
  import { branches, commitGraph, graphLoading } from "../../lib/stores/graph";
  import * as tauri from "../../lib/tauri";
  import type { BranchInfo } from "../../lib/types/git";

  const repoPath = $derived($activeRepoPath);
  const allBranches = $derived($branches);

  const localBranches = $derived(allBranches.filter((b) => !b.is_remote));
  const remoteBranches = $derived(allBranches.filter((b) => b.is_remote));

  let localExpanded = $state(true);
  let remoteExpanded = $state(true);
  let newBranchName = $state("");
  let showCreateInput = $state(false);
  let loading = $state(false);

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
        console.error("Checkout failed:", result.message);
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
        onclick={() => handleCheckout(branch)}
        title="{branch.name} — {branch.last_commit_summary}"
      >
        <span class="dot"></span>
        <span class="branch-name">{branch.name}</span>
      </button>
    {/each}
  {/if}
</div>

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

  .delete-btn {
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

  .branch-item:hover .delete-btn {
    display: flex;
  }

  .delete-btn:hover {
    background: rgba(247, 118, 142, 0.2);
    color: #f7768e;
  }
</style>
