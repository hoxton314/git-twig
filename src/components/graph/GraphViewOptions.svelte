<script lang="ts">
  import { settings, updateSettings } from "../../lib/stores/settings";
  import type { GraphDateFormat, GraphRowDensity } from "../../lib/types/git";
  import { DEFAULT_COL_WIDTHS } from "./graphLayout";
  import GraphPopover from "./GraphPopover.svelte";

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

<GraphPopover label="Graph view options" {onclose} width={250}>
  <div class="group">
    <div class="group-title">Columns</div>
    <label class="check">
      <input
        type="checkbox"
        checked={s.graph_show_author}
        onchange={(e) => updateSettings({ graph_show_author: e.currentTarget.checked })}
      />
      Author
    </label>
    <label class="check">
      <input
        type="checkbox"
        checked={s.graph_show_sha}
        onchange={(e) => updateSettings({ graph_show_sha: e.currentTarget.checked })}
      />
      SHA
    </label>
    <label class="check">
      <input
        type="checkbox"
        checked={s.graph_show_date}
        onchange={(e) => updateSettings({ graph_show_date: e.currentTarget.checked })}
      />
      Date
    </label>
    <button class="link" onclick={resetWidths}>Reset column widths</button>
  </div>

  <div class="group">
    <label class="field">
      <span class="group-title">Date format</span>
      <select
        value={s.graph_date_format}
        onchange={(e) =>
          updateSettings({ graph_date_format: e.currentTarget.value as GraphDateFormat })}
      >
        <option value="relative">Relative (5m ago)</option>
        <option value="iso">Absolute (2024-05-01 14:30)</option>
        <option value="locale">System locale</option>
      </select>
    </label>
    <label class="field">
      <span class="group-title">Row density</span>
      <select
        value={s.graph_row_density}
        onchange={(e) =>
          updateSettings({ graph_row_density: e.currentTarget.value as GraphRowDensity })}
      >
        <option value="compact">Compact</option>
        <option value="normal">Normal</option>
        <option value="comfortable">Comfortable</option>
      </select>
    </label>
  </div>

  <div class="group">
    <div class="group-title">History</div>
    <label class="check">
      <input
        type="checkbox"
        checked={s.graph_hide_remotes}
        disabled={s.graph_current_branch_only}
        onchange={(e) => updateSettings({ graph_hide_remotes: e.currentTarget.checked })}
      />
      Hide remote branches
    </label>
    <label class="check">
      <input
        type="checkbox"
        checked={s.graph_current_branch_only}
        onchange={(e) => updateSettings({ graph_current_branch_only: e.currentTarget.checked })}
      />
      Current branch only
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
