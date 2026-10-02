<script lang="ts">
  import { ExternalLink, GitBranch, Loader2, Download, ChevronRight, ChevronDown, RefreshCw } from "lucide-svelte";
  import { open as openUrl } from "@tauri-apps/plugin-shell";
  import { untrack } from "svelte";
  import DiffHunk from "../diff/DiffHunk.svelte";
  import CiBadge from "./CiBadge.svelte";
  import * as tauri from "../../lib/tauri";
  import { renderMarkdown } from "../../lib/markdown";
  import { ciStatusFor, refreshCi } from "../../lib/stores/ci";
  import { diffViewMode } from "../../lib/stores/ui";
  import { settings } from "../../lib/stores/settings";
  import { refreshAll } from "../../lib/stores/graph";
  import { toast, toastError } from "../../lib/stores/toasts";
  import type { HostedRemote, PrDetail, PrFile, PrSummary } from "../../lib/types/hosting";
  import { relativeTime, reviewLabel, stateLabel } from "./prFormat";

  interface Props {
    repoPath: string;
    remote: HostedRemote;
    summary: PrSummary;
  }

  let { repoPath, remote, summary }: Props = $props();

  type Tab = "description" | "files" | "checks";
  let tab = $state<Tab>("description");

  let detail = $state<PrDetail | null>(null);
  let detailError = $state("");
  let loadingDetail = $state(false);

  let files = $state<PrFile[] | null>(null);
  let filesError = $state("");
  let loadingFiles = $state(false);
  let collapsed = $state<Set<string>>(new Set());

  let checkingOut = $state(false);

  // Sequence guard: switching PRs quickly must not show stale results.
  let seq = 0;

  // Re-load only when a different PR (or remote) is selected — not when the
  // list refreshes and hands us a new object for the same PR.
  const prKey = $derived(`${remote.remote_name}#${summary.number}`);
  $effect(() => {
    void prKey;
    untrack(() => reset());
  });

  function reset() {
    seq++;
    detail = null;
    detailError = "";
    files = null;
    filesError = "";
    collapsed = new Set();
    tab = "description";
    loadDetail();
  }

  async function loadDetail() {
    const my = seq;
    loadingDetail = true;
    try {
      const d = await tauri.hostingGetPr(repoPath, remote.remote_name, summary.number);
      if (my !== seq) return;
      detail = d;
    } catch (err) {
      if (my !== seq) return;
      detailError = String(err);
    } finally {
      if (my === seq) loadingDetail = false;
    }
  }

  async function loadFiles() {
    if (files || loadingFiles) return;
    const my = seq;
    loadingFiles = true;
    filesError = "";
    try {
      const f = await tauri.hostingPrFiles(repoPath, remote.remote_name, summary.number);
      if (my !== seq) return;
      files = f;
      // Big PRs start collapsed so the view stays responsive.
      if (f.length > 25) collapsed = new Set(f.map((x) => x.path));
    } catch (err) {
      if (my !== seq) return;
      filesError = String(err);
    } finally {
      if (my === seq) loadingFiles = false;
    }
  }

  function selectTab(t: Tab) {
    tab = t;
    if (t === "files") loadFiles();
  }

  function toggleFile(path: string) {
    const next = new Set(collapsed);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    collapsed = next;
  }

  async function checkout() {
    if (checkingOut) return;
    checkingOut = true;
    try {
      const res = await tauri.hostingCheckoutPr(repoPath, remote.remote_name, summary.number);
      refreshAll(repoPath);
      if (res.success) toast("success", res.message, { title: `#${summary.number}` });
      else toast("error", res.message, { title: "Checkout failed" });
    } catch (err) {
      toastError("Checkout failed", err);
    } finally {
      checkingOut = false;
    }
  }

  // Links in rendered markdown open in the system browser, never in the webview.
  function handleBodyClick(e: MouseEvent) {
    const a = (e.target as HTMLElement | null)?.closest("a");
    if (!a) return;
    e.preventDefault();
    const href = a.getAttribute("href");
    if (href && /^(https?:|mailto:)/i.test(href)) openUrl(href);
  }

  const shown = $derived(detail?.summary ?? summary);
  const bodyHtml = $derived(detail ? renderMarkdown(detail.body) : "");
  const ci = $derived(ciStatusFor(summary.head_sha, repoPath, remote.remote_name));
  const checks = $derived($ci.status?.checks ?? []);
  const reviewText = $derived(reviewLabel(summary.review_state));
  const prWord = $derived(remote.provider === "gitlab" ? "merge request" : "pull request");
  const prSign = $derived(remote.provider === "gitlab" ? "!" : "#");

  const CHECK_ICON: Record<string, string> = { success: "✓", failure: "✗", pending: "●", neutral: "–" };
