<script lang="ts">
  import { settings, updateSettings } from "../../lib/stores/settings";
  import type { GraphDateFormat, GraphRowDensity } from "../../lib/types/git";
  import { DEFAULT_COL_WIDTHS } from "./graphLayout";
  import GraphPopover from "./GraphPopover.svelte";
  import { t } from "../../lib/i18n";

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  const s = $derived($settings);

  function resetWidths() {
    updateSettings({
      graph_author_width: DEFAULT_COL_WIDTHS.author,
      graph_sha_width: DEFAULT_COL_WIDTHS.sha,
      graph_date_width: DEFAULT_COL_WIDTHS.date,
    });
  }
</script>

<GraphPopover label={$t("graph.viewOptionsLabel")} {onclose} width={250}>
  <div class="group">
    <div class="group-title">{$t("graph.columns")}</div>
    <label class="check">
      <input
        type="checkbox"
        checked={s.graph_show_author}
        onchange={(e) => updateSettings({ graph_show_author: e.currentTarget.checked })}
      />
      {$t("graph.colAuthor")}
    </label>
    <label class="check">
      <input
        type="checkbox"
        checked={s.graph_show_sha}
        onchange={(e) => updateSettings({ graph_show_sha: e.currentTarget.checked })}
      />
      {$t("graph.colSha")}
    </label>
    <label class="check">
      <input
        type="checkbox"
        checked={s.graph_show_date}
        onchange={(e) => updateSettings({ graph_show_date: e.currentTarget.checked })}
      />
      {$t("graph.colDate")}
    </label>
    <button class="link" onclick={resetWidths}>{$t("graph.resetWidths")}</button>
  </div>

  <div class="group">
    <label class="field">
      <span class="group-title">{$t("graph.dateFormat")}</span>
      <select
        value={s.graph_date_format}
        onchange={(e) =>
          updateSettings({ graph_date_format: e.currentTarget.value as GraphDateFormat })}
      >
        <option value="relative">{$t("graph.dateRelative")}</option>
        <option value="iso">{$t("graph.dateAbsolute")}</option>
        <option value="locale">{$t("graph.dateLocale")}</option>
      </select>
    </label>
    <label class="field">
      <span class="group-title">{$t("graph.rowDensity")}</span>
      <select
        value={s.graph_row_density}
        onchange={(e) =>
          updateSettings({ graph_row_density: e.currentTarget.value as GraphRowDensity })}
      >
        <option value="compact">{$t("graph.densityCompact")}</option>
        <option value="normal">{$t("graph.densityNormal")}</option>
        <option value="comfortable">{$t("graph.densityComfortable")}</option>
      </select>
    </label>
  </div>

  <div class="group">
    <div class="group-title">{$t("graph.history")}</div>
    <label class="check">
      <input
        type="checkbox"
        checked={s.graph_hide_remotes}
        disabled={s.graph_current_branch_only}
        onchange={(e) => updateSettings({ graph_hide_remotes: e.currentTarget.checked })}
      />
      {$t("graph.hideRemotes")}
    </label>
    <label class="check">
      <input
        type="checkbox"
        checked={s.graph_current_branch_only}
        onchange={(e) => updateSettings({ graph_current_branch_only: e.currentTarget.checked })}
      />
      {$t("graph.currentBranchOnly")}
    </label>
  </div>
</GraphPopover>

<style>
  .group {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 4px 0 10px;
  }

  .group + .group {
    border-top: 1px solid var(--color-border);
    padding-top: 10px;
  }

  .group:last-child {
    padding-bottom: 0;
  }

  .group-title {
    color: var(--color-text-muted);
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }

  .check:has(input:disabled) {
    opacity: 0.5;
    cursor: default;
  }

  .check input {
    accent-color: var(--color-accent);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field select {
    width: 100%;
  }

  .link {
    align-self: flex-start;
    padding: 0;
    border: none;
    background: none;
    color: var(--color-accent);
    font-size: 11px;
    cursor: pointer;
  }

  .link:hover {
    text-decoration: underline;
  }
</style>
