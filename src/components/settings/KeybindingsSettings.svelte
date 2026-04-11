<script lang="ts">
  import { RotateCcw } from "lucide-svelte";
  import { ACTIONS, getShortcut, eventToShortcut } from "../../lib/keybindings";
  import { settings, updateSettings } from "../../lib/stores/settings";

  const s = $derived($settings);

  // Group actions by category
  const categories = $derived.by(() => {
    const map = new Map<string, typeof ACTIONS>();
    for (const action of ACTIONS) {
      const list = map.get(action.category) ?? [];
      list.push(action);
      map.set(action.category, list);
    }
    return [...map.entries()];
  });

  // Capture mode
  let capturingActionId = $state<string | null>(null);

  function startCapture(actionId: string) {
    capturingActionId = actionId;
  }

  function handleCaptureKeydown(e: KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();

    if (e.key === "Escape") {
      capturingActionId = null;
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

    if (action && shortcut === action.defaultShortcut) {
      delete overrides[actionId];
    } else {
      overrides[actionId] = shortcut;
    }

    updateSettings({ keybinding_overrides: overrides });
  }

  function resetBinding(actionId: string) {
    const overrides = { ...s.keybinding_overrides };
    delete overrides[actionId];
    updateSettings({ keybinding_overrides: overrides });
  }

  function resetAllBindings() {
    updateSettings({ keybinding_overrides: {} });
  }

  function isCustomized(actionId: string): boolean {
    return actionId in s.keybinding_overrides;
  }

  function effectiveShortcut(actionId: string): string {
    return s.keybinding_overrides[actionId] || ACTIONS.find((a) => a.id === actionId)?.defaultShortcut || "";
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
{#if capturingActionId}
  <div class="capture-overlay" onkeydown={handleCaptureKeydown}>
    <div class="capture-modal">
      <p class="capture-title">Press a key combination</p>
      <p class="capture-hint">Press Escape to cancel</p>
    </div>
  </div>
{/if}

<div class="section">
  <div class="section-header">
    <h1 class="section-heading">Keybindings</h1>
    {#if Object.keys(s.keybinding_overrides).length > 0}
      <button class="reset-all-btn" onclick={resetAllBindings}>
        <RotateCcw size={12} />
        Reset all to defaults
      </button>
    {/if}
  </div>

  {#each categories as [category, actions] (category)}
    <div class="setting-group">
      <h2 class="group-heading">{category}</h2>

      {#each actions as action (action.id)}
        <div class="keybinding-row">
          <span class="action-label">{action.label}</span>
          <div class="shortcut-area">
            <button
              class="shortcut-btn"
              class:customized={isCustomized(action.id)}
              class:capturing={capturingActionId === action.id}
              onclick={() => startCapture(action.id)}
            >
              {#each effectiveShortcut(action.id).split("+") as part, i}
                {#if i > 0}<span class="key-sep">+</span>{/if}
                <kbd class="key">{part}</kbd>
              {/each}
            </button>
            {#if isCustomized(action.id)}
              <button
                class="reset-btn"
                onclick={() => resetBinding(action.id)}
                title="Reset to default ({action.defaultShortcut})"
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
