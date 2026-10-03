<script lang="ts">
  import {
    AlertTriangle,
    GitMerge,
    ChevronDown,
    ChevronRight,
    Loader2,
    Play,
    SkipForward,
    X,
    CheckCircle2,
    FileWarning,
  } from "lucide-svelte";
  import ContextMenu, { type MenuItem } from "../shared/ContextMenu.svelte";
  import {
    operationState,
    operationBusy,
    operationLabel,
    requestContinue,
    abortOperation,
    skipOperation,
    openConflictResolver,
    refreshOperation,
    bisectState,
  } from "../../lib/stores/operation";
  import { bisectProgress, markBisect } from "../../lib/bisectActions";
  import { revealCommit } from "../../lib/stores/commitUi";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { refreshAll } from "../../lib/stores/graph";
  import { settings } from "../../lib/stores/settings";
  import { toast, toastError } from "../../lib/stores/toasts";
  import * as tauri from "../../lib/tauri";
  import type { ConflictFile } from "../../lib/types/git";
  import { t, tr, type MessageKey } from "../../lib/i18n";

  const st = $derived($operationState);
  const busy = $derived($operationBusy);
  const conflicts = $derived(st?.conflicts ?? []);
  const active = $derived(st !== null && (st.kind !== "none" || conflicts.length > 0));
  const inOperation = $derived(st !== null && st.kind !== "none");
  const bisect = $derived(st?.kind === "bisect" ? $bisectState : null);
  /** Both ends are known, so HEAD is a commit git checked out for testing. */
  const bisectTesting = $derived(!!bisect && !bisect.first_bad && !!bisect.bad && bisect.good.length > 0);

  let expanded = $state(true);
  let fileBusy = $state<string | null>(null);
  let menu = $state<{ x: number; y: number; file: ConflictFile } | null>(null);

  const title = $derived.by(() => {
    if (!st) return "";
    if (st.kind === "none") return $t("conflicts.unresolvedConflicts");
    if (st.kind === "bisect" && bisect) return $t("conflicts.bisecting", { progress: bisectProgress(bisect) });
    const name = operationLabel(st.kind);
    if (st.kind === "rebase") {
      let what = st.head_name
        ? $t(st.interactive ? "conflicts.interactiveRebaseBranch" : "conflicts.rebasingBranch", { branch: st.head_name })
        : $t(st.interactive ? "conflicts.interactiveRebase" : "conflicts.rebasing");
      if (st.onto) what = $t("conflicts.onto", { what, onto: st.onto });
      if (st.step && st.total) what = $t("conflicts.step", { what, step: st.step, total: st.total });
      return what;
    }
    return $t("conflicts.inProgress", { name });
  });

  const detail = $derived.by(() => {
    if (!st) return "";
    if (bisect) {
      if (bisect.first_bad)
        return $t("conflicts.firstBadIs", {
          oid: bisect.first_bad.slice(0, 7),
          subject: bisect.first_bad_subject ?? "",
          term: bisect.term_bad,
        });
      if (!bisectTesting) return $t("conflicts.rightClickToMark");
      if (bisect.current)
        return $t("conflicts.testing", { oid: bisect.current.slice(0, 7), subject: bisect.current_subject ?? "" });
    }
    if (st.kind === "rebase" && st.stopped_for_edit) {
      return $t("conflicts.stoppedForEdit", {
        oid: st.current_commit?.slice(0, 7) ?? "",
        subject: st.current_subject ?? "",
      });
    }
    if (st.kind === "merge") return st.message?.split("\n", 1)[0] ?? "";
    if (st.current_subject) return `${st.current_commit?.slice(0, 7) ?? ""} ${st.current_subject}`;
    return "";
  });

  const kindLabels: Record<ConflictFile["kind"], MessageKey> = {
    both_modified: "conflicts.kind.bothModified",
    both_added: "conflicts.kind.bothAdded",
    deleted_by_us: "conflicts.kind.deletedByUs",
    deleted_by_them: "conflicts.kind.deletedByThem",
    added_by_us: "conflicts.kind.addedByUs",
    added_by_them: "conflicts.kind.addedByThem",
    both_deleted: "conflicts.kind.bothDeleted",
  };

  async function fileOp(
    file: string,
    label: string,
    op: (path: string) => Promise<{ success: boolean; message: string }>,
  ) {
    const path = $activeRepoPath;
    if (!path || fileBusy) return;
    fileBusy = file;
    try {
      const res = await op(path);
      if (!res.success) toastError(label, res.message);
    } catch (err) {
      toastError(label, err);
    } finally {
      fileBusy = null;
      await refreshAll(path);
      await refreshOperation(path);
    }
  }

  function takeSide(files: string[], side: "ours" | "theirs") {
    const label = tr(side === "ours" ? "conflicts.takeOurs" : "conflicts.takeTheirs");
    return fileOp(files.length === 1 ? files[0] : "*", label, (p) =>
      tauri.resolveTakeSide(p, files, side),
    );
  }

  function markResolved(file: string) {
    return fileOp(file, tr("conflicts.markResolved"), (p) => tauri.markResolved(p, [file]));
  }

  async function openMergeTool(file: string) {
    const path = $activeRepoPath;
    if (!path) return;
    toast("info", tr("conflicts.openingMergeTool", { file }));
    await fileOp(file, tr("conflicts.mergeTool"), (p) =>
      tauri.openMergeTool(p, file, $settings.external_merge_tool),
    );
  }

  function menuItems(file: ConflictFile): MenuItem[] {
    const ours = st?.ours_label ?? $t("conflicts.oursLower");
    const theirs = st?.theirs_label ?? $t("conflicts.theirsLower");
    return [
      { label: $t("conflicts.resolveInEditor"), action: () => openConflictResolver(file.path) },
      { separator: true },
      {
        label: $t(file.has_ours ? "conflicts.takeSide" : "conflicts.takeSideDelete", { side: ours }),
        action: () => takeSide([file.path], "ours"),
      },
      {
        label: $t(file.has_theirs ? "conflicts.takeSide" : "conflicts.takeSideDelete", { side: theirs }),
        action: () => takeSide([file.path], "theirs"),
      },
      { label: $t("conflicts.openInMergeTool"), action: () => openMergeTool(file.path) },
      { separator: true },
      { label: $t("conflicts.markAsResolved"), action: () => markResolved(file.path) },
    ];
  }

  function onRowKey(e: KeyboardEvent, file: ConflictFile) {
    if (e.target !== e.currentTarget) return;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      openConflictResolver(file.path);
    }
  }
