<script lang="ts">
  import TabBar from "./TabBar.svelte";
  import Sidebar from "./Sidebar.svelte";
  import CommitGraph from "../graph/CommitGraph.svelte";
  import DiffViewer from "../diff/DiffViewer.svelte";
  import StagingArea from "../staging/StagingArea.svelte";
  import { activeRepo } from "../../lib/stores/repos";
  import { selectedCommitOid, selectedWorkingFile, workingFileDiff } from "../../lib/stores/graph";
  import { diffPanelRatio } from "../../lib/stores/ui";
  import { FolderOpen } from "lucide-svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import * as tauri from "../../lib/tauri";
  import { addRepo } from "../../lib/stores/repos";

  const repo = $derived($activeRepo);
  const hasSelectedCommit = $derived($selectedCommitOid !== null);
  const hasSelectedWorkingFile = $derived($selectedWorkingFile !== null);
  const showDiff = $derived(hasSelectedCommit || hasSelectedWorkingFile);
  const panelRatio = $derived($diffPanelRatio);

  // Clear working file selection when commit is selected, and vice versa
  $effect(() => {
    if ($selectedCommitOid !== null) {
      $selectedWorkingFile = null;
    }
  });

  async function handleOpenRepo() {
    const selected = await open({ directory: true, multiple: false, title: "Open Git Repository" });
    if (!selected) return;
    try {
      const info = await tauri.openRepo(selected as string);
      addRepo(info);
    } catch (err) {
      console.error("Failed to open repo:", err);
    }
  }
</script>

<div class="app-shell">
  <TabBar />

  {#if repo}
    <div class="content">
      <Sidebar />
      <main class="main-area">
        <div
          class="graph-panel"
          style="flex: {showDiff ? 1 - panelRatio : 1}"
        >
          <CommitGraph />
        </div>
        {#if showDiff}
          <div class="panel-divider"></div>
          <div class="diff-panel" style="flex: {panelRatio}">
            <DiffViewer />
          </div>
        {/if}
      </main>
      <aside class="staging-panel">
        <StagingArea />
      </aside>
    </div>
  {:else}
    <div class="empty-state">
      <div class="empty-content">
        <FolderOpen size={48} strokeWidth={1} />
        <h2>Open a repository to get started</h2>
        <p>Use the <strong>+</strong> button in the tab bar or click below.</p>
        <button class="open-button" onclick={handleOpenRepo}>
          Open Repository
        </button>
      </div>
    </div>
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

  .panel-divider {
    height: 3px;
    background: var(--color-border);
    cursor: row-resize;
    flex-shrink: 0;
  }

  .panel-divider:hover {
    background: var(--color-accent);
  }

  .diff-panel {
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .staging-panel {
    width: 300px;
    min-width: 240px;
    border-left: 1px solid var(--color-border);
    background: var(--color-surface);
    overflow: hidden;
    flex-shrink: 0;
  }

  .empty-state {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .empty-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    color: var(--color-text-muted);
    text-align: center;
  }

  .empty-content h2 {
    color: var(--color-text-primary);
    font-size: 18px;
    font-weight: 500;
    margin: 0;
  }

  .empty-content p {
    font-size: 13px;
    margin: 0;
  }

  .open-button {
    margin-top: 8px;
    padding: 8px 20px;
    border: 1px solid var(--color-accent);
    border-radius: 6px;
    background: transparent;
    color: var(--color-accent);
    font-size: 13px;
    cursor: pointer;
    transition: background 0.15s;
  }

  .open-button:hover {
    background: rgba(122, 162, 247, 0.15);
  }
</style>
