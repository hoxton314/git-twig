<script lang="ts">
  /**
   * Filterable file picker in a modal. Single mode: Enter / click picks one
   * file. Multiple mode: checkboxes plus a confirm button.
   */
  import Modal from "../shared/Modal.svelte";
  import { Search, Loader2, FileText } from "lucide-svelte";

  interface Props {
    open: boolean;
    title: string;
    /** `null` while loading. */
    items: string[] | null;
    error?: string | null;
    multiple?: boolean;
    /** Initially checked items (multiple mode). */
    initialSelected?: string[];
    /** Optional short tag shown next to an item (e.g. "M", "staged"). */
    tag?: (item: string) => string;
    confirmLabel?: string;
    onconfirm: (paths: string[]) => void;
    onclose: () => void;
  }

  let {
    open,
    title,
    items,
    error = null,
    multiple = false,
    initialSelected = [],
    tag,
    confirmLabel = "Select",
    onconfirm,
    onclose,
  }: Props = $props();

  const MAX_SHOWN = 300;

  let query = $state("");
  let cursor = $state(0);
  let selected = $state<Set<string>>(new Set());
  let listEl = $state<HTMLDivElement | null>(null);

  $effect(() => {
    if (open) {
      query = "";
      cursor = 0;
      selected = new Set(initialSelected);
    }
  });

  const tokens = $derived(query.trim().toLowerCase().split(/\s+/).filter(Boolean));
  const matches = $derived.by(() => {
    if (!items) return [];
    if (tokens.length === 0) return items;
    const out: string[] = [];
    for (const it of items) {
      const lc = it.toLowerCase();
      if (tokens.every((t) => lc.includes(t))) out.push(it);
    }
    // Prefer matches in the file name over matches in the directory.
    const last = tokens[tokens.length - 1];
    return out.sort((a, b) => {
      const an = a.slice(a.lastIndexOf("/") + 1).toLowerCase().includes(last) ? 0 : 1;
      const bn = b.slice(b.lastIndexOf("/") + 1).toLowerCase().includes(last) ? 0 : 1;
      return an - bn || a.length - b.length || a.localeCompare(b);
    });
  });
  const shown = $derived(matches.slice(0, MAX_SHOWN));

  $effect(() => {
    void tokens;
    cursor = 0;
  });

  function toggle(item: string) {
    const next = new Set(selected);
    if (next.has(item)) next.delete(item);
    else next.add(item);
    selected = next;
  }

  function choose(item: string) {
    if (multiple) toggle(item);
    else onconfirm([item]);
  }

  function scrollCursorIntoView() {
    queueMicrotask(() => {
      listEl?.querySelector<HTMLElement>(`[data-idx="${cursor}"]`)?.scrollIntoView({ block: "nearest" });
    });
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.isComposing) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      cursor = Math.min(shown.length - 1, cursor + 1);
      scrollCursorIntoView();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      cursor = Math.max(0, cursor - 1);
      scrollCursorIntoView();
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (multiple && (e.ctrlKey || e.metaKey)) {
        if (selected.size > 0) onconfirm([...selected]);
        return;
      }
      const item = shown[cursor];
      if (item) choose(item);
    } else if (e.key === " " && multiple && query === "") {
      e.preventDefault();
      const item = shown[cursor];
      if (item) toggle(item);
    }
  }

  function splitPath(p: string): [string, string] {
    const i = p.lastIndexOf("/");
    return i === -1 ? ["", p] : [p.slice(0, i + 1), p.slice(i + 1)];
  }
</script>

<Modal {open} {title} {onclose} width="640px">
  <div class="picker">
    <div class="search">
      <Search size={13} />
      <input
        type="text"
        placeholder="Filter files…"
        aria-label="Filter files"
        bind:value={query}
        onkeydown={onKeydown}
      />
      {#if items}
        <span class="count">{matches.length.toLocaleString()}</span>
      {/if}
    </div>

    <div class="list" bind:this={listEl} role="listbox" aria-multiselectable={multiple}>
      {#if error}
        <div class="empty error">{error}</div>
      {:else if !items}
        <div class="empty"><Loader2 size={14} class="spinner" /> Loading files…</div>
      {:else if shown.length === 0}
        <div class="empty">{items.length === 0 ? "No files" : "No matching files"}</div>
      {:else}
        {#each shown as item, i (item)}
          {@const [dir, name] = splitPath(item)}
          <div
            class="item"
            class:cursor={i === cursor}
            data-idx={i}
            role="option"
            aria-selected={multiple ? selected.has(item) : i === cursor}
            tabindex="-1"
            onclick={() => choose(item)}
            onkeydown={() => {}}
            onmousemove={() => (cursor = i)}
            title={item}
          >
            {#if multiple}
              <input type="checkbox" tabindex="-1" checked={selected.has(item)} onclick={(e) => e.stopPropagation()} onchange={() => toggle(item)} />
            {:else}
              <FileText size={13} />
            {/if}
            <span class="name">{name}</span>
            <span class="dir">{dir}</span>
            {#if tag}
              <span class="tag">{tag(item)}</span>
            {/if}
          </div>
        {/each}
        {#if matches.length > shown.length}
          <div class="more">{(matches.length - shown.length).toLocaleString()} more — refine the filter</div>
        {/if}
      {/if}
    </div>

    {#if multiple}
      <div class="footer">
        <span class="hint">{selected.size} selected · Space toggles · Ctrl+Enter confirms</span>
        <button class="btn" onclick={() => (selected = new Set(matches))} disabled={!items || matches.length === 0}>All</button>
        <button class="btn" onclick={() => (selected = new Set())} disabled={selected.size === 0}>None</button>
        <button class="btn primary" disabled={selected.size === 0} onclick={() => onconfirm([...selected])}>
          {confirmLabel}
        </button>
      </div>
    {/if}
  </div>
</Modal>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 10px;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-bg);
    color: var(--color-text-muted);
  }

  .search:focus-within {
    border-color: var(--color-accent);
  }

  .search input {
    flex: 1;
    min-width: 0;
    padding: 8px 0;
    border: none;
    background: transparent;
    color: var(--color-text-primary);
    font-size: 13px;
    outline: none;
  }

  .count {
    font-size: 11px;
  }

  .list {
    height: min(420px, calc(100vh - 280px));
    overflow-y: auto;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-bg);
  }

  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 10px;
    font-size: 12px;
    color: var(--color-text-muted);
    cursor: pointer;
    white-space: nowrap;
  }

  .item.cursor {
    background: var(--color-surface-elevated);
  }

  .name {
    color: var(--color-text-primary);
    font-family: var(--font-mono);
  }

  .dir {
    overflow: hidden;
    text-overflow: ellipsis;
    font-family: var(--font-mono);
    font-size: 11px;
    flex: 1;
    min-width: 0;
  }

  .tag {
    font-size: 10px;
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--color-surface);
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .empty,
  .more {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 12px;
    color: var(--color-text-muted);
    font-size: 12px;
  }

  .empty.error {
    color: var(--color-diff-del-text);
  }

  .empty :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .footer {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .hint {
    flex: 1;
    font-size: 11px;
    color: var(--color-text-muted);
  }

  .btn {
    padding: 5px 12px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .btn.primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: var(--color-bg);
  }
</style>
