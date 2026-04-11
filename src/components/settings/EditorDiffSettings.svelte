<script lang="ts">
  import { settings, updateSettings } from "../../lib/stores/settings";
  import { diffViewMode } from "../../lib/stores/ui";

  const s = $derived($settings);

  // Sync diff view mode to the UI store when changed in settings
  $effect(() => {
    $diffViewMode = s.diff_view_mode as "unified" | "split";
  });
</script>

<div class="section">
  <h1 class="section-heading">Editor & Diff</h1>

  <div class="setting-group">
    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Default diff view</span>
        <span class="label-hint">How diffs are displayed by default</span>
      </div>
      <div class="setting-control">
        <div class="segmented-control">
          <button
            class="segment"
            class:active={s.diff_view_mode === "unified"}
            onclick={() => updateSettings({ diff_view_mode: "unified" })}
          >
            Unified
          </button>
          <button
            class="segment"
            class:active={s.diff_view_mode === "split"}
            onclick={() => updateSettings({ diff_view_mode: "split" })}
          >
            Split
          </button>
        </div>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Tab size</span>
        <span class="label-hint">Number of spaces per tab in diff rendering</span>
      </div>
      <div class="setting-control">
        <select
          value={s.tab_size}
          onchange={(e) => updateSettings({ tab_size: Number(e.currentTarget.value) })}
        >
          <option value={2}>2 spaces</option>
          <option value={4}>4 spaces</option>
          <option value={8}>8 spaces</option>
        </select>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Context lines</span>
        <span class="label-hint">Lines of surrounding context shown around changes</span>
      </div>
      <div class="setting-control">
        <input
          type="number"
          min="0"
          max="20"
          step="1"
          value={s.context_lines}
          onchange={(e) => updateSettings({ context_lines: Number(e.currentTarget.value) })}
        />
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Show whitespace changes</span>
        <span class="label-hint">Display whitespace-only modifications in diffs</span>
      </div>
      <div class="setting-control">
        <label class="toggle">
          <input
            type="checkbox"
            checked={s.show_whitespace_changes}
            onchange={() => updateSettings({ show_whitespace_changes: !s.show_whitespace_changes })}
          />
          <span class="toggle-slider"></span>
        </label>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Word wrap in diffs</span>
        <span class="label-hint">Wrap long lines instead of horizontal scrolling</span>
      </div>
      <div class="setting-control">
        <label class="toggle">
          <input
            type="checkbox"
            checked={s.word_wrap_in_diffs}
            onchange={() => updateSettings({ word_wrap_in_diffs: !s.word_wrap_in_diffs })}
          />
          <span class="toggle-slider"></span>
        </label>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">External diff tool</span>
        <span class="label-hint">Path to external diff viewer (e.g. meld, kdiff3)</span>
      </div>
      <div class="setting-control">
        <input
          type="text"
          class="text-input"
          placeholder="Not set"
          value={s.external_diff_tool ?? ""}
          onchange={(e) => updateSettings({ external_diff_tool: e.currentTarget.value || null })}
        />
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">External merge tool</span>
        <span class="label-hint">Path to external merge tool</span>
      </div>
      <div class="setting-control">
        <input
          type="text"
          class="text-input"
          placeholder="Not set"
          value={s.external_merge_tool ?? ""}
          onchange={(e) => updateSettings({ external_merge_tool: e.currentTarget.value || null })}
        />
      </div>
    </div>
  </div>
</div>

<style>
  .section {
    max-width: 640px;
  }

  .section-heading {
    font-size: 20px;
    font-weight: 600;
    color: var(--color-text-primary);
    margin: 0 0 24px;
  }

  .setting-group {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    padding: 12px 0;
    border-bottom: 1px solid var(--color-border);
  }

  .setting-row:last-child {
    border-bottom: none;
  }

  .setting-label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }

  .label-text {
    font-size: 13px;
    font-weight: 500;
    color: var(--color-text-primary);
  }

  .label-hint {
    font-size: 11px;
    color: var(--color-text-muted);
  }

  .setting-control {
    flex-shrink: 0;
  }

  .segmented-control {
    display: flex;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    overflow: hidden;
  }

  .segment {
    padding: 6px 14px;
    border: none;
    background: var(--color-surface);
    color: var(--color-text-muted);
    font-size: 12px;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .segment:first-child {
    border-right: 1px solid var(--color-border);
  }

  .segment.active {
    background: var(--color-surface-elevated);
    color: var(--color-accent);
  }

  .segment:hover:not(.active) {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  select,
  input[type="number"],
  .text-input {
    padding: 6px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    font-family: inherit;
  }

  select:hover,
  input[type="number"]:hover,
  .text-input:hover {
    border-color: var(--color-text-muted);
  }

  select:focus,
  input[type="number"]:focus,
  .text-input:focus {
    outline: none;
    border-color: var(--color-accent);
  }

  input[type="number"] {
    width: 60px;
  }

  .text-input {
    width: 180px;
    font-family: var(--font-mono);
  }

  .text-input::placeholder {
    color: var(--color-text-muted);
    font-family: var(--font-sans);
  }

  /* Toggle switch */
  .toggle {
    position: relative;
    display: inline-block;
    width: 36px;
    height: 20px;
    cursor: pointer;
  }

  .toggle input {
    opacity: 0;
    width: 0;
    height: 0;
    position: absolute;
  }

  .toggle-slider {
    position: absolute;
    inset: 0;
    background: var(--color-border);
    border-radius: 10px;
    transition: background 0.15s;
  }

  .toggle-slider::before {
    content: "";
    position: absolute;
    width: 16px;
    height: 16px;
    left: 2px;
    bottom: 2px;
    background: var(--color-text-muted);
    border-radius: 50%;
    transition: transform 0.15s, background 0.15s;
  }

  .toggle input:checked + .toggle-slider {
    background: var(--color-accent);
  }

  .toggle input:checked + .toggle-slider::before {
    transform: translateX(16px);
    background: var(--color-text-primary);
  }
</style>
