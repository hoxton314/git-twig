<script lang="ts">
  /**
   * Repository dashboard for a group (or every known repository): branch,
   * ahead/behind, uncommitted changes and last fetch per repository, with
   * Fetch all / Pull all (fast-forward only), run 3 at a time and
   * cancellable. Lazy-loaded from AppShell.
   */
  import { ArrowDown, ArrowUp, Loader2, RefreshCw, X } from "lucide-svelte";
  import Modal from "../shared/Modal.svelte";
  import { dashboardScope, dashboardPaths, fetchAge, runPool } from "../../lib/dashboard";
  import { repoHistory } from "../../lib/stores/repoHistory";
  import { openRepos, addRepo, activeRepoPath, updateRepo } from "../../lib/stores/repos";
  import { trackOperation, operations, isSyncing } from "../../lib/stores/operations";
  import { currentView } from "../../lib/stores/ui";
  import { refreshAll } from "../../lib/stores/graph";
  import { toast, toastError } from "../../lib/stores/toasts";
  import * as tauri from "../../lib/tauri";
  import type { RepoStatusRow } from "../../lib/types/git";
  import { t, tr } from "../../lib/i18n";

  const CONCURRENCY = 3;

  const scope = $derived($dashboardScope);
  const group = $derived(scope?.groupId ? $repoHistory.groups.find((g) => g.id === scope.groupId) : null);
  const title = $derived(group ? $t("dashboard.titleGroup", { name: group.name }) : $t("dashboard.titleAll"));
  const paths = $derived(scope ? dashboardPaths(scope.groupId, $repoHistory, [...$openRepos.keys()]) : []);

  let rows = $state<RepoStatusRow[]>([]);
  let loading = $state(false);
  /** Per-path state of a running bulk action, and its last outcome. */
  let busy = $state<Record<string, boolean>>({});
  let outcome = $state<Record<string, { ok: boolean; text: string }>>({});
  let running = $state<"fetch" | "pull" | null>(null);
  let cancelled = false;

  async function reload() {
    loading = true;
    try {
      rows = await tauri.getDashboardStatus(paths);
    } catch (err) {
      toastError(tr("dashboard.title"), err);
    } finally {
      loading = false;
    }
  }

  // (Re)load whenever the dashboard opens or its repository list changes.
  let lastKey = "";
  $effect(() => {
    const key = scope ? paths.join("\n") : "";
    if (!scope) {
      lastKey = "";
      return;
    }
    if (key === lastKey) return;
    lastKey = key;
    outcome = {};
    void reload();
  });

  async function refreshRow(path: string) {
    try {
      const [row] = await tauri.getDashboardStatus([path]);
      if (row) rows = rows.map((r) => (r.path === path ? row : r));
    } catch {
      // Keep the old row; the outcome already says what happened.
    }
    if (!$openRepos.has(path)) return;
    // Open tabs: the visible one reloads fully; others update their badge.
    if ($activeRepoPath === path) refreshAll(path);
    else tauri.getRepoInfo(path).then(updateRepo).catch(() => {});
  }

  function firstLine(text: string): string {
    return text.split("\n").map((l) => l.trim()).find(Boolean) ?? "";
  }

  async function runOne(kind: "fetch" | "pull", path: string) {
    if (isSyncing($operations, path)) {
      outcome = { ...outcome, [path]: { ok: true, text: tr("dashboard.alreadySyncing") } };
      return;
    }
    busy = { ...busy, [path]: true };
    try {
      // Tracked like any fetch/pull: the status bar shows it, auto-fetch
      // waits for it, and the last-fetch time is recorded.
      const res = await trackOperation(
        path,
        kind,
        kind === "fetch" ? tr("dashboard.fetching") : tr("dashboard.pulling"),
        () => (kind === "fetch" ? tauri.dashboardFetch(path) : tauri.dashboardPull(path)),
        { background: true },
      );
      outcome = {
        ...outcome,
        [path]: res.success
          ? { ok: true, text: kind === "fetch" ? tr("dashboard.fetched") : firstLine(res.message) || tr("dashboard.upToDate") }
          : { ok: false, text: firstLine(res.message) || (kind === "fetch" ? tr("dashboard.fetchFailed") : tr("dashboard.pullFailed")) },
      };
    } catch (err) {
      outcome = { ...outcome, [path]: { ok: false, text: err instanceof Error ? err.message : String(err) } };
    } finally {
      busy = { ...busy, [path]: false };
      await refreshRow(path);
    }
  }

  async function runAll(kind: "fetch" | "pull") {
    if (running) return;
    const targets = rows
      .filter((r) => !r.error)
      .filter((r) => kind === "fetch" || pullable(r))
      .map((r) => r.path);
    if (targets.length === 0) {
      toast("info", kind === "fetch" ? tr("dashboard.nothingToFetch") : tr("dashboard.nothingToPull"));
      return;
    }
    running = kind;
    cancelled = false;
    try {
      await runPool(targets, CONCURRENCY, (p) => runOne(kind, p), () => cancelled);
      const failed = targets.filter((p) => outcome[p] && !outcome[p].ok).length;
      const isFetch = kind === "fetch";
      const done = targets.length - failed;
      const total = targets.length;
      if (cancelled) toast("info", isFetch ? tr("dashboard.fetchStopped") : tr("dashboard.pullStopped"));
      else if (failed > 0)
        toast("warning", tr(isFetch ? "dashboard.fetchPartial" : "dashboard.pullPartial", { done, total, failed }));
      else toast("success", tr(isFetch ? "dashboard.fetchDone" : "dashboard.pullDone", { count: total }));
    } finally {
      running = null;
    }
  }

  async function openRow(path: string) {
    if ($openRepos.has(path)) {
      $activeRepoPath = path;
      $currentView = "repos";
    } else {
      try {
        addRepo(await tauri.openRepo(path));
      } catch (err) {
        toastError(tr("tabs.openFailed"), err);
        return;
      }
    }
    $dashboardScope = null;
  }

  function close() {
    cancelled = true;
    $dashboardScope = null;
  }

  /** Behind and not ahead: a fast-forward can update it (diverged ones can't). */
  function pullable(r: RepoStatusRow): boolean {
    return !r.error && !!r.upstream && !r.detached && r.behind > 0 && r.ahead === 0;
  }

  const behindCount = $derived(rows.filter(pullable).length);
