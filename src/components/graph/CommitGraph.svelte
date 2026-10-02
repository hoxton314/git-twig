<script lang="ts">
  import { onMount, tick } from "svelte";
  import { get } from "svelte/store";
  import { activeRepoPath } from "../../lib/stores/repos";
  import {
    commitGraph,
    graphLoading,
    graphLoadingMore,
    selectedCommitOid,
    workingStatus,
    selectedWorkingFile,
    workingFileDiff,
    loadGraph,
    loadMoreCommits,
    ensureGraphLoaded,
    loadEntireGraph,
    graphOptions,
  } from "../../lib/stores/graph";
  import {
    searchOpen,
    searchMode,
    searchQuery,
    searchResult,
    searchActive,
    currentMatch,
    matchOids,
  } from "../../lib/stores/graphSearch";
  import { settings, updateSettings } from "../../lib/stores/settings";
  import { now } from "../../lib/stores/clock";
  import { toast, toastError } from "../../lib/stores/toasts";
  import { onAction } from "../../lib/keybindings";
  import * as tauri from "../../lib/tauri";
  import type { GraphEntry, RefLabel } from "../../lib/types/git";
  import CommitRow from "./CommitRow.svelte";
  import GraphCanvas from "./GraphCanvas.svelte";
  import GraphToolbar from "./GraphToolbar.svelte";
  import GraphColumnHeader from "./GraphColumnHeader.svelte";
  import {
    avatarSize,
    DEFAULT_COL_WIDTHS,
    graphColumnWidth,
    rowHeight,
    type GraphColumn,
  } from "./graphLayout";
  import { Loader2, Pencil } from "lucide-svelte";
  import CommitContextMenu from "./CommitContextMenu.svelte";
  import { openCommitMenu, revealRequest } from "../../lib/stores/commitUi";
  import { revealCommit } from "../../lib/stores/fileviews";

  const repoPath = $derived($activeRepoPath);
  const graph = $derived($commitGraph);
  const loading = $derived($graphLoading);
  const loadingMore = $derived($graphLoadingMore);
  const selected = $derived($selectedCommitOid);
  const status = $derived($workingStatus);
  const s = $derived($settings);

  // ── View settings ───────────────────────────────────────────────────
  const ROW_HEIGHT = $derived(rowHeight(s.graph_row_density));
  const AVATAR = $derived(avatarSize(s.graph_row_density));
  const OVERSCAN = 10;
  /** Start fetching the next page this many rows before the end. */
  const LOAD_AHEAD = 60;

  /** Live widths while a header separator is dragged (persisted on release). */
  let dragWidths = $state<Partial<Record<GraphColumn, number>>>({});
  const colWidths = $derived({
    author: dragWidths.author ?? s.graph_author_width,
    sha: dragWidths.sha ?? s.graph_sha_width,
    date: dragWidths.date ?? s.graph_date_width,
  });
  const WIDTH_KEYS = {
    author: "graph_author_width",
    sha: "graph_sha_width",
    date: "graph_date_width",
  } as const;

  // ── Search / filter ─────────────────────────────────────────────────
  const filterMode = $derived($searchActive && $searchMode === "filter");
  /** Dim non-matching rows while highlighting a search with results. */
  const highlighting = $derived(
    $searchActive && $searchMode === "highlight" && ($searchResult?.matches.length ?? 0) > 0,
  );
  const currentMatchOid = $derived(
    $searchResult?.matches[$currentMatch]?.commit.oid ?? null,
  );

  /** Filter mode shows search results as a flat list without lanes. */
  const filteredEntries = $derived<GraphEntry[]>(
    filterMode
      ? ($searchResult?.matches ?? []).map((m) => ({
          commit: m.commit,
          lane: 0,
          has_incoming: false,
          rails: [],
          parent_lanes: [],
          merge_ins: [],
        }))
      : [],
  );

  const entries = $derived<GraphEntry[]>(filterMode ? filteredEntries : (graph?.entries ?? []));

  const hasWip = $derived(
    !filterMode && (status.staged.length > 0 || status.unstaged.length > 0),
  );
  const wipSelected = $derived($selectedCommitOid === "__wip__");

  const unpushedSet = $derived(new Set(graph?.unpushed_oids ?? []));

  /** Branch/tag labels for "go to" suggestions. */
  const allRefs = $derived.by<RefLabel[]>(() => {
    const seen = new Set<string>();
    const out: RefLabel[] = [];
    for (const labels of Object.values(graph?.refs ?? {})) {
      for (const r of labels) {
        const key = r.ref_type + ":" + r.name;
        if (!seen.has(key)) {
          seen.add(key);
          out.push(r);
        }
      }
    }
    return out;
  });

  // ── Virtualization ──────────────────────────────────────────────────
  let containerEl: HTMLDivElement | undefined = $state(undefined);
  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  let containerOffsetWidth = $state(0);
  let containerClientWidth = $state(0);
  const scrollbarWidth = $derived(Math.max(0, containerOffsetWidth - containerClientWidth));

  const totalLanes = $derived(filterMode ? 0 : (graph?.total_lanes ?? 0));
  const graphWidth = $derived(graphColumnWidth(totalLanes));
  /**
   * Columns actually shown: the user's choice, minus author then SHA when
   * the panel is too narrow to leave the description readable.
   */
  const MIN_DESCRIPTION = 220;
  const cols = $derived.by(() => {
    let author = s.graph_show_author;
    let sha = s.graph_show_sha;
    const date = s.graph_show_date;
    const free = () =>
      (containerClientWidth || 10_000) - graphWidth - 16 -
      (author ? colWidths.author + 8 : 0) - (sha ? colWidths.sha + 8 : 0) - (date ? colWidths.date + 8 : 0);
    if (free() < MIN_DESCRIPTION) author = false;
    if (free() < MIN_DESCRIPTION) sha = false;
    return { author, sha, date };
  });

  const entryCount = $derived(entries.length);
  const totalHeight = $derived(entryCount * ROW_HEIGHT);

  // Adjust scroll offset to account for the WIP row above the virtual scroll
  const graphScrollTop = $derived(Math.max(0, scrollTop - (hasWip ? ROW_HEIGHT : 0)));
  const visibleStart = $derived(Math.max(0, Math.floor(graphScrollTop / ROW_HEIGHT) - OVERSCAN));
  // viewportHeight is bound to the container, so this updates on resize
  // (window resize, diff panel open/close) instead of only on first mount.
  const visibleCount = $derived(
    Math.max(
      0,
      Math.min(
        entryCount - visibleStart,
        Math.ceil((viewportHeight || 600) / ROW_HEIGHT) + 2 * OVERSCAN
      )
    )
  );
  const visibleEntries = $derived(entries.slice(visibleStart, visibleStart + visibleCount));

  function handleScroll(e: Event) {
    scrollTop = (e.target as HTMLDivElement).scrollTop;
  }

  // ── Pagination: auto-load the next page near the bottom ────────────
  /** Entry count at which an automatic load failed; don't retry there. */
  let autoLoadBlockedAt = -1;
  $effect(() => {
    if (filterMode || !graph || !graph.has_more || loadingMore || loading) return;
    const count = graph.entries.length;
    if (count === autoLoadBlockedAt) return;
    if (visibleStart + visibleCount < count - LOAD_AHEAD) return;
    loadMoreCommits().then(() => {
      if ((get(commitGraph)?.entries.length ?? 0) === count) autoLoadBlockedAt = count;
    });
  });

  function loadMoreManually() {
    autoLoadBlockedAt = -1;
    loadMoreCommits();
  }

  // ── Selection ───────────────────────────────────────────────────────

  function selectCommit(oid: string) {
    $selectedCommitOid = selected === oid ? null : oid;
    // WebKit doesn't focus buttons on click; focus the list so arrow keys work.
    containerEl?.focus({ preventScroll: true });
  }

  function selectWip() {
    if (wipSelected) {
      $selectedCommitOid = null;
      $selectedWorkingFile = null;
      $workingFileDiff = [];
      return;
    }
    $selectedCommitOid = "__wip__";
    // Load combined working diff
    const path = repoPath;
    if (path) {
      tauri
        .getWorkingDiff(path)
        .then((diff) => {
          // Ignore if the user switched repos or deselected WIP meanwhile.
          if ($activeRepoPath !== path || $selectedCommitOid !== "__wip__") return;
          $workingFileDiff = diff;
          $selectedWorkingFile = { path: "__all__", area: "unstaged" };
        })
        .catch((err) => console.error("Failed to load working diff:", err));
    }
  }

  // ── Scrolling helpers ───────────────────────────────────────────────

  /** Scroll so the row at `index` is fully visible. */
  function scrollRowIntoView(index: number) {
    if (!containerEl) return;
    const offset = hasWip ? ROW_HEIGHT : 0;
    const top = offset + index * ROW_HEIGHT;
    const bottom = top + ROW_HEIGHT;
    if (top < containerEl.scrollTop) {
      containerEl.scrollTop = top;
    } else if (bottom > containerEl.scrollTop + containerEl.clientHeight) {
      containerEl.scrollTop = bottom - containerEl.clientHeight;
    }
  }

  /** Scroll so the row at `index` sits in the middle of the viewport. */
  function centerRow(index: number) {
    if (!containerEl) return;
    const offset = hasWip ? ROW_HEIGHT : 0;
    const target = offset + index * ROW_HEIGHT - (containerEl.clientHeight - ROW_HEIGHT) / 2;
    containerEl.scrollTop = Math.max(0, target);
    scrollTop = containerEl.scrollTop;
  }

  /** Select and reveal a row of the displayed list. */
  function revealRow(index: number) {
    const entry = entries[index];
    if (!entry) return;
    $selectedCommitOid = entry.commit.oid;
    centerRow(index);
  }

  /**
   * Reveal a commit given its row in the full graph, paging history in up
   * to it first. Falls back to a lookup by OID if the graph changed.
   */
  async function revealGraphCommit(oid: string, index: number): Promise<boolean> {
    const path = repoPath;
    if (!path) return false;
    await ensureGraphLoaded(index + 1);
    if (get(activeRepoPath) !== path) return false;
    let list = get(commitGraph)?.entries ?? [];
    let at = list[index]?.commit.oid === oid ? index : list.findIndex((e) => e.commit.oid === oid);
    if (at < 0) {
      // History moved since the index was computed: reload and look again.
      await loadGraph(path, index + 1);
      list = get(commitGraph)?.entries ?? [];
      at = list.findIndex((e) => e.commit.oid === oid);
    }
    if (at < 0 || filterMode) return false;
    $selectedCommitOid = oid;
    // Wait for the rows to render at the new size before scrolling.
    await tick();
    centerRow(at);
    return true;
  }

  // Scroll to commits revealed from elsewhere (tags list, undo history).
  let handledReveal = 0;
  $effect(() => {
    const req = $revealRequest;
    if (!req || req.seq === handledReveal || !graph) return;
    handledReveal = req.seq;
    const idx = entries.findIndex((en) => en.commit.oid === req.oid);
    if (idx >= 0) centerRow(idx);
  });

  // ── Search navigation ───────────────────────────────────────────────

  function stepMatch(dir: 1 | -1) {
    const matches = $searchResult?.matches ?? [];
    if (matches.length === 0) return;
    let next: number;
    if ($currentMatch >= 0) {
      next = ($currentMatch + dir + matches.length) % matches.length;
    } else if (filterMode) {
      const sel = matches.findIndex((m) => m.commit.oid === selected);
      next = sel < 0 ? (dir === 1 ? 0 : matches.length - 1) : (sel + dir + matches.length) % matches.length;
    } else {
      // Start from the selected row so "next" continues from where the user is.
      const selRow = graph?.entries.findIndex((e) => e.commit.oid === selected) ?? -1;
      if (dir === 1) {
        const i = matches.findIndex((m) => m.index > selRow);
        next = i < 0 ? 0 : i;
      } else {
        let i = -1;
        for (let k = matches.length - 1; k >= 0; k--) {
          if (selRow < 0 || matches[k].index < selRow) {
            i = k;
            break;
          }
        }
        next = i < 0 ? matches.length - 1 : i;
      }
    }
    currentMatch.set(next);
    const m = matches[next];
    if (filterMode) {
      revealRow(next);
    } else {
      revealGraphCommit(m.commit.oid, m.index);
    }
  }

  function openSearch(mode?: "highlight" | "filter") {
    if (mode) searchMode.set(mode);
    searchOpen.set(true);
    searchFocusToken++;
  }

  let searchFocusToken = $state(0);
  let gotoOpen = $state(false);
  let optionsOpen = $state(false);

  // ── Jump to HEAD / ref ──────────────────────────────────────────────

  async function gotoRev(rev: string) {
    const path = repoPath;
    if (!path) return;
    try {
      const loc = await tauri.locateCommit(path, rev, graphOptions());
      if (get(activeRepoPath) !== path) return;
      if (loc.index === null) {
        toast("warning", `${rev} is not shown in the graph with the current view options.`);
        return;
      }
      if (filterMode) {
        // Leave filter mode so the commit can be shown in context.
        searchMode.set("highlight");
      }
      const ok = await revealGraphCommit(loc.oid, loc.index);
      if (!ok) toast("warning", `Could not show ${rev} in the graph.`);
      containerEl?.focus({ preventScroll: true });
    } catch (err) {
      toastError(`Can't go to ${rev}`, err);
    }
  }

  function jumpToHead() {
    gotoRev("HEAD");
  }

  // ── Keyboard navigation ─────────────────────────────────────────────

  function handleKeydown(e: KeyboardEvent) {
    // Ctrl+F (or "/") while the commit list has focus opens search.
    if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "f") {
      e.preventDefault();
      openSearch();
      return;
    }
    if (e.key === "/" && !e.ctrlKey && !e.altKey && !e.metaKey) {
      e.preventDefault();
      openSearch();
      return;
    }
    if (e.key === "F3" && $searchActive) {
      e.preventDefault();
      stepMatch(e.shiftKey ? -1 : 1);
      return;
    }
    if (entries.length === 0) return;
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    const current = selected && selected !== "__wip__"
      ? entries.findIndex((en) => en.commit.oid === selected)
      : -1;
    const pageRows = Math.max(1, Math.floor((viewportHeight || 600) / ROW_HEIGHT) - 1);

    let next: number | null = null;
    switch (e.key) {
      case "ArrowDown":
      case "j":
        next = current < 0 ? 0 : Math.min(entries.length - 1, current + 1);
        break;
      case "ArrowUp":
      case "k":
        next = current < 0 ? 0 : Math.max(0, current - 1);
        break;
      case "PageDown":
        next = Math.min(entries.length - 1, Math.max(0, current) + pageRows);
        break;
      case "PageUp":
        next = Math.max(0, current - pageRows);
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = entries.length - 1;
        break;
      case "Escape":
        if (selected) {
          e.preventDefault();
          $selectedCommitOid = null;
        }
        return;
      default:
        return;
    }
    e.preventDefault();
    $selectedCommitOid = entries[next].commit.oid;
    scrollRowIntoView(next);
  }

  // ── Loading: repo switch and view-option changes ───────────────────
  let lastLoadedPath: string | null = null;
  let lastOptionsKey = "";
  $effect(() => {
    const path = repoPath;
    const optionsKey = `${s.graph_hide_remotes}|${s.graph_current_branch_only}`;
    if (!path) {
      lastLoadedPath = null;
      return;
    }
    if (path !== lastLoadedPath) {
      lastLoadedPath = path;
      lastOptionsKey = optionsKey;
      autoLoadBlockedAt = -1;
      // New repo: start at the top instead of keeping the old scroll offset.
      scrollTop = 0;
      if (containerEl) containerEl.scrollTop = 0;
      loadGraph(path);
    } else if (optionsKey !== lastOptionsKey) {
      lastOptionsKey = optionsKey;
      autoLoadBlockedAt = -1;
      loadGraph(path);
    }
  });

  // Entering/leaving filter mode swaps the list: start at the top.
  let lastFilterMode = false;
  $effect(() => {
    const fm = filterMode;
    if (fm === lastFilterMode) return;
    lastFilterMode = fm;
    if (containerEl) containerEl.scrollTop = 0;
    scrollTop = 0;
  });

  onMount(() => {
    const unsubs = [
      onAction("graph_search", () => openSearch("highlight")),
      onAction("graph_filter", () => openSearch("filter")),
      onAction("graph_jump_head", jumpToHead),
      onAction("graph_goto_ref", () => {
        optionsOpen = false;
        gotoOpen = true;
      }),
      onAction("graph_view_options", () => {
        gotoOpen = false;
        optionsOpen = true;
      }),
      onAction("graph_load_all", () => {
        autoLoadBlockedAt = -1;
        loadEntireGraph();
      }),
      onAction("graph_toggle_remotes", () =>
        updateSettings({ graph_hide_remotes: !get(settings).graph_hide_remotes }),
      ),
      onAction("graph_toggle_current_branch", () =>
        updateSettings({ graph_current_branch_only: !get(settings).graph_current_branch_only }),
      ),
    ];
    return () => unsubs.forEach((u) => u());
  });

  function searchMatchState(oid: string): "none" | "match" | "current" {
    if (!highlighting) return "none";
    if (oid === currentMatchOid) return "current";
    return $matchOids.has(oid) ? "match" : "none";
  }
  // File history / blame "Show in graph": scroll the requested commit into view.
  $effect(() => {
    const oid = $revealCommit;
    if (!oid || !graph) return;
    const idx = entries.findIndex((en) => en.commit.oid === oid);
    $revealCommit = null;
    if (idx >= 0) centerRow(idx);
  });
