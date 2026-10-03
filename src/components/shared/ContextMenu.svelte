<script module lang="ts">
  export interface MenuItem {
    /** Omit for a separator. */
    label?: string;
    action?: () => void | Promise<void>;
    /** Shown right-aligned, e.g. "Ctrl+C". */
    shortcut?: string;
    danger?: boolean;
    disabled?: boolean;
    separator?: boolean;
  }
</script>

<script lang="ts">
  import { onMount, tick } from "svelte";

  interface Props {
    /** Viewport coordinates, usually `event.clientX` / `event.clientY`. */
    x: number;
    y: number;
    items: MenuItem[];
    onclose: () => void;
  }

  let { x, y, items, onclose }: Props = $props();

  let menu = $state<HTMLDivElement | null>(null);
  let left = $state(0);
  let top = $state(0);

  const actionable = $derived(
    items.map((it, i) => ({ it, i })).filter(({ it }) => !it.separator && !it.disabled),
  );

  // Clamp into the viewport once the menu's size is known.
  $effect(() => {
    const px = x;
    const py = y;
    if (!menu) return;
    const rect = menu.getBoundingClientRect();
    const margin = 4;
    left = Math.max(margin, Math.min(px, window.innerWidth - rect.width - margin));
    top = Math.max(margin, Math.min(py, window.innerHeight - rect.height - margin));
  });

  onMount(() => {
    const prev = document.activeElement as HTMLElement | null;
    tick().then(() => buttons()[0]?.focus());

    function onPointerDown(e: PointerEvent) {
      if (menu && !menu.contains(e.target as Node)) onclose();
    }
    function onBlur() {
      onclose();
    }
    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("blur", onBlur);
    window.addEventListener("resize", onBlur);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("resize", onBlur);
      prev?.focus?.();
    };
  });

  function buttons(): HTMLButtonElement[] {
    return menu ? Array.from(menu.querySelectorAll<HTMLButtonElement>("button:not([disabled])")) : [];
  }

  function onKeydown(e: KeyboardEvent) {
    const list = buttons();
    const idx = list.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onclose();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      list[(idx + 1) % list.length]?.focus();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      list[(idx - 1 + list.length) % list.length]?.focus();
    } else if (e.key === "Home") {
      e.preventDefault();
      list[0]?.focus();
    } else if (e.key === "End") {
      e.preventDefault();
      list[list.length - 1]?.focus();
    } else if (e.key === "Tab") {
      e.preventDefault();
      onclose();
    }
  }

  function run(item: MenuItem) {
    onclose();
    item.action?.();
  }
</script>

{#if actionable.length > 0}
  <div
    bind:this={menu}
    class="context-menu"
    role="menu"
    tabindex="-1"
    style="left: {left}px; top: {top}px;"
    onkeydown={onKeydown}
    oncontextmenu={(e) => e.preventDefault()}
  >
    {#each items as item, i (i)}
      {#if item.separator}
        <div class="separator" role="separator"></div>
      {:else}
        <button
          class="item"
          class:danger={item.danger}
          role="menuitem"
          disabled={item.disabled}
          onclick={() => run(item)}
        >
          <span class="label">{item.label}</span>
          {#if item.shortcut}<span class="shortcut">{item.shortcut}</span>{/if}
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .context-menu {
    position: fixed;
    z-index: 1000;
    min-width: 180px;
    max-width: 360px;
    max-height: calc(100vh - 8px);
    overflow-y: auto;
    padding: 4px;
    background: var(--color-surface-elevated);
    border: 1px solid var(--color-border);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
    font-size: 12px;
    outline: none;
  }

  .item {
    display: flex;
    align-items: center;
    gap: 16px;
    width: 100%;
    padding: 5px 10px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-primary);
    text-align: left;
    cursor: pointer;
  }

  .item:hover:not(:disabled),
  .item:focus-visible {
    background: var(--color-accent);
    color: var(--color-on-accent);
    outline: none;
  }

  .item.danger {
    color: var(--color-diff-del-text);
  }

  .item.danger:hover:not(:disabled),
  .item.danger:focus-visible {
    background: var(--color-diff-del-text);
    color: var(--color-bg);
  }

  .item:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .shortcut {
    color: var(--color-text-muted);
    font-size: 11px;
  }

  .item:hover:not(:disabled) .shortcut,
  .item:focus-visible .shortcut {
    color: inherit;
    opacity: 0.8;
  }

  .separator {
    height: 1px;
    margin: 4px 6px;
    background: var(--color-border);
  }
</style>
