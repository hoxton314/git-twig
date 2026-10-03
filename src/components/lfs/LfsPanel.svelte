<script lang="ts">
  /**
   * Git LFS panel: tracked patterns (track / untrack), locks (lock /
   * unlock; others' locks only with force), and fetch / prune. Opened from
   * the sidebar entry or the `lfs_manage` action; mounted once in AppShell.
   */
  import { ask } from "@tauri-apps/plugin-dialog";
  import Modal from "../shared/Modal.svelte";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { lfsError, lfsPanelOpen, lfsStatus, pruneSummary, refreshLfsStatus } from "../../lib/stores/lfs";
  import { refreshAll } from "../../lib/stores/graph";
  import { toast, toastError } from "../../lib/stores/toasts";
  import * as tauri from "../../lib/tauri";
  import type { LfsLock } from "../../lib/types/git";
  import { Loader2, Lock, LockOpen, RefreshCw, Trash2 } from "lucide-svelte";
  import { t, tr } from "../../lib/i18n";

  const open = $derived($lfsPanelOpen);
  const st = $derived($lfsStatus);

  let pattern = $state("");
  let lockable = $state(false);
  let lockPath = $state("");
  let locks = $state<LfsLock[] | null>(null);
  let locksError = $state<string | null>(null);
  let busy = $state<string | null>(null);

  // Each time the panel opens: fresh status, and the server's locks.
  $effect(() => {
    if (!open) return;
    const path = $activeRepoPath;
    if (!path) return;
    void refreshLfsStatus(path);
    void loadLocks(path);
  });

  async function loadLocks(path = $activeRepoPath) {
    if (!path) return;
    locksError = null;
    try {
      const l = await tauri.getLfsLocks(path);
      if ($activeRepoPath === path) locks = l;
    } catch (err) {
      if ($activeRepoPath !== path) return;
      locks = null;
      locksError = err instanceof Error ? err.message : String(err);
    }
  }

  async function run(key: string, fn: (path: string) => Promise<void>) {
    const path = $activeRepoPath;
    if (!path || busy) return;
    busy = key;
    try {
      await fn(path);
    } catch (err) {
      toastError("Git LFS", err);
    } finally {
      busy = null;
    }
  }

  const track = () =>
    run("track", async (p) => {
      const pat = pattern.trim();
      if (!pat) return;
      await tauri.lfsTrack(p, pat, lockable);
      pattern = "";
      lockable = false;
      await refreshLfsStatus(p);
      await refreshAll(p);
      toast("success", tr("lfs.tracking", { pattern: pat }));
    });

  const untrack = (pat: string) =>
    run(`untrack:${pat}`, async (p) => {
      await tauri.lfsUntrack(p, pat);
      await refreshLfsStatus(p);
      await refreshAll(p);
      if ($lfsStatus?.patterns.some((x) => x.pattern === pat && x.source === ".gitattributes")) {
        toast("warning", tr("lfs.stillTracked", { pattern: pat }));
        return;
      }
      toast("success", tr("lfs.stoppedTracking", { pattern: pat }));
    });

  const lockFile = () =>
    run("lock", async (p) => {
      const f = lockPath.trim();
      if (!f) return;
      await tauri.lfsLock(p, f);
      lockPath = "";
      await loadLocks(p);
      toast("success", tr("lfs.locked", { path: f }));
    });

  const unlock = (l: LfsLock) =>
    run(`unlock:${l.id}`, async (p) => {
      if (!l.ours) {
        const ok = await ask(
          tr("lfs.forceUnlockConfirm", { path: l.path, owner: l.owner || tr("lfs.someoneElse") }),
          { title: tr("lfs.forceUnlock"), kind: "warning", okLabel: tr("lfs.forceUnlock") },
        );
        if (!ok) return;
      }
      await tauri.lfsUnlock(p, l.id, !l.ours);
      await loadLocks(p);
      toast("success", tr("lfs.unlocked", { path: l.path }));
    });

  const fetchObjects = (all: boolean) =>
    run(all ? "fetch-all" : "fetch", async (p) => {
      await tauri.lfsFetch(p, all);
      toast("success", all ? tr("lfs.fetchedAll") : tr("lfs.fetchedCurrent"));
    });

  const prune = () =>
    run("prune", async (p) => {
      const dry = pruneSummary(await tauri.lfsPrune(p, true));
      if (dry.count === 0) {
        toast("info", tr("lfs.nothingToPrune"));
        return;
      }
      const ok = await ask(
        dry.size
          ? tr("lfs.pruneConfirmSize", { count: dry.count, size: dry.size })
          : tr("lfs.pruneConfirm", { count: dry.count }),
        { title: tr("lfs.pruneTitle"), kind: "warning", okLabel: tr("lfs.pruneOk") },
      );
      if (!ok) return;
      await tauri.lfsPrune(p, false);
      toast("success", tr("lfs.pruned", { count: dry.count }));
    });

  function when(iso: string): string {
    const d = new Date(iso);
    return Number.isNaN(d.getTime()) ? iso : d.toLocaleString();
  }
