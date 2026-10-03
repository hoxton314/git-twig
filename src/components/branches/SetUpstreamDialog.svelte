<!-- Pick a remote-tracking branch as the upstream of a local branch. -->
<script lang="ts">
  import { tick, untrack } from "svelte";
  import { Search } from "lucide-svelte";
  import Modal from "../shared/Modal.svelte";
  import { t, splitMessage } from "../../lib/i18n";
  import type { BranchInfo } from "../../lib/types/git";

  interface Props {
    /** Local branch to configure; null = closed. */
    branch: BranchInfo | null;
    remoteBranches: BranchInfo[];
    busy?: boolean;
    onconfirm: (upstream: string) => void;
    onclose: () => void;
  }

  let { branch, remoteBranches, busy = false, onconfirm, onclose }: Props = $props();

  let query = $state("");
  let selected = $state<string | null>(null);
  let inputEl: HTMLInputElement | undefined = $state(undefined);

  // Preselect the current upstream, else a same-named remote branch.
  $effect(() => {
    const b = branch;
    if (!b) return;
    untrack(() => {
      query = "";
      selected =
        b.upstream ??
        remoteBranches.find((r) => r.short_name === b.name && r.remote_name === "origin")?.name ??
        remoteBranches.find((r) => r.short_name === b.name)?.name ??
        null;
    });
    tick().then(() => inputEl?.focus());
  });

  const q = $derived(query.trim().toLowerCase());
  const shown = $derived(remoteBranches.filter((r) => !q || r.name.toLowerCase().includes(q)));

  function submit() {
    if (selected && !busy) onconfirm(selected);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (shown.length === 0) return;
      const idx = shown.findIndex((r) => r.name === selected);
      const next = e.key === "ArrowDown" ? Math.min(idx + 1, shown.length - 1) : Math.max(idx - 1, 0);
      selected = shown[next].name;
      tick().then(() => document.getElementById(optionId(selected ?? ""))?.scrollIntoView({ block: "nearest" }));
    } else if (e.key === "Enter") {
      e.preventDefault();
      submit();
    }
  }

  /** Message split around its `{placeholders}`: odd indices are names. */

  function optionId(name: string) {
    return `upstream-opt-${name.replace(/[^a-zA-Z0-9_-]/g, "_")}`;
  }
</script>

<Modal open={!!branch} title={$t("branches.setUpstreamTitle")} {onclose} width="420px">
  {#if branch}
    <div class="upstream-dialog">
      <p class="desc">
        {#each splitMessage($t("branches.setUpstreamDesc")) as part, i}{#if i % 2}<strong>{branch.name}</strong>{:else}{part}{/if}{/each}
      </p>
      {#if remoteBranches.length === 0}
        <p class="empty">{$t("branches.noRemoteBranches")}</p>
      {:else}
        <div class="search">
          <Search size={12} />
          <input
            bind:this={inputEl}
            bind:value={query}
            onkeydown={onKeydown}
            placeholder={$t("branches.filterRemotePlaceholder")}
            aria-label={$t("branches.filterRemote")}
            aria-controls="upstream-options"
            spellcheck="false"
          />
        </div>
        <div class="options" id="upstream-options" role="listbox" aria-label={$t("branches.remoteBranches")}>
          {#each shown as r (r.name)}
            <div
              id={optionId(r.name)}
              class="option"
              class:selected={selected === r.name}
              role="option"
              aria-selected={selected === r.name}
              tabindex="-1"
              onclick={() => (selected = r.name)}
              ondblclick={() => {
                selected = r.name;
                submit();
              }}
              onkeydown={onKeydown}
            >
              <span class="opt-name">{r.name}</span>
              {#if r.name === branch.upstream}<span class="tag">{$t("branches.current")}</span>{/if}
            </div>
          {:else}
            <div class="empty">{$t("branches.noMatches")}</div>
          {/each}
        </div>
      {/if}
      <div class="actions">
        <button class="btn-secondary" onclick={onclose} disabled={busy}>{$t("common.cancel")}</button>
        <button
          class="btn-primary"
          onclick={submit}
          disabled={!selected || busy || selected === branch.upstream}
        >
          {busy ? $t("branches.saving") : $t("branches.setUpstreamTitle")}
        </button>
      </div>
    </div>
  {/if}
</Modal>

<style>
  .upstream-dialog {
    display: flex;
    flex-direction: column;
    gap: 10px;
    font-size: 13px;
  }

  .desc {
    margin: 0;
    color: var(--color-text-muted);
    line-height: 1.5;
  }

  .desc strong {
    color: var(--color-accent);
  }

  .empty {
    margin: 0;
    padding: 8px;
    color: var(--color-text-muted);
    font-size: 12px;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-muted);
  }

  .search:focus-within {
    border-color: var(--color-accent);
  }

  .search input {
    flex: 1;
    min-width: 0;
    padding: 6px 0;
    border: none;
    background: transparent;
    color: var(--color-text-primary);
    font-size: 12px;
    outline: none;
  }

  .options {
    max-height: 240px;
    overflow-y: auto;
    border: 1px solid var(--color-border);
    border-radius: 4px;
  }

  .option {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    font-size: 12px;
    color: var(--color-text-primary);
    cursor: pointer;
  }

  .option:hover {
    background: var(--color-surface-elevated);
  }

  .option.selected {
    background: var(--color-accent);
    color: var(--color-bg);
  }

  .opt-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tag {
    font-size: 10px;
    opacity: 0.7;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .btn-primary,
  .btn-secondary {
    border-radius: 4px;
    padding: 6px 16px;
    font-size: 13px;
    cursor: pointer;
  }

  .btn-primary {
    background: var(--color-accent);
    color: var(--color-bg);
    border: none;
    font-weight: 600;
  }

  .btn-secondary {
    background: transparent;
    color: var(--color-text-muted);
    border: 1px solid var(--color-border);
  }

  .btn-primary:disabled,
  .btn-secondary:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
