<script lang="ts">
  /**
   * Hosts the conflict / rebase dialogs and registers their keybinding
   * actions (also reachable from the command palette via ACTIONS).
   */
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import ContinueDialog from "./ContinueDialog.svelte";
  import ConflictResolver from "./ConflictResolver.svelte";
  import RebaseDialog from "../rebase/RebaseDialog.svelte";
  import InteractiveRebase from "../rebase/InteractiveRebase.svelte";
  import { onAction } from "../../lib/keybindings";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { toast } from "../../lib/stores/toasts";
  import {
    operationState,
    requestContinue,
    abortOperation,
    skipOperation,
    openConflictResolver,
    openRebaseDialog,
    openInteractiveRebase,
    forcePush,
  } from "../../lib/stores/operation";

  function needsOperation(fn: () => void) {
    return () => {
      if (!get(activeRepoPath)) return;
      const st = get(operationState);
      if (!st || st.kind === "none") {
        toast("info", "No merge, rebase, cherry-pick or revert is in progress.");
        return;
      }
      fn();
    };
  }

  onMount(() => {
    const unsubs = [
      onAction("force_push", () => {
        if (get(activeRepoPath)) forcePush();
      }),
      onAction("rebase_onto", () => openRebaseDialog()),
      onAction("interactive_rebase", () => openInteractiveRebase()),
      onAction("operation_continue", needsOperation(requestContinue)),
      onAction("operation_abort", needsOperation(abortOperation)),
      onAction("operation_skip", needsOperation(skipOperation)),
      onAction("resolve_conflicts", () => {
        if (get(activeRepoPath)) openConflictResolver();
      }),
    ];
    return () => unsubs.forEach((u) => u());
  });
</script>

{#if $activeRepoPath}
  <ContinueDialog />
  <ConflictResolver />
  <RebaseDialog />
  <InteractiveRebase />
{/if}
