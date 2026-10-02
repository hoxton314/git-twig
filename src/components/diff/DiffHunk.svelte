<script lang="ts">
  import type { DiffHunk as DiffHunkType } from "../../lib/types/git";

  interface Props {
    hunk: DiffHunkType;
    mode: "unified" | "split";
    tabSize?: number;
    wrap?: boolean;
  }

  let { hunk, mode, tabSize = 4, wrap = false }: Props = $props();

  // libgit2 emits "\ No newline at end of file" as pseudo-lines with these
  // origins. They always refer to the line immediately before them.
  const EOFNL_ORIGINS = new Set(["=", ">", "<"]);

  interface Row {
    origin: string;
    oldLineno: number | null;
    newLineno: number | null;
    content: string;
    noEol: boolean;
  }

  // For split view, pair up old/new lines side by side
  interface SplitPair {
    oldLineno: number | null;
    oldContent: string;
    oldOrigin: string;
    oldEmpty: boolean;
    oldNoEol: boolean;
    newLineno: number | null;
    newContent: string;
    newOrigin: string;
    newEmpty: boolean;
    newNoEol: boolean;
  }

  // Strip the trailing line terminator git2 includes in each line's content
  // (LF or CRLF) so it doesn't render as stray whitespace / extra wrap lines.
  function stripEol(s: string): string {
    return s.replace(/\r?\n$/, "");
  }

  const rows = $derived(buildRows());
  const splitPairs = $derived(mode === "split" ? buildSplitPairs(rows) : []);

  function buildRows(): Row[] {
    const out: Row[] = [];
    for (const line of hunk.lines) {
      if (EOFNL_ORIGINS.has(line.origin)) {
        if (out.length > 0) out[out.length - 1].noEol = true;
        continue;
      }
      out.push({
        origin: line.origin,
        oldLineno: line.old_lineno,
        newLineno: line.new_lineno,
        content: stripEol(line.content),
        noEol: false,
      });
    }
    return out;
  }

  function buildSplitPairs(src: Row[]): SplitPair[] {
    const pairs: SplitPair[] = [];
    const dels: Row[] = [];
    const adds: Row[] = [];

    function flushQueues() {
      const max = Math.max(dels.length, adds.length);
      for (let i = 0; i < max; i++) {
        const d = dels[i];
        const a = adds[i];
        pairs.push({
          oldLineno: d?.oldLineno ?? null,
          oldContent: d?.content ?? "",
          oldOrigin: d?.origin ?? " ",
          oldEmpty: !d,
          oldNoEol: d?.noEol ?? false,
          newLineno: a?.newLineno ?? null,
          newContent: a?.content ?? "",
          newOrigin: a?.origin ?? " ",
          newEmpty: !a,
          newNoEol: a?.noEol ?? false,
        });
      }
      dels.length = 0;
      adds.length = 0;
    }

    for (const line of src) {
      if (line.origin === "-") {
        dels.push(line);
      } else if (line.origin === "+") {
        adds.push(line);
      } else {
        flushQueues();
        pairs.push({
          oldLineno: line.oldLineno,
          oldContent: line.content,
          oldOrigin: " ",
          oldEmpty: false,
          oldNoEol: line.noEol,
          newLineno: line.newLineno,
          newContent: line.content,
          newOrigin: " ",
          newEmpty: false,
          newNoEol: line.noEol,
        });
      }
    }
    flushQueues();
    return pairs;
  }
</script>

{#snippet eol()}<span class="no-eol" title="No newline at end of file">&#8856;</span>{/snippet}

<div class="hunk" class:wrap style:tab-size={tabSize}>
  <div class="hunk-header">{hunk.header.trim()}</div>

  <div class="hunk-scroll">
    {#if mode === "unified"}
      <table class="unified-table">
        <tbody>
          {#each rows as line, i (i)}
            <tr class="line" class:line-add={line.origin === "+"} class:line-del={line.origin === "-"}>
              <td class="lineno old-lineno">{line.oldLineno ?? ""}</td>
              <td class="lineno new-lineno">{line.newLineno ?? ""}</td>
              <td class="origin">{line.origin}</td>
              <td class="content">{line.content}{#if line.noEol}{@render eol()}{/if}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <table class="split-table">
        <tbody>
          {#each splitPairs as pair, i (i)}
            <tr class="line">
              <td class="lineno">{pair.oldLineno ?? ""}</td>
              <td
                class="content split-cell"
                class:line-del={pair.oldOrigin === "-"}
                class:empty-cell={pair.oldEmpty}
              >{pair.oldContent}{#if pair.oldNoEol}{@render eol()}{/if}</td>
              <td class="split-divider"></td>
              <td class="lineno">{pair.newLineno ?? ""}</td>
              <td
                class="content split-cell"
                class:line-add={pair.newOrigin === "+"}
                class:empty-cell={pair.newEmpty}
              >{pair.newContent}{#if pair.newNoEol}{@render eol()}{/if}</td>
            </tr>
          {/each}
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
    padding: 4px 12px;
    background: var(--color-diff-hunk-bg);
    color: var(--color-text-muted);
    font-size: 11px;
    border-bottom: 1px solid var(--color-border);
    user-select: none;
    white-space: pre;
    overflow: hidden;
    text-overflow: ellipsis;
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
    opacity: 0.6;
    padding-right: 8px;
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
</style>
