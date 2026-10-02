<script lang="ts">
  import { untrack } from "svelte";
  import * as tauri from "../../lib/tauri";
  import type { BlameHunk, BlameResult } from "../../lib/types/git";
  import { showFileHistory, showInGraph } from "../../lib/stores/fileviews";
  import { settings } from "../../lib/stores/settings";
  import { toast } from "../../lib/stores/toasts";
  import ContextMenu, { type MenuItem } from "../shared/ContextMenu.svelte";
  import { relativeTime, fullDate, copyText } from "./format";
  import { Loader2, ArrowLeft, GitCommitHorizontal, History, ChevronsDown } from "lucide-svelte";

  interface Props {
    repoPath: string;
    path: string;
    rev?: string;
  }

  let { repoPath, path, rev }: Props = $props();

  interface Frame {
    path: string;
    rev?: string;
  }

  interface Group {
    hunk: BlameHunk;
    start: number; // 1-based
    count: number;
  }

  /** Navigation stack for "blame previous revision" (top = current). */
  let stack = $state<Frame[]>([]);
  let result = $state<BlameResult | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let selectedOid = $state<string | null>(null);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  let scrollEl = $state<HTMLDivElement | null>(null);

  const current = $derived(stack.length > 0 ? stack[stack.length - 1] : null);
  const tabSize = $derived($settings.tab_size || 4);

  // A new (path, rev) from outside starts a fresh navigation stack.
  $effect(() => {
    const frame = { path, rev };
    untrack(() => {
      stack = [frame];
    });
  });

  let req = 0;
  $effect(() => {
    const f = current;
    const r = repoPath;
    if (!f) return;
    untrack(() => load(r, f));
  });

  async function load(r: string, f: Frame) {
    const id = ++req;
    loading = true;
    error = null;
    try {
      const res = await tauri.getBlame(r, f.path, f.rev);
      if (id !== req) return;
      result = res;
      selectedOid = null;
      if (scrollEl) scrollEl.scrollTop = 0;
    } catch (err) {
      if (id !== req) return;
      result = null;
      error = String(err);
    } finally {
      if (id === req) loading = false;
    }
  }

  const groups = $derived.by<Group[]>(() => {
    if (!result) return [];
    const out: Group[] = [];
    for (const h of result.hunks) {
      const last = out[out.length - 1];
      if (last && last.hunk.oid === h.oid && last.start + last.count === h.start_line) {
        last.count += h.line_count;
      } else {
        out.push({ hunk: h, start: h.start_line, count: h.line_count });
      }
    }
    return out;
  });

  const timeRange = $derived.by(() => {
    if (!result || result.hunks.length === 0) return { min: 0, max: 0 };
    let min = Infinity;
    let max = -Infinity;
    for (const h of result.hunks) {
      min = Math.min(min, h.timestamp);
      max = Math.max(max, h.timestamp);
    }
    return { min, max };
  });

  /** Age heat: newer commits get a stronger accent bar. */
  function heat(ts: number): string {
    const { min, max } = timeRange;
    const frac = max > min ? (ts - min) / (max - min) : 1;
    const pct = Math.round(12 + frac * 88);
    return `color-mix(in srgb, var(--color-lane-2) ${pct}%, var(--color-border))`;
  }

  const selectedHunk = $derived(
    selectedOid ? (result?.hunks.find((h) => h.oid === selectedOid) ?? null) : null,
  );
  const selectedLines = $derived(
    selectedOid ? groups.filter((g) => g.hunk.oid === selectedOid).reduce((n, g) => n + g.count, 0) : 0,
  );

  function lines(g: Group): { n: number; text: string }[] {
    const out = [];
    const src = result?.lines ?? [];
    for (let i = 0; i < g.count; i++) {
      const n = g.start + i;
      out.push({ n, text: src[n - 1] ?? "" });
    }
    return out;
  }

  function gutterTitle(h: BlameHunk): string {
    const lines = [
      `${h.short_oid} — ${h.summary}`,
      `${h.author_name} <${h.author_email}>`,
      fullDate(h.timestamp),
    ];
    if (h.orig_path !== (current?.path ?? path)) lines.push(`from ${h.orig_path}`);
    lines.push("Click to select · right-click for actions");
    return lines.join("\n");
  }

  function blamePrevious(h: BlameHunk) {
    if (!h.has_parent) {
      toast("info", "These lines were introduced in the root commit; there is no earlier revision.");
      return;
    }
    stack = [...stack, { path: h.orig_path, rev: `${h.oid}^` }];
  }

  function back() {
    if (stack.length > 1) stack = stack.slice(0, -1);
  }

  function select(h: BlameHunk) {
    selectedOid = selectedOid === h.oid ? null : h.oid;
  }

  function openMenu(ev: MouseEvent, h: BlameHunk) {
    ev.preventDefault();
    selectedOid = h.oid;
    menu = {
      x: ev.clientX,
      y: ev.clientY,
      items: [
        { label: "Show commit in graph", action: () => { showInGraph(h.oid); } },
        { label: "Blame previous revision", action: () => blamePrevious(h), disabled: !h.has_parent },
        { label: "File history", action: () => showFileHistory(current?.path ?? path) },
        { separator: true },
        {
          label: "Copy commit SHA",
          action: async () => {
            if (await copyText(h.oid)) toast("success", `Copied ${h.short_oid}`);
          },
        },
      ],
    };
  }

  function jumpToNext() {
    if (!selectedOid || !scrollEl) return;
    const els = [...scrollEl.querySelectorAll<HTMLElement>(`[data-oid="${selectedOid}"]`)];
    const top = scrollEl.scrollTop + 4;
    const next = els.find((el) => el.offsetTop > top) ?? els[0];
    if (next) scrollEl.scrollTop = next.offsetTop - 8;
  }
