<script lang="ts">
  import { tick } from "svelte";
  import {
    ChevronDown,
    ChevronUp,
    Crosshair,
    GitBranch,
    Loader2,
    Search,
    SlidersHorizontal,
    X,
  } from "lucide-svelte";
  import type { RefLabel } from "../../lib/types/git";
  import {
    searchOpen,
    searchQuery,
    searchMode,
    searchResult,
    searchBusy,
    searchError,
    currentMatch,
    scheduleSearch,
    runSearch,
    closeSearch,
    searchKind,
    changeOptions,
    SEARCH_MAX_RESULTS,
  } from "../../lib/stores/graphSearch";
  import GraphViewOptions from "./GraphViewOptions.svelte";
  import GotoRefPopover from "./GotoRefPopover.svelte";

  interface Props {
    loadedCount: number;
    hasMore: boolean;
    loadingMore: boolean;
    /** Ref labels for "go to" suggestions. */
    refs: RefLabel[];
    /** Bump to (re)focus and select the search field. */
    focusToken: number;
    gotoOpen: boolean;
    optionsOpen: boolean;
    onnext: () => void;
    onprev: () => void;
    onjumphead: () => void;
    ongoto: (rev: string) => void;
    onloadall: () => void;
    /** Return focus to the commit list. */
    onexit: () => void;
  }

  let {
    loadedCount,
    hasMore,
    loadingMore,
    refs,
    focusToken,
    gotoOpen = $bindable(),
    optionsOpen = $bindable(),
    onnext,
    onprev,
    onjumphead,
    ongoto,
    onloadall,
    onexit,
  }: Props = $props();

  let inputEl = $state<HTMLInputElement | null>(null);

  const result = $derived($searchResult);
  const total = $derived(result?.matches.length ?? 0);
  const hasQuery = $derived($searchQuery.trim() !== "");

  const countLabel = $derived.by(() => {
    if ($searchError) return "Search failed";
    if (!hasQuery) return "";
    if (!result) return $searchBusy ? "Searching…" : $searchKind === "changes" ? "Enter to search" : "";
    if (total === 0) return $searchKind === "changes" ? "No commits" : "No matches";
    if ($searchKind === "changes" && $currentMatch < 0) {
      const t = result.truncated ? `${SEARCH_MAX_RESULTS}+` : String(total);
      return `${t} commit${total === 1 ? "" : "s"}`;
    }
    const totalText = result.truncated ? `${SEARCH_MAX_RESULTS}+` : String(total);
    if ($searchMode === "filter") return `${totalText} ${total === 1 ? "match" : "matches"}`;
    return $currentMatch >= 0 ? `${$currentMatch + 1} of ${totalText}` : `${totalText} ${total === 1 ? "match" : "matches"}`;
  });

  $effect(() => {
    // Focus whenever asked (Ctrl+F) or when the bar opens.
    void focusToken;
    if (!$searchOpen) return;
    tick().then(() => {
      inputEl?.focus();
      inputEl?.select();
    });
  });

  function onInput() {
    currentMatch.set(-1);
    scheduleSearch();
  }

  function onSearchKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === "F3") {
      e.preventDefault();
      // Results may still be debounced: search now, then step.
      const step = () => (e.shiftKey ? onprev() : onnext());
      if (!$searchResult && hasQuery) runSearch().then(step);
      else step();
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      closeSearch();
      onexit();
    }
  }

  function setKind(kind: "commits" | "changes") {
    if ($searchKind === kind) return;
    searchKind.set(kind);
    currentMatch.set(-1);
    // Commit searches run as you type; a code-change search waits for Enter.
    scheduleSearch(0);
    inputEl?.focus();
  }

  /** A code-search option changed: results no longer match it. */
  function changedOption() {
    currentMatch.set(-1);
    scheduleSearch();
  }

  function setMode(mode: "highlight" | "filter") {
    searchMode.set(mode);
    inputEl?.focus();
  }

  function openSearch() {
    searchOpen.set(true);
  }
</script>

