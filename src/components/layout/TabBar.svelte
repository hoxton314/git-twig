<script lang="ts">
  import { X, Plus, GitBranch, House } from "lucide-svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openRepos, activeRepoPath, removeRepo, addRepo } from "../../lib/stores/repos";
  import * as tauri from "../../lib/tauri";

  const repos = $derived([...$openRepos.entries()]);
  const active = $derived($activeRepoPath);
  const homeActive = $derived(active === null);

  async function handleOpenRepo() {
    const selected = await open({ directory: true, multiple: false, title: "Open Git Repository" });
    if (!selected) return;
    try {
      const info = await tauri.openRepo(selected as string);
      addRepo(info);
    } catch (err) {
      console.error("Failed to open repo:", err);
    }
  }

  async function handleCloseTab(e: MouseEvent, path: string) {
    e.stopPropagation();
    removeRepo(path);
    try {
      await tauri.closeRepo(path);
    } catch {
      // Already removed from UI — ignore backend errors
    }
  }

  function handleSelectTab(path: string) {
    $activeRepoPath = path;
  }
</script>

<div class="tab-bar">
  <button
    class="home-btn"
    class:active={homeActive}
    onclick={() => ($activeRepoPath = null)}
    title="Home"
  >
    <House size={15} />
  </button>

  {#each repos as [path, info]}
    <div
      class="tab"
      class:active={active === path}
      onclick={() => handleSelectTab(path)}
      onkeydown={(e) => e.key === "Enter" && handleSelectTab(path)}
      role="tab"
      tabindex="0"
      title={path}
    >
      <GitBranch size={14} />
      <span class="tab-name">{info.name}</span>
      {#if info.head_name}
        <span class="tab-branch">{info.head_name}</span>
      {/if}
      <button
        class="tab-close"
        onclick={(e) => handleCloseTab(e, path)}
        title="Close tab"
      >
        <X size={12} />
      </button>
    </div>
  {/each}

  <button class="tab-new" onclick={handleOpenRepo} title="Open repository">
    <Plus size={16} />
  </button>
</div>

<style>
  .tab-bar {
    display: flex;
    align-items: center;
    height: var(--tab-height);
    background: var(--color-bg);
    border-bottom: 1px solid var(--color-border);
    overflow-x: auto;
    overflow-y: hidden;
    flex-shrink: 0;
    -webkit-app-region: drag;
  }

  .home-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--tab-height);
    height: 100%;
    border: none;
    border-right: 1px solid var(--color-border);
    background: var(--color-surface);
    color: var(--color-text-muted);
    cursor: pointer;
    flex-shrink: 0;
    -webkit-app-region: no-drag;
    transition: background 0.1s, color 0.1s;
  }

  .home-btn:hover {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .home-btn.active {
    background: var(--color-surface-elevated);
    color: var(--color-accent);
    border-bottom: 2px solid var(--color-accent);
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 100%;
    padding: 0 12px;
    border: none;
    border-right: 1px solid var(--color-border);
    background: var(--color-surface);
    color: var(--color-text-muted);
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
    -webkit-app-region: no-drag;
    transition: background 0.1s, color 0.1s;
  }

  .tab:hover {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .tab.active {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    border-bottom: 2px solid var(--color-accent);
  }

  .tab-name {
    font-weight: 500;
  }

  .tab-branch {
    color: var(--color-accent);
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .tab-close {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border: none;
    border-radius: 3px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 0;
    margin-left: 4px;
  }

  .tab-close:hover {
    background: rgba(247, 118, 142, 0.2);
    color: #f7768e;
  }

  .tab-new {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    padding: 0 10px;
    border: none;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    -webkit-app-region: no-drag;
  }

  .tab-new:hover {
    color: var(--color-accent);
    background: var(--color-surface);
  }
</style>
