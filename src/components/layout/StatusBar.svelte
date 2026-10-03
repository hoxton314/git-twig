<script lang="ts">
  import { onMount } from "svelte";
  import { GitBranch, GitCommitHorizontal, ArrowUp, ArrowDown, Loader2, AlertTriangle, CloudDownload, Cloud } from "lucide-svelte";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { branches, workingStatus } from "../../lib/stores/graph";
  import { operations, lastFetch } from "../../lib/stores/operations";
  import * as tauri from "../../lib/tauri";
  import type { RepoStatusSummary, RepoStateKind } from "../../lib/types/git";
  import { t, type MessageKey } from "../../lib/i18n";

  interface Props {
    version?: string;
  }
  let { version = "" }: Props = $props();

  let summary = $state<RepoStatusSummary | null>(null);
  let now = $state(Date.now());

  const repoPath = $derived($activeRepoPath);
  const repoOps = $derived($operations.filter((op) => op.repoPath === repoPath));
  const foregroundOp = $derived(repoOps.find((op) => !op.background) ?? repoOps[0] ?? null);
  const otherOps = $derived($operations.filter((op) => op.repoPath !== repoPath).length);
  const fetchRecord = $derived(repoPath ? $lastFetch[repoPath] : undefined);

  const STATE_LABELS: Record<Exclude<RepoStateKind, "clean">, MessageKey> = {
    merge: "state.merge",
    revert: "state.revert",
    "cherry-pick": "state.cherryPick",
    bisect: "state.bisect",
    rebase: "state.rebase",
    "rebase-interactive": "state.rebaseInteractive",
    "rebase-merge": "state.rebase",
    "apply-mailbox": "state.applyMailbox",
  };
  const stateLabel = $derived(summary && summary.state !== "clean" ? $t(STATE_LABELS[summary.state]) : "");

  // Re-read the summary when the repo, its refs or its working tree change
  // (refreshAll/refreshStatus update these stores) and when an operation ends.
  let timer: ReturnType<typeof setTimeout> | null = null;
  let gen = 0;
  $effect(() => {
    const path = repoPath;
    void $branches;
    void $workingStatus;
    void repoOps.length;
    if (timer) clearTimeout(timer);
    if (!path) {
      summary = null;
      return;
    }
    const myGen = ++gen;
    timer = setTimeout(() => {
      tauri
        .getRepoStatusSummary(path)
        .then((s) => {
          if (myGen === gen) summary = s;
        })
        .catch(() => {
          if (myGen === gen) summary = null;
        });
    }, 120);
  });

  onMount(() => {
    const id = setInterval(() => (now = Date.now()), 30_000);
    return () => {
      clearInterval(id);
      if (timer) clearTimeout(timer);
    };
  });

  function relative(ts: number, current: number): string {
    const s = Math.max(0, Math.round((current - ts) / 1000));
    if (s < 45) return $t("time.justNow");
    const m = Math.round(s / 60);
    if (m < 60) return $t("time.minutesAgo", { count: m });
    const h = Math.round(m / 60);
    if (h < 24) return $t("time.hoursAgo", { count: h });
    return $t("time.daysAgo", { count: Math.round(h / 24) });
  }

  function elapsed(startedAt: number, current: number): string {
    const s = Math.round((current - startedAt) / 1000);
    return s >= 5 ? ` ${s}s` : "";
  }

  // Tick every second while something runs, for the elapsed counter.
  $effect(() => {
    if (!foregroundOp) return;
    const id = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(id);
  });
</script>

