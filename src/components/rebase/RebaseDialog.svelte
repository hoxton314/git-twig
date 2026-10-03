<script lang="ts">
  import Modal from "../shared/Modal.svelte";
  import { GitBranch, Loader2, ListOrdered } from "lucide-svelte";
  import {
    rebaseDialog,
    rebaseOnto,
    openInteractiveRebase,
    operationBusy,
    operationState,
  } from "../../lib/stores/operation";
  import { activeRepo } from "../../lib/stores/repos";
  import { branches, workingStatus } from "../../lib/stores/graph";
  import { t } from "../../lib/i18n";

  const dialog = $derived($rebaseDialog);
  let target = $state("");
  let autostash = $state(true);
  let wasOpen = false;

  $effect(() => {
    if (dialog.open && !wasOpen) target = dialog.target;
    wasOpen = dialog.open;
  });

  const head = $derived($branches.find((b) => b.is_head && !b.is_remote) ?? null);
  const headName = $derived($activeRepo?.head_name ?? "HEAD");
  const options = $derived(
    $branches
      .filter((b) => !b.is_head && !b.name.endsWith("/HEAD"))
      .map((b) => b.name)
      .sort((a, b) => a.localeCompare(b)),
  );
  const dirty = $derived($workingStatus.staged.length + $workingStatus.unstaged.length > 0);
  const busyOp = $derived($operationState !== null && $operationState.kind !== "none");
  const pushed = $derived(head?.upstream != null);
  /** The hint, split where the bold branch name goes. */
  const replays = $derived($t("rebase.replays").split("{branch}"));

  function close() {
    rebaseDialog.set({ open: false, target: "" });
  }

  async function submit() {
    const onto = target.trim();
    if (!onto || $operationBusy || busyOp) return;
    close();
    await rebaseOnto(onto, autostash);
  }

  function interactive() {
    const base = target.trim();
    close();
    openInteractiveRebase(base);
  }
</script>

<Modal open={dialog.open} title={$t("rebase.title", { branch: headName })} onclose={close} width="480px">
  <form class="body" onsubmit={(e) => { e.preventDefault(); submit(); }}>
    <label class="field">
      <span class="label">{$t("rebase.onto")}</span>
      <div class="input-wrap">
        <GitBranch size={13} />
        <input
          type="text"
          list="rebase-targets"
          bind:value={target}
          placeholder={$t("rebase.ontoPlaceholder")}
          spellcheck="false"
          autocomplete="off"
        />
      </div>
      <datalist id="rebase-targets">
        {#each options as name (name)}
          <option value={name}></option>
        {/each}
      </datalist>
    </label>

    <p class="hint">
      {replays[0]}<strong>{headName}</strong>{replays[1] ?? ""}
    </p>

    {#if dirty}
      <label class="check">
        <input type="checkbox" bind:checked={autostash} />
        {$t("rebase.autostashLabel")}
      </label>
    {/if}

    {#if pushed}
      <p class="warn">
        {$t("rebase.tracksWarning", { branch: headName, upstream: head?.upstream ?? "" })}
      </p>
    {/if}
    {#if busyOp}
      <p class="warn">{$t("rebase.anotherInProgress")}</p>
    {/if}

    <div class="footer">
      <button type="button" class="btn" onclick={interactive} disabled={busyOp} title={$t("rebase.interactiveButtonTitle")}>
        <ListOrdered size={13} /> {$t("rebase.interactiveButton")}
      </button>
      <span class="spacer"></span>
      <button type="button" class="btn" onclick={close}>{$t("common.cancel")}</button>
      <button type="submit" class="btn primary" disabled={!target.trim() || !!$operationBusy || busyOp}>
        {#if $operationBusy}<Loader2 size={13} class="spinner" />{/if}
        {$t("rebase.rebase")}
      </button>
    </div>
  </form>
</Modal>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .label {
    font-size: 12px;
    color: var(--color-text-muted);
  }

  .input-wrap {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-muted);
  }

  .input-wrap:focus-within {
    border-color: var(--color-accent);
  }

  .input-wrap input {
    flex: 1;
    border: none;
    background: none;
    outline: none;
    padding: 7px 0;
    color: var(--color-text-primary);
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--color-text-muted);
  }

  .hint strong {
    color: var(--color-text-primary);
    font-weight: 500;
  }

  .warn {
    margin: 0;
    padding: 6px 8px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--color-lane-2) 12%, transparent);
    color: var(--color-lane-2);
    font-size: 12px;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--color-text-primary);
  }

  .footer {
    display: flex;
    align-items: center;
    gap: 8px;
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
    animation: rb-spin 1s linear infinite;
  }

  @keyframes rb-spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
