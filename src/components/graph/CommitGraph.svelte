<script lang="ts">
  import { activeRepoPath } from "../../lib/stores/repos";
  import { commitGraph, graphLoading, selectedCommitOid } from "../../lib/stores/graph";
  import * as tauri from "../../lib/tauri";
  import CommitRow from "./CommitRow.svelte";
  import GraphCanvas from "./GraphCanvas.svelte";
  import type { GraphEntry } from "../../lib/types/git";
  import { Loader2 } from "lucide-svelte";

  const repoPath = $derived($activeRepoPath);
  const graph = $derived($commitGraph);
  const loading = $derived($graphLoading);
  const selected = $derived($selectedCommitOid);

  // Virtualization state
  let containerEl: HTMLDivElement | undefined = $state(undefined);
  let scrollTop = $state(0);
  const ROW_HEIGHT = 34;
  const OVERSCAN = 10;

  const totalEntries = $derived(graph?.entries.length ?? 0);
  const totalLanes = $derived(graph?.total_lanes ?? 0);
  const totalHeight = $derived(totalEntries * ROW_HEIGHT);

  const visibleStart = $derived(Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN));
  const visibleCount = $derived(
    containerEl
      ? Math.min(totalEntries - visibleStart, Math.ceil(containerEl.clientHeight / ROW_HEIGHT) + 2 * OVERSCAN)
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
    <div class="virtual-scroll" style="height: {totalHeight}px; position: relative;">
      <div
        class="virtual-window"
        style="transform: translateY({visibleStart * ROW_HEIGHT}px); position: absolute; left: 0; right: 0;"
      >
        {#each visibleEntries as entry, i (entry.commit.oid)}
          {@const rowIndex = visibleStart + i}
          <div
            class="commit-row-wrapper"
            style="height: {ROW_HEIGHT}px;"
          >
            <GraphCanvas
              {entry}
              {totalLanes}
              height={ROW_HEIGHT}
            />
            <CommitRow
              {entry}
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
