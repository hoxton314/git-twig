<script lang="ts">
  import { X } from "lucide-svelte";
  import { toasts, dismissToast, type Toast } from "../../lib/stores/toasts";
  import { t } from "../../lib/i18n";

  async function runAction(item: Toast) {
    dismissToast(item.id);
    await item.action?.run();
  }
</script>

<div class="toaster" aria-live="polite" aria-relevant="additions">
  {#each $toasts as item (item.id)}
    <div class="toast {item.kind}" role={item.kind === "error" ? "alert" : "status"}>
      <div class="body">
        {#if item.title}<div class="title">{item.title}</div>{/if}
        <div class="message">{item.message}</div>
      </div>
      {#if item.action}
        <button class="action" onclick={() => runAction(item)}>{item.action.label}</button>
      {/if}
      <button class="close" aria-label={$t("shared.dismissNotification")} onclick={() => dismissToast(item.id)}>
        <X size={12} />
      </button>
    </div>
  {/each}
</div>

<style>
  .toaster {
    position: fixed;
    right: 16px;
    bottom: 16px;
    z-index: 1100;
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: min(420px, calc(100vw - 32px));
    pointer-events: none;
  }

  .toast {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 12px;
    border: 1px solid var(--color-border);
    border-left: 3px solid var(--color-accent);
    border-radius: 6px;
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
    font-size: 12px;
    pointer-events: auto;
  }

  .toast.success {
    border-left-color: var(--color-diff-add-text);
  }
  .toast.error {
    border-left-color: var(--color-diff-del-text);
  }
  .toast.warning {
    border-left-color: var(--color-lane-2);
  }

  .body {
    flex: 1;
    min-width: 0;
  }

  .title {
    font-weight: 600;
    margin-bottom: 2px;
  }

  .message {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    max-height: 160px;
    overflow-y: auto;
  }

  .action {
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: transparent;
    color: var(--color-accent);
    font-size: 12px;
    padding: 2px 8px;
    cursor: pointer;
  }

  .close {
    border: none;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 2px;
  }

  .close:hover {
    color: var(--color-text-primary);
  }
</style>
