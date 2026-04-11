<script lang="ts">
  import { settings, updateSettings } from "../../lib/stores/settings";

  const s = $derived($settings);

  const accentColors = [
    { value: "#7aa2f7", label: "Blue" },
    { value: "#9ece6a", label: "Green" },
    { value: "#e0af68", label: "Yellow" },
    { value: "#f7768e", label: "Red" },
    { value: "#bb9af7", label: "Purple" },
    { value: "#2ac3de", label: "Cyan" },
    { value: "#ff9e64", label: "Orange" },
  ];

  function handleAccentChange(color: string) {
    updateSettings({ accent_color: color });
  }
</script>

<div class="section">
  <h1 class="section-heading">Appearance</h1>

  <div class="setting-group">
    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Theme</span>
        <span class="label-hint">Application color theme</span>
      </div>
      <div class="setting-control">
        <select disabled>
          <option>Dark</option>
        </select>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Accent color</span>
        <span class="label-hint">Primary highlight color throughout the interface</span>
      </div>
      <div class="setting-control">
        <div class="color-swatches">
          {#each accentColors as color (color.value)}
            <button
              class="color-swatch"
              class:active={s.accent_color === color.value}
              style="--swatch-color: {color.value}"
              onclick={() => handleAccentChange(color.value)}
              title={color.label}
            ></button>
          {/each}
        </div>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Interface font size</span>
        <span class="label-hint">Size of text in the main interface ({s.font_size}px)</span>
      </div>
      <div class="setting-control">
        <input
          type="range"
          min="11"
          max="16"
          step="1"
          value={s.font_size}
          oninput={(e) => updateSettings({ font_size: Number(e.currentTarget.value) })}
        />
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Diff font size</span>
        <span class="label-hint">Size of text in diff and code views ({s.diff_font_size}px)</span>
      </div>
      <div class="setting-control">
        <input
          type="range"
          min="11"
          max="16"
          step="1"
          value={s.diff_font_size}
          oninput={(e) => updateSettings({ diff_font_size: Number(e.currentTarget.value) })}
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


  .color-swatches {
    display: flex;
    gap: 6px;
  }

  .color-swatch {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 2px solid transparent;
    background: var(--swatch-color);
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

  input[type="range"] {
    width: 120px;
    accent-color: var(--color-accent);
    cursor: pointer;
  }
</style>
