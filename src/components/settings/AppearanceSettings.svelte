<script lang="ts">
  import { Check } from "lucide-svelte";
  import CustomThemes from "./CustomThemes.svelte";
  import { t, type MessageKey } from "../../lib/i18n";
  import { globalSettings, updateGlobalSettings, cssFontFamily, UI_FONT_FALLBACK, MONO_FONT_FALLBACK } from "../../lib/stores/settings";

  const s = $derived($globalSettings);

  const accentColors: { value: string; key: MessageKey }[] = [
    { value: "#7aa2f7", key: "settings.appearance.blue" },
    { value: "#9ece6a", key: "settings.appearance.green" },
    { value: "#e0af68", key: "settings.appearance.yellow" },
    { value: "#f7768e", key: "settings.appearance.red" },
    { value: "#bb9af7", key: "settings.appearance.purple" },
    { value: "#2ac3de", key: "settings.appearance.cyan" },
    { value: "#ff9e64", key: "settings.appearance.orange" },
  ];

  /** Sample text for the monospace preview (code, not language). */
  const MONO_SAMPLE = "a1b2c3d fn main() {} -> != ===";

  const uiFontSuggestions = ["Inter", "Cantarell", "Noto Sans", "Ubuntu", "Segoe UI", "SF Pro Text", "Roboto", "IBM Plex Sans"];
  const monoFontSuggestions = [
    "JetBrains Mono",
    "Fira Code",
    "Cascadia Code",
    "Source Code Pro",
    "Hack",
    "Iosevka",
    "IBM Plex Mono",
    "DejaVu Sans Mono",
    "Menlo",
    "Consolas",
  ];

  function handleAccentChange(color: string) {
    updateGlobalSettings({ accent_color: color });
  }

  /** Commit a font family on change (blur / Enter / datalist pick). */
  function commitFont(field: "ui_font_family" | "mono_font_family", input: HTMLInputElement) {
    const value = input.value.trim();
    input.value = value;
    updateGlobalSettings({ [field]: value });
  }

  const uiPreview = $derived(
    cssFontFamily(s.ui_font_family) ? `${cssFontFamily(s.ui_font_family)}, ${UI_FONT_FALLBACK}` : UI_FONT_FALLBACK,
  );
  const monoPreview = $derived(
    cssFontFamily(s.mono_font_family) ? `${cssFontFamily(s.mono_font_family)}, ${MONO_FONT_FALLBACK}` : MONO_FONT_FALLBACK,
  );
</script>

