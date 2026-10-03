<script lang="ts">
  /**
   * Settings > This repository: per-repository overrides of selected
   * settings. Unchecked rows follow the global value; checking "Override"
   * starts from the current value and keeps changes to this repository.
   */
  import { activeRepoPath, activeRepo } from "../../lib/stores/repos";
  import {
    globalSettings,
    repoOverrides,
    setRepoOverride,
    clearRepoOverride,
    clearRepoOverrides,
  } from "../../lib/stores/settings";
  import type { OverridableKey } from "../../lib/repoSettings";
  import type { AppSettings } from "../../lib/types/git";
  import { t, type MessageKey } from "../../lib/i18n";

  type Field =
    | { key: OverridableKey; name: MessageKey; info: MessageKey | null; kind: "bool" }
    | { key: OverridableKey; name: MessageKey; info: MessageKey | null; kind: "select"; options: { value: number; minutes: number }[] }
    | { key: OverridableKey; name: MessageKey; info: MessageKey | null; kind: "number"; min: number; max: number }
    | { key: OverridableKey; name: MessageKey; info: MessageKey | null; kind: "text"; example: MessageKey };

  const FIELDS: Field[] = [
    {
      key: "auto_fetch_interval",
      name: "repoSettings.autoFetch",
      info: "repoSettings.autoFetchHint",
      kind: "select",
      options: [
        { value: 0, minutes: 0 },
        { value: 60, minutes: 1 },
        { value: 300, minutes: 5 },
        { value: 600, minutes: 10 },
        { value: 1800, minutes: 30 },
      ],
    },
    { key: "max_commits", name: "repoSettings.maxCommits", info: "repoSettings.maxCommitsHint", kind: "number", min: 100, max: 50000 },
    { key: "graph_hide_remotes", name: "repoSettings.hideRemotes", info: "repoSettings.hideRemotesHint", kind: "bool" },
    { key: "graph_current_branch_only", name: "repoSettings.currentBranchOnly", info: "repoSettings.currentBranchOnlyHint", kind: "bool" },
    { key: "tab_size", name: "repoSettings.tabSize", info: "repoSettings.tabSizeHint", kind: "number", min: 1, max: 16 },
    { key: "context_lines", name: "repoSettings.contextLines", info: "repoSettings.contextLinesHint", kind: "number", min: 0, max: 100 },
    { key: "show_whitespace_changes", name: "repoSettings.whitespace", info: "repoSettings.whitespaceHint", kind: "bool" },
    { key: "word_wrap_in_diffs", name: "repoSettings.wordWrap", info: null, kind: "bool" },
    { key: "syntax_highlighting", name: "repoSettings.syntax", info: "repoSettings.syntaxHint", kind: "bool" },
    { key: "staging_tree_view", name: "repoSettings.treeView", info: "repoSettings.treeViewHint", kind: "bool" },
    { key: "external_diff_tool", name: "repoSettings.diffTool", info: "repoSettings.diffToolHint", kind: "text", example: "repoSettings.diffToolPlaceholder" },
    { key: "external_merge_tool", name: "repoSettings.mergeTool", info: "repoSettings.mergeToolHint", kind: "text", example: "repoSettings.mergeToolPlaceholder" },
  ];

  /** Label of an auto-fetch option: "Off" or "N minutes". */
  function optionLabel(o: { minutes: number }): string {
    return o.minutes === 0 ? $t("repoSettings.off") : $t("repoSettings.minutes", { count: o.minutes });
  }

  const path = $derived($activeRepoPath);
  const mine = $derived(path ? ($repoOverrides[path] ?? {}) : {});
  const count = $derived(Object.keys(mine).length);

  function overridden(key: OverridableKey): boolean {
    return key in mine;
  }

  function value(key: OverridableKey): AppSettings[OverridableKey] {
    return (overridden(key) ? (mine as Record<string, unknown>)[key] : $globalSettings[key]) as AppSettings[OverridableKey];
  }

  function toggle(key: OverridableKey, on: boolean) {
    if (!path) return;
    if (on) setRepoOverride(path, key, $globalSettings[key]);
    else clearRepoOverride(path, key);
  }

  function set(key: OverridableKey, v: AppSettings[OverridableKey]) {
    if (path) setRepoOverride(path, key, v);
  }

  function setNumber(f: Extract<Field, { kind: "number" }>, input: HTMLInputElement) {
    const n = Math.round(Number(input.value));
    if (!Number.isFinite(n)) {
      input.value = String(value(f.key));
      return;
    }
    const clamped = Math.min(f.max, Math.max(f.min, n));
    input.value = String(clamped);
    set(f.key, clamped as never);
  }

  function show(v: unknown): string {
    if (typeof v === "boolean") return v ? $t("repoSettings.on") : $t("repoSettings.off");
    if (v === null || v === "" || v === undefined) return $t("repoSettings.notSet");
    return String(v);
  }

  function globalLabel(f: Field): string {
    const g = $globalSettings[f.key];
    if (f.kind === "select") {
      const o = f.options.find((x) => x.value === g);
      return o ? optionLabel(o) : show(g);
    }
    return show(g);
  }
