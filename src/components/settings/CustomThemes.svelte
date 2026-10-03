<script lang="ts">
  /**
   * Settings > Appearance > Custom themes: duplicate the current theme,
   * edit its colours with live preview, import / export theme files.
   */
  import { open, save, ask } from "@tauri-apps/plugin-dialog";
  import { ChevronDown, ChevronRight, Copy, Download, Pencil, Trash2, Upload } from "lucide-svelte";
  import { globalSettings, updateGlobalSettings, updateGlobalSettingsWith } from "../../lib/stores/settings";
  import { toast, toastError } from "../../lib/stores/toasts";
  import * as tauri from "../../lib/tauri";
  import { t, tr, type MessageKey } from "../../lib/i18n";
  import {
    CUSTOM_PREFIX,
    THEME_TOKENS,
    TOKEN_GROUPS,
    contrastWarnings,
    exportTheme,
    isColor,
    parseTheme,
    pickedColor,
    toHexInput,
    tokenLabel,
    type CustomTheme,
    type ThemeToken,
  } from "../../lib/themes";

  const s = $derived($globalSettings);
  const themes = $derived(s.custom_themes ?? []);
  let editing = $state<string | null>(null);
  const current = $derived(themes.find((th) => th.id === editing) ?? null);
  const warnings = $derived(current ? contrastWarnings(current.colors) : []);

  /** Catalog keys for the editor's token group titles (from lib/themes). */
  const GROUP_KEYS: Record<string, MessageKey> = {
    "Surfaces & text": "settings.themes.groupSurfaces",
    "Graph lanes": "settings.themes.groupLanes",
    Diff: "settings.themes.groupDiff",
    Syntax: "settings.themes.groupSyntax",
    Search: "settings.themes.groupSearch",
  };

  function newId(): string {
    return `t${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
  }

  /**
   * Change the theme list as a function of its latest value (so edits in
   * two windows merge instead of one stale copy replacing the other).
   */
  function changeThemes(fn: (list: CustomTheme[]) => CustomTheme[], select?: string) {
    updateGlobalSettingsWith((cur) => ({
      custom_themes: fn(cur.custom_themes ?? []),
      ...(select !== undefined ? { theme: select } : {}),
    }));
  }

  /** Copy the theme on screen now (built-in or custom) as a new custom theme. */
  function duplicateCurrent() {
    const style = getComputedStyle(document.documentElement);
    const colors: Partial<Record<ThemeToken, string>> = {};
    for (const token of THEME_TOKENS) {
      const v = style.getPropertyValue(token).trim();
      if (v && isColor(v)) colors[token] = v;
    }
    const base = (document.documentElement.getAttribute("data-theme") === "light" ? "light" : "dark") as "dark" | "light";
    const sourceName = s.theme.startsWith(CUSTOM_PREFIX)
      ? (themes.find((th) => `${CUSTOM_PREFIX}${th.id}` === s.theme)?.name ?? tr("settings.themes.custom"))
      : tr(base === "light" ? "settings.appearance.light" : "settings.appearance.dark");
    const theme: CustomTheme = { id: newId(), name: tr("settings.themes.copyName", { name: sourceName }), base, colors };
    changeThemes((list) => [...list, theme], `${CUSTOM_PREFIX}${theme.id}`);
    editing = theme.id;
  }

  function update(id: string, patch: (th: CustomTheme) => Partial<CustomTheme>) {
    changeThemes((list) => list.map((th) => (th.id === id ? { ...th, ...patch(th) } : th)));
  }

  /** Set a token; an empty value goes back to the base theme's colour. */
  function setColor(th: CustomTheme, token: ThemeToken, value: string) {
    const v = value.trim();
    if (!v) {
      update(th.id, (cur) => {
        const colors = { ...cur.colors };
        delete colors[token];
        return { colors };
      });
      return;
    }
    if (!isColor(v)) {
      toast("warning", tr("settings.themes.invalidColour", { value: v }));
      return;
    }
    update(th.id, (cur) => ({ colors: { ...cur.colors, [token]: v } }));
  }

  /** What the picker shows: the token's value, else what's on screen for it. */
  function pickerValue(th: CustomTheme, token: ThemeToken): string {
    const own = th.colors[token];
    if (own) return toHexInput(own);
    if (s.theme === `${CUSTOM_PREFIX}${th.id}`) {
      return toHexInput(getComputedStyle(document.documentElement).getPropertyValue(token).trim());
    }
    return toHexInput("");
  }

  async function remove(th: CustomTheme) {
    const ok = await ask(tr("settings.themes.deleteConfirm", { name: th.name }), {
      title: tr("settings.themes.deleteTitle"),
      kind: "warning",
      okLabel: tr("common.delete"),
    });
    if (!ok) return;
    const selected = s.theme === `${CUSTOM_PREFIX}${th.id}`;
    changeThemes((list) => list.filter((x) => x.id !== th.id), selected ? th.base : undefined);
    if (editing === th.id) editing = null;
  }

  async function importTheme() {
    const path = await open({
      title: tr("settings.themes.importTitle"),
      multiple: false,
      directory: false,
      filters: [{ name: tr("settings.themes.fileFilter"), extensions: ["json"] }],
    });
    if (!path || Array.isArray(path)) return;
    try {
      const parsed = parseTheme(await tauri.importThemeFile(path), newId());
      if ("error" in parsed) {
        toastError(tr("settings.themes.importFailed"), parsed.error);
        return;
      }
      const theme = parsed.theme;
      changeThemes((list) => [...list, theme], `${CUSTOM_PREFIX}${theme.id}`);
      toast("success", tr("settings.themes.imported", { name: parsed.theme.name }));
    } catch (err) {
      toastError(tr("settings.themes.importFailed"), err);
    }
  }

  async function exportOne(th: CustomTheme) {
    const path = await save({
      title: tr("settings.themes.exportTitle"),
      defaultPath: `${th.name.replace(/[^\w.-]+/g, "-").toLowerCase() || "theme"}.json`,
      filters: [{ name: tr("settings.themes.fileFilter"), extensions: ["json"] }],
    });
    if (!path) return;
    try {
      await tauri.exportThemeFile(path, exportTheme(th));
      toast("success", tr("settings.themes.exported", { path }));
    } catch (err) {
      toastError(tr("settings.themes.exportFailed"), err);
    }
  }
</script>

<div class="custom">
  <div class="head">
    <span class="label-text">{$t("settings.themes.title")}</span>
    <span class="actions">
      <button class="btn" onclick={duplicateCurrent} title={$t("settings.themes.duplicateTitle")}><Copy size={12} /> {$t("settings.themes.duplicate")}</button>
      <button class="btn" onclick={importTheme}><Upload size={12} /> {$t("settings.themes.import")}</button>
    </span>
  </div>
  {#if themes.length === 0}
    <p class="hint">{$t("settings.themes.empty")}</p>
  {/if}
  {#each themes as th (th.id)}
    {@const selected = s.theme === `${CUSTOM_PREFIX}${th.id}`}
    <div class="theme" class:selected>
      <div class="theme-row">
        <button class="toggle" onclick={() => (editing = editing === th.id ? null : th.id)} aria-expanded={editing === th.id}>
          {#if editing === th.id}<ChevronDown size={13} />{:else}<ChevronRight size={13} />{/if}
          <span class="swatches">
            {#each ["--color-bg", "--color-surface", "--color-text-primary", "--color-lane-0", "--color-lane-1", "--color-lane-3"] as tok (tok)}
              {@const c = th.colors[tok as ThemeToken]}
              <!-- Only validated colours reach the style attribute (imported settings are untrusted). -->
              <span class="sw" style="background: {c && isColor(c) ? c : 'transparent'}"></span>
            {/each}
          </span>
          <span class="name">{th.name}</span>
          <span class="muted">{$t(th.base === "light" ? "settings.themes.baseLight" : "settings.themes.baseDark")}</span>
        </button>
        {#if !selected}
          <button class="btn" onclick={() => updateGlobalSettings({ theme: `${CUSTOM_PREFIX}${th.id}` })}>{$t("settings.themes.use")}</button>
        {:else}
          <span class="muted">{$t("settings.themes.inUse")}</span>
        {/if}
        <button class="icon" onclick={() => (editing = th.id)} title={$t("common.edit")} aria-label={$t("settings.themes.editAria", { name: th.name })}><Pencil size={12} /></button>
        <button class="icon" onclick={() => exportOne(th)} title={$t("settings.themes.export")} aria-label={$t("settings.themes.exportAria", { name: th.name })}><Download size={12} /></button>
        <button class="icon" onclick={() => remove(th)} title={$t("common.delete")} aria-label={$t("settings.themes.deleteAria", { name: th.name })}><Trash2 size={12} /></button>
      </div>
      {#if editing === th.id && current}
        <div class="editor">
          <div class="meta">
            <label>{$t("settings.themes.name")} <input value={current.name} maxlength="60" onchange={(e) => {
              const name = e.currentTarget.value.trim();
              if (name) update(th.id, () => ({ name }));
            }} /></label>
            <label>
              {$t("settings.themes.base")}
              <select value={current.base} onchange={(e) => {
                const base = e.currentTarget.value as "dark" | "light";
                update(th.id, () => ({ base }));
              }}>
                <option value="dark">{$t("settings.appearance.dark")}</option>
                <option value="light">{$t("settings.appearance.light")}</option>
              </select>
            </label>
            {#if !selected}<span class="muted">{$t("settings.themes.previewHint")}</span>{/if}
          </div>
          {#each warnings as w (w)}<p class="warn" role="alert">{w}</p>{/each}
          {#each TOKEN_GROUPS as group (group.title)}
            <fieldset>
              <legend>{GROUP_KEYS[group.title] ? $t(GROUP_KEYS[group.title]) : group.title}</legend>
              {#each group.tokens as token (token)}
                {@const value = current.colors[token] ?? ""}
                <div class="token">
                  <input
                    type="color"
                    value={pickerValue(current, token)}
                    oninput={(e) => setColor(current, token, pickedColor(e.currentTarget.value, value))}
                    aria-label={$t("settings.themes.colourAria", { token: tokenLabel(token) })}
                  />
                  <span class="token-name">{tokenLabel(token)}</span>
                  <input
                    class="token-value"
                    value={value}
                    placeholder={$t(current.base === "light" ? "settings.themes.placeholderLight" : "settings.themes.placeholderDark")}
                    title={$t(current.base === "light" ? "settings.themes.emptyTitleLight" : "settings.themes.emptyTitleDark")}
                    spellcheck="false"
                    onchange={(e) => setColor(th, token, e.currentTarget.value)}
                    aria-label={$t("settings.themes.valueAria", { token: tokenLabel(token) })}
                  />
                </div>
              {/each}
            </fieldset>
          {/each}
        </div>
      {/if}
    </div>
  {/each}
</div>

<style>
  .custom { display: flex; flex-direction: column; gap: 8px; padding: 12px 0; border-bottom: 1px solid var(--color-border); }
  .head { display: flex; align-items: center; justify-content: space-between; }
  .label-text { font-size: 13px; font-weight: 500; color: var(--color-text-primary); }
  .actions { display: flex; gap: 8px; }
  .hint, .muted { font-size: 11px; color: var(--color-text-muted); margin: 0; }
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
  }
  .btn:hover { border-color: var(--color-accent); }
  .icon { display: flex; background: none; border: none; padding: 3px; color: var(--color-text-muted); cursor: pointer; }
  .icon:hover { color: var(--color-text-primary); }
  .theme { border: 1px solid var(--color-border); border-radius: 6px; }
  .theme.selected { border-color: var(--color-accent); }
  .theme-row { display: flex; align-items: center; gap: 8px; padding: 6px 8px; }
  .toggle { flex: 1; min-width: 0; display: flex; align-items: center; gap: 8px; background: none; border: none; color: var(--color-text-primary); cursor: pointer; text-align: left; font-size: 12px; }
  .name { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .swatches { display: flex; border: 1px solid var(--color-border); border-radius: 3px; overflow: hidden; flex-shrink: 0; }
  .sw { width: 12px; height: 14px; }
  .editor { padding: 8px 12px 12px; border-top: 1px solid var(--color-border); display: flex; flex-direction: column; gap: 8px; }
  .meta { display: flex; align-items: center; gap: 12px; font-size: 12px; color: var(--color-text-muted); }
  .meta input, .meta select, .token-value {
    padding: 4px 6px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 12px;
  }
  .warn { margin: 0; padding: 6px 8px; border-radius: 4px; background: var(--color-diff-del-bg); color: var(--color-text-primary); font-size: 12px; }
  fieldset { border: 1px solid var(--color-border); border-radius: 4px; padding: 6px 8px; margin: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 4px 12px; }
  legend { font-size: 11px; color: var(--color-text-muted); padding: 0 4px; }
  .token { display: flex; align-items: center; gap: 6px; }
  .token input[type="color"] { width: 28px; height: 22px; padding: 0; border: 1px solid var(--color-border); border-radius: 3px; background: none; cursor: pointer; }
  .token-name { flex: 1; font-size: 12px; color: var(--color-text-primary); }
  .token-value { width: 140px; font-family: var(--font-mono); font-size: 11px; }
</style>
