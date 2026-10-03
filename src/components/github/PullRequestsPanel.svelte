<!--
  Pull / merge request browser for the active repo's GitHub, GitLab or
  Gitea remote. Mounted once in AppShell; opened via the `prPanelOpen`
  store, the sidebar entry, or the "open_pull_requests" action.
-->
<script lang="ts">
  import { Loader2, RefreshCw, Search, GitPullRequest, GitMerge, GitPullRequestClosed, GitPullRequestDraft, MessageSquare, Settings } from "lucide-svelte";
  import { open as openUrl } from "@tauri-apps/plugin-shell";
  import { onMount, untrack } from "svelte";
  import Modal from "../shared/Modal.svelte";
  import ContextMenu, { type MenuItem } from "../shared/ContextMenu.svelte";
  import CiBadge from "./CiBadge.svelte";
  import PullRequestView from "./PullRequestView.svelte";
  import * as tauri from "../../lib/tauri";
  import { onAction } from "../../lib/keybindings";
  import { prPanelOpen } from "../../lib/stores/pullRequests";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { currentView } from "../../lib/stores/ui";
  import { invalidateCi } from "../../lib/stores/ci";
  import { refreshAll } from "../../lib/stores/graph";
  import { toast, toastError } from "../../lib/stores/toasts";
  import type { HostedRemote, PrFilter, PrSummary } from "../../lib/types/hosting";
  import { relativeTime, reviewLabel } from "./prFormat";
  import { t, tr, type MessageKey } from "../../lib/i18n";

  let remotes = $state<HostedRemote[]>([]);
  let remoteName = $state<string | null>(null);
  let loadingRemotes = $state(false);

  let filter = $state<PrFilter>("open");
  let query = $state("");
  let items = $state<PrSummary[]>([]);
  let nextCursor = $state<string | null>(null);
  let loading = $state(false);
  let loadingMore = $state(false);
  let error = $state("");
  let selected = $state<number | null>(null);

  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  const isOpen = $derived($prPanelOpen);
  const repoPath = $derived($activeRepoPath);
  const remote = $derived(remotes.find((r) => r.remote_name === remoteName) ?? null);
  const selectedPr = $derived(items.find((p) => p.number === selected) ?? null);
  const isGitLab = $derived(remote?.provider === "gitlab");
  const sign = $derived(isGitLab ? "!" : "#");
  const title = $derived(isGitLab ? $t("prs.titleMr") : $t("prs.titlePr"));

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return items;
    const num = q.replace(/^[#!]/, "");
    return items.filter(
      (p) =>
        p.title.toLowerCase().includes(q) ||
        p.author.toLowerCase().includes(q) ||
        p.head_ref.toLowerCase().includes(q) ||
        p.base_ref.toLowerCase().includes(q) ||
        String(p.number) === num,
    );
  });

  onMount(() => {
    const unsubs = [
      onAction("open_pull_requests", () => {
        if ($activeRepoPath) prPanelOpen.set(true);
      }),
      onAction("refresh_ci_status", () => invalidateCi()),
    ];
    return () => unsubs.forEach((u) => u());
  });

  // (Re)load remotes when the panel opens or the active repo changes.
  $effect(() => {
    if (isOpen && repoPath) {
      const path = repoPath;
      untrack(() => loadRemotes(path));
    }
  });

  let remotesSeq = 0;
  async function loadRemotes(path: string) {
    const my = ++remotesSeq;
    loadingRemotes = true;
    error = "";
    try {
      const list = await tauri.hostingListRemotes(path);
      if (my !== remotesSeq) return;
      remotes = list;
      if (!list.some((r) => r.remote_name === remoteName)) {
        remoteName = list[0]?.remote_name ?? null;
      }
      if (list.length) await reload();
      else items = [];
    } catch (err) {
      if (my !== remotesSeq) return;
      error = String(err);
    } finally {
      if (my === remotesSeq) loadingRemotes = false;
    }
  }

  let listSeq = 0;
  async function reload() {
    if (!repoPath || !remoteName) return;
    const my = ++listSeq;
    loading = true;
    error = "";
    try {
      const page = await tauri.hostingListPrs(repoPath, remoteName, filter, null);
      if (my !== listSeq) return;
      items = page.items;
      nextCursor = page.next_cursor;
      if (selected !== null && !page.items.some((p) => p.number === selected)) selected = null;
      if (selected === null && page.items.length) selected = page.items[0].number;
    } catch (err) {
      if (my !== listSeq) return;
      error = String(err);
      items = [];
      nextCursor = null;
    } finally {
      if (my === listSeq) loading = false;
    }
  }

  async function loadMore() {
    if (!repoPath || !remoteName || !nextCursor || loadingMore) return;
    const my = listSeq;
    loadingMore = true;
    try {
      const page = await tauri.hostingListPrs(repoPath, remoteName, filter, nextCursor);
      if (my !== listSeq) return;
      const seen = new Set(items.map((p) => p.number));
      items = [...items, ...page.items.filter((p) => !seen.has(p.number))];
      nextCursor = page.next_cursor;
    } catch (err) {
      toastError(tr("prs.loadMoreFailed"), err);
    } finally {
      loadingMore = false;
    }
  }

  function setFilter(f: PrFilter) {
    if (filter === f) return;
    filter = f;
    selected = null;
    reload();
  }

  function setRemote(name: string) {
    remoteName = name;
    selected = null;
    reload();
  }

  function refresh() {
    if (repoPath) invalidateCi(repoPath);
    reload();
  }

  function close() {
    menu = null;
    prPanelOpen.set(false);
  }

  function openSettings() {
    close();
    currentView.set("settings");
  }

  async function checkout(pr: PrSummary) {
    if (!repoPath || !remoteName) return;
    try {
      const res = await tauri.hostingCheckoutPr(repoPath, remoteName, pr.number);
      refreshAll(repoPath);
      toast(res.success ? "success" : "error", res.message, { title: `${sign}${pr.number}` });
    } catch (err) {
      toastError(tr("prs.checkoutFailed"), err);
    }
  }

  function copy(text: string, copied: MessageKey) {
    navigator.clipboard.writeText(text).then(
      () => toast("success", tr(copied)),
      (err) => toastError(tr("prs.copyFailed"), err),
    );
  }

  function openMenu(e: MouseEvent, pr: PrSummary) {
    e.preventDefault();
    selected = pr.number;
    menu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: tr("prs.checkoutLocally"), action: () => checkout(pr) },
        { label: tr("prs.openInBrowser"), action: () => openUrl(pr.html_url) },
        { separator: true },
        { label: tr("prs.copyUrl"), action: () => copy(pr.html_url, "prs.urlCopied") },
        { label: tr("prs.copyBranchName"), action: () => copy(pr.head_ref, "prs.branchNameCopied") },
        { label: tr("prs.copyRef", { ref: `${sign}${pr.number}` }), action: () => copy(`${sign}${pr.number}`, "prs.refCopied") },
      ],
    };
  }

  function handleListKeydown(e: KeyboardEvent) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    const list = filtered;
    if (!list.length) return;
    e.preventDefault();
    const idx = list.findIndex((p) => p.number === selected);
    const next = e.key === "ArrowDown" ? Math.min(list.length - 1, idx + 1) : Math.max(0, idx - 1);
    selected = list[next].number;
    const el = (e.currentTarget as HTMLElement).querySelector<HTMLElement>(`[data-pr="${list[next].number}"]`);
    el?.focus();
    el?.scrollIntoView({ block: "nearest" });
  }

  const FILTERS = $derived<{ id: PrFilter; label: string }[]>([
    { id: "open", label: $t("prs.filterOpen") },
    { id: "closed", label: $t("prs.filterClosed") },
    { id: "merged", label: $t("prs.filterMerged") },
    { id: "all", label: $t("prs.filterAll") },
  ]);

  const EMPTY: Record<PrFilter, [MessageKey, MessageKey]> = {
    open: ["prs.noOpenPrs", "prs.noOpenMrs"],
    closed: ["prs.noClosedPrs", "prs.noClosedMrs"],
    merged: ["prs.noMergedPrs", "prs.noMergedMrs"],
    all: ["prs.noPrs", "prs.noMrs"],
  };
  const STATE_TITLE: Record<string, MessageKey> = {
    open: "prs.stateTitleOpen",
    closed: "prs.stateTitleClosed",
    merged: "prs.stateTitleMerged",
  };
