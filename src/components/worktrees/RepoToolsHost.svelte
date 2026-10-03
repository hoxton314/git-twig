<script lang="ts">
  /**
   * Global handlers for worktree / submodule actions (so they work from the
   * command palette even when the sidebar is hidden) and the Add-worktree
   * dialog. Mounted once in AppShell.
   */
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { onAction } from "../../lib/keybindings";
  import { activeRepoPath } from "../../lib/stores/repos";
  import {
    addWorktreeOpen,
    submodules,
    refreshRepoTools,
    updateSubmodules,
    syncSubmodules,
  } from "../../lib/stores/repotools";
  import { toast } from "../../lib/stores/toasts";
  import { tr } from "../../lib/i18n";
  import AddWorktreeDialog from "./AddWorktreeDialog.svelte";

  async function withSubmodules(fn: () => void) {
    if (!get(activeRepoPath)) return;
    await refreshRepoTools();
    if (get(submodules).length === 0) {
      toast("info", tr("worktrees.noSubmodules"));
      return;
    }
    fn();
  }

  onMount(() => {
    const unsubs = [
      onAction("worktree_add", () => {
        if (get(activeRepoPath)) addWorktreeOpen.set(true);
      }),
      onAction("submodules_update", () => withSubmodules(() => updateSubmodules())),
      onAction("submodules_sync", () => withSubmodules(() => syncSubmodules())),
    ];
    return () => unsubs.forEach((u) => u());
  });
</script>

{#if $activeRepoPath}
  <AddWorktreeDialog />
{/if}
