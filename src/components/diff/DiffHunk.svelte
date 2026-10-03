<script lang="ts" module>
  /** Hunks with more display rows than this render windowed. */
  export const WINDOWED_HUNK_MIN_ROWS = 300;
</script>

<script lang="ts">
  import type {
    DiffHunk as DiffHunkType,
    DiffLine,
    HunkAction,
  } from "../../lib/types/git";
  import { highlightLines, isLanguageReady, type SyntaxToken } from "../../lib/diff/highlight";
  import { wordDiff, findMatches, buildSegments, type Range, type Segment } from "../../lib/diff/inline";
  import { getContext, tick, untrack } from "svelte";
  import { readable } from "svelte/store";
  import { t } from "../../lib/i18n";
  import { DIFF_SEARCH_CONTEXT, type DiffSearchRegistry } from "../../lib/diff/searchRegistry";

  interface Props {
    hunk: DiffHunkType;
    mode: "unified" | "split";
    tabSize?: number;
    wrap?: boolean;
    /** Loaded highlight.js language, or null for plain text. */
    language?: string | null;
    /** Render the "@@ ... @@" header row (false for expanded context blocks). */
    showHeader?: boolean;
    /** Which working-tree diff this is; enables hunk/line actions. */
    actions?: "staged" | "unstaged" | null;
    busy?: boolean;
    /** Lower-cased search query. */
    search?: string;
    /** Unique prefix for search-match ids. */
    matchPrefix?: string;
    /** `lines` is null for "whole hunk". */
    onaction?: (action: HunkAction, lines: DiffLine[] | null) => void;
  }

  let {
    hunk,
    mode,
    tabSize = 4,
    wrap = false,
    language = null,
    showHeader = true,
    actions = null,
    busy = false,
    search = "",
    matchPrefix = "",
    onaction,
  }: Props = $props();

  // libgit2 emits "\ No newline at end of file" as pseudo-lines with these
  // origins. They always refer to the line immediately before them.
  const EOFNL_ORIGINS = new Set(["=", ">", "<"]);

  interface Row {
    origin: string;
    oldLineno: number | null;
    newLineno: number | null;
    content: string;
    noEol: boolean;
    /** Index into hunk.lines. */
    src: number;
  }

  // Split view: indices into `rows` for each side (null = filler cell).
  interface SplitPair {
    oldRow: number | null;
    newRow: number | null;
  }

  // Strip the trailing line terminator git2 includes in each line's content
  // (LF or CRLF) so it doesn't render as stray whitespace / extra wrap lines.
  function stripEol(s: string): string {
    return s.replace(/\r?\n$/, "");
  }

  const rows = $derived(buildRows(hunk));
  const splitPairs = $derived(mode === "split" ? buildSplitPairs(rows) : []);
  const isChange = (r: Row) => r.origin === "+" || r.origin === "-";
  const hasChanges = $derived(rows.some(isChange));

  function buildRows(h: DiffHunkType): Row[] {
    const out: Row[] = [];
    h.lines.forEach((line, i) => {
      if (EOFNL_ORIGINS.has(line.origin)) {
        if (out.length > 0) out[out.length - 1].noEol = true;
        return;
      }
      out.push({
        origin: line.origin,
        oldLineno: line.old_lineno,
        newLineno: line.new_lineno,
        content: stripEol(line.content),
        noEol: false,
        src: i,
      });
    });
    return out;
  }

  /** Change blocks: runs of '-' rows followed by '+' rows. */
  function changeBlocks(src: Row[]): { dels: number[]; adds: number[] }[] {
    const blocks: { dels: number[]; adds: number[] }[] = [];
    let cur: { dels: number[]; adds: number[] } | null = null;
    src.forEach((r, i) => {
      if (r.origin === "-") {
        if (!cur || cur.adds.length > 0) {
          cur = { dels: [], adds: [] };
          blocks.push(cur);
        }
        cur.dels.push(i);
      } else if (r.origin === "+") {
        if (!cur) {
          cur = { dels: [], adds: [] };
          blocks.push(cur);
        }
        cur.adds.push(i);
      } else {
        cur = null;
      }
    });
    return blocks;
  }

  function buildSplitPairs(src: Row[]): SplitPair[] {
    const pairs: SplitPair[] = [];
    const dels: number[] = [];
    const adds: number[] = [];
    const flush = () => {
      const max = Math.max(dels.length, adds.length);
      for (let i = 0; i < max; i++) {
        pairs.push({ oldRow: dels[i] ?? null, newRow: adds[i] ?? null });
      }
      dels.length = 0;
      adds.length = 0;
    };
    src.forEach((r, i) => {
      if (r.origin === "-") dels.push(i);
      else if (r.origin === "+") adds.push(i);
      else {
        flush();
        pairs.push({ oldRow: i, newRow: i });
      }
    });
    flush();
    return pairs;
  }

  // ── Syntax highlighting ────────────────────────────────────────────
  // Each side is highlighted as one block so multi-line constructs work.
  // Skipped for huge hunks: highlighting is the costly part and the window
  // only shows a few hundred rows anyway.
  const SYNTAX_MAX_ROWS = 5000;
  const syntax = $derived.by((): (SyntaxToken[] | null)[] => {
    const out: (SyntaxToken[] | null)[] = rows.map(() => null);
    if (!language || !isLanguageReady(language) || rows.length > SYNTAX_MAX_ROWS) return out;
    const oldIdx: number[] = [];
    const newIdx: number[] = [];
    rows.forEach((r, i) => {
      if (r.origin !== "+") oldIdx.push(i);
      if (r.origin !== "-") newIdx.push(i);
    });
    const oldTok = highlightLines(oldIdx.map((i) => rows[i].content), language);
    const newTok = highlightLines(newIdx.map((i) => rows[i].content), language);
    if (oldTok) oldIdx.forEach((ri, k) => (out[ri] = oldTok[k]));
    // Context rows take the new side's tokens.
    if (newTok) newIdx.forEach((ri, k) => (out[ri] = newTok[k]));
    return out;
  });

  // ── Word diff ──────────────────────────────────────────────────────
  const changedRanges = $derived.by((): (Range[] | null)[] => {
    const out: (Range[] | null)[] = rows.map(() => null);
    for (const b of changeBlocks(rows)) {
      const n = Math.min(b.dels.length, b.adds.length);
      for (let k = 0; k < n; k++) {
        const d = wordDiff(rows[b.dels[k]].content, rows[b.adds[k]].content);
        if (d) {
          out[b.dels[k]] = d.old;
          out[b.adds[k]] = d.new;
        }
      }
    }
    return out;
  });

  // Segments are built on demand for rendered rows only (a fresh cache per
  // input change; mutating it doesn't need reactivity).
  const segCache = $derived.by(() => {
    void [rows, syntax, changedRanges, search];
    return new Map<number, Segment[]>();
  });
  function seg(i: number): Segment[] {
    let out = segCache.get(i);
    if (!out) {
      const r = rows[i];
      out = buildSegments(r.content, syntax[i], changedRanges[i], findMatches(r.content, search));
      segCache.set(i, out);
    }
    return out;
  }

  // ── Search (counted from data; see searchRegistry.ts) ───────────────
  const registry = getContext<DiffSearchRegistry | undefined>(DIFF_SEARCH_CONTEXT);
  const activeMatch = registry ? registry.active : readable<string | null>(null);
  const matchIds = $derived.by((): string[] => {
    if (!search) return [];
    const ids: string[] = [];
    const add = (ri: number, side: string) => {
      const n = findMatches(rows[ri].content, search).length;
      for (let m = 0; m < n; m++) ids.push(`${matchPrefix}:${ri}:${side}:${m}`);
    };
    if (mode === "unified") rows.forEach((_, i) => add(i, "u"));
    else
      for (const p of splitPairs) {
        if (p.oldRow !== null) add(p.oldRow, "o");
        if (p.newRow !== null) add(p.newRow, "n");
      }
    return ids;
  });

  let rootEl = $state<HTMLElement | null>(null);
  $effect(() => {
    if (!registry || !rootEl) return;
    const el = rootEl;
    const key = matchPrefix;
    registry.register(key, el, matchIds);
    return () => registry.unregister(key, el);
  });

  // ── Windowed rendering ─────────────────────────────────────────────
  // Large hunks render only the rows near the viewport of the nearest
  // scrolling ancestor; spacer rows keep the scroll height right.
  const WINDOW_MIN_ROWS = WINDOWED_HUNK_MIN_ROWS;
  const OVERSCAN = 60;
  const displayCount = $derived(mode === "unified" ? rows.length : splitPairs.length);
  const windowed = $derived(displayCount > WINDOW_MIN_ROWS);
  let win = $state({ start: 0, end: 2 * OVERSCAN });
  /** Measured (average) row height; 0 until measured. */
  let rowPx = $state(0);
  let tbodyEl = $state<HTMLElement | null>(null);
  let topSpacer = $state<HTMLElement | null>(null);
  let bottomSpacer = $state<HTMLElement | null>(null);
  let scrollEl: HTMLElement | null = null;
  const range = $derived(windowed ? { start: Math.min(win.start, displayCount), end: Math.min(win.end, displayCount) } : { start: 0, end: displayCount });

  function scrollParent(el: HTMLElement): HTMLElement | null {
    for (let p = el.parentElement; p; p = p.parentElement) {
      const oy = getComputedStyle(p).overflowY;
      if (oy === "auto" || oy === "scroll") return p;
    }
    return null;
  }

  function rowHeight(): number {
    if (rowPx > 0) return rowPx;
    const fs = rootEl ? parseFloat(getComputedStyle(rootEl).fontSize) : 13;
    return (fs || 13) * 1.5;
  }

  function updateWindow() {
    if (!windowed || !tbodyEl || !scrollEl) return;
    const rh = rowHeight();
    const sr = scrollEl.getBoundingClientRect();
    const tr = tbodyEl.getBoundingClientRect();
    const above = sr.top - tr.top; // px of this table above the viewport top
    let start = Math.max(0, Math.floor(above / rh) - OVERSCAN);
    let end = Math.ceil((above + sr.height) / rh) + OVERSCAN;
    start = Math.min(start, displayCount);
    end = Math.max(Math.min(end, displayCount), Math.min(start + 1, displayCount));
    if (start !== win.start || end !== win.end) win = { start, end };
  }

  $effect(() => {
    void [displayCount, tbodyEl, topSpacer, bottomSpacer];
    if (!windowed || !rootEl) return;
    scrollEl = scrollParent(rootEl);
    if (!scrollEl) {
      win = { start: 0, end: displayCount };
      return;
    }
    const el = scrollEl;
    let raf = 0;
    const schedule = () => {
      if (!raf) raf = requestAnimationFrame(() => { raf = 0; updateWindow(); });
    };
    el.addEventListener("scroll", schedule, { passive: true });
    const ro = new ResizeObserver(schedule);
    ro.observe(el);
    // The hunk can move without the container scrolling (a file above
    // collapses, context above expands): a spacer or the table entering or
    // leaving the viewport signals that the window is stale.
    const io = new IntersectionObserver(schedule, { root: el, rootMargin: "200px 0px" });
    for (const target of [tbodyEl, topSpacer, bottomSpacer]) if (target) io.observe(target);
    schedule();
    return () => {
      el.removeEventListener("scroll", schedule);
      ro.disconnect();
      io.disconnect();
      if (raf) cancelAnimationFrame(raf);
    };
  });

  // A different layout invalidates the measured height.
  $effect(() => {
    void [wrap, mode];
    untrack(() => (rowPx = 0));
  });

  // Measure rendered rows (average, so wrapped lines are accounted for).
  // The top spacer is `start × rowPx`, so a new estimate is compensated in
  // scrollTop: the content under the viewport stays where it is (WebKit has
  // no scroll anchoring to do this for us).
  $effect(() => {
    void range;
    void [wrap, mode];
    if (!windowed || !tbodyEl) return;
    const body = tbodyEl;
    tick().then(() => {
      const rendered = body.querySelectorAll<HTMLElement>("tr.line");
      if (rendered.length === 0) return;
      let total = 0;
      rendered.forEach((r) => (total += r.offsetHeight));
      const avg = total / rendered.length;
      const before = rowHeight();
      if (avg > 0 && Math.abs(avg - before) > 0.5) {
        const start = untrack(() => range.start);
        rowPx = avg;
        if (scrollEl && start > 0) scrollEl.scrollTop += start * (avg - before);
      }
    });
  });

  // The active search match lives in this hunk: bring its row into the
  // window, then into view. Runs only when the active match changes.
  $effect(() => {
    const id = $activeMatch;
    if (!id || !id.startsWith(matchPrefix + ":")) return;
    const ri = Number(id.slice(matchPrefix.length + 1).split(":")[0]);
    untrack(() => {
      const di = mode === "unified" ? ri : splitPairs.findIndex((p) => p.oldRow === ri || p.newRow === ri);
      if (windowed && scrollEl && tbodyEl && (di < range.start || di >= range.end)) {
        const offset = tbodyEl.getBoundingClientRect().top - scrollEl.getBoundingClientRect().top;
        scrollEl.scrollTop = Math.max(0, scrollEl.scrollTop + offset + di * rowHeight() - scrollEl.clientHeight / 2);
        updateWindow();
      }
    });
    tick().then(() =>
      rootEl?.querySelector(`[data-m="${CSS.escape(id)}"]`)?.scrollIntoView({ block: "nearest", inline: "nearest" }),
    );
  });

  // ── Line selection ─────────────────────────────────────────────────
  let selected = $state<Set<number>>(new Set());
  let lastClicked: number | null = null;

  // A refreshed hunk invalidates the selection.
  $effect(() => {
    void hunk;
    selected = new Set();
    lastClicked = null;
  });

  function toggleRow(i: number, e: MouseEvent) {
    const next = new Set(selected);
    if (e.shiftKey && lastClicked !== null) {
      const [a, b] = lastClicked < i ? [lastClicked, i] : [i, lastClicked];
      for (let k = a; k <= b; k++) if (isChange(rows[k])) next.add(k);
    } else if (next.has(i)) {
      next.delete(i);
    } else {
      next.add(i);
    }
    lastClicked = i;
    selected = next;
  }

  function selectAll() {
    const next = new Set<number>();
    rows.forEach((r, i) => {
      if (isChange(r)) next.add(i);
    });
    selected = next;
  }

  function runAction(action: HunkAction, whole: boolean) {
    if (!onaction || busy) return;
    if (whole) {
      onaction(action, null);
    } else {
      const lines = [...selected].sort((a, b) => a - b).map((i) => hunk.lines[rows[i].src]);
      onaction(action, lines);
    }
  }

  function segClass(s: Segment, id: string | undefined): string {
    let c = s.cls;
    if (s.changed) c += " wd-changed";
    if (s.match >= 0) c += " search-hit";
    if (id && id === $activeMatch) c += " search-active";
    return c;
  }

  const selectable = $derived(actions !== null && !!onaction);
  const selCount = $derived(selected.size);
