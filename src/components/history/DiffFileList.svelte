<script lang="ts">
  /**
   * Read-only list of file diffs (no store coupling). Used by file history
   * and the stash viewer.
   */
  import DiffHunk from "../diff/DiffHunk.svelte";
  import { diffViewMode } from "../../lib/stores/ui";
  import { settings } from "../../lib/stores/settings";
  import { t } from "../../lib/i18n";
  import { FileText, Binary, Package, ChevronDown, ChevronRight } from "lucide-svelte";
  import type { DiffFile } from "../../lib/types/git";

  interface Props {
    files: DiffFile[];
    /** Expand every file initially (otherwise only the first). */
    expandAll?: boolean;
    /** Hide the per-file header (single-file views). */
    hideHeaders?: boolean;
    emptyText?: string;
  }

  let { files, expandAll = false, hideHeaders = false, emptyText }: Props = $props();

  const LARGE_DIFF_LINES = 2000;

  const viewMode = $derived($diffViewMode);
  const tabSize = $derived($settings.tab_size || 4);
  const wordWrap = $derived($settings.word_wrap_in_diffs);

  let expanded = $state<Set<number>>(new Set());
  let forceShown = $state<Set<number>>(new Set());

  $effect(() => {
    const list = files;
    expanded = new Set(expandAll || hideHeaders ? list.map((_, i) => i) : list.length > 0 ? [0] : []);
    forceShown = new Set();
  });

  function toggle(i: number) {
    const next = new Set(expanded);
    if (next.has(i)) next.delete(i);
    else next.add(i);
    expanded = next;
  }

  function displayPath(f: DiffFile): string {
    if (f.old_path && f.new_path && f.old_path !== f.new_path) return `${f.old_path} → ${f.new_path}`;
    return f.new_path ?? f.old_path ?? $t("diff.unknownPath");
  }

  function lineCount(f: DiffFile): number {
    let n = 0;
    for (const h of f.hunks) n += h.lines.length;
    return n;
  }

  function badgeClass(status: string): string {
    switch (status) {
      case "added":
        return "badge-added";
      case "deleted":
        return "badge-deleted";
      case "renamed":
      case "copied":
        return "badge-renamed";
      default:
        return "badge-modified";
    }
  }
</script>

<div class="diff-file-list">
  {#each files as file, i (i)}
    {@const open = expanded.has(i)}
    {@const lines = lineCount(file)}
    <div class="file-section">
      {#if !hideHeaders}
        <button class="file-header" onclick={() => toggle(i)} aria-expanded={open} title={displayPath(file)}>
          {#if open}<ChevronDown size={12} />{:else}<ChevronRight size={12} />{/if}
          <span class="status-badge {badgeClass(file.status)}">{file.status.charAt(0).toUpperCase()}</span>
          {#if file.is_binary}<Binary size={13} />{:else if file.is_lfs}<Package size={13} />{:else}<FileText size={13} />{/if}
          <span class="file-path">{displayPath(file)}</span>
        </button>
      {/if}
      {#if open}
        <div class="file-diff">
          {#if file.is_lfs}
            <div class="notice"><Package size={14} /> {$t("diff.lfsObject", { size: file.lfs_size ?? $t("diff.unknownSize") })}</div>
          {:else if file.is_binary}
            <div class="notice"><Binary size={14} /> {$t("diff.binaryFile")}</div>
          {:else if file.hunks.length === 0}
            <div class="notice">
              {file.status === "renamed" || file.status === "copied"
                ? $t("diff.renamedNoChanges")
                : file.status === "added"
                  ? $t("diff.newEmptyFile")
                  : file.status === "deleted"
                    ? $t("diff.deletedEmptyFile")
                    : $t("diff.noContentChanges")}
            </div>
          {:else if lines > LARGE_DIFF_LINES && !forceShown.has(i)}
            <div class="notice">
              {$t("diff.largeDiffLinesHidden", { lines: lines.toLocaleString() })}
              <button class="show-btn" onclick={() => (forceShown = new Set([...forceShown, i]))}>{$t("diff.showAnyway")}</button>
            </div>
          {:else}
            {#each file.hunks as hunk, hi (hi)}
              <DiffHunk {hunk} mode={viewMode} {tabSize} wrap={wordWrap} />
            {/each}
          {/if}
        </div>
      {/if}
    </div>
  {:else}
    <div class="notice">{emptyText ?? $t("diff.noChangesShort")}</div>
  {/each}
</div>

<style>
  .diff-file-list {
    display: flex;
    flex-direction: column;
  }

  .file-section {
    border-bottom: 1px solid var(--color-border);
  }

  .file-header {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 6px 10px;
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

  .file-path {
    font-family: var(--font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file-diff {
    background: var(--color-bg);
  }

  .notice {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 14px 16px;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .show-btn {
    padding: 2px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
  }
</style>
