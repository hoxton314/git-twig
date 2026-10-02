<!-- Sidebar entry that opens the Git LFS panel. Shown when git-lfs is
     installed and the active repo tracks files with it. -->
<script lang="ts">
  import { HardDrive } from "lucide-svelte";
  import { lfsPanelOpen, lfsStatus } from "../../lib/stores/lfs";

  const count = $derived($lfsStatus?.patterns.length ?? 0);
</script>

{#if $lfsStatus?.version && count > 0}
  <button class="lfs-entry" onclick={() => lfsPanelOpen.set(true)} title="Tracked patterns, locks, fetch and prune">
    <HardDrive size={14} />
    <span>Git LFS</span>
    <span class="count">{count} pattern{count === 1 ? "" : "s"}</span>
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
