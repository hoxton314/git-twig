<script lang="ts">
  import Modal from "../shared/Modal.svelte";
  import { t, tr, splitMessage } from "../../lib/i18n";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { refreshAll } from "../../lib/stores/graph";
  import { createTagTarget } from "../../lib/stores/commitUi";
  import { refreshTags } from "../../lib/stores/tags";
  import { toast, toastError } from "../../lib/stores/toasts";
  import * as tauri from "../../lib/tauri";

  const target = $derived($createTagTarget);

  let name = $state("");
  let annotated = $state(false);
  let message = $state("");
  let push = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);

  $effect(() => {
    if (target) {
      name = "";
      message = "";
      annotated = false;
      push = false;
      error = null;
    }
  });

  function close() {
    if (!busy) $createTagTarget = null;
  }


  async function submit(e: SubmitEvent) {
    e.preventDefault();
    const path = $activeRepoPath;
    const tag = name.trim();
    if (!path || !target || !tag || busy) return;
    if (annotated && !message.trim()) {
      error = tr("tags.needMessage");
      return;
    }
    busy = true;
    error = null;
    try {
      const res = await tauri.createTag(path, tag, target.oid, annotated ? message : undefined);
      if (!res.success) {
        error = res.message.trim() || tr("tags.couldNotCreate");
        return;
      }
      await Promise.all([refreshAll(path), refreshTags(path)]);
      if (push) {
        const pushed = await tauri.pushTag(path, tag);
        if (pushed.success) toast("success", tr("tags.createdPushed", { name: tag, result: pushed.message.toLowerCase() }));
        else toast("error", pushed.message.trim(), { title: tr("tags.createdPushFailed", { name: tag }) });
      } else {
        toast("success", tr("tags.createdAt", { name: tag, target: target.label }));
      }
      busy = false;
      close();
    } catch (err) {
      toastError(tr("tags.createFailed"), err);
    } finally {
      busy = false;
    }
  }
</script>

<Modal open={!!target} title={$t("tags.createTitle")} onclose={close} width="420px">
  {#if target}
    <form class="dialog" onsubmit={submit}>
      <p class="desc">{#each splitMessage($t("tags.newAt")) as part, i}{#if i % 2}<code>{target.label}</code>{:else}{part}{/if}{/each}</p>
      <label class="field">
        <span>{$t("tags.name")}</span>
        <input type="text" bind:value={name} placeholder="v1.0.0" spellcheck="false" autocomplete="off" />
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={annotated} />
        <span>{$t("tags.annotatedOption")}</span>
      </label>
      {#if annotated}
        <label class="field">
          <span>{$t("tags.message")}</span>
          <textarea rows="4" bind:value={message} placeholder={$t("tags.messagePlaceholder")}></textarea>
        </label>
      {/if}
      <label class="check">
        <input type="checkbox" bind:checked={push} />
        <span>{$t("tags.pushAfter")}</span>
      </label>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <div class="actions">
        <button type="button" class="btn-secondary" onclick={close} disabled={busy}>{$t("common.cancel")}</button>
        <button type="submit" class="btn-primary" disabled={busy || !name.trim()}>
          {busy ? $t("tags.creating") : $t("tags.createTitle")}
        </button>
      </div>
    </form>
  {/if}
</Modal>

<style>
  .dialog { display: flex; flex-direction: column; gap: 12px; font-size: 13px; }
  .desc { margin: 0; color: var(--color-text-muted); }
  code { font-family: var(--font-mono); color: var(--color-accent); }
  .field { display: flex; flex-direction: column; gap: 4px; color: var(--color-text-muted); font-size: 12px; }
  .field input, .field textarea {
    padding: 6px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 13px;
    font-family: inherit;
    outline: none;
    resize: vertical;
  }
  .field input:focus, .field textarea:focus { border-color: var(--color-accent); }
  .check { display: flex; align-items: center; gap: 6px; color: var(--color-text-primary); font-size: 12px; cursor: pointer; }
  .error { margin: 0; color: var(--color-diff-del-text); font-size: 12px; white-space: pre-wrap; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; }
  .btn-primary, .btn-secondary {
    border-radius: 4px;
    padding: 6px 16px;
    font-size: 13px;
    cursor: pointer;
  }
  .btn-primary { background: var(--color-accent); color: var(--color-on-accent); border: none; font-weight: 600; }
  .btn-secondary { background: transparent; color: var(--color-text-muted); border: 1px solid var(--color-border); }
  .btn-secondary:hover:not(:disabled) { color: var(--color-text-primary); }
  .btn-primary:disabled, .btn-secondary:disabled { opacity: 0.6; cursor: not-allowed; }
</style>