<div class="graph-toolbar">
  {#if $searchOpen}
    <div class="search" role="search">
      <span class="search-icon">
        {#if $searchBusy}
          <Loader2 size={13} class="spinner" />
        {:else}
          <Search size={13} />
        {/if}
      </span>
      <input
        bind:this={inputEl}
        bind:value={$searchQuery}
        class="search-input"
        type="text"
        placeholder={$searchKind === "changes"
          ? ($changeOptions.regex ? "Regex for added/removed lines (git log -G)…" : "Text added or removed (git log -S)…")
          : "Search message, author, email or SHA…"}
        spellcheck="false"
        autocomplete="off"
        aria-label="Search commits"
        title={$searchError ?? undefined}
        oninput={onInput}
        onkeydown={onSearchKeydown}
      />
      <span class="count" class:error={$searchError !== null} aria-live="polite">{countLabel}</span>
      <div class="segmented" role="radiogroup" aria-label="Search in">
        <button
          role="radio"
          aria-checked={$searchKind === "commits"}
          class:on={$searchKind === "commits"}
          title="Search commit messages, authors and SHAs"
          onclick={() => setKind("commits")}>Commits</button
        >
        <button
          role="radio"
          aria-checked={$searchKind === "changes"}
          class:on={$searchKind === "changes"}
          title="Find commits whose changes add or remove the text (pickaxe)"
          onclick={() => setKind("changes")}>Code</button
        >
      </div>
      {#if $searchKind === "changes"}
        <label class="opt" title="Regular expression: commits whose added or removed lines match (git log -G)">
          <input type="checkbox" bind:checked={$changeOptions.regex} onchange={changedOption} /> .*
        </label>
        <label class="opt" title="Match case">
          <input type="checkbox" bind:checked={$changeOptions.matchCase} onchange={changedOption} /> Aa
        </label>
        <input
          class="paths-input"
          type="text"
          bind:value={$changeOptions.paths}
          oninput={changedOption}
          onkeydown={onSearchKeydown}
          placeholder="paths"
          spellcheck="false"
          autocomplete="off"
          aria-label="Limit code search to paths"
          title="Only commits touching these paths (e.g. src/ *.rs)"
        />
      {/if}
      <div class="segmented" role="radiogroup" aria-label="Search mode">
        <button
          role="radio"
          aria-checked={$searchMode === "highlight"}
          class:on={$searchMode === "highlight"}
          title="Highlight matches and jump between them"
          onclick={() => setMode("highlight")}>Highlight</button
        >
        <button
          role="radio"
          aria-checked={$searchMode === "filter"}
          class:on={$searchMode === "filter"}
          title="Show only matching commits"
          onclick={() => setMode("filter")}>Filter</button
        >
      </div>
      <button
        class="icon-btn"
        title="Previous match (Shift+Enter)"
        aria-label="Previous match"
        disabled={total === 0}
        onclick={onprev}
      >
        <ChevronUp size={14} />
      </button>
      <button
        class="icon-btn"
        title="Next match (Enter)"
        aria-label="Next match"
        disabled={total === 0}
        onclick={onnext}
      >
        <ChevronDown size={14} />
      </button>
      <button
        class="icon-btn"
        title="Close search (Esc)"
        aria-label="Close search"
        onclick={() => {
          closeSearch();
          onexit();
        }}
      >
        <X size={14} />
      </button>
    </div>
  {:else}
    <button class="tool-btn" title="Search commits (Ctrl+F)" onclick={openSearch}>
      <Search size={13} />
      <span>Search</span>
    </button>
  {/if}

  <span class="spacer"></span>

  <span class="status" title={hasMore ? "Scroll down to load more history" : "Entire history loaded"}>
    {#if loadingMore}
      <Loader2 size={11} class="spinner" />
    {/if}
    {loadedCount.toLocaleString()}{hasMore ? "+" : ""} commits
  </span>
  {#if hasMore}
    <button class="link-btn" title="Load the entire history" disabled={loadingMore} onclick={onloadall}>
      Load all
    </button>
  {/if}

  <button class="icon-btn" title="Jump to HEAD" aria-label="Jump to HEAD" onclick={onjumphead}>
    <Crosshair size={14} />
  </button>
  <div class="anchor">
    <button
      class="icon-btn"
      class:on={gotoOpen}
      data-popover-toggle
      title="Go to branch, tag or commit"
      aria-label="Go to branch, tag or commit"
      aria-expanded={gotoOpen}
      onclick={() => {
        optionsOpen = false;
        gotoOpen = !gotoOpen;
      }}
    >
      <GitBranch size={14} />
    </button>
    {#if gotoOpen}
      <GotoRefPopover
        {refs}
        onclose={() => (gotoOpen = false)}
        onsubmit={(rev) => {
          gotoOpen = false;
          ongoto(rev);
        }}
      />
    {/if}
  </div>
  <div class="anchor">
    <button
      class="icon-btn"
      class:on={optionsOpen}
      data-popover-toggle
      title="View options"
      aria-label="Graph view options"
      aria-expanded={optionsOpen}
      onclick={() => {
        gotoOpen = false;
        optionsOpen = !optionsOpen;
      }}
    >
      <SlidersHorizontal size={14} />
    </button>
    {#if optionsOpen}
      <GraphViewOptions onclose={() => (optionsOpen = false)} />
    {/if}
  </div>
</div>

<style>
  .opt {
    display: flex;
    align-items: center;
    gap: 2px;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-muted);
    cursor: pointer;
    user-select: none;
    flex-shrink: 0;
  }
  .opt input { margin: 0; accent-color: var(--color-accent); }
  .paths-input {
    width: 90px;
    flex-shrink: 1;
    min-width: 50px;
    padding: 2px 6px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-family: var(--font-mono);
    font-size: 11px;
    outline: none;
  }
  .paths-input:focus { border-color: var(--color-accent); }
  .graph-toolbar {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 32px;
    padding: 0 6px 0 8px;
    flex-shrink: 0;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg);
    font-size: 12px;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: 1;
    min-width: 0;
    max-width: 640px;
  }

  .search-icon {
    display: flex;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .search-input {
    flex: 1;
    min-width: 80px;
    height: 24px;
    padding: 0 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    font-family: inherit;
    outline: none;
  }

  .search-input:focus {
    border-color: var(--color-accent);
  }

  .count {
    flex-shrink: 0;
    min-width: 0;
    color: var(--color-text-muted);
    font-size: 11px;
    white-space: nowrap;
  }

  .count.error {
    color: var(--color-diff-del-text);
  }

  .segmented {
    display: flex;
    flex-shrink: 0;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    overflow: hidden;
  }

  .segmented button {
    padding: 2px 8px;
    border: none;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 11px;
    cursor: pointer;
  }

  .segmented button + button {
    border-left: 1px solid var(--color-border);
  }

  .segmented button.on {
    background: color-mix(in srgb, var(--color-accent) 18%, transparent);
    color: var(--color-text-primary);
  }

  .tool-btn,
  .icon-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    flex-shrink: 0;
  }

  .tool-btn {
    height: 24px;
    padding: 0 8px;
    font-size: 12px;
  }

  .icon-btn {
    justify-content: center;
    width: 24px;
    height: 24px;
  }

  .tool-btn:hover,
  .icon-btn:hover:not(:disabled),
  .icon-btn.on {
    background: var(--color-surface);
    color: var(--color-text-primary);
  }

  .icon-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .spacer {
    flex: 1;
  }

  .status {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--color-text-muted);
    font-size: 11px;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .link-btn {
    padding: 0 4px;
    border: none;
    background: none;
    color: var(--color-accent);
    font-size: 11px;
    cursor: pointer;
    flex-shrink: 0;
  }

  .link-btn:hover:not(:disabled) {
    text-decoration: underline;
  }

  .link-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .anchor {
    position: relative;
    display: flex;
  }

  .graph-toolbar :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }
</style>
