<!-- Commits ahead/behind between the current branch and another branch. -->
<script lang="ts">
  import Modal from "../shared/Modal.svelte";
  import { t, splitMessage } from "../../lib/i18n";
  import * as tauri from "../../lib/tauri";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { selectedCommitOid } from "../../lib/stores/graph";
  import type { BranchComparison, ComparedCommit } from "../../lib/types/git";

  interface Props {
    /** `{ base, other }` ref names; null = closed. */
    pair: { base: string; other: string } | null;
    onclose: () => void;
  }

  let { pair, onclose }: Props = $props();

  let result = $state<BranchComparison | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);

  $effect(() => {
    const p = pair;
    const path = $activeRepoPath;
    result = null;
    error = null;
    if (!p || !path) return;
    let cancelled = false;
    loading = true;
    tauri
      .compareBranches(path, p.base, p.other)
      .then((r) => {
        if (!cancelled) result = r;
      })
      .catch((err) => {
        if (!cancelled) error = String(err);
      })
      .finally(() => {
        if (!cancelled) loading = false;
      });
    return () => {
      cancelled = true;
    };
  });

  function show(c: ComparedCommit) {
    selectedCommitOid.set(c.oid);
    onclose();
  }


  function when(ts: number) {
    return new Date(ts * 1000).toLocaleDateString();
  }
</script>

{#snippet commitList(title: string, list: ComparedCommit[], total: number)}
  <section class="side">
    <h3>{title} <span class="count">{total}</span></h3>
    {#if total === 0}
      <p class="none">{$t("branches.noCommits")}</p>
    {:else}
      <ul>
        {#each list as c (c.oid)}
          <li>
            <button class="commit" onclick={() => show(c)} title={$t("branches.showInGraph", { oid: c.short_oid })}>
              <span class="sha">{c.short_oid}</span>
              <span class="summary">{c.summary}</span>
              <span class="meta">{c.author_name} · {when(c.timestamp)}</span>
            </button>
          </li>
        {/each}
      </ul>
      {#if total > list.length}
        <p class="none">{$t("branches.andMore", { count: total - list.length })}</p>
      {/if}
    {/if}
  </section>
{/snippet}

<Modal open={!!pair} title={$t("branches.compareTitle")} {onclose} width="640px">
  {#if pair}
    <div class="compare">
      <p class="desc">{#each splitMessage($t("branches.comparedWith")) as part, i}{#if i % 2}<strong>{part === "other" ? pair.other : pair.base}</strong>{:else}{part}{/if}{/each}</p>
      {#if loading}
        <p class="none">{$t("branches.comparing")}</p>
      {:else if error}
        <p class="err">{error}</p>
      {:else if result}
        {#if result.ahead_count === 0 && result.behind_count === 0}
          <p class="none">{$t("branches.sameHistory")}</p>
        {:else}
          {@render commitList($t("branches.onlyOn", { name: pair.other }), result.ahead, result.ahead_count)}
          {@render commitList($t("branches.onlyOn", { name: pair.base }), result.behind, result.behind_count)}
        {/if}
      {/if}
    </div>
  {/if}
</Modal>

<style>
  .compare {
    display: flex;
    flex-direction: column;
    gap: 12px;
    font-size: 12px;
    max-height: 60vh;
    overflow-y: auto;
  }

  .desc {
    margin: 0;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .desc strong {
    color: var(--color-accent);
  }

  h3 {
    margin: 0 0 4px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--color-text-muted);
  }

  .count {
    margin-left: 4px;
    opacity: 0.7;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--color-border);
    border-radius: 4px;
  }

  li + li {
    border-top: 1px solid var(--color-border);
  }

  .commit {
    display: flex;
    align-items: baseline;
    gap: 8px;
    width: 100%;
    padding: 5px 8px;
    border: none;
    background: none;
    color: var(--color-text-primary);
    text-align: left;
    cursor: pointer;
    font-size: 12px;
  }

  .commit:hover,
  .commit:focus-visible {
    background: var(--color-surface-elevated);
    outline: none;
  }

  .sha {
    font-family: var(--font-mono);
    color: var(--color-accent);
    flex-shrink: 0;
  }

  .summary {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    flex-shrink: 0;
    color: var(--color-text-muted);
    font-size: 11px;
  }

  .none {
    margin: 0;
    color: var(--color-text-muted);
  }

  .err {
    margin: 0;
    color: var(--color-diff-del-text);
  }
</style>
