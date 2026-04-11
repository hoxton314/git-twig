<script lang="ts">
  import type { DiffHunk as DiffHunkType } from "../../lib/types/git";

  interface Props {
    hunk: DiffHunkType;
    mode: "unified" | "split";
  }

  let { hunk, mode }: Props = $props();

  // For split view, pair up old/new lines side by side
  interface SplitPair {
    oldLineno: number | null;
    oldContent: string;
    oldOrigin: string;
    newLineno: number | null;
    newContent: string;
    newOrigin: string;
  }

  const splitPairs = $derived(buildSplitPairs());

  function buildSplitPairs(): SplitPair[] {
    const pairs: SplitPair[] = [];
    const dels: typeof hunk.lines = [];
    const adds: typeof hunk.lines = [];

    function flushQueues() {
      const max = Math.max(dels.length, adds.length);
      for (let i = 0; i < max; i++) {
        const d = dels[i];
        const a = adds[i];
        pairs.push({
          oldLineno: d?.old_lineno ?? null,
          oldContent: d?.content ?? "",
          oldOrigin: d?.origin ?? " ",
          newLineno: a?.new_lineno ?? null,
          newContent: a?.content ?? "",
          newOrigin: a?.origin ?? " ",
        });
      }
      dels.length = 0;
      adds.length = 0;
    }

    for (const line of hunk.lines) {
      if (line.origin === "-") {
        dels.push(line);
      } else if (line.origin === "+") {
        adds.push(line);
      } else {
        flushQueues();
        pairs.push({
          oldLineno: line.old_lineno,
          oldContent: line.content,
          oldOrigin: " ",
          newLineno: line.new_lineno,
          newContent: line.content,
          newOrigin: " ",
        });
      }
    }
    flushQueues();
    return pairs;
  }
</script>

<div class="hunk">
  <div class="hunk-header">{hunk.header.trim()}</div>

  {#if mode === "unified"}
    <table class="unified-table">
      <tbody>
        {#each hunk.lines as line, i (i)}
          <tr class="line" class:line-add={line.origin === "+"} class:line-del={line.origin === "-"}>
            <td class="lineno old-lineno">{line.old_lineno ?? ""}</td>
            <td class="lineno new-lineno">{line.new_lineno ?? ""}</td>
            <td class="origin">{line.origin}</td>
            <td class="content">{line.content}</td>
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
              class:empty-cell={!pair.oldContent && pair.oldOrigin === " " && pair.newOrigin !== " "}
            >{pair.oldContent}</td>
            <td class="split-divider"></td>
            <td class="lineno">{pair.newLineno ?? ""}</td>
            <td
              class="content split-cell"
              class:line-add={pair.newOrigin === "+"}
              class:empty-cell={!pair.newContent && pair.newOrigin === " " && pair.oldOrigin !== " "}
            >{pair.newContent}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</div>

<style>
  .hunk {
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.5;
  }

  .hunk-header {
    padding: 4px 12px;
    background: var(--color-diff-hunk-bg);
    color: var(--color-text-muted);
    font-size: 11px;
    border-bottom: 1px solid var(--color-border);
    user-select: none;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
  }

  .unified-table .lineno {
    width: 48px;
  }

  .unified-table .origin {
    width: 16px;
  }

  .split-table .lineno {
    width: 40px;
  }

  .split-table .split-cell {
    width: calc(50% - 41px);
  }

  .split-divider {
    width: 2px;
    background: var(--color-border);
  }

  tr.line {
    border: none;
  }

  td {
    padding: 0 4px;
    white-space: pre;
    overflow: hidden;
    vertical-align: top;
  }

  .lineno {
    color: var(--color-text-muted);
    text-align: right;
    user-select: none;
    opacity: 0.6;
    padding-right: 8px;
    flex-shrink: 0;
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
    background: rgba(86, 95, 137, 0.05);
  }
</style>