</script>

<Modal open={open} title="Git LFS" onclose={() => lfsPanelOpen.set(false)} width="min(720px, 94vw)">
  {#if open}
    <div class="lfs">
      {#if st && !st.version}
        <p class="notice">
          {$t("lfs.notInstalledBefore")}
          <code>{"git lfs install"}</code> {$t("lfs.notInstalledAfter")}
        </p>
      {:else if !st && $lfsError}
        <p class="error">{$t("lfs.readFailed", { error: $lfsError })}</p>
        <div><button class="btn" onclick={() => refreshLfsStatus()}>{$t("common.retry")}</button></div>
      {:else if !st}
        <p class="muted"><Loader2 size={13} class="spinner" /> {$t("common.loading")}</p>
      {:else}
        <p class="muted version">{st.version}</p>

        <section>
          <h3>{$t("lfs.trackedPatterns")}</h3>
          {#if st.patterns.length === 0}
            <p class="muted">{$t("lfs.noPatterns")}</p>
          {:else}
            <ul class="rows">
              {#each st.patterns as p, i (i)}
                <li class="row">
                  <code class="grow">{p.pattern}</code>
                  {#if p.lockable}<span class="badge" title={$t("lfs.lockableTitle")}>{$t("lfs.lockableBadge")}</span>{/if}
                  <span class="muted small">{p.source}</span>
                  {#if p.source === ".gitattributes"}
                    <button class="icon-btn" onclick={() => untrack(p.pattern)} disabled={!!busy} title={$t("lfs.stopTracking", { pattern: p.pattern })} aria-label={$t("lfs.untrack", { pattern: p.pattern })}>
                      <Trash2 size={13} />
                    </button>
                  {:else}
                    <span class="icon-btn placeholder" title={$t("lfs.definedIn", { source: p.source })}></span>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
          <form class="add" onsubmit={(e) => { e.preventDefault(); track(); }}>
            <input bind:value={pattern} placeholder={$t("lfs.patternPlaceholder")} spellcheck="false" autocomplete="off" aria-label={$t("lfs.patternLabel")} />
            <label class="check" title={$t("lfs.lockableCheckTitle")}><input type="checkbox" bind:checked={lockable} /> {$t("lfs.lockable")}</label>
            <button class="btn" type="submit" disabled={!!busy || !pattern.trim()}>{$t("lfs.track")}</button>
          </form>
          <p class="muted small">{$t("lfs.trackingNoteBefore")} <code>{".gitattributes"}</code>{$t("lfs.trackingNoteAfter")}</p>
        </section>

        <section>
          <h3>
            {$t("lfs.locks")}
            <button class="icon-btn" onclick={() => loadLocks()} disabled={!!busy} title={$t("lfs.reloadLocksTitle")} aria-label={$t("lfs.reloadLocks")}><RefreshCw size={12} /></button>
          </h3>
          {#if locksError}
            <p class="error">{locksError}</p>
          {:else if locks === null}
            <p class="muted"><Loader2 size={13} class="spinner" /> {$t("lfs.loadingLocks")}</p>
          {:else if locks.length === 0}
            <p class="muted">{$t("lfs.noLocks")}</p>
          {:else}
            <ul class="rows">
              {#each locks as l (l.id)}
                <li class="row">
                  <Lock size={12} />
                  <code class="grow">{l.path}</code>
                  <span class="small" class:mine={l.ours}>{l.ours ? $t("lfs.you") : l.owner}</span>
                  <span class="muted small">{when(l.locked_at)}</span>
                  <button class="icon-btn" onclick={() => unlock(l)} disabled={!!busy} title={l.ours ? $t("lfs.unlock") : $t("lfs.forceUnlockTitle", { owner: l.owner })} aria-label={$t("lfs.unlockLabel", { path: l.path })}>
                    <LockOpen size={13} />
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
          <form class="add" onsubmit={(e) => { e.preventDefault(); lockFile(); }}>
            <input bind:value={lockPath} placeholder={$t("lfs.lockPlaceholder")} spellcheck="false" autocomplete="off" aria-label={$t("lfs.lockLabel")} />
            <button class="btn" type="submit" disabled={!!busy || !lockPath.trim()}>{$t("lfs.lock")}</button>
          </form>
        </section>

        <section>
          <h3>{$t("lfs.objects")}</h3>
          <div class="actions">
            <button class="btn" onclick={() => fetchObjects(false)} disabled={!!busy} title={"git lfs fetch"}>
              {#if busy === "fetch"}<Loader2 size={12} class="spinner" />{/if} {$t("lfs.fetch")}
            </button>
            <button class="btn" onclick={() => fetchObjects(true)} disabled={!!busy} title={"git lfs fetch --all"}>
              {#if busy === "fetch-all"}<Loader2 size={12} class="spinner" />{/if} {$t("lfs.fetchAll")}
            </button>
            <button class="btn" onclick={prune} disabled={!!busy} title={$t("lfs.pruneTitleHint")}>
              {#if busy === "prune"}<Loader2 size={12} class="spinner" />{/if} {$t("lfs.prune")}
            </button>
          </div>
        </section>
      {/if}
    </div>
  {/if}
</Modal>

<style>
  .lfs { display: flex; flex-direction: column; gap: 14px; font-size: 13px; }
  section { display: flex; flex-direction: column; gap: 6px; }
  h3 { display: flex; align-items: center; gap: 6px; margin: 0; font-size: 12px; font-weight: 600; color: var(--color-text-primary); }
  p { margin: 0; }
  .muted { color: var(--color-text-muted); display: flex; align-items: center; gap: 6px; }
  .small { font-size: 11px; }
  .version { font-size: 11px; font-family: var(--font-mono); }
  .notice { padding: 8px 10px; border-radius: 4px; background: var(--color-surface); color: var(--color-text-primary); }
  .error { color: var(--color-diff-del-text); font-size: 12px; white-space: pre-wrap; }
  code { font-family: var(--font-mono); font-size: 12px; }
  .rows { list-style: none; margin: 0; padding: 0; border: 1px solid var(--color-border); border-radius: 4px; max-height: 200px; overflow: auto; }
  .row { display: flex; align-items: center; gap: 8px; padding: 4px 8px; }
  .row + .row { border-top: 1px solid var(--color-border); }
  .grow { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .badge { font-size: 10px; padding: 0 5px; border: 1px solid currentColor; border-radius: 3px; color: var(--color-accent); }
  .mine { color: var(--color-diff-add-text); }
  .add { display: flex; align-items: center; gap: 8px; }
  .add input:not([type]), .add input[placeholder] {
    flex: 1;
    min-width: 0;
    padding: 5px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-family: var(--font-mono);
    font-size: 12px;
    outline: none;
  }
  .add input[placeholder]:focus { border-color: var(--color-accent); }
  .check { display: flex; align-items: center; gap: 4px; font-size: 12px; color: var(--color-text-muted); cursor: pointer; }
  .check input { margin: 0; accent-color: var(--color-accent); }
  .actions { display: flex; gap: 8px; flex-wrap: wrap; }
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
  .icon-btn { display: flex; background: none; border: none; padding: 2px; color: var(--color-text-muted); cursor: pointer; }
  .icon-btn:hover:not(:disabled) { color: var(--color-text-primary); }
  .placeholder { width: 17px; height: 17px; cursor: help; }
  :global(.lfs .spinner) { animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
