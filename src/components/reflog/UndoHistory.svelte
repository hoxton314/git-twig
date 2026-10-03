<script lang="ts">
  /**
   * Undo history: recent HEAD movements from the reflog, each restorable.
   * Opened with the "show_undo_history" action or from the commit menu.
   */
  import { onMount } from "svelte";
  import { History, Loader2, RefreshCw, RotateCcw } from "lucide-svelte";
  import Modal from "../shared/Modal.svelte";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { branches, workingStatus } from "../../lib/stores/graph";
  import { revealCommit, undoHistoryOpen } from "../../lib/stores/commitUi";
  import { onAction } from "../../lib/keybindings";
  import { confirmDestructive, restoreHeadAction } from "../../lib/commitActions";
  import * as tauri from "../../lib/tauri";
  import type { ReflogEntry } from "../../lib/types/git";
  import { t, tr } from "../../lib/i18n";

  const open = $derived($undoHistoryOpen);
  const repoPath = $derived($activeRepoPath);
  const currentBranch = $derived($branches.find((b) => !b.is_remote && b.is_head)?.name ?? null);
  const dirty = $derived($workingStatus.staged.length > 0 || $workingStatus.unstaged.length > 0);

  let entries = $state<ReflogEntry[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let autoStash = $state(true);
  let restoring = $state<number | null>(null);

  async function load() {
    const path = repoPath;
    if (!path) return;
    loading = true;
    error = null;
    try {
      const list = await tauri.getHeadReflog(path, 200);
      if ($activeRepoPath === path) entries = list;
    } catch (err) {
      error = String(err);
      entries = [];
    } finally {
      loading = false;
    }
  }

  // Load whenever the panel opens (or the repo changes while it is open).
  $effect(() => {
    if (open && repoPath) load();
  });

  onMount(() => onAction("show_undo_history", () => {
    if ($activeRepoPath) undoHistoryOpen.set(true);
  }));

  function close() {
    if (restoring === null) undoHistoryOpen.set(false);
  }

  function plan(e: ReflogEntry): string {
    if (e.checkout_branch) return $t("reflog.checkOut", { target: e.checkout_branch });
    if (currentBranch) return $t("reflog.resetTo", { branch: currentBranch, oid: e.short_new_oid });
    return $t("reflog.checkOutDetached", { oid: e.short_new_oid });
  }

  async function restore(e: ReflogEntry) {
    const path = repoPath;
    if (!path || restoring !== null) return;
    if (dirty && !autoStash) return;
    const ok = await confirmDestructive(
      tr(dirty ? "reflog.restoreConfirmDirty" : "reflog.restoreConfirm", { plan: plan(e), message: e.message }),
      tr("reflog.restoreTitle"),
      tr("reflog.restore"),
    );
    if (!ok) return;
    restoring = e.index;
    try {
      const done = await restoreHeadAction(path, e.new_oid, e.checkout_branch, autoStash);
      if (done) await load();
    } finally {
      restoring = null;
    }
  }

  function relTime(ts: number): string {
    const diff = Math.max(0, Date.now() - ts * 1000);
    const m = Math.floor(diff / 60000);
    if (m < 1) return $t("reflog.justNow");
    if (m < 60) return $t("reflog.minutesAgo", { count: m });
    const h = Math.floor(m / 60);
    if (h < 24) return $t("reflog.hoursAgo", { count: h });
    const d = Math.floor(h / 24);
    if (d < 30) return $t("reflog.daysAgo", { count: d });
    return new Date(ts * 1000).toLocaleDateString();
  }

  /** Message without the leading "action:" keyword shown as a badge. */
  function detail(e: ReflogEntry): string {
    const i = e.message.indexOf(":");
    return i >= 0 ? e.message.slice(i + 1).trim() : e.message;
  }
</script>

<Modal open={open && !!repoPath} title={$t("reflog.title")} onclose={close} width="640px">
  <div class="undo-history">
    <div class="toolbar">
      <p class="hint">
        <History size={13} />
        {$t("reflog.hint")}
      </p>
      <button class="icon-btn" onclick={load} disabled={loading} title={$t("common.refresh")} aria-label={$t("reflog.refreshHistory")}>
        <RefreshCw size={14} />
      </button>
    </div>

    {#if dirty}
      <label class="dirty">
        <input type="checkbox" bind:checked={autoStash} />
        <span>{$t("reflog.autoStash")}</span>
      </label>
    {/if}

    {#if loading && entries.length === 0}
      <div class="state"><Loader2 size={16} class="spin" /> {$t("reflog.loading")}</div>
    {:else if error}
      <div class="state error" role="alert">{error}</div>
    {:else if entries.length === 0}
      <div class="state">{$t("reflog.empty")}</div>
    {:else}
      <ul class="list">
        {#each entries as e (e.index)}
          {@const missing = e.commit_summary === null}
          <li class="entry" class:current={e.index === 0}>
            <span class="badge badge-{e.action}">{e.action || $t("reflog.other")}</span>
            <button
              class="main"
              onclick={() => {
                if (!missing && revealCommit(e.new_oid)) close();
              }}
              disabled={missing}
              title={missing ? $t("reflog.missing") : $t("reflog.showInGraph")}
            >
              <span class="msg">{detail(e)}</span>
              <span class="sub">
                <code>{e.short_new_oid}</code>
                {#if e.commit_summary}<span class="summary">{e.commit_summary}</span>{/if}
              </span>
            </button>
            <span class="time" title={new Date(e.timestamp * 1000).toLocaleString()}>{relTime(e.timestamp)}</span>
            {#if e.index === 0}
              <span class="now">{$t("reflog.current")}</span>
            {:else}
              <button
                class="restore"
                onclick={() => restore(e)}
                disabled={missing || restoring !== null || (dirty && !autoStash)}
                title={missing
                  ? $t("reflog.missing")
                  : dirty && !autoStash
                    ? $t("reflog.stashFirst")
                    : plan(e)}
              >
                {#if restoring === e.index}
                  <Loader2 size={12} class="spin" />
                {:else}
                  <RotateCcw size={12} />
                {/if}
                {$t("reflog.restore")}
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</Modal>

<style>
  .undo-history { display: flex; flex-direction: column; gap: 10px; font-size: 12px; }
  .toolbar { display: flex; align-items: flex-start; gap: 8px; }
  .hint {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    color: var(--color-text-muted);
  }
  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
  }
  .icon-btn:hover:not(:disabled) { background: var(--color-surface-elevated); color: var(--color-text-primary); }
  .dirty {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--color-lane-2) 12%, transparent);
    color: var(--color-lane-2);
    cursor: pointer;
  }
  .state { display: flex; align-items: center; gap: 8px; padding: 16px 4px; color: var(--color-text-muted); }
  .state.error { color: var(--color-diff-del-text); white-space: pre-wrap; }
  .list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; max-height: 60vh; overflow-y: auto; }
  .entry {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 4px;
    border-bottom: 1px solid var(--color-border);
  }
  .entry.current { background: color-mix(in srgb, var(--color-accent) 8%, transparent); }
  .badge {
    flex-shrink: 0;
    min-width: 72px;
    padding: 1px 6px;
    border-radius: 3px;
    font-size: 10px;
    font-weight: 600;
    text-align: center;
    background: var(--color-surface-elevated);
    color: var(--color-text-muted);
  }
  .badge-commit { background: color-mix(in srgb, var(--color-lane-1) 18%, transparent); color: var(--color-lane-1); }
  .badge-checkout { background: color-mix(in srgb, var(--color-lane-0) 18%, transparent); color: var(--color-lane-0); }
  .badge-reset { background: color-mix(in srgb, var(--color-lane-3) 18%, transparent); color: var(--color-lane-3); }
  .badge-merge, .badge-pull { background: color-mix(in srgb, var(--color-lane-4) 18%, transparent); color: var(--color-lane-4); }
  .badge-rebase, .badge-cherry-pick, .badge-revert { background: color-mix(in srgb, var(--color-lane-2) 18%, transparent); color: var(--color-lane-2); }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 0;
    border: none;
    background: none;
    color: var(--color-text-primary);
    text-align: left;
    cursor: pointer;
  }
  .main:disabled { cursor: default; opacity: 0.6; }
  .main:hover:not(:disabled) .msg { color: var(--color-accent); }
  .msg, .summary { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sub { display: flex; gap: 6px; color: var(--color-text-muted); font-size: 11px; min-width: 0; }
  code { font-family: var(--font-mono); color: var(--color-accent); flex-shrink: 0; }
  .time { flex-shrink: 0; color: var(--color-text-muted); font-size: 11px; min-width: 56px; text-align: right; }
  .now { flex-shrink: 0; width: 76px; text-align: center; color: var(--color-accent); font-size: 11px; font-weight: 600; }
  .restore {
    flex-shrink: 0;
    width: 76px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 3px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-primary);
    font-size: 11px;
    cursor: pointer;
  }
  .restore:hover:not(:disabled) { border-color: var(--color-accent); color: var(--color-accent); }
  .restore:disabled { opacity: 0.45; cursor: not-allowed; }
  .undo-history :global(.spin) { animation: spin 1s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>