</script>

<Modal open={!!scope} {title} onclose={close} width="min(980px, 95vw)">
  {#if scope}
    <div class="dash">
      <div class="toolbar">
        <button class="btn" onclick={() => runAll("fetch")} disabled={!!running || loading || rows.length === 0} title={$t("dashboard.fetchAllTitle")}>
          {#if running === "fetch"}<Loader2 size={12} class="spinner" />{/if} {$t("dashboard.fetchAll")}
        </button>
        <button class="btn" onclick={() => runAll("pull")} disabled={!!running || loading || behindCount === 0} title={$t("dashboard.pullAllTitle")}>
          {#if running === "pull"}<Loader2 size={12} class="spinner" />{/if} {$t("dashboard.pullAll", { count: behindCount })}
        </button>
        {#if running}
          <button class="btn" onclick={() => (cancelled = true)} title={$t("dashboard.stopTitle")}><X size={12} /> {$t("dashboard.stop")}</button>
        {/if}
        <span class="spacer"></span>
        <button class="icon-btn" onclick={reload} disabled={loading || !!running} title={$t("dashboard.refresh")} aria-label={$t("dashboard.refresh")}>
          <RefreshCw size={13} />
        </button>
      </div>

      {#if loading && rows.length === 0}
        <p class="muted"><Loader2 size={13} class="spinner" /> {$t("dashboard.reading", { count: paths.length })}</p>
      {:else if paths.length === 0}
        <p class="muted">{group ? $t("dashboard.groupEmpty") : $t("dashboard.empty")}</p>
      {:else}
        <table>
          <thead>
            <tr>
              <th>{$t("dashboard.colRepository")}</th><th>{$t("dashboard.colBranch")}</th><th>{$t("dashboard.colSync")}</th>
              <th>{$t("dashboard.colChanges")}</th><th>{$t("dashboard.colFetched")}</th><th>{$t("dashboard.colStatus")}</th>
            </tr>
          </thead>
          <tbody>
            {#each rows as r (r.path)}
              <tr class:error={!!r.error}>
                <td>
                  <button class="repo" onclick={() => openRow(r.path)} disabled={!!r.error} title={r.path}>
                    <span class="name">{r.name}</span>
                    {#if $openRepos.has(r.path)}<span class="tag">{$t("tabs.open")}</span>{/if}
                  </button>
                </td>
                <td class="mono">{r.error ? "" : (r.branch ?? $t("status.noCommits"))}{r.detached ? $t("dashboard.detached") : ""}</td>
                <td><span class="sync">
                  {#if r.error}
                    —
                  {:else if !r.upstream}
                    <span class="muted" title={$t("dashboard.noUpstream")}>{$t("dashboard.local")}</span>
                  {:else if r.ahead === 0 && r.behind === 0}
                    <span class="muted" title={$t("dashboard.inSync", { upstream: r.upstream })}>✓</span>
                  {:else}
                    {#if r.ahead > 0 && r.behind > 0}<span class="diverged" title={$t("dashboard.divergedTitle", { upstream: r.upstream })}>{$t("dashboard.diverged")}</span>{/if}
                    {#if r.ahead > 0}<span class="ahead" title={$t("dashboard.toPush", { count: r.ahead })}><ArrowUp size={11} />{r.ahead}</span>{/if}
                    {#if r.behind > 0}<span class="behind" title={$t("dashboard.toPull", { count: r.behind })}><ArrowDown size={11} />{r.behind}</span>{/if}
                  {/if}
                </span></td>
                <td>{r.error ? "" : r.changes === 0 ? $t("dashboard.clean") : `${r.changes}${r.changes_capped ? "+" : ""}`}</td>
                <td class="muted">{r.error ? "" : fetchAge(r.last_fetch)}</td>
                <td class="status">
                  {#if busy[r.path]}
                    <Loader2 size={12} class="spinner" />
                  {:else if r.error}
                    <span class="bad" title={r.error}>{r.error}</span>
                  {:else if outcome[r.path]}
                    <span class:bad={!outcome[r.path].ok} title={outcome[r.path].text}>{outcome[r.path].text}</span>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  {/if}
</Modal>

<style>
  .dash { display: flex; flex-direction: column; gap: 10px; font-size: 12px; min-height: 0; }
  .toolbar { display: flex; align-items: center; gap: 8px; }
  .spacer { flex: 1; }
  .btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 5px 12px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
  }
  .btn:hover:not(:disabled) { border-color: var(--color-accent); }
  .btn:disabled { opacity: 0.6; cursor: not-allowed; }
  .icon-btn { display: flex; background: none; border: none; padding: 3px; color: var(--color-text-muted); cursor: pointer; }
  .icon-btn:hover:not(:disabled) { color: var(--color-text-primary); }
  .muted { color: var(--color-text-muted); display: inline-flex; align-items: center; gap: 6px; }
  table { width: 100%; border-collapse: collapse; }
  thead th {
    position: sticky;
    top: 0;
    text-align: left;
    font-weight: 600;
    font-size: 11px;
    color: var(--color-text-muted);
    padding: 4px 8px;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-surface-elevated);
  }
  td { padding: 4px 8px; border-bottom: 1px solid var(--color-border); vertical-align: middle; white-space: nowrap; }
  tr.error td { opacity: 0.75; }
  .repo { display: flex; align-items: center; gap: 6px; background: none; border: none; padding: 0; color: var(--color-text-primary); font-size: 12px; cursor: pointer; }
  .repo:hover:not(:disabled) .name { color: var(--color-accent); text-decoration: underline; }
  .repo:disabled { cursor: default; }
  .name { font-weight: 600; }
  .tag { font-size: 10px; color: var(--color-text-muted); }
  .mono { font-family: var(--font-mono); }
  .sync { display: inline-flex; gap: 6px; }
  .ahead, .behind { display: inline-flex; align-items: center; gap: 1px; font-family: var(--font-mono); }
  .ahead { color: var(--color-diff-add-text); }
  .behind { color: var(--color-accent); }
  .diverged { color: var(--color-lane-2); font-size: 11px; }
  .status { max-width: 280px; overflow: hidden; text-overflow: ellipsis; }
  .bad { color: var(--color-diff-del-text); }
  :global(.dash .spinner) { animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
