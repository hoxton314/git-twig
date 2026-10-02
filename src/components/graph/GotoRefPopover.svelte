<script lang="ts">
  import { onMount } from "svelte";
  import type { RefLabel } from "../../lib/types/git";
  import GraphPopover from "./GraphPopover.svelte";

  interface Props {
    /** Known refs (branches/tags) to suggest. */
    refs: RefLabel[];
    /** Called with a branch/tag name, `HEAD`, or a SHA. */
    onsubmit: (rev: string) => void;
    onclose: () => void;
  }

  let { refs, onsubmit, onclose }: Props = $props();

  let query = $state("");
  let active = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);

  const MAX_SUGGESTIONS = 50;
  const ORDER = { local: 0, tag: 1, remote: 2 } as const;

  const suggestions = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const all: RefLabel[] = [{ name: "HEAD", ref_type: "local" }, ...refs];
    return all
      .filter((r) => !q || r.name.toLowerCase().includes(q))
      .sort((a, b) => {
        // Prefix matches first, then local < tag < remote, then by name.
        const pa = q && a.name.toLowerCase().startsWith(q) ? 0 : 1;
        const pb = q && b.name.toLowerCase().startsWith(q) ? 0 : 1;
        return pa - pb || ORDER[a.ref_type] - ORDER[b.ref_type] || a.name.localeCompare(b.name);
      })
      .slice(0, MAX_SUGGESTIONS);
  });

  $effect(() => {
    // Keep the highlighted row in range as the list changes.
    if (active >= suggestions.length) active = Math.max(0, suggestions.length - 1);
  });

  onMount(() => {
    inputEl?.focus();
  });

  function submit(rev: string) {
    const r = rev.trim();
    if (r) onsubmit(r);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      active = Math.min(suggestions.length - 1, active + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      active = Math.max(0, active - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      const pick = suggestions[active];
      // An exact typed rev (e.g. a SHA) wins over a fuzzy suggestion.
      const typed = query.trim();
      if (pick && (!typed || pick.name.toLowerCase().includes(typed.toLowerCase()))) submit(pick.name);
      else submit(typed);
    }
  }
</script>

<GraphPopover label="Go to branch, tag or commit" {onclose} width={300}>
  <input
    bind:this={inputEl}
    bind:value={query}
    class="goto-input"
    type="text"
    placeholder="Branch, tag, HEAD or SHA…"
    spellcheck="false"
    autocomplete="off"
    aria-label="Branch, tag or commit to go to"
    aria-controls="goto-ref-list"
    onkeydown={onKeydown}
    oninput={() => (active = 0)}
  />
  {#if suggestions.length > 0}
    <ul class="list" id="goto-ref-list" role="listbox" aria-label="Matching refs">
      {#each suggestions as ref, i (ref.ref_type + ":" + ref.name)}
        <li role="option" aria-selected={i === active}>
          <button
            class="item"
            class:active={i === active}
            tabindex="-1"
            onmouseenter={() => (active = i)}
            onclick={() => submit(ref.name)}
          >
            <span class="name">{ref.name}</span>
            <span class="kind kind-{ref.ref_type}">{ref.name === "HEAD" ? "head" : ref.ref_type}</span>
          </button>
        </li>
      {/each}
    </ul>
  {:else if query.trim()}
    <div class="hint">Press Enter to go to “{query.trim()}”</div>
  {/if}
</GraphPopover>

<style>
  .goto-input {
    width: 100%;
    padding: 6px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    font-family: inherit;
    outline: none;
  }

  .goto-input:focus {
    border-color: var(--color-accent);
  }

  .list {
    list-style: none;
    margin: 6px 0 0;
    padding: 0;
    max-height: 260px;
    overflow-y: auto;
  }

  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 4px 6px;
    border: none;
    border-radius: 3px;
    background: transparent;
    color: var(--color-text-primary);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .item.active {
    background: var(--color-surface);
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .kind {
    flex-shrink: 0;
    font-size: 10px;
    color: var(--color-text-muted);
  }

  .kind-local {
    color: var(--color-lane-0);
  }

  .kind-tag {
    color: var(--color-lane-2);
  }

  .hint {
    margin-top: 6px;
    color: var(--color-text-muted);
    font-size: 11px;
  }
</style>
