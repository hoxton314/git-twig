<script lang="ts">
  import { sidebarOpen, sidebarWidth } from "../../lib/stores/ui";
  import BranchList from "../branches/BranchList.svelte";

  const isOpen = $derived($sidebarOpen);
  const width = $derived($sidebarWidth);
</script>

{#if isOpen}
  <aside class="sidebar" style="width: {width}px; min-width: {width}px;">
    <BranchList />
  </aside>
{/if}

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    background: var(--color-surface);
    border-right: 1px solid var(--color-border);
    overflow-y: auto;
    overflow-x: hidden;
    flex-shrink: 0;
  }
</style>
