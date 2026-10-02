<script lang="ts">
  import Modal from "../shared/Modal.svelte";
  import {
    ChevronUp,
    ChevronDown,
    Loader2,
    Save,
    CheckCircle2,
    Wrench,
    Undo2,
    Pencil,
    Eye,
  } from "lucide-svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { untrack } from "svelte";
  import {
    conflictResolverFile,
    operationState,
    refreshOperation,
  } from "../../lib/stores/operation";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { refreshAll } from "../../lib/stores/graph";
  import { settings } from "../../lib/stores/settings";
  import { toast, toastError } from "../../lib/stores/toasts";
  import * as tauri from "../../lib/tauri";
  import type { ConflictVersions } from "../../lib/types/git";
  import {
    parseConflicts,
    serializeSegments,
    resolveSegment,
    markerLabel,
    type Segment,
    type ConflictSegment,
  } from "../../lib/conflictMarkers";

  type Tab = "result" | "ours" | "theirs" | "base";

  const file = $derived($conflictResolverFile);
  const st = $derived($operationState);
  const conflictInfo = $derived(st?.conflicts.find((c) => c.path === file) ?? null);

  let versions = $state<ConflictVersions | null>(null);
  let loading = $state(false);
  let loadError = $state<string | null>(null);
  let segments = $state<Segment[]>([]);
  let tab = $state<Tab>("result");
  let editMode = $state(false);
  let editText = $state("");
  let dirty = $state(false);
  let saving = $state(false);
  let current = $state(0);
  let scroller = $state<HTMLDivElement | null>(null);

  const conflictIdx = $derived.by(() => {
    const idx: number[] = [];
    segments.forEach((s, i) => {
      if (s.kind === "conflict") idx.push(i);
    });
    return idx;
  });
  const unresolved = $derived(
    conflictIdx.filter((i) => (segments[i] as ConflictSegment).resolution === null).length,
  );
  /** Why the versions can't be shown as editable text, if they can't. */
  const noTextReason = $derived.by(() => {
    if (!versions) return null;
    if (versions.is_symlink) return "Symbolic link";
    if (versions.is_binary) return "Binary file";
    if (versions.not_utf8) return "Not UTF-8 text";
    if (versions.too_large) return "File too large to display";
    return null;
  });
  const canEditResult = $derived(
    versions !== null && noTextReason === null && versions.merged !== null,
  );

  let loadReq = 0;
  async function load(path: string, f: string) {
    const req = ++loadReq;
    loading = true;
    loadError = null;
    versions = null;
    segments = [];
    dirty = false;
    editMode = false;
    current = 0;
    tab = "result";
    try {
      const v = await tauri.getConflictVersions(path, f);
      if (req !== loadReq) return;
      versions = v;
      segments = v.merged !== null ? parseConflicts(v.merged) : [];
    } catch (err) {
      if (req !== loadReq) return;
      loadError = String(err);
    } finally {
      if (req === loadReq) loading = false;
    }
  }

  $effect(() => {
    const path = $activeRepoPath;
    const f = file;
    if (path && f) untrack(() => load(path, f));
  });

  function currentText(): string {
    return editMode ? editText : serializeSegments(segments);
  }

  function toggleEdit() {
    if (editMode) {
      segments = parseConflicts(editText);
      current = 0;
      editMode = false;
    } else {
      editText = serializeSegments(segments);
      editMode = true;
    }
  }

  function choose(si: number, choice: NonNullable<ConflictSegment["choice"]>) {
    const seg = segments[si];
    if (seg.kind !== "conflict") return;
    segments[si] = resolveSegment(seg, choice);
    dirty = true;
    // Advance to the next unresolved conflict.
    const next = conflictIdx.findIndex(
      (i, n) => n > conflictIdx.indexOf(si) && (segments[i] as ConflictSegment).resolution === null,
    );
    if (next !== -1) goTo(next);
  }

  function undo(si: number) {
    const seg = segments[si];
    if (seg.kind !== "conflict") return;
    segments[si] = { ...seg, resolution: null, choice: null };
    dirty = true;
  }

  function goTo(n: number) {
    if (conflictIdx.length === 0) return;
    current = (n + conflictIdx.length) % conflictIdx.length;
    tab = "result";
    queueMicrotask(() => {
      scroller
        ?.querySelector(`[data-conflict="${current}"]`)
        ?.scrollIntoView({ block: "center", behavior: "smooth" });
    });
  }

  async function close() {
    if (dirty && !saving) {
      const discard = await ask("Discard your unsaved resolution changes?", {
        title: "Unsaved Changes",
        kind: "warning",
      });
      if (!discard) return;
    }
    conflictResolverFile.set(null);
  }

  async function afterResolved(path: string, resolvedFile: string) {
    await refreshAll(path);
    await refreshOperation(path);
    const remaining = ($operationState?.conflicts ?? []).filter((c) => c.path !== resolvedFile);
    dirty = false;
    if (remaining.length > 0) {
      conflictResolverFile.set(remaining[0].path);
    } else {
      conflictResolverFile.set(null);
      toast("success", "All conflicts resolved. Continue when ready.");
    }
  }

  async function save(stage: boolean) {
    const path = $activeRepoPath;
    if (!path || !file || saving) return;
    const text = currentText();
    if (stage && /^(<{7}|>{7})( |$)/m.test(text)) {
      const ok = await ask(
        "The file still contains conflict markers. Mark it as resolved anyway?",
        { title: "Unresolved Conflicts", kind: "warning" },
      );
      if (!ok) return;
    }
    saving = true;
    const f = file;
    try {
      const res = await tauri.saveResolvedFile(path, f, text, stage);
      if (!res.success) {
        toastError("Save failed", res.message);
        return;
      }
      if (stage) {
        await afterResolved(path, f);
      } else {
        dirty = false;
        toast("success", `Saved ${f}`);
        await refreshAll(path);
      }
    } catch (err) {
      toastError("Save failed", err);
    } finally {
      saving = false;
    }
  }

  async function takeSide(side: "ours" | "theirs") {
    const path = $activeRepoPath;
    if (!path || !file || saving) return;
    saving = true;
    const f = file;
    try {
      const res = await tauri.resolveTakeSide(path, [f], side);
      if (!res.success) {
        toastError(side === "ours" ? "Take ours" : "Take theirs", res.message);
        return;
      }
      await afterResolved(path, f);
    } catch (err) {
      toastError("Resolve failed", err);
    } finally {
      saving = false;
    }
  }

  async function mergeTool() {
    const path = $activeRepoPath;
    if (!path || !file || saving) return;
    saving = true;
    const f = file;
    toast("info", `Opening merge tool for ${f}… Close it when done.`);
    try {
      const res = await tauri.openMergeTool(path, f, $settings.external_merge_tool);
      if (!res.success) toastError("Merge tool", res.message);
      await refreshAll(path);
      await refreshOperation(path);
      if (!($operationState?.conflicts ?? []).some((c) => c.path === f)) {
        await afterResolved(path, f);
      } else {
        await load(path, f);
      }
    } catch (err) {
      toastError("Merge tool", err);
    } finally {
      saving = false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (!file || editMode) return;
    if (e.altKey && e.key === "ArrowDown") {
      e.preventDefault();
      goTo(current + 1);
    } else if (e.altKey && e.key === "ArrowUp") {
      e.preventDefault();
      goTo(current - 1);
    }
  }

  function linesOf(text: string | null): string[] {
    if (text === null) return [];
    const l = text.split("\n");
    if (l.length > 1 && l[l.length - 1] === "") l.pop();
    return l;
  }

  const sideText = $derived(
    tab === "ours" ? versions?.ours ?? null : tab === "theirs" ? versions?.theirs ?? null : versions?.base ?? null,
  );
  const oursLabel = $derived(st?.ours_label ?? "Ours");
  const theirsLabel = $derived(st?.theirs_label ?? "Theirs");
</script>

<svelte:window onkeydown={onKeydown} />

<Modal open={file !== null} title={`Resolve conflict: ${file ?? ""}`} onclose={close} width="min(1200px, 94vw)">
  <div class="resolver">
    <div class="toolbar">
      <div class="tabs" role="tablist">
        <button role="tab" class="tab" class:active={tab === "result"} aria-selected={tab === "result"} onclick={() => (tab = "result")}>Result</button>
        <button role="tab" class="tab" class:active={tab === "ours"} aria-selected={tab === "ours"} onclick={() => (tab = "ours")}>{oursLabel}</button>
        <button role="tab" class="tab" class:active={tab === "theirs"} aria-selected={tab === "theirs"} onclick={() => (tab = "theirs")}>{theirsLabel}</button>
        <button role="tab" class="tab" class:active={tab === "base"} aria-selected={tab === "base"} onclick={() => (tab = "base")}>Base</button>
      </div>
      <span class="spacer"></span>
      {#if tab === "result" && canEditResult && !editMode}
        <span class="counter">
          {#if conflictIdx.length === 0}
            No conflict markers
          {:else}
            Conflict {current + 1} of {conflictIdx.length}
            {#if unresolved > 0}<span class="unresolved">· {unresolved} unresolved</span>{/if}
          {/if}
        </span>
        <button class="icon-btn" onclick={() => goTo(current - 1)} disabled={conflictIdx.length === 0} title="Previous conflict (Alt+Up)" aria-label="Previous conflict">
          <ChevronUp size={14} />
        </button>
        <button class="icon-btn" onclick={() => goTo(current + 1)} disabled={conflictIdx.length === 0} title="Next conflict (Alt+Down)" aria-label="Next conflict">
          <ChevronDown size={14} />
        </button>
      {/if}
      {#if tab === "result" && canEditResult}
        <button class="btn small" onclick={toggleEdit} title={editMode ? "Back to conflict view" : "Edit the result as text"}>
          {#if editMode}<Eye size={12} /> View{:else}<Pencil size={12} /> Edit text{/if}
        </button>
      {/if}
    </div>

    <div class="content" bind:this={scroller}>
      {#if loading}
        <div class="notice"><Loader2 size={14} class="spinner" /> Loading…</div>
      {:else if loadError}
        <div class="notice error">{loadError}</div>
      {:else if versions}
        {#if tab === "result"}
          {#if noTextReason}
            <div class="notice">
              {noTextReason} — choose a side or use the merge tool.
            </div>
          {:else if versions.merged === null}
            <div class="notice">
              {#if conflictInfo?.kind === "deleted_by_us"}
                The file was deleted on the "{oursLabel}" side and modified on the other.
              {:else if conflictInfo?.kind === "deleted_by_them"}
                The file was deleted on the "{theirsLabel}" side and modified on the other.
              {:else}
                The file does not exist in the working tree.
              {/if}
              Keep it by taking the side that has it, or delete it by taking the side that removed it.
            </div>
          {:else if editMode}
            <textarea class="editor" bind:value={editText} oninput={() => (dirty = true)} spellcheck="false" aria-label="Resolved file content"></textarea>
          {:else}
            <div class="code">
              {#each segments as seg, si (si)}
                {#if seg.kind === "text"}
                  {#each seg.lines as line, li (li)}
                    <div class="line">{line || " "}</div>
                  {/each}
                {:else}
                  {@const ci = conflictIdx.indexOf(si)}
                  <div class="block" class:current={ci === current} class:resolved={seg.resolution !== null} data-conflict={ci}>
                    <div class="block-head">
                      <span class="block-title">Conflict {ci + 1}</span>
                      {#if seg.resolution !== null}
                        <span class="badge">resolved: {seg.choice}</span>
                        <span class="spacer"></span>
                        <button class="chip" onclick={() => undo(si)}><Undo2 size={11} /> Undo</button>
                      {:else}
                        <span class="spacer"></span>
                        <button class="chip ours" onclick={() => choose(si, "ours")} title={oursLabel}>Use ours</button>
                        <button class="chip theirs" onclick={() => choose(si, "theirs")} title={theirsLabel}>Use theirs</button>
                        <button class="chip" onclick={() => choose(si, "both")} title="Ours, then theirs">Both</button>
                        <button class="chip" onclick={() => choose(si, "both-reversed")} title="Theirs, then ours">Both (theirs first)</button>
                        {#if seg.base}
                          <button class="chip" onclick={() => choose(si, "base")}>Base</button>
                        {/if}
                      {/if}
                    </div>
                    {#if seg.resolution !== null}
                      {#each seg.resolution as line, li (li)}
                        <div class="line res">{line || " "}</div>
                      {:else}
                        <div class="line empty">(empty)</div>
                      {/each}
                    {:else}
                      <div class="marker ours">{markerLabel(seg.markers.start) || "ours"} — {oursLabel}</div>
                      {#each seg.ours as line, li (li)}
                        <div class="line ours">{line || " "}</div>
                      {/each}
                      {#if seg.base}
                        <div class="marker base">base</div>
                        {#each seg.base as line, li (li)}
                          <div class="line base">{line || " "}</div>
                        {/each}
                      {/if}
                      <div class="marker theirs">{markerLabel(seg.markers.end) || "theirs"} — {theirsLabel}</div>
                      {#each seg.theirs as line, li (li)}
                        <div class="line theirs">{line || " "}</div>
                      {/each}
                    {/if}
                  </div>
                {/if}
              {/each}
            </div>
          {/if}
        {:else if noTextReason}
          <div class="notice">{noTextReason}.</div>
        {:else if sideText === null}
          <div class="notice">The file does not exist in this version.</div>
        {:else}
          <div class="code numbered">
            {#each linesOf(sideText) as line, i (i)}
              <div class="line"><span class="ln">{i + 1}</span>{line || " "}</div>
            {/each}
          </div>
        {/if}
      {/if}
    </div>

    <div class="footer">
      <button class="btn" onclick={() => takeSide("ours")} disabled={saving || !conflictInfo} title="Resolve the whole file with {oursLabel}">
        {conflictInfo && !conflictInfo.has_ours ? "Take ours (delete)" : "Take ours"}
      </button>
      <button class="btn" onclick={() => takeSide("theirs")} disabled={saving || !conflictInfo} title="Resolve the whole file with {theirsLabel}">
        {conflictInfo && !conflictInfo.has_theirs ? "Take theirs (delete)" : "Take theirs"}
      </button>
      <button class="btn" onclick={mergeTool} disabled={saving} title="Open in the external merge tool">
        <Wrench size={12} /> Merge tool
      </button>
      <span class="spacer"></span>
      {#if saving}<Loader2 size={14} class="spinner" />{/if}
      <button class="btn" onclick={close}>Cancel</button>
      {#if canEditResult}
        <button class="btn" onclick={() => save(false)} disabled={saving || !dirty}>
          <Save size={12} /> Save
        </button>
        <button class="btn primary" onclick={() => save(true)} disabled={saving}>
          <CheckCircle2 size={12} /> Save & mark resolved
        </button>
      {/if}
    </div>
  </div>
</Modal>

<style>
  .resolver {
    display: flex;
    flex-direction: column;
    gap: 10px;
    height: min(70vh, 760px);
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .tabs {
    display: flex;
    gap: 2px;
    border: 1px solid var(--color-border);
    border-radius: 5px;
    padding: 2px;
  }

  .tab {
    border: none;
    background: none;
    padding: 4px 10px;
    border-radius: 3px;
    color: var(--color-text-muted);
    font-size: 12px;
    cursor: pointer;
  }

  .tab.active {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .spacer {
    flex: 1;
  }

  .counter {
    font-size: 12px;
    color: var(--color-text-primary);
  }

  .unresolved {
    color: var(--color-diff-del-text);
  }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 24px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    cursor: pointer;
  }

  .content {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
  }

  .notice {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 16px;
    color: var(--color-text-muted);
    font-size: 12px;
  }

  .notice.error {
    color: var(--color-diff-del-text);
  }

  .code {
    font-family: var(--font-mono);
    font-size: var(--diff-font-size, 12px);
    line-height: 1.5;
    padding: 6px 0;
    min-width: max-content;
  }

  .line {
    white-space: pre;
    padding: 0 12px;
    color: var(--color-text-primary);
  }

  .ln {
    display: inline-block;
    width: 4ch;
    margin-right: 12px;
    text-align: right;
    color: var(--color-text-muted);
    user-select: none;
  }

  .line.ours {
    background: var(--color-diff-add-bg);
  }

  .line.theirs {
    background: var(--color-diff-hunk-bg);
  }

  .line.base {
    color: var(--color-text-muted);
  }

  .line.res {
    background: color-mix(in srgb, var(--color-diff-add-bg) 50%, transparent);
  }

  .line.empty {
    color: var(--color-text-muted);
    font-style: italic;
  }

  .block {
    margin: 4px 0;
    border-top: 1px solid var(--color-border);
    border-bottom: 1px solid var(--color-border);
    border-left: 3px solid var(--color-diff-del-text);
  }

  .block.resolved {
    border-left-color: var(--color-diff-add-text);
  }

  .block.current {
    box-shadow: inset 0 0 0 1px var(--color-accent);
  }

  .block-head {
    position: sticky;
    left: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    background: var(--color-surface);
    font-family: var(--font-sans);
    font-size: 11px;
  }

  .block-title {
    font-weight: 600;
    color: var(--color-text-primary);
  }

  .badge {
    color: var(--color-diff-add-text);
  }

  .chip {
    display: flex;
    align-items: center;
    gap: 3px;
    padding: 2px 8px;
    border: 1px solid var(--color-border);
    border-radius: 10px;
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    font-size: 11px;
    cursor: pointer;
  }

  .chip:hover {
    border-color: var(--color-accent);
  }

  .chip.ours {
    border-color: var(--color-diff-add-text);
  }

  .chip.theirs {
    border-color: var(--color-accent);
  }

  .marker {
    padding: 1px 12px;
    font-family: var(--font-sans);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.3px;
    color: var(--color-text-muted);
  }

  .marker.ours {
    color: var(--color-diff-add-text);
  }

  .marker.theirs {
    color: var(--color-accent);
  }

  .editor {
    width: 100%;
    height: 100%;
    box-sizing: border-box;
    border: none;
    padding: 8px 12px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-family: var(--font-mono);
    font-size: var(--diff-font-size, 12px);
    line-height: 1.5;
    resize: none;
    outline: none;
    white-space: pre;
  }

  .footer {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 6px 12px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
  }

  .btn.small {
    padding: 3px 8px;
    font-size: 11px;
  }

  .btn:hover:not(:disabled),
  .icon-btn:hover:not(:disabled) {
    background: var(--color-surface-elevated);
  }

  .btn.primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: var(--color-bg);
    font-weight: 500;
  }

  .btn:disabled,
  .icon-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .resolver :global(.spinner) {
    animation: cr-spin 1s linear infinite;
  }

  @keyframes cr-spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
