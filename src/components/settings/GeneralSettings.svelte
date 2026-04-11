<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { settings, updateSettings } from "../../lib/stores/settings";

  const s = $derived($settings);

  const fetchIntervalOptions = [
    { value: 0, label: "Off" },
    { value: 60, label: "1 minute" },
    { value: 300, label: "5 minutes" },
    { value: 600, label: "10 minutes" },
    { value: 1800, label: "30 minutes" },
  ];

  async function pickDefaultDir() {
    const selected = await open({ directory: true, multiple: false, title: "Default Repository Directory" });
    if (selected) {
      updateSettings({ default_repo_dir: selected as string });
    }
  }
</script>

<div class="section">
  <h1 class="section-heading">General</h1>

  <div class="setting-group">
    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Default repository directory</span>
        <span class="label-hint">Pre-fills the directory picker when opening repos</span>
      </div>
      <div class="setting-control path-control">
        <span class="path-value">{s.default_repo_dir ?? "Not set"}</span>
        <button class="btn-secondary" onclick={pickDefaultDir}>Browse</button>
        {#if s.default_repo_dir}
          <button class="btn-ghost" onclick={() => updateSettings({ default_repo_dir: null })}>Clear</button>
        {/if}
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Auto-fetch interval</span>
        <span class="label-hint">Periodically fetch from remotes for open repositories</span>
      </div>
      <div class="setting-control">
        <select
          value={s.auto_fetch_interval}
          onchange={(e) => updateSettings({ auto_fetch_interval: Number(e.currentTarget.value) })}
        >
          {#each fetchIntervalOptions as opt (opt.value)}
            <option value={opt.value}>{opt.label}</option>
          {/each}
        </select>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Maximum commits to load</span>
        <span class="label-hint">Limits how many commits are loaded in the graph</span>
      </div>
      <div class="setting-control">
        <input
          type="number"
          min="100"
          max="50000"
          step="500"
          value={s.max_commits}
          onchange={(e) => updateSettings({ max_commits: Number(e.currentTarget.value) })}
        />
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Confirm destructive operations</span>
        <span class="label-hint">Warn before force-deleting branches, discarding changes, etc.</span>
      </div>
      <div class="setting-control">
        <label class="toggle">
          <input
            type="checkbox"
            checked={s.confirm_destructive_ops}
            onchange={() => updateSettings({ confirm_destructive_ops: !s.confirm_destructive_ops })}
          />
          <span class="toggle-slider"></span>
        </label>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Restore tabs on startup</span>
        <span class="label-hint">Reopen previously active repositories when launching</span>
      </div>
      <div class="setting-control">
        <label class="toggle">
          <input
            type="checkbox"
            checked={s.restore_tabs_on_startup}
            onchange={() => updateSettings({ restore_tabs_on_startup: !s.restore_tabs_on_startup })}
          />
          <span class="toggle-slider"></span>
        </label>
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

  .path-control {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .path-value {
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--color-text-muted);
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  select,
  input[type="number"] {
    padding: 6px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    font-family: inherit;
  }

  select:hover,
  input[type="number"]:hover {
    border-color: var(--color-text-muted);
  }

  select:focus,
  input[type="number"]:focus {
    outline: none;
    border-color: var(--color-accent);
  }

  input[type="number"] {
    width: 90px;
  }

  .btn-secondary {
    padding: 5px 12px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
    transition: border-color 0.1s;
  }

  .btn-secondary:hover {
    border-color: var(--color-accent);
  }

  .btn-ghost {
    padding: 5px 8px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 11px;
    cursor: pointer;
  }

  .btn-ghost:hover {
    color: var(--color-diff-del-text);
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