<footer class="status-bar" aria-label={$t("status.label")}>
  {#if summary}
    <span class="item" title={summary.detached ? $t("status.detachedTitle") : $t("status.branchTitle")}>
      {#if summary.detached}
        <GitCommitHorizontal size={13} aria-hidden="true" />
        <span>{$t("status.detachedAt")} <span class="mono">{summary.head_short_oid}</span></span>
      {:else}
        <GitBranch size={13} aria-hidden="true" />
        <span class="mono">{summary.branch ?? "HEAD"}</span>
        {#if summary.unborn}<span class="muted">{$t("status.noCommits")}</span>{/if}
      {/if}
    </span>

    {#if !summary.detached && !summary.unborn}
      {#if summary.upstream}
        <span
          class="item"
          title={$t("status.aheadBehindTitle", { ahead: summary.ahead, behind: summary.behind, upstream: summary.upstream })}
          aria-label={$t("status.aheadBehindLabel", { ahead: summary.ahead, behind: summary.behind, upstream: summary.upstream })}
        >
          <Cloud size={13} aria-hidden="true" />
          <span class="mono muted">{summary.upstream}</span>
          <span class="count" class:active={summary.ahead > 0}><ArrowUp size={11} aria-hidden="true" />{summary.ahead}</span>
          <span class="count" class:active={summary.behind > 0}><ArrowDown size={11} aria-hidden="true" />{summary.behind}</span>
        </span>
      {:else}
        <span class="item muted" title={$t("status.noUpstreamTitle")}>{$t("status.noUpstream")}</span>
      {/if}
    {/if}

    {#if summary.state !== "clean"}
      <span class="item state-badge" role="status" title={$t("status.stateTitle", { state: stateLabel.toLowerCase() })}>
        <AlertTriangle size={12} aria-hidden="true" />
        {stateLabel}
      </span>
    {/if}
  {/if}

  <span class="spacer"></span>

  <span class="item" aria-live="polite">
    {#if foregroundOp}
      <Loader2 size={13} class="spin" aria-hidden="true" />
      <span>{foregroundOp.label}{elapsed(foregroundOp.startedAt, now)}</span>
      {#if repoOps.length > 1}<span class="muted">+{repoOps.length - 1}</span>{/if}
    {/if}
  </span>

  {#if otherOps > 0}
    <span class="item muted" title={$t("status.otherOpsTitle")}>{$t("status.otherOps", { count: otherOps })}</span>
  {/if}

  {#if fetchRecord}
    <span
      class="item"
      class:error={!fetchRecord.ok}
      title={fetchRecord.ok
        ? $t("status.lastFetched", { time: new Date(fetchRecord.at).toLocaleString() })
        : $t("status.lastFetchFailed", { time: new Date(fetchRecord.at).toLocaleString(), error: fetchRecord.error ?? "" })}
    >
      {#if fetchRecord.ok}
        <CloudDownload size={13} aria-hidden="true" />
      {:else}
        <AlertTriangle size={13} aria-hidden="true" />
      {/if}
      <span>{fetchRecord.ok ? $t("status.fetched", { when: relative(fetchRecord.at, now) }) : $t("status.fetchFailed", { when: relative(fetchRecord.at, now) })}</span>
    </span>
  {/if}

  {#if version}
    <span class="item muted version">v{version}</span>
  {/if}
</footer>

<style>
  .status-bar {
    display: flex;
    align-items: center;
    gap: 14px;
    height: 24px;
    padding: 0 10px;
    border-top: 1px solid var(--color-border);
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 11px;
    flex-shrink: 0;
    overflow: hidden;
    white-space: nowrap;
    user-select: none;
  }

  .item {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
  }

  .item :global(svg) {
    flex-shrink: 0;
    color: var(--color-text-muted);
  }

  .mono {
    font-family: var(--font-mono);
  }

  .muted {
    color: var(--color-text-muted);
  }

  .count {
    display: inline-flex;
    align-items: center;
    color: var(--color-text-muted);
  }

  .count.active {
    color: var(--color-accent);
  }

  .count.active :global(svg) {
    color: var(--color-accent);
  }

  .state-badge {
    padding: 1px 6px;
    border-radius: 3px;
    background: var(--color-diff-hunk-bg);
    color: var(--color-lane-2);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.3px;
    font-size: 10px;
  }

  .state-badge :global(svg) {
    color: var(--color-lane-2);
  }

  .item.error,
  .item.error :global(svg) {
    color: var(--color-diff-del-text);
  }

  .spacer {
    flex: 1;
  }

  .version {
    opacity: 0.7;
  }

  .status-bar :global(.spin) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
