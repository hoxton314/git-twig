<script lang="ts">
  import { FolderOpen, GitBranch, GitFork, Plus, FolderGit2 } from "lucide-svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openRepos, addRepo, activeRepoPath } from "../../lib/stores/repos";
  import { currentView } from "../../lib/stores/ui";
  import { settings } from "../../lib/stores/settings";
  import CloneFromGitHub from "../github/CloneFromGitHub.svelte";
  import CreateRepoOnGitHub from "../github/CreateRepoOnGitHub.svelte";
  import * as tauri from "../../lib/tauri";
  import type { RepoInfo } from "../../lib/types/git";

  type SortMode = "recent" | "name";

  const repos = $derived([...$openRepos.entries()]);
  const openPaths = $derived(new Set(repos.map(([p]) => p)));
  let showCloneModal = $state(false);
  let showCreateRepoModal = $state(false);
  let discoveredRepos = $state<RepoInfo[]>([]);
  let sortMode = $state<SortMode>("recent");

  const sortedDiscoveredRepos = $derived(
    [...discoveredRepos].sort((a, b) => {
      if (sortMode === "recent") return b.last_commit_time - a.last_commit_time;
      return a.name.localeCompare(b.name);
    }),
  );

  $effect(() => {
    const dir = $settings.default_repo_dir;
    if (dir) {
      tauri.listReposInDir(dir).then((r) => (discoveredRepos = r)).catch(() => (discoveredRepos = []));
    } else {
      discoveredRepos = [];
    }
  });

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

  async function openDiscoveredRepo(path: string) {
    try {
      const info = await tauri.openRepo(path);
      addRepo(info);
    } catch (err) {
      console.error("Failed to open repo:", err);
    }
  }

  function switchToRepo(path: string) {
    $activeRepoPath = path;
  }

  function handleCloned(info: RepoInfo) {
    addRepo(info);
    $currentView = "repos";
  }

  function handleRepoCreated(info: RepoInfo) {
    addRepo(info);
    $currentView = "repos";
  }
</script>

<div class="home-screen">
  <div class="hero">
    <h1 class="logo">Twig</h1>
    <p class="tagline">Lighter than the rest.</p>
    <button class="open-button" onclick={handleOpenRepo}>
      <FolderOpen size={16} />
      Open Repository
    </button>
    <div class="github-buttons">
      <button class="github-button" onclick={() => (showCloneModal = true)}>
        <GitFork size={15} />
        Clone from GitHub
      </button>
      <button class="github-button" onclick={() => (showCreateRepoModal = true)}>
        <Plus size={15} />
        New GitHub Repo
      </button>
    </div>
  </div>

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

  {#if repos.length > 0 && !$settings.default_repo_dir}
    <div class="open-repos">
      <h2 class="section-title">Open repositories</h2>
      <div class="repo-list">
        {#each repos as [path, info] (path)}
          <button class="repo-card" onclick={() => switchToRepo(path)}>
            <GitBranch size={16} class="repo-icon" />
            <div class="repo-info">
              <span class="repo-name">{info.name}</span>
              <span class="repo-path">{path}</span>
            </div>
            {#if info.head_name}
              <span class="repo-branch">{info.head_name}</span>
            {/if}
          </button>
        {/each}
      </div>
    </div>
  {/if}

  {#if discoveredRepos.length > 0}
    <div class="open-repos">
      <div class="section-header">
        <h2 class="section-title">
          <FolderGit2 size={13} />
          {$settings.default_repo_dir}
        </h2>
        <select class="sort-select" bind:value={sortMode}>
          <option value="recent">Recent</option>
          <option value="name">Name</option>
        </select>
      </div>
      <div class="repo-list">
        {#each sortedDiscoveredRepos as repo (repo.path)}
          {#if openPaths.has(repo.path)}
            <button class="repo-card repo-card-open" onclick={() => switchToRepo(repo.path)}>
              <GitBranch size={16} class="repo-icon" />
              <div class="repo-info">
                <span class="repo-name">{repo.name}</span>
                <span class="repo-path">{repo.path}</span>
              </div>
              {#if repo.head_name}
                <span class="repo-branch">{repo.head_name}</span>
              {/if}
            </button>
          {:else}
            <button class="repo-card" onclick={() => openDiscoveredRepo(repo.path)}>
              <GitBranch size={16} class="repo-icon" />
              <div class="repo-info">
                <span class="repo-name">{repo.name}</span>
                <span class="repo-path">{repo.path}</span>
              </div>
              {#if repo.head_name}
                <span class="repo-branch">{repo.head_name}</span>
              {/if}
            </button>
          {/if}
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .home-screen {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    overflow-y: auto;
    padding: 60px 24px 40px;
    gap: 48px;
  }

  .hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
  }

  .logo {
    font-size: 36px;
    font-weight: 700;
    color: var(--color-text-primary);
    margin: 0;
    letter-spacing: -0.5px;
  }

  .tagline {
    font-size: 14px;
    color: var(--color-text-muted);
    margin: 0;
  }

  .open-button {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 12px;
    padding: 10px 24px;
    border: 1px solid var(--color-accent);
    border-radius: 6px;
    background: transparent;
    color: var(--color-accent);
    font-size: 13px;
    cursor: pointer;
    transition: background 0.15s;
  }

  .open-button:hover {
    background: rgba(122, 162, 247, 0.15);
  }

  .github-buttons {
    display: flex;
    gap: 8px;
    margin-top: 4px;
  }

  .github-button {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 12px;
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s, color 0.15s;
  }

  .github-button:hover {
    background: var(--color-surface-elevated);
    border-color: var(--color-text-muted);
    color: var(--color-text-primary);
  }

  .open-repos {
    width: 100%;
    max-width: 480px;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: 0 0 10px;
  }

  .section-title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--color-text-muted);
    margin: 0;
  }

  .section-title :global(svg) {
    flex-shrink: 0;
  }

  .sort-select {
    padding: 2px 6px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-muted);
    font-size: 11px;
    cursor: pointer;
    outline: none;
    transition: border-color 0.15s, color 0.15s;
  }

  .sort-select:hover,
  .sort-select:focus {
    border-color: var(--color-text-muted);
    color: var(--color-text-primary);
  }

  .repo-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .repo-card {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 10px 12px;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    cursor: pointer;
    text-align: left;
    transition: background 0.1s, border-color 0.1s;
  }

  .repo-card:hover {
    background: var(--color-surface-elevated);
    border-color: var(--color-accent);
  }

  .repo-card :global(.repo-icon) {
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .repo-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .repo-name {
    font-size: 13px;
    font-weight: 500;
  }

  .repo-path {
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .repo-branch {
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--color-accent);
    flex-shrink: 0;
    padding: 2px 8px;
    border-radius: 3px;
    background: rgba(122, 162, 247, 0.1);
  }

  .repo-card-open {
    opacity: 0.5;
  }
</style>
