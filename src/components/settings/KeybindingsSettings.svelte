<script lang="ts">
  import { RotateCcw } from "lucide-svelte";
  import { ACTIONS, eventToShortcut, normalizeShortcut, resolveShortcut } from "../../lib/keybindings";
  import { globalSettings, updateGlobalSettings } from "../../lib/stores/settings";
  import { t, type MessageKey } from "../../lib/i18n";

  const s = $derived($globalSettings);

  // Group actions by category
  const categories = $derived.by(() => {
    const map = new Map<MessageKey, typeof ACTIONS>();
    for (const action of ACTIONS) {
      const list = map.get(action.categoryKey) ?? [];
      list.push(action);
      map.set(action.categoryKey, list);
    }
    return [...map.entries()];
  });

  // Capture mode
  let capturingActionId = $state<string | null>(null);

  function startCapture(actionId: string) {
    capturingActionId = actionId;
  }

  // The overlay never receives focus, so an element-level onkeydown on it
  // would never fire. Listen on window in the capture phase instead, which
  // also runs before the global keybinding handler.
  $effect(() => {
    if (!capturingActionId) return;
    window.addEventListener("keydown", handleCaptureKeydown, true);
    return () => window.removeEventListener("keydown", handleCaptureKeydown, true);
  });

  // Map of action id -> labels of other actions bound to the same shortcut.
  const conflicts = $derived.by(() => {
    const byShortcut = new Map<string, string[]>();
    for (const action of ACTIONS) {
      const norm = normalizeShortcut(effectiveShortcut(action.id));
      if (!norm) continue;
      byShortcut.set(norm, [...(byShortcut.get(norm) ?? []), action.id]);
    }
    const result = new Map<string, string[]>();
    const labelOf = (o: string) => {
      const a = ACTIONS.find((x) => x.id === o);
      return a ? $t(a.labelKey) : o;
    };
    for (const ids of byShortcut.values()) {
      if (ids.length < 2) continue;
      for (const id of ids) {
        result.set(
          id,
          ids.filter((o) => o !== id).map(labelOf),
        );
      }
    }
    return result;
  });

  function handleCaptureKeydown(e: KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();

    if (e.key === "Escape") {
      capturingActionId = null;
      return;
    }
    // Backspace / Delete (no modifiers) unbinds the action.
    if ((e.key === "Backspace" || e.key === "Delete") && !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey) {
      const actionId = capturingActionId;
      capturingActionId = null;
      const action = ACTIONS.find((a) => a.id === actionId);
      if (!actionId || !action) return;
      const overrides = { ...s.keybinding_overrides };
      if (action.defaultShortcut) overrides[actionId] = "";
      else delete overrides[actionId];
      updateGlobalSettings({ keybinding_overrides: overrides });
      return;
    }

    const shortcut = eventToShortcut(e);
    if (!shortcut) return;

    const actionId = capturingActionId;
    capturingActionId = null;

    if (!actionId) return;

    // Check if this shortcut matches the default — if so, remove the override
    const action = ACTIONS.find((a) => a.id === actionId);
    const overrides = { ...s.keybinding_overrides };

    if (action && normalizeShortcut(shortcut) === normalizeShortcut(action.defaultShortcut)) {
      delete overrides[actionId];
    } else {
      overrides[actionId] = shortcut;
    }

    updateGlobalSettings({ keybinding_overrides: overrides });
  }

  function resetBinding(actionId: string) {
    const overrides = { ...s.keybinding_overrides };
    delete overrides[actionId];
    updateGlobalSettings({ keybinding_overrides: overrides });
  }

  function resetAllBindings() {
    updateGlobalSettings({ keybinding_overrides: {} });
  }

  function isCustomized(actionId: string): boolean {
    return actionId in s.keybinding_overrides;
  }

  function effectiveShortcut(actionId: string): string {
    return resolveShortcut(s.keybinding_overrides, actionId);
  }
</script>