</script>

<div class="section">
  <h1 class="section-heading">{$t("repoSettings.heading")}</h1>
  {#if !path}
    <p class="hint">{$t("repoSettings.noRepo")}</p>
  {:else}
    <p class="hint">
      {$t("repoSettings.overridesBefore")} <strong>{$activeRepo?.name ?? path}</strong> <span class="path">{path}</span>{$t("repoSettings.overridesAfter")}
    </p>
    <div class="setting-group">
      {#each FIELDS as f (f.key)}
        <div class="setting-row" class:on={overridden(f.key)}>
          <label class="override" title={$t("repoSettings.overrideTitle")}>
            <input type="checkbox" checked={overridden(f.key)} onchange={(e) => toggle(f.key, e.currentTarget.checked)} aria-label={$t("repoSettings.overrideLabel", { label: $t(f.name) })} />
          </label>
          <div class="setting-label">
            <span class="label-text">{$t(f.name)}</span>
            <span class="label-hint">{overridden(f.key) ? $t("repoSettings.globalValue", { value: globalLabel(f) }) : f.info ? $t(f.info) : $t("repoSettings.usesGlobal")}</span>
          </div>
          <div class="setting-control">
            {#if f.kind === "bool"}
              <input
                type="checkbox"
                checked={Boolean(value(f.key))}
                disabled={!overridden(f.key)}
                onchange={(e) => set(f.key, e.currentTarget.checked as never)}
                aria-label={$t(f.name)}
              />
            {:else if f.kind === "select"}
              <select value={value(f.key)} disabled={!overridden(f.key)} onchange={(e) => set(f.key, Number(e.currentTarget.value) as never)} aria-label={$t(f.name)}>
                {#each f.options as o (o.value)}
                  <option value={o.value}>{optionLabel(o)}</option>
                {/each}
              </select>
            {:else if f.kind === "number"}
              <input
                type="number"
                min={f.min}
                max={f.max}
                value={value(f.key)}
                disabled={!overridden(f.key)}
                onchange={(e) => setNumber(f, e.currentTarget)}
                aria-label={$t(f.name)}
              />
            {:else}
              <input
                type="text"
                value={(value(f.key) as string | null) ?? ""}
                placeholder={$t(f.example)}
                disabled={!overridden(f.key)}
                onchange={(e) => set(f.key, (e.currentTarget.value.trim() || null) as never)}
                spellcheck="false"
                aria-label={$t(f.name)}
              />
            {/if}
          </div>
        </div>
      {/each}
    </div>
    {#if count > 0}
      <div class="footer">
        <button class="btn-secondary" onclick={() => path && clearRepoOverrides(path)}>
          {$t("repoSettings.resetAll", { count })}
        </button>
      </div>
    {/if}
  {/if}
</div>

<style>
  .section { max-width: 680px; }
  .section-heading { font-size: 20px; font-weight: 600; color: var(--color-text-primary); margin: 0 0 12px; }
  .hint { margin: 0 0 16px; font-size: 12px; color: var(--color-text-muted); }
  .hint strong { color: var(--color-text-primary); }
  .path { font-family: var(--font-mono); font-size: 11px; }
  .setting-group { display: flex; flex-direction: column; gap: 2px; }
  .setting-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 0;
    border-bottom: 1px solid var(--color-border);
  }
  .setting-row:last-child { border-bottom: none; }
  .setting-row:not(.on) .label-text { color: var(--color-text-muted); }
  .override input, .setting-control input[type="checkbox"] { accent-color: var(--color-accent); cursor: pointer; }
  .setting-label { display: flex; flex-direction: column; gap: 2px; flex: 1; min-width: 0; }
  .label-text { font-size: 13px; font-weight: 500; color: var(--color-text-primary); }
  .label-hint { font-size: 11px; color: var(--color-text-muted); }
  .setting-control { flex-shrink: 0; }
  .setting-control input[type="number"], .setting-control input[type="text"], .setting-control select {
    padding: 5px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    font-family: inherit;
  }
  .setting-control input[type="number"] { width: 90px; }
  .setting-control input[type="text"] { width: 180px; font-family: var(--font-mono); }
  .setting-control :disabled { opacity: 0.5; }
  .footer { margin-top: 16px; }
  .btn-secondary {
    padding: 6px 14px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
  }
  .btn-secondary:hover { border-color: var(--color-accent); }
</style>
