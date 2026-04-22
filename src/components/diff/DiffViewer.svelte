<script lang="ts">
  import {
    selectedCommitOid,
    selectedDiff,
    diffLoading,
    selectedWorkingFile,
    workingFileDiff,
    commitGraph,
  } from "../../lib/stores/graph";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { diffViewMode } from "../../lib/stores/ui";
  import * as tauri from "../../lib/tauri";
  import DiffHunk from "./DiffHunk.svelte";
  import ImageDiff from "./ImageDiff.svelte";
  import { FileText, Binary, Package, Loader2, Columns2, AlignJustify, Image } from "lucide-svelte";
  import type { DiffFile } from "../../lib/types/git";

  const IMAGE_EXTENSIONS = new Set([
    "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico", "avif", "tiff", "tif",
  ]);

  const repoPath = $derived($activeRepoPath);
  const commitOid = $derived($selectedCommitOid);
  const commitDiff = $derived($selectedDiff);
  const workingFile = $derived($selectedWorkingFile);
  const workingDiff = $derived($workingFileDiff);
  const loading = $derived($diffLoading);
  const viewMode = $derived($diffViewMode);

  // Show working file diff when a working file is selected or WIP row clicked, otherwise commit diff
  const isWipMode = $derived(commitOid === "__wip__");
  const isWorkingMode = $derived(workingFile !== null || isWipMode);
  const diff = $derived(isWorkingMode ? workingDiff : commitDiff);

  let expandedFiles = $state<Set<string>>(new Set());

  // Image blob cache: fileKey -> { old, new, loading }
  let imageBlobs = $state<Record<string, { old: string | null; new: string | null; loading: boolean }>>({});

  // Load diff when selected commit changes
  let lastLoadedOid: string | null = null;
  $effect(() => {
    const oid = commitOid;
    const path = repoPath;
    if (path && oid && oid !== "__wip__" && oid !== lastLoadedOid) {
      lastLoadedOid = oid;
      loadDiff(path, oid);
    }
  });

  // Auto-expand when working file diff loads
  $effect(() => {
    if (isWorkingMode && workingDiff.length > 0) {
      expandedFiles = new Set(workingDiff.map(fileKey));
    }
  });

  async function loadDiff(path: string, oid: string) {
    $diffLoading = true;
    expandedFiles = new Set();
    imageBlobs = {};
    try {
      const result = await tauri.getCommitDiff(path, oid);
      $selectedDiff = result;
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

  function isImageFile(f: DiffFile): boolean {
    const path = f.new_path ?? f.old_path ?? "";
    const ext = path.split(".").pop()?.toLowerCase() ?? "";
    return IMAGE_EXTENSIONS.has(ext);
  }

  function toggleFile(f: DiffFile) {
    const key = fileKey(f);
    const next = new Set(expandedFiles);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
      if (isImageFile(f) && !imageBlobs[key]) {
        loadImageBlobs(f);
      }
    }
    expandedFiles = next;
  }

  async function loadImageBlobs(f: DiffFile) {
    const path = repoPath;
    if (!path) return;

    const key = fileKey(f);
    imageBlobs[key] = { old: null, new: null, loading: true };

    try {
      const filePath = f.new_path ?? f.old_path ?? "";
      const oldFilePath = f.old_path ?? f.new_path ?? "";

      let oldSource: string | null = null;
      let newSource: string | null = null;

      if (isWorkingMode && workingFile) {
        // Single file selected in staging area
        if (workingFile.area === "staged") {
          // staged: old=HEAD, new=index
          oldSource = f.status !== "added" ? "head" : null;
          newSource = f.status !== "deleted" ? "index" : null;
        } else {
          // unstaged: old=index, new=workdir
          oldSource = f.status !== "added" ? "index" : null;
          newSource = f.status !== "deleted" ? "workdir" : null;
        }
      } else if (isWipMode) {
        // WIP row clicked — combined working changes: old=HEAD, new=workdir
        oldSource = f.status !== "added" ? "head" : null;
        newSource = f.status !== "deleted" ? "workdir" : null;
      } else if (commitOid && commitOid !== "__wip__") {
        // Commit diff: old=parent, new=commit
        let parentOid: string | null = null;
        const g = $commitGraph;
        if (g) {
          const entry = g.entries.find((e) => e.commit.oid === commitOid);
          if (entry && entry.commit.parent_oids.length > 0) {
            parentOid = entry.commit.parent_oids[0];
          }
        }
        oldSource = f.status !== "added" && parentOid ? parentOid : null;
        newSource = f.status !== "deleted" ? commitOid : null;
      }

      const [oldData, newData] = await Promise.all([
        oldSource ? tauri.getFileBlob(path, oldFilePath, oldSource) : Promise.resolve(null),
        newSource ? tauri.getFileBlob(path, filePath, newSource) : Promise.resolve(null),
      ]);

      imageBlobs[key] = { old: oldData, new: newData, loading: false };
    } catch (err) {
      console.error("Failed to load image blobs:", err);
      imageBlobs[key] = { old: null, new: null, loading: false };
    }
  }

  // Auto-load image blobs for any expanded image file (works for all modes)
  $effect(() => {
    for (const f of diff) {
      const key = fileKey(f);
      if (isImageFile(f) && expandedFiles.has(key) && !imageBlobs[key]) {
        loadImageBlobs(f);
      }
    }
  });

  function statusBadgeClass(status: string): string {
    switch (status) {
      case "added":
      case "untracked":
        return "badge-added";
      case "deleted":
        return "badge-deleted";
      case "modified":
        return "badge-modified";
      case "renamed":
        return "badge-renamed";
      default:
        return "";
    }
  }

  function statusLabel(status: string): string {
    return status.charAt(0).toUpperCase();
  }
</script>

<div class="diff-viewer">
  <div class="diff-header">
    <span class="diff-title">
      {#if isWipMode}
        <span class="label">Working changes</span>
        — {diff.length} file{diff.length !== 1 ? "s" : ""} changed
      {:else if isWorkingMode && workingFile}
        <span class="label">{workingFile.area === "staged" ? "Staged" : "Unstaged"}</span>
        — {workingFile.path}
      {:else if commitOid}
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
            {#if isImageFile(file)}
              <Image size={14} />
            {:else if file.is_binary}
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
              {:else if isImageFile(file)}
                {@const blob = imageBlobs[fileKey(file)]}
                <ImageDiff
                  oldData={blob?.old ?? null}
                  newData={blob?.new ?? null}
                  filePath={file.new_path ?? file.old_path ?? ""}
                  loading={blob?.loading ?? true}
                />
              {:else if file.is_binary}
                <div class="binary-notice">
                  <Binary size={16} />
                  Binary file
                </div>
              {:else if file.hunks.length === 0}
                <div class="binary-notice">New file (empty diff)</div>
              {:else}
                {#each file.hunks as hunk, hi (hi)}
                  <DiffHunk {hunk} mode={viewMode} />
                {/each}
              {/if}
            </div>
          {/if}
        </div>
      {/each}
      {#if diff.length === 0 && !loading}
        <div class="empty-diff">No changes to display</div>
      {/if}
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
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .oid {
    font-family: var(--font-mono);
    color: var(--color-accent);
  }

  .label {
    color: var(--color-accent-secondary);
    font-weight: 500;
  }

  .view-toggle {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
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

  .loading,
  .empty-diff {
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
