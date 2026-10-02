<script lang="ts">
  import {
    ArrowUpFromLine,
    ArrowDownToLine,
    RefreshCw,
    Loader2,
    GitPullRequest,
    ChevronDown,
  } from "lucide-svelte";
  import StashPanel from "./StashPanel.svelte";
  import ChangedFiles from "./ChangedFiles.svelte";
  import CommitBox from "./CommitBox.svelte";
  import CreatePullRequest from "../github/CreatePullRequest.svelte";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { refreshStatus, refreshAll } from "../../lib/stores/graph";
  import * as tauri from "../../lib/tauri";
  import { message } from "@tauri-apps/plugin-dialog";
  import { onAction, shortcutLabels, withShortcut } from "../../lib/keybindings";
  import { onMount } from "svelte";
  import { trackOperation } from "../../lib/stores/operations";
  // Force push (with lease)
  import ContextMenu from "../shared/ContextMenu.svelte";
  import { forcePush } from "../../lib/stores/operation";
  let pushMenu = $state<{ x: number; y: number } | null>(null);

  const repoPath = $derived($activeRepoPath);
  let pushLoading = $state(false);
  let pullLoading = $state(false);
  let showPrModal = $state(false);

  // Load working status when the repo changes. Commit message drafts and
  // file lists live in CommitBox / ChangedFiles.
  let lastLoadedPath: string | null = null;
  $effect(() => {
    const path = repoPath;
    if (path && path !== lastLoadedPath) {
      lastLoadedPath = path;
      refreshStatus(path);
    }
  });

  async function handlePush() {
    if (!repoPath) return;
    pushLoading = true;
    try {
      const info = await tauri.getRepoInfo(repoPath);
      const branch = info.head_name ?? "HEAD";
      const path = repoPath;
      const result = await trackOperation(path, "push", `Pushing ${branch}…`, () =>
        tauri.pushBranch(path, branch, undefined, true),
      );
      if (result.success) {
        await refreshAll();
      } else {
        await message(result.message, { title: "Push Failed", kind: "error" });
      }
    } catch (err) {
      await message(String(err), { title: "Push Failed", kind: "error" });
    } finally {
      pushLoading = false;
    }
  }

  async function handlePull() {
    if (!repoPath) return;
    pullLoading = true;
    try {
      const path = repoPath;
      const result = await trackOperation(path, "pull", "Pulling…", () => tauri.pull(path));
      await refreshAll();
      if (!result.success) {
        await message(result.message, { title: "Pull Failed", kind: "error" });
      } else if (result.message.includes("conflicts")) {
        await message(result.message, { title: "Pull — Stash Conflicts", kind: "warning" });
      }
    } catch (err) {
      await message(String(err), { title: "Pull Failed", kind: "error" });
    } finally {
      pullLoading = false;
    }
  }

  // Hosting integrations: "Create pull request" palette/keybinding action.
  onMount(() => onAction("create_pull_request", () => (showPrModal = true)));
</script>

<div class="staging-area">
  <!-- Push/Pull toolbar -->
  <div class="toolbar">
    <button
      class="toolbar-btn"
      onclick={handlePull}
      disabled={pullLoading}
      title={withShortcut("Pull", $shortcutLabels["pull"])}
    >
      {#if pullLoading}
        <Loader2 size={14} class="spinner" />
      {:else}
        <ArrowDownToLine size={14} />
      {/if}
      <span>Pull</span>
    </button>
    <button
      class="toolbar-btn"
      onclick={handlePush}
      disabled={pushLoading}
      title={withShortcut("Push", $shortcutLabels["push"])}
    >
      {#if pushLoading}
        <Loader2 size={14} class="spinner" />
      {:else}
        <ArrowUpFromLine size={14} />
      {/if}
      <span>Push</span>
    </button>
    <!-- Force push (with lease) menu -->
    <button
      class="toolbar-btn push-more"
      onclick={(e) => {
        const r = e.currentTarget.getBoundingClientRect();
        pushMenu = { x: r.left, y: r.bottom + 2 };
      }}
      disabled={pushLoading}
      title="More push options"
      aria-label="More push options"
      aria-haspopup="menu"
    >
      <ChevronDown size={12} />
    </button>
    {#if pushMenu}
      <ContextMenu
        x={pushMenu.x}
        y={pushMenu.y}
        items={[
          { label: "Push", action: handlePush },
          { label: "Force push (with lease)…", action: () => { forcePush(); }, danger: true },
        ]}
        onclose={() => (pushMenu = null)}
      />
    {/if}
    <button
      class="toolbar-btn"
      onclick={() => (showPrModal = true)}
      title="Create Pull Request"
    >
      <GitPullRequest size={14} />
      <span>PR</span>
    </button>
    <button
      class="toolbar-btn icon-only"
      onclick={() => refreshStatus()}
      title="Refresh"
    >
      <RefreshCw size={14} />
    </button>
  </div>

  {#if repoPath}
    <CreatePullRequest
      open_={showPrModal}
      onclose={() => (showPrModal = false)}
      repoPath={repoPath}
    />
  {/if}

  <ChangedFiles />

  <!-- Stash -->
  <StashPanel />

  <CommitBox />
</div>

<style>
  .staging-area {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    font-size: 12px;
  }

  /* Toolbar */
  .toolbar {
    display: flex;
    gap: 4px;
    padding: 8px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .toolbar-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 11px;
    cursor: pointer;
    transition: background 0.1s;
  }

  .toolbar-btn:hover {
    background: var(--color-surface-elevated);
  }

  .toolbar-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .toolbar-btn.push-more {
    padding: 4px 3px;
    margin-left: -3px;
  }

  .toolbar-btn.icon-only {
    padding: 4px 6px;
    margin-left: auto;
  }

  .toolbar :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