{#if capturingActionId}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="capture-overlay" onclick={() => (capturingActionId = null)}>
    <div
      class="capture-modal"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="capture-title"
      aria-describedby="capture-hint"
    >
      <p class="capture-title" id="capture-title" aria-live="assertive">
        {$t("settings.keys.capture", { action: ACTIONS.find((a) => a.id === capturingActionId)?.label ?? capturingActionId })}
      </p>
      <p class="capture-hint" id="capture-hint">{$t("settings.keys.captureHint")}</p>
    </div>
  </div>
{/if}

<div class="section">
  <div class="section-header">
    <h1 class="section-heading">{$t("settings.nav.keybindings")}</h1>
    {#if Object.keys(s.keybinding_overrides).length > 0}
      <button class="reset-all-btn" onclick={resetAllBindings}>
        <RotateCcw size={12} />
        {$t("settings.keys.resetAll")}
      </button>
    {/if}
  </div>

  {#each categories as [category, actions] (category)}
    <div class="setting-group">
      <h2 class="group-heading">{$t(category)}</h2>

      {#each actions as action (action.id)}
        <div class="keybinding-row">
          <span class="action-label">{$t(action.labelKey)}</span>
          <div class="shortcut-area">
            <button
              class="shortcut-btn"
              class:customized={isCustomized(action.id)}
              class:capturing={capturingActionId === action.id}
              class:conflict={conflicts.has(action.id)}
              onclick={() => startCapture(action.id)}
              title={conflicts.has(action.id)
                ? $t("settings.keys.conflictsTitle", { actions: conflicts.get(action.id)?.join(", ") ?? "" })
                : $t("settings.keys.clickToChange")}
              aria-label={conflicts.has(action.id)
                ? $t("settings.keys.changeAriaConflict", {
                    action: action.label,
                    shortcut: effectiveShortcut(action.id) || $t("settings.keys.notSetLower"),
                    actions: conflicts.get(action.id)?.join(", ") ?? "",
                  })
                : $t("settings.keys.changeAria", {
                    action: action.label,
                    shortcut: effectiveShortcut(action.id) || $t("settings.keys.notSetLower"),
                  })}
            >
              {#if effectiveShortcut(action.id)}
                {#each effectiveShortcut(action.id).split(/\+(?!$)/) as part, i}
                  {#if i > 0}<span class="key-sep">+</span>{/if}
                  <kbd class="key">{part}</kbd>
                {/each}
              {:else}
                <span class="key-unset">{$t("settings.notSet")}</span>
              {/if}
            </button>
            {#if isCustomized(action.id)}
              <button
                class="reset-btn"
                onclick={() => resetBinding(action.id)}
                title={$t("settings.keys.resetTitle", { shortcut: action.defaultShortcut || $t("settings.keys.notSetLower") })}
                aria-label={$t("settings.keys.resetAria", {
                  action: action.label,
                  shortcut: action.defaultShortcut || $t("settings.keys.notSetLower"),
                })}
              >
                <RotateCcw size={11} />
              </button>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/each}
</div>

<style>
  .section {
    max-width: 640px;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 24px;
  }

  .section-heading {
    font-size: 20px;
    font-weight: 600;
    color: var(--color-text-primary);
    margin: 0;
  }

  .reset-all-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 11px;
    cursor: pointer;
    transition: border-color 0.1s, color 0.1s;
  }

  .reset-all-btn:hover {
    border-color: var(--color-text-muted);
    color: var(--color-text-primary);
  }

  .setting-group {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-bottom: 24px;
  }

  .group-heading {
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--color-text-muted);
    margin: 0 0 8px;
  }

  .keybinding-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 0;
    border-bottom: 1px solid var(--color-border);
  }

  .keybinding-row:last-child {
    border-bottom: none;
  }

  .action-label {
    font-size: 13px;
    color: var(--color-text-primary);
  }

  .shortcut-area {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .shortcut-btn {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 4px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    cursor: pointer;
    transition: border-color 0.1s;
  }

  .shortcut-btn:hover {
    border-color: var(--color-accent);
  }

  .shortcut-btn.capturing {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 1px var(--color-accent);
  }

  .shortcut-btn.conflict,
  .shortcut-btn.customized.conflict {
    border-color: var(--color-diff-del-text);
  }

  .shortcut-btn.customized {
    border-color: var(--color-accent-secondary);
  }

  .key {
    display: inline-block;
    padding: 2px 6px;
    border-radius: 3px;
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.3;
  }

  .key-unset {
    padding: 2px 4px;
    color: var(--color-text-muted);
    font-size: 11px;
    font-style: italic;
  }

  .key-sep {
    color: var(--color-text-muted);
    font-size: 10px;
    margin: 0 1px;
  }

  .reset-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border: none;
    border-radius: 3px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 0;
  }

  .reset-btn:hover {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  /* Capture overlay */
  .capture-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .capture-modal {
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: 8px;
    padding: 32px 48px;
    text-align: center;
  }

  .capture-title {
    font-size: 15px;
    font-weight: 500;
    color: var(--color-text-primary);
    margin: 0 0 8px;
  }

  .capture-hint {
    font-size: 12px;
    color: var(--color-text-muted);
    margin: 0;
  }
</style>
