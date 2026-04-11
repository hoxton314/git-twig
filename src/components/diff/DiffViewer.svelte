<script lang="ts">
  import { selectedCommitOid, selectedDiff, diffLoading } from "../../lib/stores/graph";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { diffViewMode } from "../../lib/stores/ui";
  import * as tauri from "../../lib/tauri";
  import DiffHunk from "./DiffHunk.svelte";
  import { FileText, Binary, Package, Loader2, Columns2, AlignJustify } from "lucide-svelte";
  import type { DiffFile } from "../../lib/types/git";

  const repoPath = $derived($activeRepoPath);
  const commitOid = $derived($selectedCommitOid);
  const diff = $derived($selectedDiff);
  const loading = $derived($diffLoading);
  const viewMode = $derived($diffViewMode);

  let expandedFiles = $state<Set<string>>(new Set());

  // Load diff when selected commit changes
  let lastLoadedOid: string | null = null;
  $effect(() => {
    const oid = commitOid;
    const path = repoPath;
    if (path && oid && oid !== lastLoadedOid) {
      lastLoadedOid = oid;
      loadDiff(path, oid);
    }
  });

  async function loadDiff(path: string, oid: string) {
    $diffLoading = true;
    expandedFiles = new Set();
    try {
      const result = await tauri.getCommitDiff(path, oid);
      $selectedDiff = result;
      // Auto-expand first file
      if (result.length > 0) {
        expandedFiles = new Set([fileKey(result[0])]);
      }
    } catch (err) {
      console.error("Failed to load diff:", err);
      $selectedDiff = [];
    } finally {
      $diffLoading = false;
    }
  }

  function fileKey(f: DiffFile): string {
    return f.new_path ?? f.old_path ?? "unknown";
  }

  function toggleFile(f: DiffFile) {
    const key = fileKey(f);
    const next = new Set(expandedFiles);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    expandedFiles = next;
  }

  function statusBadgeClass(status: string): string {
    switch (status) {
      case "added": return "badge-added";
      case "deleted": return "badge-deleted";
      case "modified": return "badge-modified";
      case "renamed": return "badge-renamed";
      default: return "";
    }
  }

  function statusLabel(status: string): string {
    return status.charAt(0).toUpperCase();
  }
</script>

<div class="diff-viewer">
  <div class="diff-header">
    <span class="diff-title">
      {#if commitOid}
        <span class="oid">{commitOid?.slice(0, 7)}</span>
        — {diff.length} file{diff.length !== 1 ? "s" : ""} changed
      {/if}
    </span>
    <div class="view-toggle">
      <button
        class="toggle-btn"
        class:active={viewMode === "unified"}
        onclick={() => ($diffViewMode = "unified")}
        title="Unified view"
      >
        <AlignJustify size={14} />
      </button>
      <button
        class="toggle-btn"
        class:active={viewMode === "split"}
        onclick={() => ($diffViewMode = "split")}
        title="Split view"
      >
        <Columns2 size={14} />
      </button>
    </div>
  </div>

  <div class="diff-files">
    {#if loading}
      <div class="loading">
        <Loader2 size={20} class="spinner" />
        <span>Loading diff...</span>
      </div>
    {:else}
      {#each diff as file (fileKey(file))}
        {@const expanded = expandedFiles.has(fileKey(file))}
        <div class="file-section">
          <button class="file-header" onclick={() => toggleFile(file)}>
            <span class="status-badge {statusBadgeClass(file.status)}">{statusLabel(file.status)}</span>
            {#if file.is_binary}
              <Binary size={14} />
            {:else if file.is_lfs}
              <Package size={14} />
            {:else}
              <FileText size={14} />
            {/if}
            <span class="file-path">{file.new_path ?? file.old_path ?? "unknown"}</span>
          </button>

          {#if expanded}
            <div class="file-diff">
              {#if file.is_lfs}
                <div class="lfs-notice">
                  <Package size={16} />
                  LFS object — {file.lfs_size ?? "unknown size"}
                </div>
              {:else if file.is_binary}
                <div class="binary-notice">
                  <Binary size={16} />
                  Binary file
                </div>
              {:else}
                {#each file.hunks as hunk, hi (hi)}
                  <DiffHunk {hunk} mode={viewMode} />
                {/each}
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    {/if}
  </div>
</div>

<style>
  .diff-viewer {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .diff-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 12px;
    background: var(--color-surface);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .diff-title {
    font-size: 12px;
    color: var(--color-text-muted);
  }

  .oid {
    font-family: var(--font-mono);
    color: var(--color-accent);
  }

  .view-toggle {
    display: flex;
    gap: 2px;
  }

  .toggle-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 24px;
    border: 1px solid var(--color-border);
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    border-radius: 4px;
    padding: 0;
  }

  .toggle-btn.active {
    background: var(--color-surface-elevated);
    color: var(--color-accent);
    border-color: var(--color-accent);
  }

  .diff-files {
    flex: 1;
    overflow-y: auto;
  }

  .loading {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 16px;
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

  .file-section {
    border-bottom: 1px solid var(--color-border);
  }

  .file-header {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 12px;
    border: none;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
    text-align: left;
  }

  .file-header:hover {
    background: var(--color-surface-elevated);
  }

  .status-badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border-radius: 3px;
    font-size: 10px;
    font-weight: 700;
    flex-shrink: 0;
  }

  .badge-added {
    background: rgba(158, 206, 106, 0.2);
    color: #9ece6a;
  }

  .badge-deleted {
    background: rgba(247, 118, 142, 0.2);
    color: #f7768e;
  }

  .badge-modified {
    background: rgba(224, 175, 104, 0.2);
    color: #e0af68;
  }

  .badge-renamed {
    background: rgba(122, 162, 247, 0.2);
    color: #7aa2f7;
  }

  .file-path {
    font-family: var(--font-mono);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file-diff {
    background: var(--color-bg);
  }

  .lfs-notice,
  .binary-notice {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 16px 20px;
    color: var(--color-text-muted);
    font-size: 13px;
  }
</style>
