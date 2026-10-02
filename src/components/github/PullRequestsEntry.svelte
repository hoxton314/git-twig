<!-- Sidebar entry that opens the Pull Requests panel. Hidden when the
     active repo has no GitHub / GitLab / Gitea remote. -->
<script lang="ts">
  import { GitPullRequest } from "lucide-svelte";
  import * as tauri from "../../lib/tauri";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { prPanelOpen } from "../../lib/stores/pullRequests";
  import { getShortcut } from "../../lib/keybindings";
  import type { ProviderKind } from "../../lib/types/hosting";

  let provider = $state<ProviderKind | null>(null);

  $effect(() => {
    const path = $activeRepoPath;
    provider = null;
    if (!path) return;
    let cancelled = false;
    tauri
      .hostingListRemotes(path)
      .then((r) => {
        if (!cancelled) provider = r[0]?.provider ?? null;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  });

  const label = $derived(provider === "gitlab" ? "Merge Requests" : "Pull Requests");
  const shortcut = $derived(getShortcut("open_pull_requests"));
</script>

{#if provider}
  <button
    class="pr-entry"
    onclick={() => prPanelOpen.set(true)}
    title={shortcut ? `${label} (${shortcut})` : label}
  >
    <GitPullRequest size={14} />
    <span>{label}</span>
  </button>
{/if}

<style>
  .pr-entry {
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

  .pr-entry:hover {
    background: var(--color-surface-elevated);
  }

  .pr-entry :global(svg) {
    color: var(--color-accent);
  }
</style>
