<script lang="ts">
  import type { GraphEntry } from "../../lib/types/git";

  interface Props {
    entry: GraphEntry;
    isSelected: boolean;
    onSelect: () => void;
  }

  let { entry, isSelected, onSelect }: Props = $props();

  const commit = $derived(entry.commit);
  const gravatarUrl = $derived(
    `https://www.gravatar.com/avatar/${commit.author_gravatar}?s=28&d=identicon`
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
  <span class="summary" title={commit.summary}>{commit.summary}</span>
  <span class="spacer"></span>
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
    padding: 0 12px;
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
