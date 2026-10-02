<script lang="ts">
  import { sidebarOpen, sidebarWidth } from "../../lib/stores/ui";
  import BranchList from "../branches/BranchList.svelte";
  import TagList from "../tags/TagList.svelte";
  import BranchActionsHost from "../branches/BranchActionsHost.svelte";

  const isOpen = $derived($sidebarOpen);
  const width = $derived($sidebarWidth);
</script>

{#if isOpen}
  <aside class="sidebar" style="width: {width}px; min-width: 160px;">
    <BranchList />
    <TagList />
  </aside>
{/if}
<!-- Branch list / remotes: global actions + remotes dialog, mounted even when the sidebar is hidden -->
<BranchActionsHost />

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    background: var(--color-surface);
    overflow-y: auto;
    overflow-x: hidden;
    flex-shrink: 0;
  }
</style>
