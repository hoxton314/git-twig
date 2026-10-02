<script lang="ts">
  import { clampColWidth, type GraphColumn } from "./graphLayout";

  interface Props {
    /** Width of the lanes column, so labels line up with the rows. */
    graphWidth: number;
    showAuthor: boolean;
    showSha: boolean;
    showDate: boolean;
    widths: Record<GraphColumn, number>;
    /** Live width while dragging a separator. */
    onresize: (col: GraphColumn, width: number) => void;
    /** Final width when the drag ends (persist here). */
    onresizeend: (col: GraphColumn, width: number) => void;
    /** Double-click on a separator resets that column. */
    onreset: (col: GraphColumn) => void;
    /** Width of the row list's vertical scrollbar, so right columns align. */
    rightInset?: number;
  }

  let {
    graphWidth,
    showAuthor,
    showSha,
    showDate,
    widths,
    onresize,
    onresizeend,
    onreset,
    rightInset = 0,
  }: Props = $props();

  let dragging = $state<GraphColumn | null>(null);
  let startX = 0;
  let startWidth = 0;
  let lastWidth = 0;

  function onPointerDown(col: GraphColumn, e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    dragging = col;
    startX = e.clientX;
    startWidth = widths[col];
    lastWidth = startWidth;
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragging) return;
    // Separators sit on a column's left edge: dragging left widens it.
    lastWidth = clampColWidth(startWidth - (e.clientX - startX));
    onresize(dragging, lastWidth);
  }

  function onPointerUp() {
    if (!dragging) return;
    const col = dragging;
    dragging = null;
    if (lastWidth !== startWidth) onresizeend(col, lastWidth);
  }

  function onKeydown(col: GraphColumn, e: KeyboardEvent) {
    const step = e.shiftKey ? 20 : 5;
    let w: number | null = null;
    if (e.key === "ArrowLeft") w = widths[col] + step;
    else if (e.key === "ArrowRight") w = widths[col] - step;
    if (w === null) return;
    e.preventDefault();
    onresizeend(col, clampColWidth(w));
  }

  const LABELS: Record<GraphColumn, string> = { author: "Author", sha: "SHA", date: "Date" };
  const columns = $derived(
    (["author", "sha", "date"] as GraphColumn[]).filter((c) =>
      c === "author" ? showAuthor : c === "sha" ? showSha : showDate,
    ),
  );
</script>

<div
  class="column-header"
  class:dragging={dragging !== null}
  style="padding-right: {12 + rightInset}px"
>
  <span class="graph-col" style="width: {graphWidth}px">Graph</span>
  <span class="desc-col">Description</span>
  {#each columns as col (col)}
    <span class="fixed-col col-{col}" style="width: var(--graph-col-{col})">
      <!-- A focusable separator is a widget (arrow keys resize it). -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
      <span
        class="resizer"
        class:active={dragging === col}
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize {LABELS[col]} column"
        aria-valuenow={widths[col]}
        tabindex="0"
        title="Drag to resize, double-click to reset"
        onpointerdown={(e) => onPointerDown(col, e)}
        onpointermove={onPointerMove}
        onpointerup={onPointerUp}
        onpointercancel={onPointerUp}
        ondblclick={() => onreset(col)}
        onkeydown={(e) => onKeydown(col, e)}
      ></span>
      {LABELS[col]}
    </span>
  {/each}
</div>

<style>
  .column-header {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 22px;
    padding-right: 12px;
    flex-shrink: 0;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-surface);
    color: var(--color-text-muted);
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    user-select: none;
  }

  .column-header.dragging {
    cursor: col-resize;
  }

  .graph-col {
    flex-shrink: 0;
    padding-left: 12px;
    /* The row's own 4px left padding follows the canvas; the gap covers it. */
    margin-right: -4px;
    overflow: hidden;
    white-space: nowrap;
  }

  .desc-col {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
  }

  .fixed-col {
    position: relative;
    flex-shrink: 0;
    overflow: visible;
    white-space: nowrap;
  }

  .col-date {
    text-align: right;
  }

  .resizer {
    position: absolute;
    top: -4px;
    bottom: -4px;
    left: -7px;
    width: 6px;
    cursor: col-resize;
    border-left: 1px solid var(--color-border);
    margin-left: 2px;
  }

  .resizer:hover,
  .resizer.active {
    border-left: 2px solid var(--color-accent);
  }
</style>