</script>

{#snippet eol()}<span class="no-eol" title={$t("diff.noNewlineAtEof")}>&#8856;</span>{/snippet}

{#snippet text(ri: number, side: string)}{#each seg(ri) as s, si (si)}{#if s.cls || s.changed || s.match >= 0}{@const mid = s.match >= 0 ? `${matchPrefix}:${ri}:${side}:${s.match}` : undefined}<span class={segClass(s, mid)} data-m={mid} data-match-start={s.matchStart ? "" : undefined}>{s.text}</span>{:else}{s.text}{/if}{/each}{#if rows[ri].noEol}{@render eol()}{/if}{/snippet}

{#snippet lineno(ri: number, n: number | null)}{#if selectable && isChange(rows[ri])}<button class="ln-btn" class:on={selected.has(ri)} aria-pressed={selected.has(ri)} title={$t("diff.selectLine")} onclick={(e) => toggleRow(ri, e)}>{n ?? ""}</button>{:else}{n ?? ""}{/if}{/snippet}

<div class="hunk" class:wrap class:syn={!!language} class:hunk-anchor={showHeader} style:tab-size={tabSize} bind:this={rootEl}>
  {#if showHeader}
    <div class="hunk-header">
      <span class="hunk-header-text">{hunk.header.trim()}</span>
      {#if selectable && hasChanges}
        <div class="hunk-actions">
          {#if selCount > 0}
            <span class="sel-count">{$t("diff.lineCount", { count: selCount })}</span>
            {#if actions === "unstaged"}
              <button class="hunk-btn" disabled={busy} onclick={() => runAction("stage", false)}>{$t("diff.stageLines")}</button>
              <button class="hunk-btn danger" disabled={busy} onclick={() => runAction("discard", false)}>{$t("diff.discardLines")}</button>
            {:else}
              <button class="hunk-btn" disabled={busy} onclick={() => runAction("unstage", false)}>{$t("diff.unstageLines")}</button>
            {/if}
            <button class="hunk-btn subtle" onclick={() => (selected = new Set())}>{$t("common.clear")}</button>
          {:else}
            <button class="hunk-btn subtle" onclick={selectAll} title={$t("diff.selectHunkLines")}>{$t("diff.select")}</button>
            {#if actions === "unstaged"}
              <button class="hunk-btn" disabled={busy} onclick={() => runAction("stage", true)}>{$t("diff.stageHunk")}</button>
              <button class="hunk-btn danger" disabled={busy} onclick={() => runAction("discard", true)}>{$t("diff.discardHunk")}</button>
            {:else}
              <button class="hunk-btn" disabled={busy} onclick={() => runAction("unstage", true)}>{$t("diff.unstageHunk")}</button>
            {/if}
          {/if}
        </div>
      {/if}
    </div>
  {/if}

  <div class="hunk-scroll">
    {#if mode === "unified"}
      <table class="unified-table">
        <tbody bind:this={tbodyEl}>
          <tr class="spacer" aria-hidden="true" bind:this={topSpacer}><td colspan="4" style:height="{range.start * rowHeight()}px"></td></tr>
          {#each rows.slice(range.start, range.end) as line, k (range.start + k)}
            {@const i = range.start + k}
            <tr
              class="line"
              class:line-add={line.origin === "+"}
              class:line-del={line.origin === "-"}
              class:selected={selected.has(i)}
            >
              <td class="lineno old-lineno">{@render lineno(i, line.oldLineno)}</td>
              <td class="lineno new-lineno">{@render lineno(i, line.newLineno)}</td>
              <td class="origin">{line.origin}</td>
              <td class="content">{@render text(i, "u")}</td>
            </tr>
          {/each}
          <tr class="spacer" aria-hidden="true" bind:this={bottomSpacer}><td colspan="4" style:height="{(displayCount - range.end) * rowHeight()}px"></td></tr>
        </tbody>
      </table>
    {:else}
      <table class="split-table">
        <tbody bind:this={tbodyEl}>
          <tr class="spacer" aria-hidden="true" bind:this={topSpacer}><td colspan="5" style:height="{range.start * rowHeight()}px"></td></tr>
          {#each splitPairs.slice(range.start, range.end) as pair, k (range.start + k)}
            {@const o = pair.oldRow}
            {@const n = pair.newRow}
            {@const ctx = o !== null && o === n}
            <tr class="line">
              <td class="lineno" class:selected={o !== null && !ctx && selected.has(o)}>{#if o !== null}{#if ctx}{rows[o].oldLineno ?? ""}{:else}{@render lineno(o, rows[o].oldLineno)}{/if}{/if}</td>
              <td
                class="content split-cell"
                class:line-del={o !== null && !ctx}
                class:empty-cell={o === null}
                class:selected={o !== null && !ctx && selected.has(o)}
              >{#if o !== null}{@render text(o, "o")}{/if}</td>
              <td class="split-divider"></td>
              <td class="lineno" class:selected={n !== null && !ctx && selected.has(n)}>{#if n !== null}{#if ctx}{rows[n].newLineno ?? ""}{:else}{@render lineno(n, rows[n].newLineno)}{/if}{/if}</td>
              <td
                class="content split-cell"
                class:line-add={n !== null && !ctx}
                class:empty-cell={n === null}
                class:selected={n !== null && !ctx && selected.has(n)}
              >{#if n !== null}{@render text(n, "n")}{/if}</td>
            </tr>
          {/each}
          <tr class="spacer" aria-hidden="true" bind:this={bottomSpacer}><td colspan="5" style:height="{(displayCount - range.end) * rowHeight()}px"></td></tr>
        </tbody>
      </table>
    {/if}
  </div>
</div>

<style>
  .hunk {
    font-family: var(--font-mono);
    font-size: var(--diff-font-size);
    line-height: 1.5;
  }

  .hunk-header {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 26px;
    padding: 2px 8px 2px 12px;
    background: var(--color-diff-hunk-bg);
    color: var(--color-text-muted);
    font-size: 11px;
    border-bottom: 1px solid var(--color-border);
    user-select: none;
  }

  .hunk-header-text {
    flex: 1;
    min-width: 0;
    white-space: pre;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .hunk-actions {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
    font-family: var(--font-sans, system-ui, sans-serif);
  }

  .sel-count {
    color: var(--color-accent);
    margin-right: 4px;
  }

  .hunk-btn {
    padding: 1px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 11px;
    line-height: 18px;
    cursor: pointer;
  }

  .hunk-btn:hover:not(:disabled) {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }

  .hunk-btn.danger:hover:not(:disabled) {
    border-color: var(--color-diff-del-text);
    color: var(--color-diff-del-text);
  }

  .hunk-btn.subtle {
    background: transparent;
    color: var(--color-text-muted);
  }

  .hunk-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  /* Without word wrap, long lines scroll horizontally instead of being clipped. */
  .hunk-scroll {
    overflow-x: auto;
  }

  table {
    border-collapse: collapse;
    width: max-content;
    min-width: 100%;
  }

  .wrap table {
    width: 100%;
    table-layout: fixed;
  }

  .unified-table .lineno {
    width: 48px;
    min-width: 48px;
  }

  .unified-table .origin {
    width: 16px;
    min-width: 16px;
  }

  .unified-table .content {
    width: 100%;
  }

  .split-table .lineno {
    width: 40px;
    min-width: 40px;
  }

  .split-table .split-cell {
    width: 50%;
  }

  .wrap .split-table .split-cell {
    width: calc(50% - 41px);
  }

  .split-divider {
    width: 2px;
    min-width: 2px;
    padding: 0;
    background: var(--color-border);
  }

  tr.line {
    border: none;
  }

  tr.spacer td {
    padding: 0;
    border: none;
  }

  td {
    padding: 0 4px;
    white-space: pre;
    vertical-align: top;
  }

  .wrap td.content {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .lineno {
    color: var(--color-text-muted);
    text-align: right;
    user-select: none;
    padding-right: 8px;
  }

  .unified-table .lineno,
  .split-table .lineno {
    opacity: 0.6;
  }

  .ln-btn {
    display: block;
    width: 100%;
    min-height: 1.5em;
    padding: 0;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: right;
    cursor: pointer;
  }

  .ln-btn:hover {
    color: var(--color-accent);
  }

  .ln-btn.on {
    color: var(--color-accent);
    font-weight: 700;
  }

  tr.selected .lineno,
  td.lineno.selected {
    opacity: 1;
    box-shadow: inset 3px 0 0 var(--color-accent);
  }

  tr.selected td,
  td.selected {
    background-image: linear-gradient(
      color-mix(in srgb, var(--color-accent) 14%, transparent),
      color-mix(in srgb, var(--color-accent) 14%, transparent)
    );
  }

  .origin {
    color: var(--color-text-muted);
    text-align: center;
    user-select: none;
  }

  .content {
    color: var(--color-text-primary);
  }

  .line-add {
    background: var(--color-diff-add-bg);
  }

  .line-add .content,
  td.line-add {
    color: var(--color-diff-add-text);
  }

  .line-del {
    background: var(--color-diff-del-bg);
  }

  .line-del .content,
  td.line-del {
    color: var(--color-diff-del-text);
  }

  /* With syntax colors, the row tint alone marks additions/deletions. */
  .syn .line-add .content,
  .syn td.line-add,
  .syn .line-del .content,
  .syn td.line-del {
    color: var(--color-text-primary);
  }

  .empty-cell {
    background: var(--color-surface);
  }

  .no-eol {
    margin-left: 4px;
    color: var(--color-diff-del-text);
    opacity: 0.8;
    user-select: none;
    cursor: help;
  }

  /* ── Word diff ── */
  .line-add :global(.wd-changed),
  td.line-add :global(.wd-changed) {
    background: var(--color-diff-add-word-bg);
    border-radius: 2px;
  }

  .line-del :global(.wd-changed),
  td.line-del :global(.wd-changed) {
    background: var(--color-diff-del-word-bg);
    border-radius: 2px;
  }

  /* ── Search ── */
  .hunk :global(.search-hit) {
    background: var(--color-search-hit-bg);
    color: var(--color-search-hit-text);
    border-radius: 2px;
  }

  .hunk :global(.search-hit.search-active) {
    background: var(--color-search-active-bg);
    outline: 1px solid var(--color-search-active-outline);
  }

  /* ── Syntax (colors are theme tokens in app.css) ── */
  .hunk :global(.hljs-keyword),
  .hunk :global(.hljs-built_in),
  .hunk :global(.hljs-selector-tag),
  .hunk :global(.hljs-doctag) {
    color: var(--color-syntax-keyword);
  }

  .hunk :global(.hljs-string),
  .hunk :global(.hljs-regexp),
  .hunk :global(.hljs-template-tag),
  .hunk :global(.hljs-addition) {
    color: var(--color-syntax-string);
  }

  .hunk :global(.hljs-number),
  .hunk :global(.hljs-literal),
  .hunk :global(.hljs-symbol),
  .hunk :global(.hljs-bullet) {
    color: var(--color-syntax-number);
  }

  .hunk :global(.hljs-comment),
  .hunk :global(.hljs-quote) {
    color: var(--color-syntax-comment);
    font-style: italic;
  }

  .hunk :global(.hljs-title),
  .hunk :global(.hljs-section),
  .hunk :global(.hljs-selector-id),
  .hunk :global(.hljs-selector-class) {
    color: var(--color-syntax-function);
  }

  .hunk :global(.hljs-type),
  .hunk :global(.hljs-class),
  .hunk :global(.hljs-title.class_),
  .hunk :global(.hljs-name) {
    color: var(--color-syntax-type);
  }

  .hunk :global(.hljs-attr),
  .hunk :global(.hljs-attribute),
  .hunk :global(.hljs-variable),
  .hunk :global(.hljs-template-variable),
  .hunk :global(.hljs-property),
  .hunk :global(.hljs-params) {
    color: var(--color-syntax-variable);
  }

  .hunk :global(.hljs-meta),
  .hunk :global(.hljs-tag),
  .hunk :global(.hljs-punctuation),
  .hunk :global(.hljs-operator) {
    color: var(--color-syntax-meta);
  }

  .hunk :global(.hljs-emphasis) {
    font-style: italic;
  }

  .hunk :global(.hljs-strong) {
    font-weight: 700;
  }
</style>
