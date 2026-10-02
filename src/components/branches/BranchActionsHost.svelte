<!--
  Always-mounted host for branch/remote global actions (palette + shortcuts).
  The branch list itself unmounts when the sidebar is hidden, so actions are
  registered here and forwarded to it via `requestBranchList`.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { onAction } from "../../lib/keybindings";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { remotesDialogOpen, requestBranchList } from "../../lib/stores/remotes";
  import RemotesDialog from "./RemotesDialog.svelte";

  onMount(() => {
    const whenRepo = (fn: () => void) => () => {
      if (get(activeRepoPath)) fn();
    };
    const unsubs = [
      onAction("remotes_manage", whenRepo(() => remotesDialogOpen.set(true))),
      onAction("branch_create", whenRepo(() => requestBranchList("create"))),
      onAction("branch_rename_current", whenRepo(() => requestBranchList("rename_current"))),
      onAction("branch_filter", whenRepo(() => requestBranchList("filter"))),
    ];
    return () => unsubs.forEach((u) => u());
  });
</script>

<RemotesDialog />
