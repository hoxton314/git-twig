<script lang="ts">
  import { activeRepoPath } from "../../lib/stores/repos";
  import {
    commitGraph,
    graphLoading,
    selectedCommitOid,
    workingStatus,
    selectedWorkingFile,
    workingFileDiff,
  } from "../../lib/stores/graph";
  import * as tauri from "../../lib/tauri";
  import CommitRow from "./CommitRow.svelte";
  import GraphCanvas from "./GraphCanvas.svelte";
  import type { GraphEntry } from "../../lib/types/git";
  import { Loader2, Pencil } from "lucide-svelte";

  const repoPath = $derived($activeRepoPath);
  const graph = $derived($commitGraph);
  const loading = $derived($graphLoading);
  const selected = $derived($selectedCommitOid);
  const status = $derived($workingStatus);

  const hasWip = $derived(status.staged.length > 0 || status.unstaged.length > 0);
  const wipSelected = $derived($selectedCommitOid === "__wip__");

  const unpushedSet = $derived(
    new Set(graph?.unpushed_oids ?? [])
  );

  // Virtualization state
  let containerEl: HTMLDivElement | undefined = $state(undefined);
  let scrollTop = $state(0);
  const ROW_HEIGHT = 34;
  const OVERSCAN = 10;

  const totalLanes = $derived(graph?.total_lanes ?? 0);
  const graphEntryCount = $derived(graph?.entries.length ?? 0);
  const totalHeight = $derived(graphEntryCount * ROW_HEIGHT);

  // Adjust scroll offset to account for the WIP row above the virtual scroll
  const graphScrollTop = $derived(Math.max(0, scrollTop - (hasWip ? ROW_HEIGHT : 0)));
  const visibleStart = $derived(Math.max(0, Math.floor(graphScrollTop / ROW_HEIGHT) - OVERSCAN));
  const visibleCount = $derived.by(() =>
    containerEl
      ? Math.min(graphEntryCount - visibleStart, Math.ceil(containerEl.clientHeight / ROW_HEIGHT) + 2 * OVERSCAN)
      : 50
  );
  const visibleEntries = $derived(
    graph ? graph.entries.slice(visibleStart, visibleStart + visibleCount) : []
  );

  function handleScroll(e: Event) {
    scrollTop = (e.target as HTMLDivElement).scrollTop;
  }

  function selectCommit(oid: string) {
    $selectedCommitOid = selected === oid ? null : oid;
  }

  function selectWip() {
    if (wipSelected) {
      $selectedCommitOid = null;
      $selectedWorkingFile = null;
      $workingFileDiff = [];
      return;
    }
    $selectedCommitOid = "__wip__";
    // Load combined working diff
    if (repoPath) {
      tauri.getWorkingDiff(repoPath).then((diff) => {
        $workingFileDiff = diff;
        $selectedWorkingFile = { path: "__all__", area: "unstaged" };
      });
    }
  }

  // Load graph when active repo changes
  let lastLoadedPath: string | null = null;
  $effect(() => {
    const path = repoPath;
    if (path && path !== lastLoadedPath) {
      lastLoadedPath = path;
      loadGraph(path);
    }
  });

  async function loadGraph(path: string) {
    $graphLoading = true;
    try {
      const result = await tauri.getCommitGraph(path, 5000);
      $commitGraph = result;
    } catch (err) {
      console.error("Failed to load commit graph:", err);
      $commitGraph = null;
    } finally {
      $graphLoading = false;
    }
  }
</script>

<div class="commit-graph" bind:this={containerEl} onscroll={handleScroll}>
  {#if loading}
    <div class="loading">
      <Loader2 size={24} class="spinner" />
      <span>Loading commits...</span>
    </div>
  {:else if graph && graph.entries.length > 0}
    {#if hasWip}
      <button
        class="wip-row"
        class:selected={wipSelected}
        style="height: {ROW_HEIGHT}px;"
        onclick={selectWip}
      >
        <div class="wip-icon">
          <Pencil size={12} />
        </div>
        <span class="wip-label">Uncommitted changes</span>
        <span class="wip-counts">
          {#if status.staged.length > 0}
            <span class="wip-staged">{status.staged.length} staged</span>
          {/if}
          {#if status.unstaged.length > 0}
            <span class="wip-unstaged">{status.unstaged.length} modified</span>
          {/if}
        </span>
      </button>
    {/if}
    <div class="virtual-scroll" style="height: {totalHeight}px; position: relative;">
      <div
        class="virtual-window"
        style="transform: translateY({visibleStart * ROW_HEIGHT}px); position: absolute; left: 0; right: 0;"
      >
        {#each visibleEntries as entry, i (entry.commit.oid)}
          {@const isUnpushed = unpushedSet.has(entry.commit.oid)}
          {@const entryRefs = graph.refs[entry.commit.oid] ?? []}
          <div
            class="commit-row-wrapper"
            style="height: {ROW_HEIGHT}px;"
          >
            <GraphCanvas
              {entry}
              {totalLanes}
              height={ROW_HEIGHT}
              {isUnpushed}
            />
            <CommitRow
              {entry}
              {isUnpushed}
              refs={entryRefs}
              isSelected={selected === entry.commit.oid}
              onSelect={() => selectCommit(entry.commit.oid)}
            />
          </div>
        {/each}
      </div>
    </div>
  {:else if graph}
    <div class="empty">No commits found.</div>
  {/if}
</div>

<style>
  .commit-graph {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    background: var(--color-bg);
  }

  .loading {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 24px;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .loading :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .wip-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 0 12px;
    border: none;
    border-bottom: 1px solid var(--color-border);
    background: rgba(224, 175, 104, 0.06);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
    text-align: left;
    flex-shrink: 0;
    transition: background 0.1s;
  }

  .wip-row:hover {
    background: rgba(224, 175, 104, 0.12);
  }

  .wip-row.selected {
    background: rgba(224, 175, 104, 0.15);
    border-left: 2px solid #e0af68;
  }

  .wip-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    border: 1.5px dashed #e0af68;
    color: #e0af68;
    flex-shrink: 0;
  }

  .wip-label {
    font-weight: 500;
    color: #e0af68;
  }

  .wip-counts {
    display: flex;
    gap: 8px;
    margin-left: auto;
    font-size: 11px;
  }

  .wip-staged {
    color: var(--color-diff-add-text);
  }

  .wip-unstaged {
    color: var(--color-text-muted);
  }

  .commit-row-wrapper {
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--color-border);
  }

  .empty {
    padding: 24px;
    color: var(--color-text-muted);
    font-size: 13px;
  }
</style>