</script>

<div
  class="graph-view"
  style="--graph-col-author: {colWidths.author}px; --graph-col-sha: {colWidths.sha}px; --graph-col-date: {colWidths.date}px;"
>
  <GraphToolbar
    loadedCount={graph?.entries.length ?? 0}
    hasMore={graph?.has_more ?? false}
    {loadingMore}
    refs={allRefs}
    focusToken={searchFocusToken}
    bind:gotoOpen
    bind:optionsOpen
    onnext={() => stepMatch(1)}
    onprev={() => stepMatch(-1)}
    onjumphead={jumpToHead}
    ongoto={gotoRev}
    onloadall={() => {
      autoLoadBlockedAt = -1;
      loadEntireGraph();
    }}
    onexit={() => containerEl?.focus({ preventScroll: true })}
  />

  {#if graph && (graph.entries.length > 0 || filterMode)}
    <GraphColumnHeader
      {graphWidth}
      showAuthor={cols.author}
      showSha={cols.sha}
      showDate={cols.date}
      widths={colWidths}
      rightInset={scrollbarWidth}
      onresize={(col, w) => (dragWidths = { ...dragWidths, [col]: w })}
      onresizeend={(col, w) => {
        updateSettings({ [WIDTH_KEYS[col]]: w });
        const { [col]: _, ...rest } = dragWidths;
        dragWidths = rest;
      }}
      onreset={(col) => updateSettings({ [WIDTH_KEYS[col]]: DEFAULT_COL_WIDTHS[col] })}
    />
  {/if}

  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="commit-graph"
    bind:this={containerEl}
    bind:clientHeight={viewportHeight}
    bind:clientWidth={containerClientWidth}
    bind:offsetWidth={containerOffsetWidth}
    onscroll={handleScroll}
    onkeydown={handleKeydown}
    tabindex="0"
    role="region"
    aria-label={filterMode ? "Matching commits" : "Commit history"}
  >
    {#if loading && !graph}
      <div class="loading">
        <Loader2 size={24} class="spinner" />
        <span>Loading commits...</span>
      </div>
    {:else if filterMode && entries.length === 0}
      <div class="empty">No commits match “{$searchQuery.trim()}”.</div>
    {:else if graph && entries.length > 0}
      {@render wipRow()}
      <div class="virtual-scroll" style="height: {totalHeight}px; position: relative;">
        <div
          class="virtual-window"
          style="transform: translateY({visibleStart * ROW_HEIGHT}px); position: absolute; left: 0; right: 0;"
        >
          {#each visibleEntries as entry (entry.commit.oid)}
            {@const isUnpushed = unpushedSet.has(entry.commit.oid)}
            {@const entryRefs = graph.refs[entry.commit.oid] ?? []}
            {@const matchState = searchMatchState(entry.commit.oid)}
            <div
              class="commit-row-wrapper"
              class:dimmed={highlighting && matchState === "none"}
              style="height: {ROW_HEIGHT}px;"
            >
              <GraphCanvas
                {entry}
                {totalLanes}
                height={ROW_HEIGHT}
                {isUnpushed}
              />
              <CommitRow
                {entry}
                {isUnpushed}
                refs={entryRefs}
                isSelected={selected === entry.commit.oid}
                onSelect={() => selectCommit(entry.commit.oid)}
                oncontextmenu={(e) => openCommitMenu(e, entry.commit.oid)}
                showAuthor={cols.author}
                showSha={cols.sha}
                showDate={cols.date}
                dateFormat={s.graph_date_format}
                now={$now}
                avatarSize={AVATAR}
                searchMatch={matchState}
              />
            </div>
          {/each}
        </div>
      </div>
      {#if filterMode}
        {#if $searchResult?.truncated}
          <div class="list-footer">
            Showing the first {entries.length.toLocaleString()} matches. Refine the search to narrow it down.
          </div>
        {/if}
      {:else if graph.has_more}
        <div class="list-footer" style="height: {ROW_HEIGHT}px;">
          {#if loadingMore}
            <Loader2 size={13} class="spinner" />
            <span>Loading more…</span>
          {:else}
            <button class="load-more" onclick={loadMoreManually}>Load more commits</button>
          {/if}
        </div>
      {/if}
    {:else if graph}
      {@render wipRow()}
      <div class="empty">No commits yet.</div>
    {:else if !loading && repoPath}
      <div class="empty">Could not load commit history.</div>
    {/if}
  </div>
</div>

<CommitContextMenu />

{#snippet wipRow()}
    {#if hasWip}
      <button
        class="wip-row"
        class:selected={wipSelected}
        style="height: {ROW_HEIGHT}px;"
        onclick={selectWip}
      >
        <div class="wip-icon">
          <Pencil size={12} />
        </div>
        <span class="wip-label">Uncommitted changes</span>
        <span class="wip-counts">
          {#if status.staged.length > 0}
            <span class="wip-staged">{status.staged.length} staged</span>
          {/if}
          {#if status.unstaged.length > 0}
            <span class="wip-unstaged">{status.unstaged.length} modified</span>
          {/if}
        </span>
      </button>
    {/if}
{/snippet}

<style>
  .graph-view {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .commit-graph {
    flex: 1;
    min-height: 0;
    outline: none;
    overflow-y: auto;
    overflow-x: hidden;
    background: var(--color-bg);
  }

  .loading {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 24px;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .graph-view :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .wip-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 0 12px;
    border: none;
    border-bottom: 1px solid var(--color-border);
    background: color-mix(in srgb, var(--color-lane-2) 6%, transparent);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
    text-align: left;
    flex-shrink: 0;
    transition: background 0.1s;
  }

  .wip-row:hover {
    background: color-mix(in srgb, var(--color-lane-2) 12%, transparent);
  }

  .wip-row.selected {
    background: color-mix(in srgb, var(--color-lane-2) 15%, transparent);
    border-left: 2px solid var(--color-lane-2);
  }

  .wip-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    border: 1.5px dashed var(--color-lane-2);
    color: var(--color-lane-2);
    flex-shrink: 0;
  }

  .wip-label {
    font-weight: 500;
    color: var(--color-lane-2);
  }

  .wip-counts {
    display: flex;
    gap: 8px;
    margin-left: auto;
    font-size: 11px;
  }

  .wip-staged {
    color: var(--color-diff-add-text);
  }

  .wip-unstaged {
    color: var(--color-text-muted);
  }

  .commit-row-wrapper {
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--color-border);
    transition: opacity 0.1s;
  }

  .commit-row-wrapper.dimmed {
    opacity: 0.4;
  }

  .commit-row-wrapper.dimmed:hover {
    opacity: 0.75;
  }

  .list-footer {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    min-height: 34px;
    padding: 0 12px;
    color: var(--color-text-muted);
    font-size: 12px;
  }

  .load-more {
    padding: 4px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
  }

  .load-more:hover {
    border-color: var(--color-accent);
  }

  .empty {
    padding: 24px;
    color: var(--color-text-muted);
    font-size: 13px;
  }
</style>