</script>

<div class="blame">
  <div class="toolbar">
    <button class="icon-btn" onclick={back} disabled={stack.length <= 1} title="Back to newer revision" aria-label="Back">
      <ArrowLeft size={14} />
    </button>
    <span class="where">
      <span class="mono path">{current?.path ?? path}</span>
      {#if result}
        <span class="at">at</span>
        <span class="oid" title={result.rev_oid}>{current?.rev ? result.rev_short : `HEAD (${result.rev_short})`}</span>
        {#if current?.rev?.endsWith("^")}
          <span class="muted">parent of {current.rev.slice(0, 7)}</span>
        {/if}
      {/if}
    </span>
    <span class="legend" aria-hidden="true">
      older <span class="legend-bar"></span> newer
    </span>
  </div>

  {#if selectedHunk}
    <div class="selected-bar">
      <div class="sel-meta">
        <div class="sel-summary">{selectedHunk.summary || "(no message)"}</div>
        <div class="sel-sub">
          <span class="oid">{selectedHunk.short_oid}</span>
          · {selectedHunk.author_name}
          · <span title={fullDate(selectedHunk.timestamp)}>{relativeTime(selectedHunk.timestamp)}</span>
          · {selectedLines} line{selectedLines === 1 ? "" : "s"}
        </div>
      </div>
      <button class="btn" onclick={jumpToNext} title="Jump to the next block from this commit">
        <ChevronsDown size={13} /> Next
      </button>
      <button class="btn" onclick={() => showInGraph(selectedHunk.oid)}>
        <GitCommitHorizontal size={13} /> Show in graph
      </button>
      <button class="btn" onclick={() => blamePrevious(selectedHunk)} disabled={!selectedHunk.has_parent}
        title="Re-blame the file at this commit's parent">
        <History size={13} /> Blame previous
      </button>
    </div>
  {/if}

  <div class="code" bind:this={scrollEl} style="tab-size: {tabSize}">
    {#if loading && !result}
      <div class="empty"><Loader2 size={14} class="spinner" /> Computing blame…</div>
    {:else if error}
      <div class="empty error">{error}</div>
    {:else if result && result.lines.length === 0}
      <div class="empty">Empty file</div>
    {:else if result}
      {#if loading}
        <div class="reloading"><Loader2 size={12} class="spinner" /></div>
      {/if}
      {#each groups as g (g.start)}
        <div
          class="group"
          class:selected={selectedOid === g.hunk.oid}
          data-oid={g.hunk.oid}
        >
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div
            class="gutter"
            role="button"
            tabindex="0"
            style="--heat: {heat(g.hunk.timestamp)}"
            onclick={() => select(g.hunk)}
            onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), select(g.hunk))}
            oncontextmenu={(e) => openMenu(e, g.hunk)}
            title={gutterTitle(g.hunk)}
          >
            <span class="g-oid">{g.hunk.short_oid}</span>
            <span class="g-author">{g.hunk.author_name}</span>
            <span class="g-time">{relativeTime(g.hunk.timestamp)}</span>
          </div>
          <div class="lines">
            {#each lines(g) as l (l.n)}
              <div class="line"><span class="ln">{l.n}</span><span class="text">{l.text}</span></div>
            {/each}
          </div>
        </div>
      {/each}
    {/if}
  </div>
</div>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<style>
  .blame {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-surface);
    font-size: 12px;
  }

  .where {
    flex: 1;
    min-width: 0;
    display: flex;
    gap: 6px;
    align-items: baseline;
    overflow: hidden;
    white-space: nowrap;
  }

  .path {
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .at,
  .muted {
    color: var(--color-text-muted);
  }

  .mono {
    font-family: var(--font-mono);
  }

  .oid {
    font-family: var(--font-mono);
    color: var(--color-accent);
  }

  .legend {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .legend-bar {
    width: 60px;
    height: 6px;
    border-radius: 3px;
    background: linear-gradient(
      to right,
      color-mix(in srgb, var(--color-lane-2) 12%, var(--color-border)),
      var(--color-lane-2)
    );
  }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 0;
  }

  .icon-btn:hover:not(:disabled) {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .icon-btn:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .selected-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--color-border);
    background: color-mix(in srgb, var(--color-accent) 8%, var(--color-surface));
  }

  .sel-meta {
    flex: 1;
    min-width: 0;
  }

  .sel-summary {
    font-size: 13px;
    font-weight: 600;
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sel-sub {
    font-size: 11px;
    color: var(--color-text-muted);
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
    flex-shrink: 0;
  }

  .btn:hover:not(:disabled) {
    border-color: var(--color-accent);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .code {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: auto;
    background: var(--color-bg);
    font-family: var(--font-mono);
    font-size: var(--diff-font-size, 13px);
  }

  .group {
    display: flex;
    border-bottom: 1px solid color-mix(in srgb, var(--color-border) 60%, transparent);
    content-visibility: auto;
    contain-intrinsic-size: auto 40px;
    min-width: max-content;
  }

  .group.selected {
    background: color-mix(in srgb, var(--color-accent) 10%, transparent);
  }

  .gutter {
    position: sticky;
    left: 0;
    z-index: 1;
    width: 250px;
    flex-shrink: 0;
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 1px 8px 1px 10px;
    font-family: var(--font-sans);
    font-size: 11px;
    line-height: 20px;
    color: var(--color-text-muted);
    background: var(--color-surface);
    border-left: 3px solid var(--heat);
    border-right: 1px solid var(--color-border);
    cursor: pointer;
    white-space: nowrap;
    outline: none;
  }

  .gutter:hover,
  .gutter:focus-visible,
  .group.selected .gutter {
    background: var(--color-surface-elevated);
  }

  .g-oid {
    font-family: var(--font-mono);
    color: var(--color-accent);
    flex-shrink: 0;
  }

  .g-author {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--color-text-primary);
  }

  .g-time {
    flex-shrink: 0;
  }

  .lines {
    flex: 1;
  }

  .line {
    display: flex;
    line-height: 20px;
    white-space: pre;
  }

  .ln {
    width: 52px;
    flex-shrink: 0;
    padding-right: 10px;
    text-align: right;
    color: var(--color-text-muted);
    user-select: none;
  }

  .text {
    color: var(--color-text-primary);
    padding-right: 16px;
  }

  .empty {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 14px;
    font-family: var(--font-sans);
    font-size: 12px;
    color: var(--color-text-muted);
  }

  .empty.error {
    color: var(--color-diff-del-text);
    white-space: pre-wrap;
  }

  .reloading {
    position: sticky;
    top: 0;
    height: 0;
    display: flex;
    justify-content: flex-end;
    padding-right: 10px;
    z-index: 2;
    color: var(--color-text-muted);
  }

  .blame :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
