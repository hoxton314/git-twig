<script lang="ts">
  import { tick } from "svelte";
  import { Search } from "lucide-svelte";
  import {
    ACTIONS,
    hasActionHandler,
    runAction,
    shortcutLabels,
    eventToShortcut,
    normalizeShortcut,
  } from "../../lib/keybindings";
  import {
    paletteState,
    closePalette,
    getPaletteProviders,
    recentPaletteIds,
    markPaletteItemUsed,
    fuzzyMatch,
    type PaletteItem,
  } from "../../lib/palette";
  import { toastError } from "../../lib/stores/toasts";

  interface Ranked {
    item: PaletteItem;
    indices: number[];
    score: number;
  }

  const MAX_RESULTS = 200;
  const listId = "command-palette-list";

  let query = $state("");
  let selected = $state(0);
  let results = $state<Ranked[]>([]);
  let inputEl: HTMLInputElement | undefined = $state(undefined);
  let listEl: HTMLUListElement | undefined = $state(undefined);
  let previousFocus: HTMLElement | null = null;
  let generation = 0;

  const isOpen = $derived($paletteState.open);

  // Sync the requested initial query and focus when opening.
  $effect(() => {
    if (!isOpen) return;
    previousFocus = document.activeElement as HTMLElement | null;
    query = $paletteState.query;
    selected = 0;
    tick().then(() => {
      inputEl?.focus();
      inputEl?.select();
    });
    return () => {
      const el = previousFocus;
      previousFocus = null;
      if (el && document.contains(el)) el.focus();
    };
  });

  function actionItems(labels: Record<string, string>): PaletteItem[] {
    return ACTIONS.filter((a) => !a.hideInPalette && hasActionHandler(a.id)).map((a) => ({
      id: `action:${a.id}`,
      label: a.label,
      category: a.category,
      shortcut: labels[a.id],
      run: () => {
        runAction(a.id);
      },
    }));
  }

  async function collect(q: string): Promise<PaletteItem[]> {
    const providers = getPaletteProviders().sort((a, b) => (b.priority ?? 0) - (a.priority ?? 0));
    const lists = await Promise.all(
      providers.map(async (p) => {
        try {
          return await p.getItems(q);
        } catch (e) {
          console.error(`Palette provider ${p.id} failed:`, e);
          return [];
        }
      }),
    );
    return [...actionItems($shortcutLabels), ...lists.flat()];
  }

  function rank(items: PaletteItem[], q: string): Ranked[] {
    const recent = recentPaletteIds();
    const recency = (id: string) => {
      const i = recent.indexOf(id);
      return i === -1 ? 0 : Math.max(1, 20 - i);
    };

    if (!q.trim()) {
      return items
        .map((item, order) => ({ item, indices: [], score: recency(item.id) * 1000 - order }))
        .sort((a, b) => b.score - a.score)
        .slice(0, MAX_RESULTS);
    }

    const ranked: Ranked[] = [];
    for (const item of items) {
      const onLabel = fuzzyMatch(q, item.label);
      const onDetail = item.detail ? fuzzyMatch(q, item.detail) : null;
      const onExtra = fuzzyMatch(q, `${item.category} ${item.keywords ?? ""}`);
      let best: Ranked | null = null;
      if (onLabel) best = { item, indices: onLabel.indices, score: onLabel.score };
      if (onDetail && (!best || onDetail.score - 20 > best.score)) best = { item, indices: [], score: onDetail.score - 20 };
      if (onExtra && (!best || onExtra.score - 40 > best.score)) best = { item, indices: [], score: onExtra.score - 40 };
      if (!best) continue;
      best.score += recency(item.id) - (item.disabled ? 50 : 0);
      ranked.push(best);
    }
    return ranked.sort((a, b) => b.score - a.score).slice(0, MAX_RESULTS);
  }

  // Recompute results whenever the query changes while open.
  $effect(() => {
    if (!isOpen) return;
    const q = query;
    const gen = ++generation;
    collect(q).then((items) => {
      if (gen !== generation) return;
      results = rank(items, q);
      selected = 0;
    });
  });

  function scrollSelectedIntoView() {
    tick().then(() => {
      listEl?.querySelector<HTMLElement>(`[data-index="${selected}"]`)?.scrollIntoView({ block: "nearest" });
    });
  }

  function move(delta: number) {
    if (results.length === 0) return;
    selected = (selected + delta + results.length) % results.length;
    scrollSelectedIntoView();
  }

  async function execute(index: number) {
    const r = results[index];
    if (!r || r.item.disabled) return;
    closePalette();
    markPaletteItemUsed(r.item.id);
    // Let the palette unmount (and focus return) before the action runs, so
    // actions that focus something or open a dialog aren't undone.
    await tick();
    try {
      await r.item.run();
    } catch (err) {
      toastError(r.item.label, err);
    }
  }

  function onKeydown(e: KeyboardEvent) {
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        move(1);
        break;
      case "ArrowUp":
        e.preventDefault();
        move(-1);
        break;
      case "PageDown":
        e.preventDefault();
        jump(Math.min(results.length - 1, selected + 8));
        break;
      case "PageUp":
        e.preventDefault();
        jump(Math.max(0, selected - 8));
        break;
      case "Enter":
        e.preventDefault();
        execute(selected);
        break;
      case "Escape":
        e.preventDefault();
        e.stopPropagation();
        closePalette();
        break;
      case "Tab":
        // Keep focus in the input; Tab/Shift+Tab move the selection.
        e.preventDefault();
        move(e.shiftKey ? -1 : 1);
        break;
      default: {
        // Pressing the palette shortcut again closes it.
        const pressed = eventToShortcut(e);
        const binding = $shortcutLabels["command_palette"];
        if (pressed && binding && normalizeShortcut(pressed) === normalizeShortcut(binding)) {
          e.preventDefault();
          closePalette();
        }
      }
    }
  }

  function jump(index: number) {
    if (results.length === 0) return;
    selected = index;
    scrollSelectedIntoView();
  }

  /** Split a label into plain / highlighted runs for the matched indices. */
  function segments(text: string, indices: number[]): { text: string; hit: boolean }[] {
    if (indices.length === 0) return [{ text, hit: false }];
    const set = new Set(indices);
    const out: { text: string; hit: boolean }[] = [];
    for (let i = 0; i < text.length; i++) {
      const hit = set.has(i);
      const last = out[out.length - 1];
      if (last && last.hit === hit) last.text += text[i];
      else out.push({ text: text[i], hit });
    }
    return out;
  }
