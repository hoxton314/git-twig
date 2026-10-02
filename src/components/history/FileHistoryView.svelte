<script lang="ts">
  import { untrack } from "svelte";
  import * as tauri from "../../lib/tauri";
  import type { DiffFile, FileHistoryEntry } from "../../lib/types/git";
  import { showBlame, showInGraph } from "../../lib/stores/fileviews";
  import { toast } from "../../lib/stores/toasts";
  import DiffFileList from "./DiffFileList.svelte";
  import ContextMenu, { type MenuItem } from "../shared/ContextMenu.svelte";
  import { relativeTime, fullDate, copyText } from "./format";
  import { Loader2, GitCommitHorizontal, ScanLine, History } from "lucide-svelte";

  interface Props {
    repoPath: string;
    path: string;
  }

  let { repoPath, path }: Props = $props();

  const PAGE = 100;

  let entries = $state<FileHistoryEntry[]>([]);
  let hasMore = $state(false);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let selectedIdx = $state<number>(-1);

  let diff = $state<DiffFile[]>([]);
  let diffLoading = $state(false);
  let diffError = $state<string | null>(null);

  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  let listEl = $state<HTMLDivElement | null>(null);

  // Generation counter: a reload (new path/repo) invalidates in-flight pages.
  let gen = 0;

  $effect(() => {
    const p = path;
    const r = repoPath;
    untrack(() => reset(r, p));
  });

  async function reset(r: string, p: string) {
    const g = ++gen;
    entries = [];
    hasMore = false;
    selectedIdx = -1;
    diff = [];
    error = null;
    await loadPage(r, p, g);
    if (g === gen && entries.length > 0) select(0);
  }

  async function loadPage(r: string, p: string, g: number) {
    loading = true;
    try {
      const page = await tauri.getFileHistory(r, p, entries.length, PAGE);
      if (g !== gen) return;
      entries = [...entries, ...page.entries];
      hasMore = page.has_more;
    } catch (err) {
      if (g !== gen) return;
      error = String(err);
    } finally {
      if (g === gen) loading = false;
    }
  }

  function loadMore() {
    if (loading || !hasMore) return;
    loadPage(repoPath, path, gen);
  }

  function onListScroll() {
    if (!listEl) return;
    if (listEl.scrollTop + listEl.clientHeight >= listEl.scrollHeight - 200) loadMore();
  }

  let diffReq = 0;
  async function select(i: number) {
    const e = entries[i];
    if (!e) return;
    selectedIdx = i;
    const req = ++diffReq;
    diffLoading = true;
    diffError = null;
    try {
      const d = await tauri.getFileDiffAtCommit(repoPath, e.commit.oid, e.path, e.old_path);
      if (req === diffReq) diff = d;
    } catch (err) {
      if (req === diffReq) {
        diff = [];
        diffError = String(err);
      }
    } finally {
      if (req === diffReq) diffLoading = false;
    }
    queueMicrotask(() => {
      listEl?.querySelector<HTMLElement>(`[data-idx="${i}"]`)?.scrollIntoView({ block: "nearest" });
    });
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    if (e.key === "ArrowDown" || e.key === "j") {
      e.preventDefault();
      if (selectedIdx < entries.length - 1) select(selectedIdx + 1);
      if (selectedIdx >= entries.length - 5) loadMore();
    } else if (e.key === "ArrowUp" || e.key === "k") {
      e.preventDefault();
      if (selectedIdx > 0) select(selectedIdx - 1);
    }
  }

  function blameAt(e: FileHistoryEntry) {
    if (e.status === "deleted") {
      toast("info", "The file does not exist in this commit (it was deleted here).");
      return;
    }
    showBlame(e.path, e.commit.oid);
  }

  function openMenu(ev: MouseEvent, e: FileHistoryEntry) {
    ev.preventDefault();
    menu = {
      x: ev.clientX,
      y: ev.clientY,
      items: [
        { label: "Show in graph", action: () => { showInGraph(e.commit.oid); } },
        { label: "Blame at this commit", action: () => blameAt(e), disabled: e.status === "deleted" },
        { separator: true },
        {
          label: "Copy commit SHA",
          action: async () => {
            if (await copyText(e.commit.oid)) toast("success", `Copied ${e.commit.short_oid}`);
          },
        },
      ],
    };
  }

  const selected = $derived(selectedIdx >= 0 ? entries[selectedIdx] : null);

  function statusLetter(s: string) {
    return s.charAt(0).toUpperCase();
  }
</script>

