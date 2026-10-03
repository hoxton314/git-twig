<script lang="ts">
  /** "Squash N commits" dialog, opened from a multi-selection in the graph. */
  import Modal from "../shared/Modal.svelte";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { commitSelection, refreshAll, selectedCommitOid } from "../../lib/stores/graph";
  import { squashTarget, undoHistoryOpen } from "../../lib/stores/commitUi";
  import { toast, toastError } from "../../lib/stores/toasts";
  import { operationState, refreshOperation } from "../../lib/stores/operation";
  import { EMPTY_SELECTION } from "../../lib/graphSelection";
  import * as tauri from "../../lib/tauri";
  import { t, tr } from "../../lib/i18n";
  import type { SquashPlan } from "../../lib/types/git";

  const target = $derived($squashTarget);

  let plan = $state<SquashPlan | null>(null);
  let message = $state("");
  let autostash = $state(true);
  let busy = $state(false);
  let error = $state<string | null>(null);

  // Validate (and fetch the default message) each time the dialog opens.
  let request = 0;
  $effect(() => {
    const oids = target;
    const path = $activeRepoPath;
    plan = null;
    error = null;
    message = "";
    if (!oids || !path) return;
    const req = ++request;
    tauri.planSquash(path, oids).then(
      (p) => {
        if (req !== request) return;
        plan = p;
        message = p.message;
      },
      (err) => {
        if (req !== request) return;
        error = err instanceof Error ? err.message : String(err);
      },
    );
  });

  function close() {
    if (!busy) $squashTarget = null;
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    const path = $activeRepoPath;
    const oids = target;
    if (!path || !oids || !plan || !message.trim() || busy) return;
    busy = true;
    error = null;
    try {
      const res = await tauri.squashCommits(path, oids, message, autostash);
      await refreshAll(path);
      await refreshOperation(path);
      if ($operationState?.kind === "rebase") {
        // Stopped part-way (e.g. a newer commit conflicts): the banner owns it now.
        $commitSelection = EMPTY_SELECTION;
        toast("warning", tr("graph.squashPaused"), {
          title: tr("graph.squashTitle"),
          duration: 0,
        });
        busy = false;
        close();
        return;
      }
      if (!res.success) {
        error = res.message.trim() || tr("graph.squashIncomplete");
        return;
      }
      // The squashed commits no longer exist.
      $commitSelection = EMPTY_SELECTION;
      $selectedCommitOid = null;
      toast("success", tr("graph.squashed", { count: plan.count }), {
        action: { label: tr("commits.undoHistory"), run: () => undoHistoryOpen.set(true) },
      });
      busy = false;
      close();
    } catch (err) {
      toastError(tr("graph.squashFailed"), err);
    } finally {
      busy = false;
    }
  }
</script>

<Modal open={!!target} title={$t("graph.squashDialogTitle")} onclose={close} width="520px">
  {#if target}
    <form class="dialog" onsubmit={submit}>
      {#if plan}
        <p class="desc">
          {$t("graph.squashCombine", { count: plan.count })} <code>{plan.first.slice(0, 7)}</code>.
          {#if plan.later > 0}
            {$t("graph.squashLater", { count: plan.later })}
          {/if}
        </p>
        {#if plan.pushed_to.length > 0}
          <p class="warn" role="alert">
            {$t("graph.squashPushed", { branches: plan.pushed_to.join(", ") })}
          </p>
        {:else if plan.pushed_unknown}
          <p class="desc">{$t("graph.squashPushedUnknown")}</p>
        {/if}
        <label class="field">
          <span>{$t("graph.squashMessage")}</span>
          <textarea bind:value={message} rows="8" spellcheck="false" aria-label={$t("graph.squashMessageLabel")}></textarea>
        </label>
        <label class="check">
          <input type="checkbox" bind:checked={autostash} />
          <span>{$t("graph.squashAutostash")}</span>
        </label>
      {:else if !error}
        <p class="desc">{$t("graph.squashChecking")}</p>
      {/if}
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <div class="actions">
        <button type="button" class="btn-secondary" onclick={close} disabled={busy}>
          {plan ? $t("common.cancel") : $t("common.close")}
        </button>
        {#if plan}
          <button type="submit" class="btn-primary" disabled={busy || !message.trim()}>
            {busy ? $t("graph.squashing") : $t("graph.squashButton", { count: plan.count })}
          </button>
        {/if}
      </div>
    </form>
  {/if}
</Modal>

<style>
  .dialog { display: flex; flex-direction: column; gap: 12px; font-size: 13px; }
  .desc { margin: 0; color: var(--color-text-muted); }
  code { font-family: var(--font-mono); color: var(--color-accent); }
  .field { display: flex; flex-direction: column; gap: 4px; color: var(--color-text-muted); font-size: 12px; }
  .field textarea {
    padding: 6px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-family: var(--font-mono);
    font-size: 12px;
    resize: vertical;
    outline: none;
  }
  .field textarea:focus { border-color: var(--color-accent); }
  .check { display: flex; align-items: center; gap: 6px; color: var(--color-text-primary); font-size: 12px; cursor: pointer; }
  .warn {
    margin: 0;
    padding: 8px 10px;
    border-radius: 4px;
    background: var(--color-diff-del-bg);
    color: var(--color-text-primary);
    font-size: 12px;
  }
  .error { margin: 0; color: var(--color-diff-del-text); font-size: 12px; white-space: pre-wrap; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; }
  .btn-primary, .btn-secondary {
    border-radius: 4px;
    padding: 6px 16px;
    font-size: 13px;
    cursor: pointer;
  }
  .btn-primary { background: var(--color-accent); color: var(--color-bg); border: none; font-weight: 600; }
  .btn-secondary { background: transparent; color: var(--color-text-muted); border: 1px solid var(--color-border); }
  .btn-secondary:hover:not(:disabled) { color: var(--color-text-primary); }
  .btn-primary:disabled, .btn-secondary:disabled { opacity: 0.6; cursor: not-allowed; }
</style>