</script>

<div class="pr-view">
  <header class="pr-header">
    <div class="title-row">
      <h3 class="pr-title">{shown.title} <span class="pr-num">{prSign}{shown.number}</span></h3>
    </div>
    <div class="meta-row">
      <span class="state-pill {shown.draft && shown.state === 'open' ? 'draft' : shown.state}">
        {shown.draft && shown.state === "open" ? "Draft" : stateLabel(shown.state)}
      </span>
      <span class="meta">
        <strong>{shown.author}</strong> wants to merge
        <code class="ref">{shown.head_repo ? `${shown.head_repo}:` : ""}{shown.head_ref}</code>
        into <code class="ref">{shown.base_ref}</code>
      </span>
    </div>
    <div class="meta-row secondary">
      <span>opened {relativeTime(shown.created_at)}</span>
      <span>· updated {relativeTime(shown.updated_at)}</span>
      {#if reviewText}
        <span class="review {summary.review_state}">· {reviewText}</span>
      {/if}
      {#if detail?.mergeable_state}
        <span>· merge state: {detail.mergeable_state}</span>
      {/if}
      {#if detail?.additions != null}
        <span>· <span class="add">+{detail.additions}</span> <span class="del">−{detail.deletions ?? 0}</span></span>
      {/if}
    </div>
    {#if shown.labels.length}
      <div class="labels">
        {#each shown.labels as l (l.name)}
          <span class="label" style={l.color ? `--label-color: #${l.color}` : ""}>{l.name}</span>
        {/each}
      </div>
    {/if}
    <div class="actions">
      <button class="btn" onclick={checkout} disabled={checkingOut} title="Fetch the {prWord} head and check it out as pr/{shown.number}">
        {#if checkingOut}<Loader2 size={13} class="spinner" />{:else}<Download size={13} />{/if}
        <span>Checkout</span>
      </button>
      <button class="btn" onclick={() => openUrl(shown.html_url)} title="Open in browser">
        <ExternalLink size={13} />
        <span>Open in browser</span>
      </button>
    </div>
  </header>

  <div class="tabs" role="tablist">
    <button class="tab" class:active={tab === "description"} role="tab" aria-selected={tab === "description"} onclick={() => selectTab("description")}>Description</button>
    <button class="tab" class:active={tab === "files"} role="tab" aria-selected={tab === "files"} onclick={() => selectTab("files")}>
      Files{#if detail?.changed_files != null}&nbsp;<span class="count">{detail.changed_files}</span>{/if}
    </button>
    <button class="tab" class:active={tab === "checks"} role="tab" aria-selected={tab === "checks"} onclick={() => selectTab("checks")}>
      Checks <CiBadge sha={summary.head_sha} {repoPath} remote={remote.remote_name} />
    </button>
  </div>

  <div class="tab-body">
    {#if tab === "description"}
      {#if loadingDetail}
        <div class="placeholder"><Loader2 size={16} class="spinner" /> Loading…</div>
      {:else if detailError}
        <div class="error">{detailError}</div>
      {:else if detail && detail.body.trim()}
        <!-- Safe: renderMarkdown escapes all input and emits only whitelisted markup. -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="markdown" onclick={handleBodyClick}>{@html bodyHtml}</div>
      {:else if detail}
        <div class="placeholder">No description provided.</div>
      {/if}
    {:else if tab === "files"}
      {#if loadingFiles}
        <div class="placeholder"><Loader2 size={16} class="spinner" /> Loading changed files…</div>
      {:else if filesError}
        <div class="error">{filesError}</div>
      {:else if files && files.length === 0}
        <div class="placeholder">No changed files.</div>
      {:else if files}
        {#each files as f (f.path + (f.old_path ?? ""))}
          <div class="file">
            <button class="file-header" onclick={() => toggleFile(f.path)} aria-expanded={!collapsed.has(f.path)}>
              {#if collapsed.has(f.path)}<ChevronRight size={13} />{:else}<ChevronDown size={13} />{/if}
              <span class="file-status {f.status}">{f.status[0].toUpperCase()}</span>
              <span class="file-path" title={f.path}>
                {#if f.old_path}<span class="old-path">{f.old_path} → </span>{/if}{f.path}
              </span>
              <span class="file-stats"><span class="add">+{f.additions}</span> <span class="del">−{f.deletions}</span></span>
            </button>
            {#if !collapsed.has(f.path)}
              <div class="file-diff">
                {#if f.patch_missing}
                  <div class="placeholder small">Diff not available (binary or too large).</div>
                {:else if f.hunks.length === 0}
                  <div class="placeholder small">No textual changes.</div>
                {:else}
                  {#each f.hunks as hunk, hi (hi)}
                    <DiffHunk {hunk} mode={$diffViewMode} tabSize={$settings.tab_size} wrap={$settings.word_wrap_in_diffs} />
                  {/each}
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      {/if}
    {:else}
      <div class="checks-head">
        <span class="muted">Head <code>{summary.head_sha.slice(0, 8)}</code></span>
        <button class="icon-btn" title="Refresh checks" aria-label="Refresh checks" onclick={() => refreshCi(summary.head_sha, repoPath, remote.remote_name, true)}>
          <RefreshCw size={13} />
        </button>
      </div>
      {#if $ci.loading && !$ci.status}
        <div class="placeholder"><Loader2 size={16} class="spinner" /> Loading checks…</div>
      {:else if $ci.error}
        <div class="error">{$ci.error}</div>
      {:else if checks.length === 0}
        <div class="placeholder">No CI checks reported for this commit.</div>
      {:else}
        <ul class="checks">
          {#each checks as c, ci_i (ci_i)}
            <li class="check">
              <span class="check-icon {c.state}">{CHECK_ICON[c.state]}</span>
              <span class="check-name">{c.name}</span>
              {#if c.description}<span class="check-desc">{c.description}</span>{/if}
              {#if c.url}
                {@const url = c.url}
                <button class="icon-btn" title="Open details" aria-label="Open details for {c.name}" onclick={() => openUrl(url)}>
                  <ExternalLink size={12} />
                </button>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
  </div>
  <div class="footer-hint"><GitBranch size={11} /> {remote.project_path} via <code>{remote.remote_name}</code></div>
</div>

<style>
  .pr-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-width: 0;
  }

  .pr-header {
    padding: 0 0 12px;
    border-bottom: 1px solid var(--color-border);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .pr-title {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: var(--color-text-primary);
    word-break: break-word;
  }

  .pr-num {
    color: var(--color-text-muted);
    font-weight: 400;
  }

  .meta-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--color-text-primary);
  }

  .meta-row.secondary {
    gap: 4px;
    color: var(--color-text-muted);
    font-size: 11px;
  }

  .ref {
    font-family: var(--font-mono);
    font-size: 11px;
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--color-surface-elevated);
    color: var(--color-accent);
  }

  .state-pill {
    font-size: 11px;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 10px;
    border: 1px solid currentColor;
  }

  .state-pill.open {
    color: var(--color-diff-add-text);
  }

  .state-pill.closed {
    color: var(--color-diff-del-text);
  }

  .state-pill.merged {
    color: var(--color-accent-secondary);
  }

  .state-pill.draft {
    color: var(--color-text-muted);
  }

  .review.approved {
    color: var(--color-diff-add-text);
  }

  .review.changes_requested {
    color: var(--color-diff-del-text);
  }

  .add {
    color: var(--color-diff-add-text);
  }

  .del {
    color: var(--color-diff-del-text);
  }

  .labels {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .label {
    --label-color: var(--color-text-muted);
    font-size: 10px;
    padding: 1px 7px;
    border-radius: 10px;
    border: 1px solid var(--label-color);
    color: var(--color-text-primary);
  }

  .actions {
    display: flex;
    gap: 8px;
    margin-top: 4px;
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

  .btn:hover:not(:disabled) {
    border-color: var(--color-accent);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .tabs {
    display: flex;
    gap: 2px;
    border-bottom: 1px solid var(--color-border);
    padding-top: 8px;
  }

  .tab {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 6px 12px;
    border: none;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 12px;
    cursor: pointer;
  }

  .tab:hover {
    color: var(--color-text-primary);
  }

  .tab.active {
    color: var(--color-text-primary);
    border-bottom-color: var(--color-accent);
  }

  .count {
    font-size: 10px;
    padding: 0 5px;
    border-radius: 8px;
    background: var(--color-surface-elevated);
  }

  .tab-body {
    flex: 1;
    overflow-y: auto;
    padding: 12px 0;
    min-height: 0;
  }

  .placeholder {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 16px 4px;
    color: var(--color-text-muted);
    font-size: 12px;
  }

  .placeholder.small {
    padding: 8px 12px;
  }

  .error {
    padding: 8px 12px;
    border-radius: 4px;
    background: var(--color-diff-del-bg);
    color: var(--color-diff-del-text);
    font-size: 12px;
    word-break: break-word;
  }

  .markdown {
    font-size: 13px;
    line-height: 1.55;
    color: var(--color-text-primary);
    word-wrap: break-word;
  }

  .markdown :global(h1),
  .markdown :global(h2),
  .markdown :global(h3),
  .markdown :global(h4),
  .markdown :global(h5),
  .markdown :global(h6) {
    margin: 14px 0 6px;
    font-weight: 600;
    line-height: 1.3;
  }

  .markdown :global(h1) { font-size: 17px; }
  .markdown :global(h2) { font-size: 15px; }
  .markdown :global(h3) { font-size: 14px; }
  .markdown :global(h4),
  .markdown :global(h5),
  .markdown :global(h6) { font-size: 13px; }

  .markdown :global(p) {
    margin: 0 0 10px;
  }

  .markdown :global(ul),
  .markdown :global(ol) {
    margin: 0 0 10px;
    padding-left: 22px;
  }

  .markdown :global(li.md-task) {
    list-style: none;
    margin-left: -18px;
  }

  .markdown :global(.md-check) {
    color: var(--color-text-muted);
  }

  .markdown :global(.md-check.done) {
    color: var(--color-diff-add-text);
  }

  .markdown :global(code) {
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 1px 4px;
    border-radius: 3px;
    background: var(--color-surface-elevated);
  }

  .markdown :global(pre) {
    margin: 0 0 10px;
    padding: 10px 12px;
    border-radius: 6px;
    background: var(--color-bg);
    border: 1px solid var(--color-border);
    overflow-x: auto;
  }

  .markdown :global(pre code) {
    padding: 0;
    background: none;
  }

  .markdown :global(blockquote) {
    margin: 0 0 10px;
    padding: 0 12px;
    border-left: 3px solid var(--color-border);
    color: var(--color-text-muted);
  }

  .markdown :global(hr) {
    border: none;
    border-top: 1px solid var(--color-border);
    margin: 12px 0;
  }

  .markdown :global(table) {
    border-collapse: collapse;
    margin: 0 0 10px;
    font-size: 12px;
  }

  .markdown :global(th),
  .markdown :global(td) {
    border: 1px solid var(--color-border);
    padding: 4px 8px;
  }

  .markdown :global(a.md-link) {
    color: var(--color-accent);
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }

  .file {
    border: 1px solid var(--color-border);
    border-radius: 6px;
    margin-bottom: 8px;
    overflow: hidden;
  }

  .file-header {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 6px 10px;
    border: none;
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .file-status {
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 700;
    width: 14px;
    text-align: center;
  }

  .file-status.added { color: var(--color-diff-add-text); }
  .file-status.deleted { color: var(--color-diff-del-text); }
  .file-status.modified { color: var(--color-lane-2); }
  .file-status.renamed { color: var(--color-accent-secondary); }

  .file-path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .old-path {
    color: var(--color-text-muted);
  }

  .file-stats {
    font-family: var(--font-mono);
    font-size: 11px;
    flex-shrink: 0;
  }

  .file-diff {
    font-size: var(--diff-font-size);
    overflow-x: auto;
    background: var(--color-bg);
  }

  .checks-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
    font-size: 11px;
  }

  .muted {
    color: var(--color-text-muted);
  }

  .muted code {
    font-family: var(--font-mono);
  }

  .checks {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 4px;
    border-bottom: 1px solid var(--color-border);
    font-size: 12px;
  }

  .check-icon {
    width: 14px;
    text-align: center;
    font-weight: 700;
  }

  .check-icon.success { color: var(--color-diff-add-text); }
  .check-icon.failure { color: var(--color-diff-del-text); }
  .check-icon.pending { color: var(--color-lane-2); }
  .check-icon.neutral { color: var(--color-text-muted); }

  .check-name {
    color: var(--color-text-primary);
    font-weight: 500;
  }

  .check-desc {
    flex: 1;
    min-width: 0;
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    margin-left: auto;
    width: 24px;
    height: 24px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
  }

  .icon-btn:hover {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .footer-hint {
    display: flex;
    align-items: center;
    gap: 4px;
    padding-top: 6px;
    border-top: 1px solid var(--color-border);
    font-size: 10px;
    color: var(--color-text-muted);
  }

  .footer-hint code {
    font-family: var(--font-mono);
  }

  :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