<div class="history">
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="list"
    bind:this={listEl}
    onscroll={onListScroll}
    onkeydown={onKeydown}
    tabindex="0"
    role="listbox"
    aria-label="Commits that changed {path}"
  >
    {#if error}
      <div class="empty error">{error}</div>
    {:else if entries.length === 0 && loading}
      <div class="empty"><Loader2 size={14} class="spinner" /> Walking history…</div>
    {:else if entries.length === 0}
      <div class="empty">No commits touch this file.</div>
    {/if}

    {#each entries as e, i (e.commit.oid)}
      <div
        class="entry"
        class:selected={i === selectedIdx}
        data-idx={i}
        role="option"
        aria-selected={i === selectedIdx}
        tabindex="-1"
        onclick={() => select(i)}
        onkeydown={() => {}}
        oncontextmenu={(ev) => openMenu(ev, e)}
        ondblclick={() => showInGraph(e.commit.oid)}
      >
        <div class="row1">
          <span class="badge badge-{e.status}" title={e.status}>{statusLetter(e.status)}</span>
          <span class="summary" title={e.commit.summary}>{e.commit.summary || "(no message)"}</span>
        </div>
        <div class="row2">
          <span class="oid">{e.commit.short_oid}</span>
          <span class="author">{e.commit.author_name}</span>
          <span class="time" title={fullDate(e.commit.timestamp)}>{relativeTime(e.commit.timestamp)}</span>
        </div>
        {#if e.old_path}
          <div class="renamed" title="{e.old_path} → {e.path}">renamed from {e.old_path}</div>
        {/if}
      </div>
    {/each}

    {#if entries.length > 0 && (hasMore || loading)}
      <div class="more">
        {#if loading}
          <Loader2 size={12} class="spinner" /> Loading…
        {:else}
          <button class="link-btn" onclick={loadMore}>Load more</button>
        {/if}
      </div>
    {/if}
  </div>

  <div class="detail">
    {#if selected}
      <div class="detail-header">
        <div class="commit-meta">
          <div class="commit-summary">{selected.commit.summary}</div>
          <div class="commit-sub">
            <span class="oid">{selected.commit.short_oid}</span>
            · {selected.commit.author_name}
            · <span title={fullDate(selected.commit.timestamp)}>{relativeTime(selected.commit.timestamp)}</span>
            · <span class="mono">{selected.path}</span>
          </div>
        </div>
        <div class="actions">
          <button class="btn" onclick={() => showInGraph(selected.commit.oid)} title="Select this commit in the graph">
            <GitCommitHorizontal size={13} /> Show in graph
          </button>
          <button class="btn" onclick={() => blameAt(selected)} disabled={selected.status === "deleted"} title="Blame the file as of this commit">
            <ScanLine size={13} /> Blame here
          </button>
        </div>
      </div>
      {#if selected.commit.body.trim()}
        <pre class="commit-body">{selected.commit.body.trim()}</pre>
      {/if}
      <div class="diff-scroll">
        {#if diffLoading}
          <div class="empty"><Loader2 size={14} class="spinner" /> Loading diff…</div>
        {:else if diffError}
          <div class="empty error">{diffError}</div>
        {:else}
          <DiffFileList files={diff} hideHeaders={diff.length === 1} emptyText="No changes to this file in this commit" />
        {/if}
      </div>
    {:else if !loading && !error && entries.length > 0}
      <div class="empty center"><History size={16} /> Select a commit</div>
    {/if}
  </div>
</div>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<style>
  .history {
    display: flex;
    height: 100%;
    min-height: 0;
  }

  .list {
    width: 340px;
    flex-shrink: 0;
    overflow-y: auto;
    border-right: 1px solid var(--color-border);
    outline: none;
  }

  .list:focus-visible {
    box-shadow: inset 0 0 0 1px var(--color-accent);
  }

  .entry {
    padding: 7px 12px;
    border-bottom: 1px solid var(--color-border);
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .entry:hover {
    background: var(--color-surface-elevated);
  }

  .entry.selected {
    background: color-mix(in srgb, var(--color-accent) 16%, transparent);
  }

  .row1,
  .row2 {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .summary {
    font-size: 12px;
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row2 {
    font-size: 11px;
    color: var(--color-text-muted);
  }

  .author {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }

  .time {
    flex-shrink: 0;
  }

  .oid {
    font-family: var(--font-mono);
    color: var(--color-accent);
    flex-shrink: 0;
  }

  .mono {
    font-family: var(--font-mono);
  }

  .renamed {
    font-size: 11px;
    color: var(--color-accent-secondary);
    font-family: var(--font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    border-radius: 3px;
    font-size: 9px;
    font-weight: 700;
    flex-shrink: 0;
    background: color-mix(in srgb, var(--color-lane-2) 20%, transparent);
    color: var(--color-lane-2);
  }

  .badge-added {
    background: var(--color-diff-add-bg);
    color: var(--color-diff-add-text);
  }

  .badge-deleted {
    background: var(--color-diff-del-bg);
    color: var(--color-diff-del-text);
  }

  .badge-renamed {
    background: color-mix(in srgb, var(--color-accent) 20%, transparent);
    color: var(--color-accent);
  }

  .detail {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .detail-header {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-surface);
  }

  .commit-meta {
    flex: 1;
    min-width: 0;
  }

  .commit-summary {
    font-size: 13px;
    font-weight: 600;
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .commit-sub {
    font-size: 11px;
    color: var(--color-text-muted);
    margin-top: 3px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .commit-body {
    margin: 0;
    padding: 8px 14px;
    max-height: 90px;
    overflow-y: auto;
    font-family: var(--font-sans);
    font-size: 12px;
    white-space: pre-wrap;
    color: var(--color-text-muted);
    border-bottom: 1px solid var(--color-border);
  }

  .actions {
    display: flex;
    gap: 6px;
    flex-shrink: 0;
  }

  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
  }

  .btn:hover:not(:disabled) {
    border-color: var(--color-accent);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .diff-scroll {
    flex: 1;
    overflow: auto;
    min-height: 0;
  }

  .empty,
  .more {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 14px;
    font-size: 12px;
    color: var(--color-text-muted);
  }

  .empty.center {
    justify-content: center;
    height: 100%;
  }

  .empty.error {
    color: var(--color-diff-del-text);
    white-space: pre-wrap;
  }

  .link-btn {
    border: none;
    background: none;
    color: var(--color-accent);
    cursor: pointer;
    font-size: 12px;
    padding: 0;
  }

  .history :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
