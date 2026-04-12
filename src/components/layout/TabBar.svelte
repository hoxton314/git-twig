<script lang="ts">
  import { X, Plus, GitBranch, House, Settings, FolderOpen, GitFork } from "lucide-svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openRepos, activeRepoPath, removeRepo, addRepo } from "../../lib/stores/repos";
  import { currentView } from "../../lib/stores/ui";
  import { settings } from "../../lib/stores/settings";
  import CloneFromGitHub from "../github/CloneFromGitHub.svelte";
  import CreateRepoOnGitHub from "../github/CreateRepoOnGitHub.svelte";
  import * as tauri from "../../lib/tauri";
  import type { RepoInfo } from "../../lib/types/git";

  const repos = $derived([...$openRepos.entries()]);
  const openPaths = $derived(new Set(repos.map(([p]) => p)));
  const active = $derived($activeRepoPath);
  const view = $derived($currentView);
  const homeActive = $derived(active === null && view !== "settings");

  let showMenu = $state(false);
  let showCloneModal = $state(false);
  let showCreateRepoModal = $state(false);
  let plusBtnEl: HTMLButtonElement | undefined = $state(undefined);
  let menuLeft = $state(0);
  let suggestedRepos = $state<RepoInfo[]>([]);

  function toggleMenu() {
    showMenu = !showMenu;
    if (showMenu && plusBtnEl) {
      menuLeft = plusBtnEl.getBoundingClientRect().left;
      const dir = $settings.default_repo_dir;
      if (dir) {
        tauri.listReposInDir(dir).then((r) => (suggestedRepos = r)).catch(() => (suggestedRepos = []));
      } else {
        suggestedRepos = [];
      }
    }
  }

  async function handleOpenRepo() {
    showMenu = false;
    const selected = await open({ directory: true, multiple: false, title: "Open Git Repository" });
    if (!selected) return;
    try {
      const info = await tauri.openRepo(selected as string);
      addRepo(info);
    } catch (err) {
      console.error("Failed to open repo:", err);
    }
  }

  async function openSuggestedRepo(path: string) {
    showMenu = false;
    try {
      const info = await tauri.openRepo(path);
      addRepo(info);
    } catch (err) {
      console.error("Failed to open repo:", err);
    }
  }

  function handleCloned(info: RepoInfo) {
    addRepo(info);
    $currentView = "repos";
  }

  function handleRepoCreated(info: RepoInfo) {
    addRepo(info);
    $currentView = "repos";
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
    $currentView = "repos";
  }

  function handleOpenSettings() {
    $currentView = "settings";
    $activeRepoPath = null;
  }
</script>

<div class="tab-bar">
  <button
    class="home-btn"
    class:active={homeActive}
    onclick={() => { $activeRepoPath = null; $currentView = "repos"; }}
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

  <button
    class="tab-new"
    bind:this={plusBtnEl}
    onclick={toggleMenu}
    title="Add repository"
  >
    <Plus size={16} />
  </button>

  <div class="tab-spacer"></div>

  <button
    class="settings-btn"
    class:active={view === "settings"}
    onclick={handleOpenSettings}
    title="Settings"
  >
    <Settings size={15} />
  </button>
</div>

{#if showMenu}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="tab-menu-backdrop" onclick={() => (showMenu = false)}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="tab-menu" style="left: {menuLeft}px" onclick={(e) => e.stopPropagation()}>
      {#if suggestedRepos.length > 0}
        <div class="tab-menu-section">Repositories</div>
        <div class="tab-menu-suggestions">
          {#each suggestedRepos as repo (repo.path)}
            {#if openPaths.has(repo.path)}
              <button class="tab-menu-item tab-menu-item-open" onclick={() => { showMenu = false; $activeRepoPath = repo.path; $currentView = "repos"; }}>
                <GitBranch size={14} />
                <span class="tab-menu-repo-name">{repo.name}</span>
                {#if repo.head_name}
                  <span class="tab-menu-repo-branch">{repo.head_name}</span>
                {/if}
              </button>
            {:else}
              <button class="tab-menu-item" onclick={() => openSuggestedRepo(repo.path)}>
                <GitBranch size={14} />
                <span class="tab-menu-repo-name">{repo.name}</span>
                {#if repo.head_name}
                  <span class="tab-menu-repo-branch">{repo.head_name}</span>
                {/if}
              </button>
            {/if}
          {/each}
        </div>
        <div class="tab-menu-divider"></div>
      {/if}
      <button class="tab-menu-item" onclick={handleOpenRepo}>
        <FolderOpen size={14} />
        Open local...
      </button>
      <button class="tab-menu-item" onclick={() => { showMenu = false; showCloneModal = true; }}>
        <GitFork size={14} />
        Clone from GitHub...
      </button>
      <button class="tab-menu-item" onclick={() => { showMenu = false; showCreateRepoModal = true; }}>
        <Plus size={14} />
        New GitHub repo...
      </button>
    </div>
  </div>
{/if}

<CloneFromGitHub
  open_={showCloneModal}
  onclose={() => (showCloneModal = false)}
  oncloned={handleCloned}
/>
<CreateRepoOnGitHub
  open_={showCreateRepoModal}
  onclose={() => (showCreateRepoModal = false)}
  oncreated={handleRepoCreated}
/>

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

  .tab-menu-backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
  }

  .tab-menu {
    position: fixed;
    top: var(--tab-height);
    z-index: 101;
    min-width: 200px;
    padding: 4px;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
  }

  .tab-menu-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 8px 10px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
    text-align: left;
    transition: background 0.1s;
  }

  .tab-menu-item:hover {
    background: var(--color-surface-elevated);
  }

  .tab-menu-item-open {
    opacity: 0.5;
  }

  .tab-menu-section {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--color-text-muted);
    padding: 6px 10px 4px;
  }

  .tab-menu-suggestions {
    max-height: 240px;
    overflow-y: auto;
  }

  .tab-menu-repo-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tab-menu-repo-branch {
    font-size: 10px;
    font-family: var(--font-mono);
    color: var(--color-accent);
    flex-shrink: 0;
  }

  .tab-menu-divider {
    height: 1px;
    background: var(--color-border);
    margin: 4px 0;
  }

  .tab-spacer {
    flex: 1;
    -webkit-app-region: drag;
  }

  .settings-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--tab-height);
    height: 100%;
    border: none;
    border-left: 1px solid var(--color-border);
    background: var(--color-surface);
    color: var(--color-text-muted);
    cursor: pointer;
    flex-shrink: 0;
    -webkit-app-region: no-drag;
    transition: background 0.1s, color 0.1s;
  }

  .settings-btn:hover {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .settings-btn.active {
    background: var(--color-surface-elevated);
    color: var(--color-accent);
    border-bottom: 2px solid var(--color-accent);
  }
</style>
