<script lang="ts">
  import type { GraphEntry, RefLabel } from "../../lib/types/git";
  import { ArrowUp } from "lucide-svelte";

  interface Props {
    entry: GraphEntry;
    isSelected: boolean;
    isUnpushed: boolean;
    refs: RefLabel[];
    onSelect: () => void;
  }

  let { entry, isSelected, isUnpushed, refs, onSelect }: Props = $props();

  const commit = $derived(entry.commit);
  const gravatarUrl = $derived(
    `https://www.gravatar.com/avatar/${commit.author_gravatar}?s=28&d=identicon`
  );

  // Sort refs: local first, then remote, then tags
  const sortedRefs = $derived(
    [...refs].sort((a, b) => {
      const order = { local: 0, tag: 1, remote: 2 };
      return (order[a.ref_type] ?? 3) - (order[b.ref_type] ?? 3);
    })
  );

  function formatTime(ts: number): string {
    const date = new Date(ts * 1000);
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const minutes = Math.floor(diff / 60000);
    const hours = Math.floor(minutes / 60);
    const days = Math.floor(hours / 24);

    if (minutes < 60) return `${minutes}m ago`;
    if (hours < 24) return `${hours}h ago`;
    if (days < 30) return `${days}d ago`;
    return date.toLocaleDateString();
  }
</script>

<button
  class="commit-row"
  class:selected={isSelected}
  onclick={onSelect}
>
  <img
    class="avatar"
    src={gravatarUrl}
    alt={commit.author_name}
    width="20"
    height="20"
    loading="lazy"
  />
  {#if sortedRefs.length > 0}
    <span class="ref-labels">
      {#each sortedRefs as ref (ref.name)}
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
  <span class="author">{commit.author_name}</span>
  <span class="oid">{commit.short_oid}</span>
  <span class="time">{formatTime(commit.timestamp)}</span>
</button>

<style>
  .commit-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
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

  .avatar {
    border-radius: 50%;
    flex-shrink: 0;
  }

  .ref-labels {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
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
    background: rgba(122, 162, 247, 0.2);
    color: #7aa2f7;
  }

  .ref-remote {
    background: rgba(86, 95, 137, 0.2);
    color: var(--color-text-muted);
  }

  .ref-tag {
    background: rgba(224, 175, 104, 0.2);
    color: #e0af68;
  }

  .summary {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .spacer {
    flex: 1;
  }

  .unpushed {
    display: flex;
    align-items: center;
    color: #e0af68;
    flex-shrink: 0;
    opacity: 0.8;
  }

  .author {
    color: var(--color-text-muted);
    font-size: 11px;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .oid {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-accent);
    flex-shrink: 0;
  }

  .time {
    color: var(--color-text-muted);
    font-size: 11px;
    white-space: nowrap;
    min-width: 60px;
    text-align: right;
    flex-shrink: 0;
  }
</style>
