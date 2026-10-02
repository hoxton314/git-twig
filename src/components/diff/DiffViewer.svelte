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
  import { settings } from "../../lib/stores/settings";
  import * as tauri from "../../lib/tauri";
  import DiffHunk from "./DiffHunk.svelte";
  import ImageDiff from "./ImageDiff.svelte";
  import AudioPreview from "./AudioPreview.svelte";
  import { FileText, Binary, Package, Loader2, Columns2, AlignJustify, Image, Music } from "lucide-svelte";
  import type { DiffFile } from "../../lib/types/git";

  const IMAGE_EXTENSIONS = new Set([
    "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico", "avif", "tiff", "tif",
  ]);

  const AUDIO_EXTENSIONS = new Set([
    "mp3", "wav", "ogg", "oga", "opus", "flac", "m4a", "aac", "weba",
  ]);

  const repoPath = $derived($activeRepoPath);
  const commitOid = $derived($selectedCommitOid);
  const commitDiff = $derived($selectedDiff);
  const workingFile = $derived($selectedWorkingFile);
  const workingDiff = $derived($workingFileDiff);
  const loading = $derived($diffLoading);
  const viewMode = $derived($diffViewMode);
  const tabSize = $derived($settings.tab_size || 4);
  const wordWrap = $derived($settings.word_wrap_in_diffs);

  // Files with more diff lines than this are collapsed behind a "show anyway"
  // button so a huge generated/minified file can't freeze the UI.
  const LARGE_DIFF_LINES = 2000;

  // Show working file diff when a working file is selected or WIP row clicked, otherwise commit diff
  const isWipMode = $derived(commitOid === "__wip__");
  const isWorkingMode = $derived(workingFile !== null || isWipMode);
  const diff = $derived(isWorkingMode ? workingDiff : commitDiff);

  let expandedFiles = $state<Set<string>>(new Set());

  // Large files the user explicitly asked to render.
  let forceShown = $state<Set<string>>(new Set());

  // Image blob cache: fileKey -> { old, new, loading }
  let imageBlobs = $state<Record<string, { old: string | null; new: string | null; loading: boolean }>>({});
  // Bumped whenever the displayed diff changes so in-flight blob loads for the
  // previous diff (other area / commit / file contents) are discarded.
  let blobGen = 0;

  // Load diff when selected commit (or repo) changes. The guard keys on both
  // path and oid so switching repos reloads even if the same oid is selected.
  let lastLoaded: string | null = null;
  let diffRequest = 0;
  $effect(() => {
    const oid = commitOid;
    const path = repoPath;
    const key = path && oid ? `${path} ${oid}` : null;
    if (!oid || oid === "__wip__") {
      // Forget the last commit so re-selecting it later reloads and
      // re-expands instead of showing stale expansion state.
      lastLoaded = null;
      return;
    }
    if (path && key !== lastLoaded) {
      lastLoaded = key;
      loadDiff(path, oid);
    }
  });

  // Any new diff (different file/area/commit, or refreshed contents)
  // invalidates cached image/audio blobs.
  $effect(() => {
    void diff;
    blobGen++;
    imageBlobs = {};
    forceShown = new Set();
  });

  // Auto-expand when working file diff loads
  $effect(() => {
    if (isWorkingMode && workingDiff.length > 0) {
      expandedFiles = new Set(workingDiff.map(fileKey));
    }
  });

  async function loadDiff(path: string, oid: string) {
    const req = ++diffRequest;
    $diffLoading = true;
    expandedFiles = new Set();
    try {
      const result = await tauri.getCommitDiff(path, oid);
      // Drop the result if another commit/repo was selected while loading,
      // so a slow earlier request can't overwrite the current diff.
      if (req !== diffRequest || repoPath !== path) return;
      $selectedDiff = result;
      if (result.length > 0) {
        expandedFiles = new Set([fileKey(result[0])]);
      }
    } catch (err) {
      if (req !== diffRequest) return;
      console.error("Failed to load diff:", err);
      $selectedDiff = [];
    } finally {
      if (req === diffRequest) $diffLoading = false;
    }
  }

  function lineCount(f: DiffFile): number {
    let n = 0;
    for (const h of f.hunks) n += h.lines.length;
    return n;
  }

  function showLarge(f: DiffFile) {
    const next = new Set(forceShown);
    next.add(fileKey(f));
    forceShown = next;
  }

  function displayPath(f: DiffFile): string {
    if (f.old_path && f.new_path && f.old_path !== f.new_path) {
      return `${f.old_path} → ${f.new_path}`;
    }
    return f.new_path ?? f.old_path ?? "unknown";
  }

  function fileKey(f: DiffFile): string {
    return f.new_path ?? f.old_path ?? "unknown";
  }

  function fileExt(f: DiffFile): string {
    const path = f.new_path ?? f.old_path ?? "";
    return path.split(".").pop()?.toLowerCase() ?? "";
  }

  function isImageFile(f: DiffFile): boolean {
    return IMAGE_EXTENSIONS.has(fileExt(f));
  }

  function isAudioFile(f: DiffFile): boolean {
    return AUDIO_EXTENSIONS.has(fileExt(f));
  }

  // Files whose preview needs raw blob contents rather than text hunks.
  function isBlobPreviewFile(f: DiffFile): boolean {
    return isImageFile(f) || isAudioFile(f);
  }

  function toggleFile(f: DiffFile) {
    const key = fileKey(f);
    const next = new Set(expandedFiles);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
      // Image blobs are loaded by the effect that watches expandedFiles, so we
      // don't also kick off a load here (which could double-fetch).
    }
    expandedFiles = next;
  }

  async function loadImageBlobs(f: DiffFile) {
    const path = repoPath;
    if (!path) return;

    const key = fileKey(f);
    const gen = blobGen;
    imageBlobs[key] = { old: null, new: null, loading: true };

    try {
      const filePath = f.new_path ?? f.old_path ?? "";
      const oldFilePath = f.old_path ?? f.new_path ?? "";

      let oldSource: string | null = null;
      let newSource: string | null = null;

      if (isWipMode) {
        // WIP row clicked — combined working changes: old=HEAD, new=workdir.
        // Checked first: WIP mode also sets a placeholder selectedWorkingFile.
        oldSource = f.status !== "added" ? "head" : null;
        newSource = f.status !== "deleted" ? "workdir" : null;
      } else if (isWorkingMode && workingFile) {
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

      // Drop stale results if the repo or displayed diff changed mid-load.
      if (repoPath !== path || gen !== blobGen) return;
      imageBlobs[key] = { old: oldData, new: newData, loading: false };
    } catch (err) {
      if (gen !== blobGen) return;
      console.error("Failed to load image blobs:", err);
      imageBlobs[key] = { old: null, new: null, loading: false };
    }
  }

  // Auto-load blobs for any expanded image/audio file (works for all modes)
  $effect(() => {
    for (const f of diff) {
      const key = fileKey(f);
      if (isBlobPreviewFile(f) && expandedFiles.has(key) && !imageBlobs[key]) {
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
      case "copied":
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
      <!-- Keyed by index too: the WIP diff concatenates staged + unstaged, so the
           same path can legitimately appear twice. -->
      {#each diff as file, fi (`${fi}:${fileKey(file)}`)}
        {@const expanded = expandedFiles.has(fileKey(file))}
        {@const lines = lineCount(file)}
        <div class="file-section">
          <button
            class="file-header"
            onclick={() => toggleFile(file)}
            aria-expanded={expanded}
            title={displayPath(file)}
          >
            <span class="status-badge {statusBadgeClass(file.status)}">{statusLabel(file.status)}</span>
            {#if isImageFile(file)}
              <Image size={14} />
            {:else if isAudioFile(file)}
              <Music size={14} />
            {:else if file.is_binary}
              <Binary size={14} />
            {:else if file.is_lfs}
              <Package size={14} />
            {:else}
              <FileText size={14} />
            {/if}
            <span class="file-path">{displayPath(file)}</span>
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
              {:else if isAudioFile(file)}
                {@const blob = imageBlobs[fileKey(file)]}
                <AudioPreview
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
                <div class="binary-notice">
                  {file.status === "renamed" || file.status === "copied"
                    ? "File renamed without content changes"
                    : file.status === "added"
                      ? "New empty file"
                      : file.status === "deleted"
                        ? "Deleted empty file"
                        : "No content changes"}
                </div>
              {:else if lines > LARGE_DIFF_LINES && !forceShown.has(fileKey(file))}
                <div class="binary-notice">
                  Large diff ({lines.toLocaleString()} lines) hidden
                  <button class="show-large-btn" onclick={() => showLarge(file)}>Show anyway</button>
                </div>
              {:else}
                {#each file.hunks as hunk, hi (hi)}
                  <DiffHunk {hunk} mode={viewMode} {tabSize} wrap={wordWrap} />
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
    background: var(--color-diff-add-bg);
    color: var(--color-diff-add-text);
  }

  .badge-deleted {
    background: var(--color-diff-del-bg);
    color: var(--color-diff-del-text);
  }

  .badge-modified {
    background: color-mix(in srgb, var(--color-lane-2) 20%, transparent);
    color: var(--color-lane-2);
  }

  .badge-renamed {
    background: color-mix(in srgb, var(--color-accent) 20%, transparent);
    color: var(--color-accent);
  }

  .show-large-btn {
    padding: 2px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
  }

  .show-large-btn:hover {
    background: var(--color-surface-elevated);
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
