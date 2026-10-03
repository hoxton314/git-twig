<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { globalSettings, updateGlobalSettings } from "../../lib/stores/settings";
  import { FolderOpen, Download, Upload } from "lucide-svelte";
  import { openSettingsFolder, exportSettingsToFile, importSettingsFromFile } from "../../lib/appActions";
  import { LOCALES } from "../../lib/i18n";

  const s = $derived($globalSettings);

  const fetchIntervalOptions = [
    { value: 0, label: "Off" },
    { value: 60, label: "1 minute" },
    { value: 300, label: "5 minutes" },
    { value: 600, label: "10 minutes" },
    { value: 1800, label: "30 minutes" },
  ];

  /** Parse and clamp a numeric input; on invalid input restore the current value. */
  function commitNumber(
    input: HTMLInputElement,
    current: number,
    min: number,
    max: number,
  ): number | null {
    const n = Math.round(Number(input.value));
    if (input.value.trim() === "" || !Number.isFinite(n)) {
      input.value = String(current);
      return null;
    }
    const clamped = Math.min(max, Math.max(min, n));
    input.value = String(clamped);
    return clamped;
  }

  async function pickDefaultDir() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Default Repository Directory",
      defaultPath: s.default_repo_dir ?? undefined,
    });
    if (selected) {
      updateGlobalSettings({ default_repo_dir: selected as string });
    }
  }
</script>

<div class="section">
  <h1 class="section-heading">General</h1>

  <div class="setting-group">
    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Language</span>
        <span class="label-hint">Language of the interface; System follows your operating system</span>
      </div>
      <div class="setting-control">
        <select
          value={s.language}
          aria-label="Language"
          onchange={(e) => updateGlobalSettings({ language: e.currentTarget.value })}
        >
          <option value="system">System</option>
          {#each LOCALES as l (l.id)}
            <option value={l.id}>{l.name}</option>
          {/each}
        </select>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Default repository directory</span>
        <span class="label-hint">Pre-fills the directory picker when opening repos</span>
      </div>
      <div class="setting-control path-control">
        <span class="path-value">{s.default_repo_dir ?? "Not set"}</span>
        <button class="btn-secondary" onclick={pickDefaultDir}>Browse</button>
        {#if s.default_repo_dir}
          <button class="btn-ghost" onclick={() => updateGlobalSettings({ default_repo_dir: null })}>Clear</button>
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
          onchange={(e) => updateGlobalSettings({ auto_fetch_interval: Number(e.currentTarget.value) })}
        >
          {#each fetchIntervalOptions as opt (opt.value)}
            <option value={opt.value}>{opt.label}</option>
          {/each}
        </select>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Notify about new upstream commits</span>
        <span class="label-hint">After an auto-fetch: a system notification when Twig is in the background, a toast otherwise</span>
      </div>
      <div class="setting-control">
        <select
          value={s.notify_new_commits}
          onchange={(e) => updateGlobalSettings({ notify_new_commits: e.currentTarget.value as "off" | "current" | "all" })}
          aria-label="Notify about new upstream commits"
        >
          <option value="current">Current branch</option>
          <option value="all">All branches with an upstream</option>
          <option value="off">Off</option>
        </select>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Notify when CI finishes</span>
        <span class="label-hint">For the checked-out commit of open repositories with a GitHub, GitLab or Gitea remote</span>
      </div>
      <div class="setting-control">
        <select
          value={s.notify_ci}
          onchange={(e) => updateGlobalSettings({ notify_ci: e.currentTarget.value as "off" | "failures" | "all" })}
          aria-label="Notify when CI finishes"
        >
          <option value="all">Passed or failed</option>
          <option value="failures">Failures only</option>
          <option value="off">Off</option>
        </select>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Commits per page</span>
        <span class="label-hint">How many commits the graph loads at a time; more load as you scroll</span>
      </div>
      <div class="setting-control">
        <input
          type="number"
          min="100"
          max="50000"
          step="500"
          value={s.max_commits}
          onchange={(e) => {
            const v = commitNumber(e.currentTarget, s.max_commits, 100, 50000);
            if (v !== null) updateGlobalSettings({ max_commits: v });
          }}
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
            aria-label="Confirm destructive operations"
            checked={s.confirm_destructive_ops}
            onchange={() => updateGlobalSettings({ confirm_destructive_ops: !s.confirm_destructive_ops })}
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
            aria-label="Restore tabs on startup"
            checked={s.restore_tabs_on_startup}
            onchange={() => updateGlobalSettings({ restore_tabs_on_startup: !s.restore_tabs_on_startup })}
          />
          <span class="toggle-slider"></span>
        </label>
      </div>
    </div>

    <!-- Staging panel -->
    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Show changed files as a tree</span>
        <span class="label-hint">Group staged and unstaged files by folder in the staging panel</span>
      </div>
      <div class="setting-control">
        <label class="toggle">
          <input
            type="checkbox"
            checked={s.staging_tree_view}
            onchange={() => updateGlobalSettings({ staging_tree_view: !s.staging_tree_view })}
          />
          <span class="toggle-slider"></span>
        </label>
      </div>
    </div>
  </div>

  <h2 class="group-heading">Settings file</h2>
  <div class="setting-group">
    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Settings folder</span>
        <span class="label-hint">Where settings.json, session.json and the recent repositories list are stored</span>
      </div>
      <div class="setting-control">
        <button class="btn-secondary" onclick={openSettingsFolder}>
          <FolderOpen size={12} aria-hidden="true" /> Open folder
        </button>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Import / export</span>
        <span class="label-hint">Copy your preferences to another machine. Your GitHub token is never exported.</span>
      </div>
      <div class="setting-control path-control">
        <button class="btn-secondary" onclick={exportSettingsToFile}>
          <Download size={12} aria-hidden="true" /> Export…
        </button>
        <button class="btn-secondary" onclick={importSettingsFromFile}>
          <Upload size={12} aria-hidden="true" /> Import…
        </button>
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

  input[type="number"] {
    padding: 6px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    font-family: inherit;
  }

  input[type="number"]:hover {
    border-color: var(--color-text-muted);
  }

  input[type="number"]:focus {
    outline: none;
    border-color: var(--color-accent);
  }

  input[type="number"] {
    width: 90px;
  }

  .group-heading {
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--color-text-muted);
    margin: 28px 0 4px;
  }

  .btn-secondary {
    display: inline-flex;
    align-items: center;
    gap: 6px;
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
