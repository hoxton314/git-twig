<script lang="ts">
  import Modal from "../shared/Modal.svelte";
  import { Loader2, Play } from "lucide-svelte";
  import {
    continueDialog,
    continueOperation,
    operationState,
    operationBusy,
    operationLabel,
  } from "../../lib/stores/operation";

  const st = $derived($operationState);
  const dialog = $derived($continueDialog);
  let message = $state("");
  let wasOpen = false;

  // Load the prepared message each time the dialog opens.
  $effect(() => {
    if (dialog.open && !wasOpen) message = dialog.message;
    wasOpen = dialog.open;
  });

  const kind = $derived(st?.kind ?? "none");
  const title = $derived(
    kind === "merge" ? "Commit Merge" : `Continue ${operationLabel(kind)}`,
  );

  function close() {
    continueDialog.set({ open: false, message: "" });
  }

  async function submit() {
    if (!message.trim() || $operationBusy) return;
    const msg = message;
    close();
    await continueOperation(msg);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      submit();
    }
  }
</script>

<Modal open={dialog.open} {title} onclose={close} width="560px">
  <div class="body">
    <p class="hint">
      {#if kind === "rebase"}
        Message for the rebased commit{st?.current_subject ? ` "${st.current_subject}"` : ""}.
      {:else if kind === "merge"}
        All conflicts are resolved. Review the merge commit message.
      {:else}
        Message for the {operationLabel(kind).toLowerCase()} commit.
      {/if}
    </p>
    <textarea
      class="msg"
      bind:value={message}
      rows="8"
      spellcheck="true"
      aria-label="Commit message"
      onkeydown={onKeydown}
    ></textarea>
    <div class="footer">
      <span class="shortcut">Ctrl+Enter to continue</span>
      <button class="btn" onclick={close}>Cancel</button>
      <button class="btn primary" onclick={submit} disabled={!message.trim() || !!$operationBusy}>
        {#if $operationBusy}<Loader2 size={13} class="spinner" />{:else}<Play size={13} />{/if}
        {kind === "merge" ? "Commit merge" : "Continue"}
      </button>
    </div>
  </div>
</Modal>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .hint {
    margin: 0;
    color: var(--color-text-muted);
    font-size: 12px;
  }

  .msg {
    width: 100%;
    box-sizing: border-box;
    padding: 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-family: var(--font-mono);
    font-size: 12px;
    resize: vertical;
    outline: none;
  }

  .msg:focus {
    border-color: var(--color-accent);
  }

  .footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
  }

  .shortcut {
    margin-right: auto;
    color: var(--color-text-muted);
    font-size: 11px;
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

  .body :global(.spinner) {
    animation: cd-spin 1s linear infinite;
  }

  @keyframes cd-spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
