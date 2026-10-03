<!-- Name prompt used for "Rename…" and "Create branch from here…". -->
<script lang="ts">
  import { tick, untrack } from "svelte";
  import Modal from "../shared/Modal.svelte";
  import { t } from "../../lib/i18n";

  interface Props {
    open: boolean;
    title: string;
    /** Rendered above the input, e.g. "Create a branch at origin/main". */
    description?: string;
    initial?: string;
    confirmLabel: string;
    /** Show a "Check out" checkbox (create flow). */
    showCheckout?: boolean;
    /** Names that already exist; a clash is reported inline. */
    existing?: string[];
    busy?: boolean;
    onconfirm: (name: string, checkout: boolean) => void;
    onclose: () => void;
  }

  let {
    open,
    title,
    description = "",
    initial = "",
    confirmLabel,
    showCheckout = false,
    existing = [],
    busy = false,
    onconfirm,
    onclose,
  }: Props = $props();

  let name = $state("");
  let checkout = $state(true);
  let inputEl: HTMLInputElement | undefined = $state(undefined);

  // Reset and focus/select the input each time the dialog opens.
  $effect(() => {
    if (!open) return;
    name = untrack(() => initial);
    checkout = true;
    tick().then(() => {
      inputEl?.focus();
      inputEl?.select();
    });
  });

  const trimmed = $derived(name.trim());
  /** Mirrors the most common `git check-ref-format` rules for instant feedback. */
  const error = $derived.by(() => {
    if (!trimmed) return "";
    if (trimmed !== initial && existing.includes(trimmed)) return $t("branches.nameExists");
    if (/\s/.test(trimmed)) return $t("branches.nameSpaces");
    if (trimmed.startsWith("-")) return $t("branches.nameDash");
    if (/\.\.|[~^:?*[\\]|@\{/.test(trimmed)) return $t("branches.nameChars");
    if (trimmed.endsWith("/") || trimmed.endsWith(".") || trimmed.endsWith(".lock") || trimmed.includes("//"))
      return $t("branches.nameInvalid");
    return "";
  });
  const canSubmit = $derived(!!trimmed && !error && trimmed !== initial && !busy);

  function submit() {
    if (canSubmit) onconfirm(trimmed, checkout);
  }
</script>

<Modal {open} {title} {onclose} width="400px">
  <form
    class="name-dialog"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    {#if description}<p class="desc">{description}</p>{/if}
    <input
      bind:this={inputEl}
      bind:value={name}
      placeholder={$t("branches.name")}
      aria-label={$t("branches.name")}
      aria-invalid={!!error}
      spellcheck="false"
      autocomplete="off"
    />
    {#if error}<p class="error">{error}</p>{/if}
    {#if showCheckout}
      <label class="checkbox">
        <input type="checkbox" bind:checked={checkout} />
        {$t("branches.checkoutAfter")}
      </label>
    {/if}
    <div class="actions">
      <button type="button" class="btn-secondary" onclick={onclose} disabled={busy}>{$t("common.cancel")}</button>
      <button type="submit" class="btn-primary" disabled={!canSubmit}>
        {busy ? $t("branches.working") : confirmLabel}
      </button>
    </div>
  </form>
</Modal>

<style>
  .name-dialog {
    display: flex;
    flex-direction: column;
    gap: 10px;
    font-size: 13px;
  }

  .desc {
    margin: 0;
    color: var(--color-text-muted);
    line-height: 1.5;
    word-break: break-word;
  }

  input:not([type="checkbox"]) {
    padding: 6px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 13px;
    outline: none;
  }

  input:not([type="checkbox"]):focus {
    border-color: var(--color-accent);
  }

  input[aria-invalid="true"] {
    border-color: var(--color-diff-del-text);
  }

  .error {
    margin: -4px 0 0;
    color: var(--color-diff-del-text);
    font-size: 12px;
  }

  .checkbox {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .btn-primary,
  .btn-secondary {
    border-radius: 4px;
    padding: 6px 16px;
    font-size: 13px;
    cursor: pointer;
  }

  .btn-primary {
    background: var(--color-accent);
    color: var(--color-bg);
    border: none;
    font-weight: 600;
  }

  .btn-secondary {
    background: transparent;
    color: var(--color-text-muted);
    border: 1px solid var(--color-border);
  }

  .btn-primary:disabled,
  .btn-secondary:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
