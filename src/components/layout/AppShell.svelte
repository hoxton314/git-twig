<script lang="ts">
  import TabBar from "./TabBar.svelte";
  import TitleBar from "./TitleBar.svelte";
  import Sidebar from "./Sidebar.svelte";
  import CommitGraph from "../graph/CommitGraph.svelte";
  import DiffViewer from "../diff/DiffViewer.svelte";
  import StagingArea from "../staging/StagingArea.svelte";
  import HomeScreen from "./HomeScreen.svelte";
  import { activeRepo, restoreSession, activeRepoPath } from "../../lib/stores/repos";
  import { selectedCommitOid, selectedWorkingFile, workingFileDiff, refreshAll } from "../../lib/stores/graph";
  import { diffPanelRatio, sidebarWidth, stagingWidth } from "../../lib/stores/ui";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import * as tauri from "../../lib/tauri";
  import { onMount } from "svelte";

  let showTitleBar = $state(false);

  onMount(() => {
    restoreSession();

    tauri.isTilingWm().then((tiling) => {
      showTitleBar = !tiling;
    });

    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused && $activeRepoPath) {
        refreshAll();
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  });

  const repo = $derived($activeRepo);
  const hasSelectedCommit = $derived($selectedCommitOid !== null);
  const hasSelectedWorkingFile = $derived($selectedWorkingFile !== null);
  const showDiff = $derived(hasSelectedCommit || hasSelectedWorkingFile);
  const panelRatio = $derived($diffPanelRatio);
  const sbWidth = $derived($sidebarWidth);
  const stWidth = $derived($stagingWidth);

  // Clear working file selection when commit is selected, and vice versa
  $effect(() => {
    if ($selectedCommitOid !== null) {
      $selectedWorkingFile = null;
    }
  });

  // ── Drag resize ────────────────────────────────────────────────────

  let mainAreaEl: HTMLElement | undefined = $state(undefined);
  let contentEl: HTMLElement | undefined = $state(undefined);
  let dragging = $state<"sidebar" | "diff" | "staging" | null>(null);

  function onDragStart(kind: "sidebar" | "diff" | "staging") {
    return (e: MouseEvent) => {
      e.preventDefault();
      dragging = kind;
      document.body.style.userSelect = "none";
      document.body.style.cursor =
        kind === "diff" ? "row-resize" : "col-resize";

      const onMove = (ev: MouseEvent) => {
        if (kind === "diff" && mainAreaEl) {
          const rect = mainAreaEl.getBoundingClientRect();
          const ratio = 1 - (ev.clientY - rect.top) / rect.height;
          $diffPanelRatio = Math.max(0.1, Math.min(0.8, ratio));
        } else if (kind === "sidebar") {
          $sidebarWidth = Math.max(160, Math.min(500, ev.clientX));
        } else if (kind === "staging" && contentEl) {
          const rect = contentEl.getBoundingClientRect();
          $stagingWidth = Math.max(200, Math.min(600, rect.right - ev.clientX));
        }
      };

      const onUp = () => {
        dragging = null;
        document.body.style.userSelect = "";
        document.body.style.cursor = "";
        document.removeEventListener("mousemove", onMove);
        document.removeEventListener("mouseup", onUp);
      };

      document.addEventListener("mousemove", onMove);
      document.addEventListener("mouseup", onUp);
    };
  }

</script>

<div class="app-shell">
  {#if showTitleBar}
    <TitleBar />
  {/if}
  <TabBar />

  {#if repo}
    <div class="content" bind:this={contentEl}>
      <Sidebar />
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="resize-handle-v"
        class:active={dragging === "sidebar"}
        onmousedown={onDragStart("sidebar")}
      ></div>
      <main class="main-area" bind:this={mainAreaEl}>
        <div
          class="graph-panel"
          style="flex: {showDiff ? 1 - panelRatio : 1}"
        >
          <CommitGraph />
        </div>
        {#if showDiff}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="panel-divider"
            class:active={dragging === "diff"}
            onmousedown={onDragStart("diff")}
          ></div>
          <div class="diff-panel" style="flex: {panelRatio}">
            <DiffViewer />
          </div>
        {/if}
      </main>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="resize-handle-v"
        class:active={dragging === "staging"}
        onmousedown={onDragStart("staging")}
      ></div>
      <aside class="staging-panel" style="width: {stWidth}px; min-width: 200px;">
        <StagingArea />
      </aside>
    </div>
  {:else}
    <HomeScreen />
  {/if}
</div>

<style>
  .app-shell {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--color-bg);
  }

  .content {
    display: flex;
    flex: 1;
    overflow: hidden;
  }

  .main-area {
    display: flex;
    flex-direction: column;
    flex: 1;
    overflow: hidden;
  }

  .graph-panel {
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  /* Horizontal divider (graph / diff) */
  .panel-divider {
    height: 3px;
    background: var(--color-border);
    cursor: row-resize;
    flex-shrink: 0;
  }

  .panel-divider:hover,
  .panel-divider.active {
    background: var(--color-accent);
  }

  /* Vertical resize handles (sidebar edges) */
  .resize-handle-v {
    width: 3px;
    cursor: col-resize;
    background: var(--color-border);
    flex-shrink: 0;
  }

  .resize-handle-v:hover,
  .resize-handle-v.active {
    background: var(--color-accent);
  }

  .diff-panel {
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .staging-panel {
    border-left: none;
    background: var(--color-surface);
    overflow: hidden;
    flex-shrink: 0;
  }
</style>
