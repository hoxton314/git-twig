<script lang="ts">
  import { onMount, type Snippet } from "svelte";

  interface Props {
    /** Accessible name of the dialog. */
    label: string;
    onclose: () => void;
    width?: number;
    children: Snippet;
  }

  let { label, onclose, width = 260, children }: Props = $props();

  let el = $state<HTMLDivElement | null>(null);
  // Fixed-positioned under the anchoring parent so the graph panel's
  // `overflow: hidden` can't clip it when the panel is short.
  let top = $state(0);
  let right = $state(0);
  let maxHeight = $state(400);

  function place() {
    const anchor = el?.parentElement;
    if (!anchor) return;
    const r = anchor.getBoundingClientRect();
    top = r.bottom + 4;
    right = Math.max(4, window.innerWidth - r.right);
    maxHeight = Math.max(160, window.innerHeight - top - 8);
  }

  onMount(() => {
    place();
    const prev = document.activeElement as HTMLElement | null;
    function onPointerDown(e: PointerEvent) {
      const target = e.target as HTMLElement;
      if (!el || el.contains(target)) return;
      // Clicks on the button that toggles this popover are handled by it.
      if (target.closest?.("[data-popover-toggle]")) return;
      onclose();
    }
    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("resize", onclose);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("resize", onclose);
      if (prev && el?.contains(document.activeElement)) prev.focus({ preventScroll: true });
    };
  });

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onclose();
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="graph-popover"
  bind:this={el}
  role="dialog"
  tabindex="-1"
  aria-label={label}
  style="width: {width}px; top: {top}px; right: {right}px; max-height: {maxHeight}px"
  onkeydown={onKeydown}
>
  {@render children()}
</div>

<style>
  .graph-popover {
    position: fixed;
    z-index: 50;
    overflow-y: auto;
    padding: 10px;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-surface-elevated);
    box-shadow: 0 8px 24px color-mix(in srgb, var(--color-bg) 60%, transparent);
    color: var(--color-text-primary);
    font-size: 12px;
  }
</style>
