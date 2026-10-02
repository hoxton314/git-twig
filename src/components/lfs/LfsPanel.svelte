<script lang="ts">
  /**
   * Git LFS panel: tracked patterns (track / untrack), locks (lock /
   * unlock; others' locks only with force), and fetch / prune. Opened from
   * the sidebar entry or the `lfs_manage` action; mounted once in AppShell.
   */
  import { ask } from "@tauri-apps/plugin-dialog";
  import Modal from "../shared/Modal.svelte";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { lfsPanelOpen, lfsStatus, pruneSummary, refreshLfsStatus } from "../../lib/stores/lfs";
  import { refreshAll } from "../../lib/stores/graph";
  import { toast, toastError } from "../../lib/stores/toasts";
  import * as tauri from "../../lib/tauri";
  import type { LfsLock } from "../../lib/types/git";
  import { Loader2, Lock, LockOpen, RefreshCw, Trash2 } from "lucide-svelte";

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
      toast("success", `Tracking ${pat} with LFS. Commit .gitattributes to share it.`);
    });

  const untrack = (pat: string) =>
    run(`untrack:${pat}`, async (p) => {
      await tauri.lfsUntrack(p, pat);
      await refreshLfsStatus(p);
      await refreshAll(p);
      toast("success", `Stopped tracking ${pat}. Commit .gitattributes to share it.`);
    });

  const lockFile = () =>
    run("lock", async (p) => {
      const f = lockPath.trim();
      if (!f) return;
      await tauri.lfsLock(p, f);
      lockPath = "";
      await loadLocks(p);
      toast("success", `Locked ${f}`);
    });

  const unlock = (l: LfsLock) =>
    run(`unlock:${l.id}`, async (p) => {
      if (!l.ours) {
        const ok = await ask(
          `${l.path} is locked by ${l.owner || "someone else"}. Force-unlock it? They may lose work they haven't pushed.`,
          { title: "Force Unlock", kind: "warning", okLabel: "Force Unlock" },
        );
        if (!ok) return;
      }
      await tauri.lfsUnlock(p, l.id, !l.ours);
      await loadLocks(p);
      toast("success", `Unlocked ${l.path}`);
    });

  const fetchObjects = (all: boolean) =>
    run(all ? "fetch-all" : "fetch", async (p) => {
      await tauri.lfsFetch(p, all);
      toast("success", all ? "Fetched LFS objects for all refs" : "Fetched LFS objects for the current checkout");
    });

  const prune = () =>
    run("prune", async (p) => {
      const dry = pruneSummary(await tauri.lfsPrune(p, true));
      if (dry.count === 0) {
        toast("info", "Nothing to prune: every local LFS object is still needed.");
        return;
      }
      const ok = await ask(
        `Delete ${dry.count} local LFS object${dry.count === 1 ? "" : "s"}${dry.size ? ` (${dry.size})` : ""} that no recent commit needs? They can be fetched again from the server.`,
        { title: "Prune LFS Objects", kind: "warning", okLabel: "Prune" },
      );
      if (!ok) return;
      await tauri.lfsPrune(p, false);
      toast("success", `Pruned ${dry.count} LFS object${dry.count === 1 ? "" : "s"}`);
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
          git-lfs is not installed. Install it (e.g. from git-lfs.com or your package manager) and run
          <code>git lfs install</code> once, then reopen this panel.
        </p>
      {:else if !st}
        <p class="muted"><Loader2 size={13} class="spinner" /> Loading…</p>
      {:else}
        <p class="muted version">{st.version}</p>

        <section>
          <h3>Tracked patterns</h3>
          {#if st.patterns.length === 0}
            <p class="muted">No files are tracked with LFS in this repository yet.</p>
          {:else}
            <ul class="rows">
              {#each st.patterns as p (p.pattern + p.source)}
                <li class="row">
                  <code class="grow">{p.pattern}</code>
                  {#if p.lockable}<span class="badge" title="Read-only unless locked">lockable</span>{/if}
                  <span class="muted small">{p.source}</span>
                  <button class="icon-btn" onclick={() => untrack(p.pattern)} disabled={!!busy} title="Stop tracking {p.pattern}" aria-label="Untrack {p.pattern}">
                    <Trash2 size={13} />
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
          <form class="add" onsubmit={(e) => { e.preventDefault(); track(); }}>
            <input bind:value={pattern} placeholder="Pattern to track, e.g. *.psd" spellcheck="false" autocomplete="off" aria-label="Pattern to track" />
            <label class="check" title="Read-only in the working copy unless locked"><input type="checkbox" bind:checked={lockable} /> Lockable</label>
            <button class="btn" type="submit" disabled={!!busy || !pattern.trim()}>Track</button>
          </form>
          <p class="muted small">Tracking changes <code>.gitattributes</code>; commit it so others use LFS for these files too.</p>
        </section>

        <section>
          <h3>
            Locks
            <button class="icon-btn" onclick={() => loadLocks()} disabled={!!busy} title="Reload locks from the server" aria-label="Reload locks"><RefreshCw size={12} /></button>
          </h3>
          {#if locksError}
            <p class="error">{locksError}</p>
          {:else if locks === null}
            <p class="muted"><Loader2 size={13} class="spinner" /> Loading locks…</p>
          {:else if locks.length === 0}
            <p class="muted">No files are locked.</p>
          {:else}
            <ul class="rows">
              {#each locks as l (l.id)}
                <li class="row">
                  <Lock size={12} />
                  <code class="grow">{l.path}</code>
                  <span class="small" class:mine={l.ours}>{l.ours ? "you" : l.owner}</span>
                  <span class="muted small">{when(l.locked_at)}</span>
                  <button class="icon-btn" onclick={() => unlock(l)} disabled={!!busy} title={l.ours ? "Unlock" : `Force-unlock ${l.owner}'s lock`} aria-label="Unlock {l.path}">
                    <LockOpen size={13} />
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
          <form class="add" onsubmit={(e) => { e.preventDefault(); lockFile(); }}>
            <input bind:value={lockPath} placeholder="File to lock, e.g. art/cover.psd" spellcheck="false" autocomplete="off" aria-label="File to lock" />
            <button class="btn" type="submit" disabled={!!busy || !lockPath.trim()}>Lock</button>
          </form>
        </section>

        <section>
          <h3>Objects</h3>
          <div class="actions">
            <button class="btn" onclick={() => fetchObjects(false)} disabled={!!busy} title="git lfs fetch">
              {#if busy === "fetch"}<Loader2 size={12} class="spinner" />{/if} Fetch
            </button>
            <button class="btn" onclick={() => fetchObjects(true)} disabled={!!busy} title="git lfs fetch --all">
              {#if busy === "fetch-all"}<Loader2 size={12} class="spinner" />{/if} Fetch all refs
            </button>
            <button class="btn" onclick={prune} disabled={!!busy} title="Delete local LFS objects no recent commit needs (git lfs prune)">
              {#if busy === "prune"}<Loader2 size={12} class="spinner" />{/if} Prune…
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
  :global(.lfs .spinner) { animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
