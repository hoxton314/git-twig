<script lang="ts">
  import { Loader2, UserPlus } from "lucide-svelte";
  import Modal from "../shared/Modal.svelte";
  import * as tauri from "../../lib/tauri";
  import type { AuthorInfo } from "../../lib/types/git";
  import { t } from "../../lib/i18n";

  interface Props {
    open: boolean;
    repoPath: string;
    onclose: () => void;
    /** Called with "Name <email>". */
    onpick: (ident: string) => void;
  }

  let { open, repoPath, onclose, onpick }: Props = $props();

  let query = $state("");
  let authors = $state<AuthorInfo[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let active = $state(0);

  // Refresh the author list each time the picker opens.
  $effect(() => {
    if (!open) return;
    query = "";
    active = 0;
    const path = repoPath;
    loading = true;
    error = null;
    tauri
      .getRecentAuthors(path)
      .then((list) => {
        if (repoPath !== path) return;
        authors = list;
      })
      .catch((err) => {
        if (repoPath === path) error = String(err);
      })
      .finally(() => {
        if (repoPath === path) loading = false;
      });
  });

  const matches = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const list = q
      ? authors.filter((a) => a.name.toLowerCase().includes(q) || a.email.toLowerCase().includes(q))
      : authors;
    return list.slice(0, 50);
  });

  // A typed "Name <email>" can be added even if it's not in history.
  const custom = $derived(/^[^<>\n]+\s<[^<>\s@]+@[^<>\s]+>$/.test(query.trim()) ? query.trim() : null);

  $effect(() => {
    // Keep the highlighted row in range as the list shrinks.
    if (active >= matches.length) active = Math.max(0, matches.length - 1);
  });

  // The note wraps the trailer name in <code>; split the message around it.
  const TRAILER = "Co-authored-by:";
  const note = $derived($t("coauthor.note").split("{trailer}"));

  function pick(ident: string) {
    onpick(ident);
    onclose();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      active = Math.min(active + 1, matches.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      active = Math.max(active - 1, 0);
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (custom) pick(custom);
      else if (matches[active]) pick(`${matches[active].name} <${matches[active].email}>`);
    }
  }
</script>

<Modal {open} title={$t("coauthor.title")} {onclose} width="420px">
  <div class="picker">
    <input
      class="search"
      type="text"
      placeholder={$t("coauthor.placeholder")}
      bind:value={query}
      onkeydown={onKey}
      spellcheck="false"
      aria-label={$t("coauthor.searchLabel")}
      aria-controls="coauthor-list"
    />
    {#if custom}
      <button class="row custom" onclick={() => pick(custom)}>
        <UserPlus size={13} />
        <span class="name">{$t("coauthor.addCustom", { ident: custom })}</span>
      </button>
    {/if}
    <div class="list" id="coauthor-list" role="listbox" aria-label={$t("coauthor.recent")}>
      {#if loading}
        <div class="hint"><Loader2 size={13} class="spinner" /> {$t("coauthor.loading")}</div>
      {:else if error}
        <div class="hint error">{error}</div>
      {:else if matches.length === 0}
        <div class="hint">
          {authors.length === 0 ? $t("coauthor.noAuthors") : $t("coauthor.noMatches")}
        </div>
      {:else}
        {#each matches as a, i (a.email)}
          <button
            class="row"
            class:active={i === active}
            role="option"
            aria-selected={i === active}
            onclick={() => pick(`${a.name} <${a.email}>`)}
            onmouseenter={() => (active = i)}
          >
            <span class="name">{a.name}</span>
            <span class="email">{a.email}</span>
            <span class="count" title={$t("coauthor.countTitle")}>{a.count}</span>
          </button>
        {/each}
      {/if}
    </div>
    <p class="note">{note[0]}<code>{TRAILER}</code>{note[1] ?? ""}</p>
  </div>
</Modal>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .search {
    width: 100%;
    box-sizing: border-box;
    padding: 6px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 12px;
    outline: none;
  }

  .search:focus {
    border-color: var(--color-accent);
  }

  .list {
    max-height: 280px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 8px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-primary);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .row.active,
  .row:focus-visible {
    background: var(--color-surface-elevated);
    outline: none;
  }

  .row.custom {
    color: var(--color-accent);
  }

  .name {
    flex-shrink: 0;
    max-width: 50%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .email {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .count {
    color: var(--color-text-muted);
    font-size: 10px;
  }

  .hint {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px;
    color: var(--color-text-muted);
    font-size: 12px;
    font-style: italic;
  }

  .hint.error {
    color: var(--color-diff-del-text);
    font-style: normal;
  }

  .note {
    margin: 0;
    color: var(--color-text-muted);
    font-size: 11px;
  }

  .picker :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