</script>

{#if active && st}
  <section class="op-banner" aria-label={$t("conflicts.operationInProgress")}>
    <div class="op-header">
      <span class="op-icon">
        {#if conflicts.length > 0}
          <AlertTriangle size={16} />
        {:else}
          <GitMerge size={16} />
        {/if}
      </span>
      <div class="op-text">
        <div class="op-title">{title}</div>
        {#if detail}
          <div class="op-detail" title={detail}>{detail}</div>
        {/if}
      </div>
      <div class="op-actions">
        {#if busy}
          <span class="op-busy"><Loader2 size={13} class="spinner" /> {busy}…</span>
        {/if}
        {#if bisect}
          {#if bisect.first_bad}
            <button class="op-btn primary" onclick={() => bisect.first_bad && revealCommit(bisect.first_bad)} title={$t("conflicts.selectInGraph")}>
              {$t("conflicts.showCommit")}
            </button>
          {:else if bisectTesting}
            <button class="op-btn" onclick={() => markBisect("good")} disabled={!!busy} title={$t("conflicts.bisectMarkTitle", { term: bisect.term_good })}>
              <CheckCircle2 size={12} /> {bisect.term_good === "good" ? $t("conflicts.good") : bisect.term_good}
            </button>
            <button class="op-btn" onclick={() => markBisect("bad")} disabled={!!busy} title={$t("conflicts.bisectMarkTitle", { term: bisect.term_bad })}>
              <AlertTriangle size={12} /> {bisect.term_bad === "bad" ? $t("conflicts.bad") : bisect.term_bad}
            </button>
            <button class="op-btn" onclick={() => markBisect("skip")} disabled={!!busy} title={$t("conflicts.bisectSkipTitle")}>
              <SkipForward size={12} /> {$t("conflicts.skip")}
            </button>
          {/if}
        {/if}
        {#if inOperation}
          {#if st.kind !== "bisect"}
            <button
              class="op-btn primary"
              onclick={requestContinue}
              disabled={!!busy || conflicts.length > 0}
              title={conflicts.length > 0 ? $t("conflicts.resolveAllFirst") : $t("conflicts.continue")}
            >
              <Play size={12} /> {$t("conflicts.continue")}
            </button>
          {/if}
          {#if st.can_skip}
            <button class="op-btn" onclick={skipOperation} disabled={!!busy} title={$t("conflicts.skipCurrent")}>
              <SkipForward size={12} /> {$t("conflicts.skip")}
            </button>
          {/if}
          <button
            class="op-btn danger"
            onclick={abortOperation}
            disabled={!!busy}
            title={st.kind === "bisect" ? $t("conflicts.endBisectTitle") : $t("conflicts.abortTitle")}
          >
            <X size={12} /> {st.kind === "bisect" ? $t("common.reset") : $t("conflicts.abort")}
          </button>
        {/if}
      </div>
    </div>

    {#if conflicts.length > 0}
      <div class="conflicts">
        <div class="conflicts-header">
          <button
            class="toggle"
            onclick={() => (expanded = !expanded)}
            aria-expanded={expanded}
          >
            {#if expanded}<ChevronDown size={13} />{:else}<ChevronRight size={13} />{/if}
            {$t("conflicts.conflictedFiles", { count: conflicts.length })}
          </button>
          <span class="spacer"></span>
          <button
            class="link-btn"
            disabled={!!fileBusy}
            onclick={() => takeSide(conflicts.map((c) => c.path), "ours")}
            title={$t("conflicts.resolveAllWith", { side: st.ours_label })}
          >{$t("conflicts.allOurs")}</button>
          <button
            class="link-btn"
            disabled={!!fileBusy}
            onclick={() => takeSide(conflicts.map((c) => c.path), "theirs")}
            title={$t("conflicts.resolveAllWith", { side: st.theirs_label })}
          >{$t("conflicts.allTheirs")}</button>
        </div>
        {#if expanded}
          <div class="conflict-list">
            {#each conflicts as file (file.path)}
              <div
                class="conflict-row"
                role="button"
                tabindex="0"
                onclick={() => openConflictResolver(file.path)}
                onkeydown={(e) => onRowKey(e, file)}
                oncontextmenu={(e) => {
                  e.preventDefault();
                  menu = { x: e.clientX, y: e.clientY, file };
                }}
                title={$t("conflicts.rowTitle")}
              >
                {#if fileBusy === file.path || fileBusy === "*"}
                  <Loader2 size={13} class="spinner" />
                {:else}
                  <FileWarning size={13} class="conflict-icon" />
                {/if}
                <span class="path">{file.path}</span>
                <span class="kind">{$t(kindLabels[file.kind])}</span>
                <span class="row-actions">
                  <button
                    class="mini-btn"
                    disabled={!!fileBusy}
                    onclick={(e) => { e.stopPropagation(); takeSide([file.path], "ours"); }}
                    title={st.ours_label}
                  >{$t("conflicts.ours")}</button>
                  <button
                    class="mini-btn"
                    disabled={!!fileBusy}
                    onclick={(e) => { e.stopPropagation(); takeSide([file.path], "theirs"); }}
                    title={st.theirs_label}
                  >{$t("conflicts.theirs")}</button>
                  <button
                    class="mini-btn"
                    disabled={!!fileBusy}
                    onclick={(e) => { e.stopPropagation(); openMergeTool(file.path); }}
                    title={$t("conflicts.openInExternalTool")}
                  >{$t("conflicts.tool")}</button>
                  <button
                    class="mini-btn icon"
                    disabled={!!fileBusy}
                    onclick={(e) => { e.stopPropagation(); markResolved(file.path); }}
                    title={$t("conflicts.markResolvedStage")}
                    aria-label={$t("conflicts.markFileResolved", { file: file.path })}
                  ><CheckCircle2 size={12} /></button>
                </span>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {:else if inOperation && st.kind !== "bisect"}
      <div class="all-resolved">
        <CheckCircle2 size={13} />
        {st.stopped_for_edit ? $t("conflicts.noConflictsContinue") : $t("conflicts.allResolvedContinue")}
      </div>
    {/if}
  </section>
{/if}

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menuItems(menu.file)} onclose={() => (menu = null)} />
{/if}

<style>
  .op-banner {
    flex-shrink: 0;
    border-bottom: 1px solid var(--color-border);
    background: color-mix(in srgb, var(--color-lane-2) 10%, var(--color-surface));
    font-size: 12px;
  }

  .op-header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
  }

  .op-icon {
    display: flex;
    color: var(--color-lane-2);
    flex-shrink: 0;
  }

  .op-text {
    flex: 1;
    min-width: 0;
  }

  .op-title {
    font-weight: 600;
    color: var(--color-text-primary);
  }

  .op-detail {
    color: var(--color-text-muted);
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .op-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .op-busy {
    display: flex;
    align-items: center;
    gap: 4px;
    color: var(--color-text-muted);
    font-size: 11px;
  }

  .op-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 11px;
    cursor: pointer;
  }

  .op-btn:hover:not(:disabled) {
    background: var(--color-surface-elevated);
  }

  .op-btn.primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: var(--color-on-accent);
    font-weight: 500;
  }

  .op-btn.primary:hover:not(:disabled) {
    background: var(--color-accent);
    opacity: 0.9;
  }

  .op-btn.danger:hover:not(:disabled) {
    background: var(--color-diff-del-bg);
    color: var(--color-diff-del-text);
    border-color: var(--color-diff-del-text);
  }

  .op-btn:disabled,
  .mini-btn:disabled,
  .link-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .conflicts {
    border-top: 1px solid var(--color-border);
  }

  .conflicts-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 12px;
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: 4px;
    border: none;
    background: none;
    padding: 2px 0;
    color: var(--color-diff-del-text);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.3px;
    cursor: pointer;
  }

  .spacer {
    flex: 1;
  }

  .link-btn {
    border: none;
    background: none;
    color: var(--color-accent);
    font-size: 11px;
    cursor: pointer;
    padding: 2px 4px;
  }

  .link-btn:hover:not(:disabled) {
    text-decoration: underline;
  }

  .conflict-list {
    max-height: 160px;
    overflow-y: auto;
    padding-bottom: 4px;
  }

  .conflict-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 12px 3px 24px;
    cursor: pointer;
    color: var(--color-text-primary);
  }

  .conflict-row:hover,
  .conflict-row:focus-visible {
    background: color-mix(in srgb, var(--color-text-primary) 6%, transparent);
  }

  .conflict-row :global(.conflict-icon) {
    color: var(--color-diff-del-text);
    flex-shrink: 0;
  }

  .path {
    flex: 1;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .kind {
    color: var(--color-text-muted);
    font-size: 10px;
    flex-shrink: 0;
  }

  .row-actions {
    display: flex;
    gap: 3px;
    flex-shrink: 0;
  }

  .mini-btn {
    display: flex;
    align-items: center;
    padding: 1px 6px;
    border: 1px solid var(--color-border);
    border-radius: 3px;
    background: var(--color-surface);
    color: var(--color-text-muted);
    font-size: 10px;
    cursor: pointer;
  }

  .mini-btn.icon {
    padding: 1px 4px;
  }

  .mini-btn:hover:not(:disabled) {
    color: var(--color-accent);
    border-color: var(--color-accent);
  }

  .all-resolved {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 12px 8px;
    color: var(--color-diff-add-text);
    font-size: 11px;
  }

  .op-banner :global(.spinner) {
    animation: op-spin 1s linear infinite;
  }

  @keyframes op-spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
