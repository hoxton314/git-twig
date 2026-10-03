<!-- Sidebar entry that opens the Git LFS panel. Shown when git-lfs is
     installed and the active repo tracks files with it. -->
<script lang="ts">
  import { HardDrive } from "lucide-svelte";
  import { lfsPanelOpen, lfsStatus } from "../../lib/stores/lfs";
  import { t } from "../../lib/i18n";

  const count = $derived($lfsStatus?.patterns.length ?? 0);
</script>

{#if $lfsStatus?.version && count > 0}
  <button class="lfs-entry" onclick={() => lfsPanelOpen.set(true)} title={$t("lfs.entryTitle")}>
    <HardDrive size={14} />
    <span>Git LFS</span>
    <span class="count">{$t("lfs.patternCount", { count })}</span>
  </button>
{/if}

<style>
  .lfs-entry {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 8px 12px;
    border: none;
    border-top: 1px solid var(--color-border);
    background: transparent;
    color: var(--color-text-primary);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
    flex-shrink: 0;
  }
  .lfs-entry:hover { background: var(--color-surface); }
  .count { margin-left: auto; font-size: 11px; color: var(--color-text-muted); }
</style>
