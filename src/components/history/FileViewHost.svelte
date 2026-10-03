<script lang="ts">
  /**
   * Hosts the file history / blame overlay and the file picker, and handles
   * the "file_history" / "blame_file" actions. Mounted once in AppShell.
   */
  import { onMount } from "svelte";
  import Modal from "../shared/Modal.svelte";
  import FilePicker from "./FilePicker.svelte";
  import FileHistoryView from "./FileHistoryView.svelte";
  import BlameView from "./BlameView.svelte";
  import {
    fileView,
    filePickerFor,
    closeFileView,
    pickFileFor,
    showBlame,
    showFileHistory,
  } from "../../lib/stores/fileviews";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { onAction } from "../../lib/keybindings";
  import * as tauri from "../../lib/tauri";
  import { History, ScanLine } from "lucide-svelte";
  import { t } from "../../lib/i18n";

  const view = $derived($fileView);
  const repoPath = $derived($activeRepoPath);
  const picker = $derived($filePickerFor);

  let pickerItems = $state<string[] | null>(null);
  let pickerError = $state<string | null>(null);

  $effect(() => {
    const kind = picker;
    const path = repoPath;
    if (!kind || !path) return;
    pickerItems = null;
    pickerError = null;
    tauri
      .listTrackedFiles(path)
      .then((files) => {
        if ($activeRepoPath === path) pickerItems = files;
      })
      .catch((err) => (pickerError = String(err)));
  });

  onMount(() => {
    const unsubs = [
      onAction("file_history", () => {
        if ($activeRepoPath) pickFileFor("history");
      }),
      onAction("blame_file", () => {
        if ($activeRepoPath) pickFileFor("blame");
      }),
    ];
    return () => unsubs.forEach((u) => u());
  });

  function onPick(paths: string[]) {
    const p = paths[0];
    if (!p) return;
    if (picker === "blame") showBlame(p);
    else showFileHistory(p);
  }

  const title = $derived(view ? (view.path.split("/").pop() ?? view.path) : "");
</script>

{#if repoPath}
  <Modal open={view !== null} title={title} onclose={closeFileView} width="min(1320px, 95vw)">
    {#if view}
      <div class="file-view">
        <div class="tabs" role="tablist">
          <button
            role="tab"
            class="tab"
            class:active={view.kind === "history"}
            aria-selected={view.kind === "history"}
            onclick={() => showFileHistory(view.path)}
          >
            <History size={13} /> {$t("history.tab")}
          </button>
          <button
            role="tab"
            class="tab"
            class:active={view.kind === "blame"}
            aria-selected={view.kind === "blame"}
            onclick={() => view.kind !== "blame" && showBlame(view.path)}
          >
            <ScanLine size={13} /> {$t("blame.tab")}
          </button>
          <span class="full-path" title={view.path}>{view.path}</span>
        </div>
        <div class="pane">
          {#if view.kind === "history"}
            <FileHistoryView {repoPath} path={view.path} />
          {:else}
            <BlameView {repoPath} path={view.path} rev={view.rev} line={view.line} />
          {/if}
        </div>
      </div>
    {/if}
  </Modal>

  <FilePicker
    open={picker !== null}
    title={picker === "blame" ? $t("blame.pickerTitle") : $t("history.pickerTitle")}
    items={pickerItems}
    error={pickerError}
    onconfirm={onPick}
    onclose={() => filePickerFor.set(null)}
  />
{/if}

<style>
  .file-view {
    display: flex;
    flex-direction: column;
    height: calc(100vh - 150px);
    margin: -20px;
  }

  .tabs {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 6px 10px 0;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .tab {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border: none;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 12px;
    cursor: pointer;
  }

  .tab:hover {
    color: var(--color-text-primary);
  }

  .tab.active {
    color: var(--color-text-primary);
    border-bottom-color: var(--color-accent);
  }

  .full-path {
    margin-left: auto;
    padding-bottom: 4px;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .pane {
    flex: 1;
    min-height: 0;
  }
</style>