<div class="section">
  <h1 class="section-heading">{$t("settings.nav.appearance")}</h1>

  <div class="setting-group">
    <div class="setting-row">
      <div class="setting-label">
        <label class="label-text" for="theme-select">{$t("settings.appearance.theme")}</label>
        <span class="label-hint">{$t("settings.appearance.themeHint")}</span>
      </div>
      <div class="setting-control">
        <select
          id="theme-select"
          value={s.theme}
          onchange={(e) => updateGlobalSettings({ theme: e.currentTarget.value })}
        >
          <option value="dark">{$t("settings.appearance.dark")}</option>
          <option value="light">{$t("settings.appearance.light")}</option>
          {#each s.custom_themes ?? [] as theme (theme.id)}
            <option value="custom:{theme.id}">{theme.name}</option>
          {/each}
        </select>
      </div>
    </div>

    <CustomThemes />

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text" id="accent-label">{$t("settings.appearance.accent")}</span>
        <span class="label-hint">{$t("settings.appearance.accentHint")}</span>
      </div>
      <div class="setting-control">
        <div class="color-swatches" role="radiogroup" aria-labelledby="accent-label">
          {#each accentColors as color (color.value)}
            {@const selected = s.accent_color.toLowerCase() === color.value}
            <button
              class="color-swatch"
              class:active={selected}
              style="--swatch-color: {color.value}"
              onclick={() => handleAccentChange(color.value)}
              title={$t(color.key)}
              role="radio"
              aria-checked={selected}
              aria-label={$t("settings.appearance.accentAria", { color: $t(color.key) })}
            >
              {#if selected}
                <Check size={14} strokeWidth={3} aria-hidden="true" />
              {/if}
            </button>
          {/each}
        </div>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">{$t("settings.appearance.fontSize")}</span>
        <span class="label-hint">{$t("settings.appearance.fontSizeHint", { size: s.font_size })}</span>
      </div>
      <div class="setting-control">
        <input
          type="range"
          aria-label={$t("settings.appearance.fontSize")}
          aria-valuetext={$t("settings.appearance.pixels", { count: s.font_size })}
          min="11"
          max="16"
          step="1"
          value={s.font_size}
          oninput={(e) => updateGlobalSettings({ font_size: Number(e.currentTarget.value) })}
        />
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">{$t("settings.appearance.diffFontSize")}</span>
        <span class="label-hint">{$t("settings.appearance.diffFontSizeHint", { size: s.diff_font_size })}</span>
      </div>
      <div class="setting-control">
        <input
          type="range"
          aria-label={$t("settings.appearance.diffFontSize")}
          aria-valuetext={$t("settings.appearance.pixels", { count: s.diff_font_size })}
          min="11"
          max="16"
          step="1"
          value={s.diff_font_size}
          oninput={(e) => updateGlobalSettings({ diff_font_size: Number(e.currentTarget.value) })}
        />
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <label class="label-text" for="ui-font-input">{$t("settings.appearance.uiFont")}</label>
        <span class="label-hint">
          {$t("settings.appearance.uiFontHint")}
          <span class="font-preview" style="font-family: {uiPreview}">{$t("settings.appearance.uiFontSample")}</span>
        </span>
      </div>
      <div class="setting-control font-control">
        <input
          id="ui-font-input"
          type="text"
          list="ui-font-suggestions"
          placeholder={$t("settings.appearance.uiFontPlaceholder")}
          spellcheck="false"
          value={s.ui_font_family}
          onchange={(e) => commitFont("ui_font_family", e.currentTarget)}
          onkeydown={(e) => e.key === "Enter" && e.currentTarget.blur()}
        />
        <datalist id="ui-font-suggestions">
          {#each uiFontSuggestions as f (f)}<option value={f}></option>{/each}
        </datalist>
        {#if s.ui_font_family}
          <button class="btn-ghost" onclick={() => updateGlobalSettings({ ui_font_family: "" })} aria-label={$t("settings.appearance.uiFontReset")}>{$t("common.reset")}</button>
        {/if}
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <label class="label-text" for="mono-font-input">{$t("settings.appearance.monoFont")}</label>
        <span class="label-hint">
          {$t("settings.appearance.monoFontHint")}
          <span class="font-preview mono" style="font-family: {monoPreview}">{MONO_SAMPLE}</span>
        </span>
      </div>
      <div class="setting-control font-control">
        <input
          id="mono-font-input"
          type="text"
          list="mono-font-suggestions"
          placeholder={$t("settings.appearance.monoFontPlaceholder")}
          spellcheck="false"
          value={s.mono_font_family}
          onchange={(e) => commitFont("mono_font_family", e.currentTarget)}
          onkeydown={(e) => e.key === "Enter" && e.currentTarget.blur()}
        />
        <datalist id="mono-font-suggestions">
          {#each monoFontSuggestions as f (f)}<option value={f}></option>{/each}
        </datalist>
        {#if s.mono_font_family}
          <button class="btn-ghost" onclick={() => updateGlobalSettings({ mono_font_family: "" })} aria-label={$t("settings.appearance.monoFontReset")}>{$t("common.reset")}</button>
        {/if}
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


  .color-swatches {
    display: flex;
    gap: 6px;
  }

  .color-swatch {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 2px solid transparent;
    background: var(--swatch-color);
    /* Checkmark contrasts with every swatch color in both themes. */
    color: #1a1b26;
    cursor: pointer;
    padding: 0;
    transition: border-color 0.1s, transform 0.1s;
  }

  .color-swatch:hover {
    transform: scale(1.15);
  }

  .color-swatch.active {
    border-color: var(--color-text-primary);
    box-shadow: 0 0 0 2px var(--color-bg), 0 0 0 4px var(--swatch-color);
  }

  .color-swatch:focus-visible {
    outline: 2px solid var(--color-text-primary);
    outline-offset: 3px;
  }

  input[type="range"] {
    width: 120px;
    accent-color: var(--color-accent);
    cursor: pointer;
  }

  .font-control {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  input[type="text"] {
    width: 180px;
    padding: 6px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    font-family: inherit;
  }

  input[type="text"]:hover {
    border-color: var(--color-text-muted);
  }

  input[type="text"]:focus {
    outline: none;
    border-color: var(--color-accent);
  }

  .font-preview {
    display: block;
    margin-top: 4px;
    font-size: 13px;
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
    color: var(--color-text-primary);
  }
</style>
