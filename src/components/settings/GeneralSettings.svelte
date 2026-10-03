<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { globalSettings, updateGlobalSettings } from "../../lib/stores/settings";
  import { FolderOpen, Download, Upload } from "lucide-svelte";
  import { openSettingsFolder, exportSettingsToFile, importSettingsFromFile } from "../../lib/appActions";
  import { LOCALES, t, tr } from "../../lib/i18n";

  const s = $derived($globalSettings);

  const fetchIntervalOptions = $derived([
    { value: 0, text: $t("settings.off") },
    ...[1, 5, 10, 30].map((m) => ({ value: m * 60, text: $t("settings.general.minutes", { count: m }) })),
  ]);

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
      title: tr("settings.general.defaultDirDialog"),
      defaultPath: s.default_repo_dir ?? undefined,
    });
    if (selected) {
      updateGlobalSettings({ default_repo_dir: selected as string });
    }
  }
</script>

<div class="section">
  <h1 class="section-heading">{$t("settings.nav.general")}</h1>

  <div class="setting-group">
    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">{$t("settings.general.language")}</span>
        <span class="label-hint">{$t("settings.general.languageHint")}</span>
      </div>
      <div class="setting-control">
        <select
          value={s.language}
          aria-label={$t("settings.general.language")}
          onchange={(e) => updateGlobalSettings({ language: e.currentTarget.value })}
        >
          <option value="system">{$t("settings.general.languageSystem")}</option>
          {#each LOCALES as l (l.id)}
            <option value={l.id}>{l.name}</option>
          {/each}
        </select>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">{$t("settings.general.defaultDir")}</span>
        <span class="label-hint">{$t("settings.general.defaultDirHint")}</span>
      </div>
      <div class="setting-control path-control">
        <span class="path-value">{s.default_repo_dir ?? $t("settings.notSet")}</span>
        <button class="btn-secondary" onclick={pickDefaultDir}>{$t("common.browse")}</button>
        {#if s.default_repo_dir}
          <button class="btn-ghost" onclick={() => updateGlobalSettings({ default_repo_dir: null })}>{$t("common.clear")}</button>
        {/if}
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">{$t("settings.general.autoFetch")}</span>
        <span class="label-hint">{$t("settings.general.autoFetchHint")}</span>
      </div>
      <div class="setting-control">
        <select
          value={s.auto_fetch_interval}
          onchange={(e) => updateGlobalSettings({ auto_fetch_interval: Number(e.currentTarget.value) })}
        >
          {#each fetchIntervalOptions as opt (opt.value)}
            <option value={opt.value}>{opt.text}</option>
          {/each}
        </select>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">{$t("settings.general.notifyCommits")}</span>
        <span class="label-hint">{$t("settings.general.notifyCommitsHint")}</span>
      </div>
      <div class="setting-control">
        <select
          value={s.notify_new_commits}
          onchange={(e) => updateGlobalSettings({ notify_new_commits: e.currentTarget.value as "off" | "current" | "all" })}
          aria-label={$t("settings.general.notifyCommits")}
        >
          <option value="current">{$t("settings.general.notifyCurrent")}</option>
          <option value="all">{$t("settings.general.notifyAll")}</option>
          <option value="off">{$t("settings.off")}</option>
        </select>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">{$t("settings.general.notifyCi")}</span>
        <span class="label-hint">{$t("settings.general.notifyCiHint")}</span>
      </div>
      <div class="setting-control">
        <select
          value={s.notify_ci}
          onchange={(e) => updateGlobalSettings({ notify_ci: e.currentTarget.value as "off" | "failures" | "all" })}
          aria-label={$t("settings.general.notifyCi")}
        >
          <option value="all">{$t("settings.general.ciAll")}</option>
          <option value="failures">{$t("settings.general.ciFailures")}</option>
          <option value="off">{$t("settings.off")}</option>
        </select>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">{$t("settings.general.commitsPerPage")}</span>
        <span class="label-hint">{$t("settings.general.commitsPerPageHint")}</span>
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
        <span class="label-text">{$t("settings.general.confirmDestructive")}</span>
        <span class="label-hint">{$t("settings.general.confirmDestructiveHint")}</span>
      </div>
      <div class="setting-control">
        <label class="toggle">
          <input
            type="checkbox"
            aria-label={$t("settings.general.confirmDestructive")}
            checked={s.confirm_destructive_ops}
            onchange={() => updateGlobalSettings({ confirm_destructive_ops: !s.confirm_destructive_ops })}
          />
          <span class="toggle-slider"></span>
        </label>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">{$t("settings.general.restoreTabs")}</span>
        <span class="label-hint">{$t("settings.general.restoreTabsHint")}</span>
      </div>
      <div class="setting-control">
        <label class="toggle">
          <input
            type="checkbox"
            aria-label={$t("settings.general.restoreTabs")}
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
        <span class="label-text">{$t("settings.general.treeView")}</span>
        <span class="label-hint">{$t("settings.general.treeViewHint")}</span>
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

  <h2 class="group-heading">{$t("settings.general.settingsFile")}</h2>
  <div class="setting-group">
    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">{$t("settings.general.settingsFolder")}</span>
        <span class="label-hint">{$t("settings.general.settingsFolderHint")}</span>
      </div>
      <div class="setting-control">
        <button class="btn-secondary" onclick={openSettingsFolder}>
          <FolderOpen size={12} aria-hidden="true" /> {$t("settings.general.openFolder")}
        </button>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">{$t("settings.general.importExport")}</span>
        <span class="label-hint">{$t("settings.general.importExportHint")}</span>
      </div>
      <div class="setting-control path-control">
        <button class="btn-secondary" onclick={exportSettingsToFile}>
          <Download size={12} aria-hidden="true" /> {$t("settings.general.export")}
        </button>
        <button class="btn-secondary" onclick={importSettingsFromFile}>
          <Upload size={12} aria-hidden="true" /> {$t("settings.general.import")}
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
