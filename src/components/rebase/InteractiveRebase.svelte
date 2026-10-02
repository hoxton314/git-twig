<script lang="ts">
  import Modal from "../shared/Modal.svelte";
  import { GripVertical, ArrowUp, ArrowDown, Loader2, RefreshCw, AlertTriangle } from "lucide-svelte";
  import { untrack } from "svelte";
  import {
    interactiveRebaseDialog,
    operationState,
    refreshOperation,
    operationLabel,
  } from "../../lib/stores/operation";
  import { activeRepoPath, activeRepo } from "../../lib/stores/repos";
  import { branches, workingStatus, refreshAll } from "../../lib/stores/graph";
  import { toast, toastError } from "../../lib/stores/toasts";
  import * as tauri from "../../lib/tauri";
  import type { RebaseAction, RebaseCommit, RebaseTodoItem } from "../../lib/types/git";

  interface Row {
    commit: RebaseCommit;
    action: RebaseAction;
    message: string;
  }

  const ACTIONS: { id: RebaseAction; key: string; hint: string }[] = [
    { id: "pick", key: "p", hint: "Use the commit" },
    { id: "reword", key: "r", hint: "Use the commit, edit its message" },
    { id: "edit", key: "e", hint: "Stop after this commit to amend it" },
    { id: "squash", key: "s", hint: "Meld into the previous commit, combine messages" },
    { id: "fixup", key: "f", hint: "Meld into the previous commit, discard this message" },
    { id: "drop", key: "d", hint: "Remove the commit" },
  ];

  const dialog = $derived($interactiveRebaseDialog);
  const headName = $derived($activeRepo?.head_name ?? "HEAD");
  const head = $derived($branches.find((b) => b.is_head && !b.is_remote) ?? null);
  const dirty = $derived($workingStatus.staged.length + $workingStatus.unstaged.length > 0);

  let base = $state("");
  let fromRoot = $state(false);
  let rows = $state<Row[]>([]);
  /** OIDs in the order they were loaded, to detect "nothing changed". */
  let originalOrder = $state<string[]>([]);
  let mergesSkipped = $state(0);
  let ontoNewBase = $state(false);
  let loading = $state(false);
  let loadError = $state<string | null>(null);
  let loadedFor = $state<string | null>(null);
  let running = $state(false);
  let autostash = $state(true);
  let wasOpen = false;

  // Drag state (pointer-based so it works regardless of the webview's
  // native drag-and-drop handling).
  let dragFrom = $state<number | null>(null);
  let dropAt = $state<number | null>(null);
  let listEl = $state<HTMLDivElement | null>(null);

  $effect(() => {
    const open = dialog.open;
    if (open && !wasOpen) {
      untrack(() => {
        rows = [];
        loadError = null;
        loadedFor = null;
        fromRoot = dialog.base === null;
        base = dialog.base ?? "";
        if (!fromRoot && !base && head?.upstream) base = head.upstream;
        if (fromRoot || base) load();
      });
    }
    wasOpen = open;
  });

  async function load() {
    const path = $activeRepoPath;
    if (!path) return;
    const b = fromRoot ? null : base.trim();
    if (b === "") {
      loadError = "Enter a base branch or commit.";
      return;
    }
    loading = true;
    loadError = null;
    try {
      const list = await tauri.listRebaseCommits(path, b);
      // Like `git rebase`, leave out changes the new base already has (they
      // would pick as empty and pause the rebase); the user can still pick them.
      rows = list.commits.map((c) => ({
        commit: c,
        action: c.already_upstream ? "drop" : "pick",
        message: "",
      }));
      originalOrder = list.commits.map((c) => c.oid);
      mergesSkipped = list.merges_skipped;
      ontoNewBase = list.onto_new_base;
      loadedFor = b ?? "--root";
      if (list.commits.length === 0) loadError = `No commits between ${b ?? "root"} and ${headName}.`;
    } catch (err) {
      rows = [];
      loadedFor = null;
      loadError = String(err);
    } finally {
      loading = false;
    }
  }

  function close() {
    if (running) return;
    interactiveRebaseDialog.set({ open: false, base: "" });
  }

  function setAction(i: number, action: RebaseAction) {
    const row = rows[i];
    if (action === "reword" && !row.message) row.message = row.commit.message;
    row.action = action;
  }

  function move(from: number, to: number) {
    if (to < 0 || to >= rows.length || from === to) return;
    const next = [...rows];
    const [item] = next.splice(from, 1);
    next.splice(to, 0, item);
    rows = next;
  }

  // ── Pointer drag ───────────────────────────────────────────────────

  function onHandleDown(e: PointerEvent, i: number) {
    if (e.button !== 0) return;
    e.preventDefault();
    dragFrom = i;
    dropAt = i;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onHandleMove(e: PointerEvent) {
    if (dragFrom === null || !listEl) return;
    const items = Array.from(listEl.querySelectorAll<HTMLElement>("[data-row]"));
    let at = items.length;
    for (let k = 0; k < items.length; k++) {
      const r = items[k].getBoundingClientRect();
      if (e.clientY < r.top + r.height / 2) {
        at = k;
        break;
      }
    }
    dropAt = at;
    // Auto-scroll near the edges.
    const lr = listEl.getBoundingClientRect();
    if (e.clientY < lr.top + 24) listEl.scrollTop -= 8;
    else if (e.clientY > lr.bottom - 24) listEl.scrollTop += 8;
  }

  function onHandleUp() {
    if (dragFrom !== null && dropAt !== null) {
      const to = dropAt > dragFrom ? dropAt - 1 : dropAt;
      move(dragFrom, to);
    }
    dragFrom = null;
    dropAt = null;
  }

  function onRowKey(e: KeyboardEvent, i: number) {
    const tag = (e.target as HTMLElement).tagName;
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
    if (e.altKey && e.key === "ArrowUp") {
      e.preventDefault();
      move(i, i - 1);
      focusRow(i - 1);
      return;
    }
    if (e.altKey && e.key === "ArrowDown") {
      e.preventDefault();
      move(i, i + 1);
      focusRow(i + 1);
      return;
    }
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    const a = ACTIONS.find((x) => x.key === e.key.toLowerCase());
    if (a) {
      e.preventDefault();
      setAction(i, a.id);
    }
  }

  function focusRow(i: number) {
    queueMicrotask(() => listEl?.querySelector<HTMLElement>(`[data-row="${i}"]`)?.focus());
  }

  // ── Validation & summary ───────────────────────────────────────────

  const errors = $derived.by(() => {
    const errs: string[] = [];
    const first = rows.find((r) => r.action !== "drop");
    if (first && (first.action === "squash" || first.action === "fixup")) {
      errs.push(`The first kept commit (${first.commit.short_oid}) cannot be a ${first.action}.`);
    }
    for (const r of rows) {
      if (r.action === "reword" && !r.message.trim()) {
        errs.push(`Reword of ${r.commit.short_oid} needs a message.`);
      }
    }
    return errs;
  });

  const changed = $derived(
    ontoNewBase || rows.some((r, i) => r.action !== "pick" || r.commit.oid !== originalOrder[i]),
  );

  const upstreamCount = $derived(
    rows.filter((r) => r.commit.already_upstream && r.action === "drop").length,
  );

  const summary = $derived.by(() => {
    const count = (a: RebaseAction) => rows.filter((r) => r.action === a).length;
    const parts: string[] = [];
    for (const a of ACTIONS) {
      const n = count(a.id);
      if (n > 0 && a.id !== "pick") parts.push(`${n} ${a.id}`);
    }
    return parts.join(", ");
  });

  async function start() {
    const path = $activeRepoPath;
    if (!path || running || rows.length === 0 || errors.length > 0) return;
    const b = fromRoot ? null : base.trim();
    const items: RebaseTodoItem[] = rows.map((r) => ({
      oid: r.commit.oid,
      action: r.action,
      message: r.action === "reword" || r.action === "squash" ? r.message.trim() || null : null,
    }));
    running = true;
    try {
      const res = await tauri.interactiveRebase(path, b, items, autostash);
      await refreshAll(path);
      await refreshOperation(path);
      const st = $operationState;
      if (st && st.kind === "rebase") {
        const why = st.conflicts.length > 0 ? `${st.conflicts.length} conflicted file(s)` : "an edit stop";
        toast("warning", `Rebase paused at ${why}. Use the banner to continue or abort.`, {
          title: "Interactive rebase",
        });
        running = false;
        close();
      } else if (res.success) {
        toast("success", `Rewrote history of ${headName}`);
        running = false;
        close();
      } else {
        toastError("Interactive rebase failed", res.message);
      }
    } catch (err) {
      toastError("Interactive rebase failed", err);
    } finally {
      running = false;
    }
  }

  const busyOp = $derived($operationState !== null && $operationState.kind !== "none");
</script>

<Modal open={dialog.open} title="Interactive rebase: {headName}" onclose={close} width="min(820px, 94vw)">
  <div class="irb">
    <form class="base-row" onsubmit={(e) => { e.preventDefault(); load(); }}>
      <label class="base-label" for="irb-base">Base (exclusive)</label>
      <input
        id="irb-base"
        type="text"
        bind:value={base}
        disabled={fromRoot}
        placeholder="branch, tag or commit, e.g. main or HEAD~5"
        spellcheck="false"
        autocomplete="off"
      />
      <label class="check">
        <input type="checkbox" bind:checked={fromRoot} onchange={() => load()} />
        From root
      </label>
      <button type="submit" class="btn" disabled={loading} title="Load commits">
        {#if loading}<Loader2 size={13} class="spinner" />{:else}<RefreshCw size={13} />{/if}
        Load
      </button>
    </form>

    {#if busyOp}
      <div class="warn"><AlertTriangle size={13} /> A {operationLabel($operationState?.kind ?? "")} is in progress. Finish or abort it first.</div>
    {/if}
    {#if upstreamCount > 0}
      <div class="warn">
        <AlertTriangle size={13} /> {upstreamCount} commit{upstreamCount !== 1 ? "s are" : " is"} already in {loadedFor} and
        {upstreamCount !== 1 ? "are" : "is"} set to drop.
      </div>
    {/if}
    {#if mergesSkipped > 0}
      <div class="warn">
        <AlertTriangle size={13} /> {mergesSkipped} merge commit{mergesSkipped !== 1 ? "s" : ""} in range will be flattened (merges are not preserved).
      </div>
    {/if}

    <div class="hint">
      Commits are applied top to bottom. Drag the handle (or Alt+↑/↓) to reorder; keys
      <kbd>p</kbd> <kbd>r</kbd> <kbd>e</kbd> <kbd>s</kbd> <kbd>f</kbd> <kbd>d</kbd> set the action of the focused row.
    </div>

    <div class="list" bind:this={listEl} class:dragging={dragFrom !== null}>
      {#if loading}
        <div class="empty"><Loader2 size={14} class="spinner" /> Loading commits…</div>
      {:else if loadError}
        <div class="empty error">{loadError}</div>
      {:else if rows.length === 0}
        <div class="empty">Choose a base and load the commits to edit.</div>
      {:else}
        {#each rows as row, i (row.commit.oid)}
          {#if dropAt === i && dragFrom !== null && dropAt !== dragFrom && dropAt !== dragFrom + 1}
            <div class="drop-line"></div>
          {/if}
          <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
          <div
            class="row action-{row.action}"
            class:dragged={dragFrom === i}
            data-row={i}
            tabindex="0"
            role="group"
            aria-label="{row.action} {row.commit.short_oid} {row.commit.summary}"
            onkeydown={(e) => onRowKey(e, i)}
          >
            <div class="row-main">
              <span
                class="handle"
                role="button"
                tabindex="-1"
                aria-label="Drag to reorder"
                onpointerdown={(e) => onHandleDown(e, i)}
                onpointermove={onHandleMove}
                onpointerup={onHandleUp}
                onpointercancel={onHandleUp}
              >
                <GripVertical size={14} />
              </span>
              <select
                class="action"
                value={row.action}
                onchange={(e) => setAction(i, e.currentTarget.value as RebaseAction)}
                aria-label="Action for {row.commit.short_oid}"
              >
                {#each ACTIONS as a (a.id)}
                  <option value={a.id} title={a.hint}>{a.id}</option>
                {/each}
              </select>
              <span class="oid">{row.commit.short_oid}</span>
              <span class="summary" title={row.commit.message}>{row.commit.summary}</span>
              {#if row.commit.already_upstream}
                <span class="upstream-badge" title="The new base already contains this change">in base</span>
              {/if}
              <span class="author">{row.commit.author_name}</span>
              <span class="moves">
                <button class="icon-btn" onclick={() => move(i, i - 1)} disabled={i === 0} aria-label="Move up" title="Move up (Alt+↑)">
                  <ArrowUp size={12} />
                </button>
                <button class="icon-btn" onclick={() => move(i, i + 1)} disabled={i === rows.length - 1} aria-label="Move down" title="Move down (Alt+↓)">
                  <ArrowDown size={12} />
                </button>
              </span>
            </div>
            {#if row.action === "reword"}
              <textarea
                class="msg"
                rows="3"
                bind:value={row.message}
                aria-label="New message for {row.commit.short_oid}"
                spellcheck="true"
              ></textarea>
            {:else if row.action === "squash"}
              <textarea
                class="msg"
                rows="2"
                bind:value={row.message}
                placeholder="Optional: message for the combined commit (default: both messages)"
                aria-label="Combined message for squash of {row.commit.short_oid}"
                spellcheck="true"
              ></textarea>
            {/if}
          </div>
        {/each}
        {#if dropAt === rows.length && dragFrom !== null && dragFrom !== rows.length - 1}
          <div class="drop-line"></div>
        {/if}
      {/if}
    </div>

    {#if errors.length > 0}
      <ul class="errors">
        {#each errors as err (err)}<li>{err}</li>{/each}
      </ul>
    {/if}

    <div class="footer">
      <span class="summary-text">
        {rows.length} commit{rows.length !== 1 ? "s" : ""}{summary ? ` · ${summary}` : ""}
      </span>
      {#if dirty}
        <label class="check">
          <input type="checkbox" bind:checked={autostash} /> Autostash
        </label>
      {/if}
      <span class="spacer"></span>
      <button class="btn" onclick={close} disabled={running}>Cancel</button>
      <button
        class="btn primary"
        onclick={start}
        disabled={running || busyOp || rows.length === 0 || errors.length > 0 || !changed}
        title={!changed ? "Nothing changed" : "Start the rebase"}
      >
        {#if running}<Loader2 size={13} class="spinner" />{/if}
        Start rebase
      </button>
    </div>
  </div>
</Modal>

<style>
  .irb {
    display: flex;
    flex-direction: column;
    gap: 10px;
    height: min(70vh, 720px);
  }

  .base-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .base-label {
    font-size: 12px;
    color: var(--color-text-muted);
    white-space: nowrap;
  }

  .base-row input[type="text"] {
    flex: 1;
    padding: 6px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-family: var(--font-mono);
    font-size: 12px;
    outline: none;
  }

  .base-row input[type="text"]:focus {
    border-color: var(--color-accent);
  }

  .base-row input:disabled {
    opacity: 0.5;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: var(--color-text-primary);
    white-space: nowrap;
  }

  .warn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--color-lane-2) 12%, transparent);
    color: var(--color-lane-2);
    font-size: 12px;
  }

  .hint {
    font-size: 11px;
    color: var(--color-text-muted);
  }

  kbd {
    font-family: var(--font-mono);
    font-size: 10px;
    padding: 0 4px;
    border: 1px solid var(--color-border);
    border-radius: 3px;
    background: var(--color-surface-elevated);
  }

  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
  }

  .list.dragging {
    cursor: grabbing;
    user-select: none;
  }

  .empty {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 16px;
    color: var(--color-text-muted);
    font-size: 12px;
  }

  .empty.error {
    color: var(--color-diff-del-text);
  }

  .row {
    border-bottom: 1px solid var(--color-border);
    border-left: 3px solid transparent;
    outline: none;
  }

  .row:focus-visible {
    background: var(--color-surface);
    box-shadow: inset 0 0 0 1px var(--color-accent);
  }

  .row.dragged {
    opacity: 0.4;
  }

  .row.action-reword { border-left-color: var(--color-accent); }
  .row.action-edit { border-left-color: var(--color-lane-2); }
  .row.action-squash,
  .row.action-fixup { border-left-color: var(--color-accent-secondary); }
  .row.action-drop { border-left-color: var(--color-diff-del-text); }

  .row.action-drop .summary,
  .row.action-drop .oid {
    text-decoration: line-through;
    color: var(--color-text-muted);
  }

  .row.action-squash .row-main,
  .row.action-fixup .row-main {
    padding-left: 22px;
  }

  .row-main {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 8px;
  }

  .handle {
    display: flex;
    color: var(--color-text-muted);
    cursor: grab;
    touch-action: none;
  }

  .handle:hover {
    color: var(--color-text-primary);
  }

  .action {
    width: 92px;
    padding: 3px 24px 3px 8px;
    font-size: 11px;
    font-family: var(--font-mono);
  }

  .oid {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-accent);
    flex-shrink: 0;
  }

  .summary {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    color: var(--color-text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .upstream-badge {
    font-size: 10px;
    padding: 1px 5px;
    border-radius: 3px;
    flex-shrink: 0;
    color: var(--color-text-muted);
    border: 1px solid var(--color-border);
  }

  .author {
    font-size: 11px;
    color: var(--color-text-muted);
    flex-shrink: 0;
    max-width: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .moves {
    display: flex;
    gap: 2px;
  }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border: none;
    border-radius: 3px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 0;
  }

  .icon-btn:hover:not(:disabled) {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .icon-btn:disabled {
    opacity: 0.3;
    cursor: default;
  }

  .msg {
    display: block;
    width: calc(100% - 40px);
    margin: 0 8px 6px 32px;
    box-sizing: border-box;
    padding: 6px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-family: var(--font-mono);
    font-size: 12px;
    resize: vertical;
    outline: none;
  }

  .msg:focus {
    border-color: var(--color-accent);
  }

  .drop-line {
    height: 2px;
    background: var(--color-accent);
  }

  .errors {
    margin: 0;
    padding-left: 18px;
    color: var(--color-diff-del-text);
    font-size: 12px;
  }

  .footer {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .summary-text {
    font-size: 12px;
    color: var(--color-text-muted);
  }

  .spacer {
    flex: 1;
  }

  .btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 14px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
  }

  .btn:hover:not(:disabled) {
    background: var(--color-surface-elevated);
  }

  .btn.primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: var(--color-bg);
    font-weight: 500;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .irb :global(.spinner) {
    animation: irb-spin 1s linear infinite;
  }

  @keyframes irb-spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