</script>

<Modal open={isOpen && !!repoPath} {title} onclose={close} width="min(1180px, 96vw)">
  <div class="panel">
    <div class="toolbar">
      {#if remotes.length > 1}
        <select class="remote-select" value={remoteName} onchange={(e) => setRemote(e.currentTarget.value)} aria-label={$t("prs.remote")}>
          {#each remotes as r (r.remote_name)}
            <option value={r.remote_name}>{r.remote_name} — {r.project_path}</option>
          {/each}
        </select>
      {:else if remote}
        <span class="remote-label" title={remote.web_url}>{remote.project_path}</span>
      {/if}
      <div class="filters" role="tablist">
        {#each FILTERS as f (f.id)}
          <button class="filter" class:active={filter === f.id} role="tab" aria-selected={filter === f.id} onclick={() => setFilter(f.id)}>{f.label}</button>
        {/each}
      </div>
      <label class="search">
        <Search size={13} />
        <input type="text" placeholder={$t("prs.filterPlaceholder")} bind:value={query} aria-label={$t("prs.filterLabel")} />
      </label>
      <button class="icon-btn" onclick={refresh} title={$t("common.refresh")} aria-label={$t("common.refresh")} disabled={loading}>
        <RefreshCw size={14} class={loading ? "spinner" : ""} />
      </button>
    </div>

    {#if loadingRemotes && !remotes.length}
      <div class="empty"><Loader2 size={18} class="spinner" /> {$t("prs.detectingRemotes")}</div>
    {:else if !remotes.length && !error}
      <div class="empty column">
        <span>{$t("prs.noRemote")}</span>
        <span class="hint">{$t("prs.noRemoteHint")}</span>
        <button class="btn" onclick={openSettings}><Settings size={13} /> {$t("prs.openSettings")}</button>
      </div>
    {:else}
      <div class="split">
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <div class="list" role="listbox" tabindex="-1" aria-label={title} onkeydown={handleListKeydown}>
          {#if error}
            <div class="error">{error}</div>
          {/if}
          {#if loading && !items.length}
            <div class="empty"><Loader2 size={18} class="spinner" /> {$t("common.loading")}</div>
          {:else if !filtered.length && !error}
            <div class="empty">{query ? $t("prs.noMatches") : $t(EMPTY[filter][isGitLab ? 1 : 0])}</div>
          {/if}
          {#each filtered as pr (pr.number)}
            <button
              class="row"
              class:selected={pr.number === selected}
              data-pr={pr.number}
              role="option"
              aria-selected={pr.number === selected}
              onclick={() => (selected = pr.number)}
              oncontextmenu={(e) => openMenu(e, pr)}
            >
              <span class="state-icon {pr.draft && pr.state === 'open' ? 'draft' : pr.state}" title={pr.draft ? $t("prs.draft") : STATE_TITLE[pr.state] ? $t(STATE_TITLE[pr.state]) : pr.state}>
                {#if pr.state === "merged"}<GitMerge size={14} />
                {:else if pr.state === "closed"}<GitPullRequestClosed size={14} />
                {:else if pr.draft}<GitPullRequestDraft size={14} />
                {:else}<GitPullRequest size={14} />{/if}
              </span>
              <span class="row-main">
                <span class="row-title">{pr.title}</span>
                <span class="row-meta">
                  {sign}{pr.number} · {pr.author} · {relativeTime(pr.updated_at)}
                </span>
                <span class="row-meta branch">
                  {pr.head_repo ? `${pr.head_repo}:` : ""}{pr.head_ref} → {pr.base_ref}
                </span>
                {#if pr.labels.length}
                  <span class="row-labels">
                    {#each pr.labels.slice(0, 4) as l (l.name)}
                      <span class="label" style={l.color ? `--label-color: #${l.color}` : ""}>{l.name}</span>
                    {/each}
                  </span>
                {/if}
              </span>
              <span class="row-side">
                {#if pr.ci_state}
                  <CiBadge state={pr.ci_state} />
                {:else if pr.state === "open" && pr.head_sha}
                  <CiBadge sha={pr.head_sha} {repoPath} remote={remoteName} />
                {/if}
                {#if pr.review_state}
                  <span class="review {pr.review_state}" title={reviewLabel(pr.review_state)}>
                    {pr.review_state === "approved" ? `✓ ${$t("prs.badgeApproved")}` : pr.review_state === "changes_requested" ? `± ${$t("prs.badgeChanges")}` : $t("prs.badgeReview")}
                  </span>
                {/if}
                {#if pr.comments > 0}
                  <span class="comments" title={$t("prs.commentCount", { count: pr.comments })}><MessageSquare size={11} /> {pr.comments}</span>
                {/if}
              </span>
            </button>
          {/each}
          {#if nextCursor && !query}
            <button class="load-more" onclick={loadMore} disabled={loadingMore}>
              {#if loadingMore}<Loader2 size={13} class="spinner" />{/if}
              {$t("github.loadMore")}
            </button>
          {/if}
        </div>

        <div class="detail">
          {#if selectedPr && remote && repoPath}
            <PullRequestView {repoPath} {remote} summary={selectedPr} />
          {:else}
            <div class="empty">{isGitLab ? $t("prs.selectMr") : $t("prs.selectPr")}</div>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</Modal>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: 10px;
    height: calc(100vh - 190px);
    min-height: 360px;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  .remote-select {
    padding: 5px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 12px;
    max-width: 260px;
  }

  .remote-label {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--color-text-muted);
  }

  .filters {
    display: flex;
    border: 1px solid var(--color-border);
    border-radius: 5px;
    overflow: hidden;
  }

  .filter {
    padding: 4px 10px;
    border: none;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 12px;
    cursor: pointer;
  }

  .filter + .filter {
    border-left: 1px solid var(--color-border);
  }

  .filter.active {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .search {
    flex: 1;
    min-width: 160px;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
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
    border: none;
    background: transparent;
    color: var(--color-text-primary);
    font-size: 12px;
    outline: none;
  }

  .icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
  }

  .icon-btn:hover:not(:disabled) {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .split {
    flex: 1;
    display: flex;
    gap: 14px;
    min-height: 0;
  }

  .list {
    width: 40%;
    min-width: 280px;
    max-width: 440px;
    overflow-y: auto;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    outline: none;
  }

  .detail {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .row {
    display: flex;
    gap: 8px;
    width: 100%;
    padding: 8px 10px;
    border: none;
    border-bottom: 1px solid var(--color-border);
    background: transparent;
    color: var(--color-text-primary);
    text-align: left;
    cursor: pointer;
  }

  .row:hover {
    background: var(--color-surface-elevated);
  }

  .row.selected {
    background: var(--color-diff-hunk-bg);
    box-shadow: inset 2px 0 0 var(--color-accent);
  }

  .row:focus-visible {
    outline: 1px solid var(--color-accent);
    outline-offset: -1px;
  }

  .state-icon {
    flex-shrink: 0;
    padding-top: 1px;
  }

  .state-icon.open { color: var(--color-diff-add-text); }
  .state-icon.closed { color: var(--color-diff-del-text); }
  .state-icon.merged { color: var(--color-accent-secondary); }
  .state-icon.draft { color: var(--color-text-muted); }

  .row-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .row-title {
    font-size: 12.5px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .row-meta {
    font-size: 11px;
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-meta.branch {
    font-family: var(--font-mono);
    font-size: 10.5px;
  }

  .row-labels {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
    margin-top: 2px;
  }

  .label {
    --label-color: var(--color-text-muted);
    font-size: 9.5px;
    padding: 0 6px;
    border-radius: 8px;
    border: 1px solid var(--label-color);
    color: var(--color-text-primary);
  }

  .row-side {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
    flex-shrink: 0;
    font-size: 10.5px;
  }

  .review {
    color: var(--color-text-muted);
    white-space: nowrap;
  }

  .review.approved { color: var(--color-diff-add-text); }
  .review.changes_requested { color: var(--color-diff-del-text); }

  .comments {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--color-text-muted);
  }

  .load-more {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: 100%;
    padding: 8px;
    border: none;
    background: transparent;
    color: var(--color-accent);
    font-size: 12px;
    cursor: pointer;
  }

  .load-more:hover:not(:disabled) {
    background: var(--color-surface-elevated);
  }

  .empty {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 28px 16px;
    color: var(--color-text-muted);
    font-size: 12px;
    text-align: center;
  }

  .empty.column {
    flex-direction: column;
  }

  .hint {
    font-size: 11px;
  }

  .btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 5px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
  }

  .btn:hover {
    border-color: var(--color-accent);
  }

  .error {
    margin: 8px;
    padding: 8px 10px;
    border-radius: 4px;
    background: var(--color-diff-del-bg);
    color: var(--color-diff-del-text);
    font-size: 12px;
    word-break: break-word;
  }
</style>
