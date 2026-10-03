<script module lang="ts">
  // Shared stack of currently-open modals so a single Escape press closes only
  // the topmost one instead of every mounted modal at once.
  const openStack: symbol[] = [];
</script>

<script lang="ts">
  import { X } from "lucide-svelte";
  import { onMount } from "svelte";
  import { t } from "../../lib/i18n";

  interface Props {
    open: boolean;
    title: string;
    onclose: () => void;
    width?: string;
    children?: import("svelte").Snippet;
  }

  let { open, title, onclose, width = "480px", children }: Props = $props();

  const id = Symbol("modal");
  const titleId = `modal-title-${Math.random().toString(36).slice(2, 10)}`;

  let card = $state<HTMLDivElement | null>(null);

  const FOCUSABLE =
    'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

  function focusables(): HTMLElement[] {
    if (!card) return [];
    return Array.from(card.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
      (el) => el.offsetParent !== null || el === document.activeElement,
    );
  }

  // Keep this modal's presence in the shared stack in sync with `open`.
  $effect(() => {
    if (open) {
      openStack.push(id);
      return () => {
        const i = openStack.lastIndexOf(id);
        if (i !== -1) openStack.splice(i, 1);
      };
    }
  });

  // Move focus into the dialog when it opens and restore it on close.
  $effect(() => {
    if (!open || !card) return;
    const previouslyFocused = document.activeElement as HTMLElement | null;
    const el = card;
    queueMicrotask(() => {
      if (el.contains(document.activeElement)) return;
      // Prefer the first form field; otherwise focus the dialog itself rather
      // than a button, so a stray Enter can't trigger e.g. "Merge".
      const field = focusables().find((f) => /^(INPUT|TEXTAREA|SELECT)$/.test(f.tagName));
      (field ?? el).focus();
    });
    return () => {
      if (previouslyFocused && document.contains(previouslyFocused)) {
        previouslyFocused.focus();
      }
    };
  });

  function handleKeydown(e: KeyboardEvent) {
    if (!open || openStack[openStack.length - 1] !== id) return;
    if (e.key === "Escape") {
      e.stopPropagation();
      onclose();
      return;
    }
    // Trap Tab focus within the topmost dialog.
    if (e.key === "Tab" && card) {
      const items = focusables();
      if (items.length === 0) {
        e.preventDefault();
        card.focus();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;
      if (e.shiftKey && (active === first || !card.contains(active))) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && (active === last || !card.contains(active))) {
        e.preventDefault();
        first.focus();
      }
    }
  }

  // Only close when the press *started* on the backdrop too; otherwise a text
  // selection drag that ends outside the card would dismiss the dialog.
  let pressStartedOnBackdrop = false;

  function handleBackdropMousedown(e: MouseEvent) {
    pressStartedOnBackdrop = e.target === e.currentTarget;
  }

  function handleBackdropClick(e: MouseEvent) {
    if (pressStartedOnBackdrop && e.target === e.currentTarget) {
      onclose();
    }
    pressStartedOnBackdrop = false;
  }

  onMount(() => {
    document.addEventListener("keydown", handleKeydown);
    return () => document.removeEventListener("keydown", handleKeydown);
  });
</script>

{#if open}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="modal-backdrop" onmousedown={handleBackdropMousedown} onclick={handleBackdropClick}>
    <div
      class="modal-card"
      style="width: {width}; max-width: calc(100vw - 48px);"
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      tabindex="-1"
      bind:this={card}
    >
      <div class="modal-header">
        <h2 class="modal-title" id={titleId}>{title}</h2>
        <button class="modal-close" onclick={onclose} title={$t("common.close")} aria-label={$t("common.close")}>
          <X size={16} />
        </button>
      </div>
      <div class="modal-body">
        {#if children}
          {@render children()}
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.5);
  }

  .modal-card {
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: 8px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.4);
    display: flex;
    flex-direction: column;
    max-height: calc(100vh - 80px);
    overflow: hidden;
    outline: none;
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .modal-title {
    font-size: 15px;
    font-weight: 600;
    color: var(--color-text-primary);
    margin: 0;
  }

  .modal-close {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 0;
  }

  .modal-close:hover {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .modal-body {
    padding: 20px;
    overflow-y: auto;
    flex: 1;
  }
</style>