</script>

{#if isOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="palette-backdrop" onclick={closePalette}>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
      class="palette"
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
    >
      <div class="palette-input-row">
        <Search size={15} aria-hidden="true" />
        <input
          bind:this={inputEl}
          bind:value={query}
          onkeydown={onKeydown}
          class="palette-input"
          type="text"
          placeholder="Type a command, branch, tab or repository…"
          spellcheck="false"
          autocomplete="off"
          role="combobox"
          aria-expanded="true"
          aria-controls={listId}
          aria-autocomplete="list"
          aria-activedescendant={results.length > 0 ? `palette-opt-${selected}` : undefined}
        />
      </div>

      <ul class="palette-list" id={listId} role="listbox" aria-label="Results" bind:this={listEl}>
        {#each results as r, i (r.item.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <li
            id="palette-opt-{i}"
            data-index={i}
            role="option"
            aria-selected={i === selected}
            aria-disabled={r.item.disabled ? "true" : undefined}
            class="palette-item"
            class:selected={i === selected}
            class:disabled={r.item.disabled}
            onclick={() => execute(i)}
            onmousemove={() => (selected = i)}
          >
            <div class="item-main">
              <span class="item-label">
                {#each segments(r.item.label, r.indices) as seg, si (si)}
                  {#if seg.hit}<mark>{seg.text}</mark>{:else}{seg.text}{/if}
                {/each}
              </span>
              {#if r.item.detail}
                <span class="item-detail">{r.item.detail}</span>
              {/if}
            </div>
            <span class="item-category">{r.item.category}</span>
            {#if r.item.shortcut}
              <span class="item-shortcut">
                {#each r.item.shortcut.split(/\+(?!$)/) as part, pi (pi)}<kbd>{part}</kbd>{/each}
              </span>
            {/if}
          </li>
        {:else}
          <li class="palette-empty" role="option" aria-selected="false" aria-disabled="true">No matching commands</li>
        {/each}
      </ul>

      <div class="palette-footer" aria-hidden="true">
        <span><kbd>↑</kbd><kbd>↓</kbd> navigate</span>
        <span><kbd>Enter</kbd> run</span>
        <span><kbd>Esc</kbd> close</span>
      </div>
    </div>
  </div>
{/if}

<style>
  .palette-backdrop {
    position: fixed;
    inset: 0;
    z-index: 2000;
    background: rgba(0, 0, 0, 0.35);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
  }

  .palette {
    width: min(600px, calc(100vw - 32px));
    max-height: 70vh;
    display: flex;
    flex-direction: column;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: 8px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.45);
    overflow: hidden;
  }

  .palette:focus {
    outline: none;
  }

  .palette-input-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    border-bottom: 1px solid var(--color-border);
    color: var(--color-text-muted);
  }

  .palette-input {
    flex: 1;
    padding: 12px 0;
    border: none;
    background: transparent;
    color: var(--color-text-primary);
    font-size: 14px;
    font-family: inherit;
  }

  .palette-input:focus,
  .palette-input:focus-visible {
    outline: none;
  }

  .palette-list {
    list-style: none;
    margin: 0;
    padding: 4px;
    overflow-y: auto;
    flex: 1;
  }

  .palette-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    border-radius: 4px;
    cursor: pointer;
    color: var(--color-text-primary);
  }

  .palette-item.selected {
    background: var(--color-surface-elevated);
    box-shadow: inset 2px 0 0 var(--color-accent);
  }

  .palette-item.disabled {
    opacity: 0.45;
    cursor: default;
  }

  .item-main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .item-label {
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .item-label mark {
    background: transparent;
    color: var(--color-accent);
    font-weight: 600;
  }

  .item-detail {
    font-size: 11px;
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .item-category {
    font-size: 11px;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .item-shortcut {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
  }

  kbd {
    display: inline-block;
    padding: 1px 5px;
    border: 1px solid var(--color-border);
    border-radius: 3px;
    background: var(--color-surface-elevated);
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 1.4;
  }

  .palette-empty {
    padding: 16px;
    text-align: center;
    color: var(--color-text-muted);
    font-size: 12px;
  }

  .palette-footer {
    display: flex;
    gap: 14px;
    padding: 6px 12px;
    border-top: 1px solid var(--color-border);
    font-size: 11px;
    color: var(--color-text-muted);
  }

  .palette-footer kbd {
    margin-right: 2px;
  }
</style>
