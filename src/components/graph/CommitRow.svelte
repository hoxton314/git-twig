<script lang="ts">
  import type { GraphEntry, RefLabel, GraphDateFormat } from "../../lib/types/git";
  import { ArrowUp } from "lucide-svelte";
  import { formatCommitDate } from "./graphLayout";

  interface Props {
    entry: GraphEntry;
    isSelected: boolean;
    isUnpushed: boolean;
    refs: RefLabel[];
    onSelect: () => void;
    /** Column visibility (widths come from CSS vars set by CommitGraph). */
    showAuthor?: boolean;
    showSha?: boolean;
    showDate?: boolean;
    dateFormat?: GraphDateFormat;
    /** Shared clock (ms) so relative dates refresh. */
    now?: number;
    avatarSize?: number;
    /** Search highlight state. */
    searchMatch?: "none" | "match" | "current";
  }

  let {
    entry,
    isSelected,
    isUnpushed,
    refs,
    onSelect,
    showAuthor = true,
    showSha = true,
    showDate = true,
    dateFormat = "relative",
    now = Date.now(),
    avatarSize = 20,
    searchMatch = "none",
  }: Props = $props();

  const commit = $derived(entry.commit);
  const gravatarUrl = $derived(
    `https://www.gravatar.com/avatar/${commit.author_gravatar}?s=${avatarSize * 2}&d=identicon`
  );

  // Sort refs: local first, then remote, then tags
  const sortedRefs = $derived(
    [...refs].sort((a, b) => {
      const order = { local: 0, tag: 1, remote: 2 };
      return (order[a.ref_type] ?? 3) - (order[b.ref_type] ?? 3);
    })
  );

  const dateLabel = $derived(formatCommitDate(commit.timestamp, dateFormat, now));
</script>

<button
  class="commit-row"
  class:selected={isSelected}
  class:search-match={searchMatch !== "none"}
  class:search-current={searchMatch === "current"}
  aria-pressed={isSelected}
  tabindex="-1"
  onclick={onSelect}
>
  <img
    class="avatar"
    src={gravatarUrl}
    alt=""
    title={`${commit.author_name} <${commit.author_email}>`}
    width={avatarSize}
    height={avatarSize}
    loading="lazy"
  />
  {#if sortedRefs.length > 0}
    <span class="ref-labels">
      {#each sortedRefs as ref (ref.ref_type + ":" + ref.name)}
        <span class="ref-pill ref-{ref.ref_type}" title={ref.name}>{ref.name}</span>
      {/each}
    </span>
  {/if}
  <span class="summary" title={commit.summary}>{commit.summary}</span>
  <span class="spacer"></span>
  {#if isUnpushed}
    <span class="unpushed" title="Not pushed to remote">
      <ArrowUp size={11} />
    </span>
  {/if}
  {#if showAuthor}
    <span class="author" title={`${commit.author_name} <${commit.author_email}>`}>{commit.author_name}</span>
  {/if}
  {#if showSha}
    <span class="oid">{commit.short_oid}</span>
  {/if}
  {#if showDate}
    <span class="time" title={new Date(commit.timestamp * 1000).toLocaleString()}>{dateLabel}</span>
  {/if}
</button>

<style>
  .commit-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    padding: 0 12px 0 4px;
    height: 100%;
    border: none;
    background: transparent;
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
    text-align: left;
    overflow: hidden;
    transition: background 0.1s;
  }

  .commit-row:hover {
    background: var(--color-surface);
  }

  .commit-row.selected {
    background: var(--color-surface-elevated);
    border-left: 2px solid var(--color-accent);
  }

  .commit-row.search-match {
    background: color-mix(in srgb, var(--color-accent) 10%, transparent);
  }

  .commit-row.search-match:hover {
    background: color-mix(in srgb, var(--color-accent) 16%, transparent);
  }

  .commit-row.search-current {
    background: color-mix(in srgb, var(--color-accent) 22%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-accent) 60%, transparent);
  }

  .avatar {
    border-radius: 50%;
    flex-shrink: 0;
  }

  .ref-labels {
    display: flex;
    gap: 4px;
    /* Shrinks (clipping pills) before the fixed columns get pushed out. */
    flex-shrink: 1;
    min-width: 0;
    max-width: 300px;
    overflow: hidden;
  }

  .ref-pill {
    display: inline-flex;
    align-items: center;
    padding: 1px 6px;
    border-radius: 3px;
    font-size: 10px;
    font-weight: 600;
    white-space: nowrap;
    line-height: 16px;
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ref-local {
    background: color-mix(in srgb, var(--color-lane-0) 20%, transparent);
    color: var(--color-lane-0);
  }

  .ref-remote {
    background: color-mix(in srgb, var(--color-text-muted) 20%, transparent);
    color: var(--color-text-muted);
  }

  .ref-tag {
    background: color-mix(in srgb, var(--color-lane-2) 20%, transparent);
    color: var(--color-lane-2);
  }

  .summary {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  /* The summary takes all free space; the fixed columns align right. */
  .spacer {
    flex: 0 0 0;
  }

  .unpushed {
    display: flex;
    align-items: center;
    color: var(--color-lane-2);
    flex-shrink: 0;
    opacity: 0.8;
  }

  /* Fixed-width columns; widths are CSS vars set by CommitGraph so the
     header and every row line up and resize together. */
  .author,
  .oid,
  .time {
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 0;
  }

  .author {
    color: var(--color-text-muted);
    width: var(--graph-col-author, 120px);
  }

  .oid {
    font-family: var(--font-mono);
    color: var(--color-accent);
    width: var(--graph-col-sha, 64px);
  }

  .time {
    color: var(--color-text-muted);
    text-align: right;
    width: var(--graph-col-date, 90px);
  }
</style>
