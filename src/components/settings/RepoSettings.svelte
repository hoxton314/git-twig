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

  type Field =
    | { key: OverridableKey; label: string; hint: string; kind: "bool" }
    | { key: OverridableKey; label: string; hint: string; kind: "select"; options: { value: number; label: string }[] }
    | { key: OverridableKey; label: string; hint: string; kind: "number"; min: number; max: number }
    | { key: OverridableKey; label: string; hint: string; kind: "text"; placeholder: string };

  const FIELDS: Field[] = [
    {
      key: "auto_fetch_interval",
      label: "Auto-fetch interval",
      hint: "How often this repository is fetched in the background",
      kind: "select",
      options: [
        { value: 0, label: "Off" },
        { value: 60, label: "1 minute" },
        { value: 300, label: "5 minutes" },
        { value: 600, label: "10 minutes" },
        { value: 1800, label: "30 minutes" },
      ],
    },
    { key: "max_commits", label: "Commits per page", hint: "Graph page size", kind: "number", min: 100, max: 50000 },
    { key: "graph_hide_remotes", label: "Hide remote branches", hint: "Graph shows local branches only", kind: "bool" },
    { key: "graph_current_branch_only", label: "Current branch only", hint: "Graph shows HEAD's history only", kind: "bool" },
    { key: "tab_size", label: "Tab size", hint: "Columns per tab in diffs", kind: "number", min: 1, max: 16 },
    { key: "context_lines", label: "Context lines", hint: "Unchanged lines around each change", kind: "number", min: 0, max: 100 },
    { key: "show_whitespace_changes", label: "Show whitespace changes", hint: "Off hides whitespace-only changes (diff -w)", kind: "bool" },
    { key: "word_wrap_in_diffs", label: "Wrap long lines in diffs", hint: "", kind: "bool" },
    { key: "syntax_highlighting", label: "Syntax highlighting", hint: "Highlight code in diffs", kind: "bool" },
    { key: "staging_tree_view", label: "Changed files as a tree", hint: "Staging panel shows folders", kind: "bool" },
    { key: "external_diff_tool", label: "External diff tool", hint: "Command for “Open in external diff”", kind: "text", placeholder: "e.g. meld" },
    { key: "external_merge_tool", label: "External merge tool", hint: "Command for resolving conflicts", kind: "text", placeholder: "e.g. kdiff3" },
  ];

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
    if (typeof v === "boolean") return v ? "On" : "Off";
    if (v === null || v === "" || v === undefined) return "Not set";
    return String(v);
  }

  function globalLabel(f: Field): string {
    const g = $globalSettings[f.key];
    if (f.kind === "select") return f.options.find((o) => o.value === g)?.label ?? show(g);
    return show(g);
  }
</script>

<div class="section">
  <h1 class="section-heading">This repository</h1>
  {#if !path}
    <p class="hint">Open a repository to override settings for it.</p>
  {:else}
    <p class="hint">
      Overrides for <strong>{$activeRepo?.name ?? path}</strong> <span class="path">{path}</span>. Unchecked settings follow
      the global value.
    </p>
    <div class="setting-group">
      {#each FIELDS as f (f.key)}
        <div class="setting-row" class:on={overridden(f.key)}>
          <label class="override" title="Override this setting for this repository">
            <input type="checkbox" checked={overridden(f.key)} onchange={(e) => toggle(f.key, e.currentTarget.checked)} aria-label="Override {f.label}" />
          </label>
          <div class="setting-label">
            <span class="label-text">{f.label}</span>
            <span class="label-hint">{overridden(f.key) ? `Global: ${globalLabel(f)}` : f.hint || "Uses the global value"}</span>
          </div>
          <div class="setting-control">
            {#if f.kind === "bool"}
              <input
                type="checkbox"
                checked={Boolean(value(f.key))}
                disabled={!overridden(f.key)}
                onchange={(e) => set(f.key, e.currentTarget.checked as never)}
                aria-label={f.label}
              />
            {:else if f.kind === "select"}
              <select value={value(f.key)} disabled={!overridden(f.key)} onchange={(e) => set(f.key, Number(e.currentTarget.value) as never)} aria-label={f.label}>
                {#each f.options as o (o.value)}
                  <option value={o.value}>{o.label}</option>
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
                aria-label={f.label}
              />
            {:else}
              <input
                type="text"
                value={(value(f.key) as string | null) ?? ""}
                placeholder={f.placeholder}
                disabled={!overridden(f.key)}
                onchange={(e) => set(f.key, (e.currentTarget.value.trim() || null) as never)}
                spellcheck="false"
                aria-label={f.label}
              />
            {/if}
          </div>
        </div>
      {/each}
    </div>
    {#if count > 0}
      <div class="footer">
        <button class="btn-secondary" onclick={() => path && clearRepoOverrides(path)}>
          Reset all {count} override{count === 1 ? "" : "s"} to global
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
