<script lang="ts">
  import { FolderOpen, GitBranch, GitFork, Plus } from "lucide-svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openRepos, addRepo, activeRepoPath } from "../../lib/stores/repos";
  import { currentView } from "../../lib/stores/ui";
  import CloneFromGitHub from "../github/CloneFromGitHub.svelte";
  import CreateRepoOnGitHub from "../github/CreateRepoOnGitHub.svelte";
  import * as tauri from "../../lib/tauri";
  import type { RepoInfo } from "../../lib/types/git";

  const repos = $derived([...$openRepos.entries()]);
  let showCloneModal = $state(false);
  let showCreateRepoModal = $state(false);

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

  {#if repos.length > 0}
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

  .section-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--color-text-muted);
    margin: 0 0 10px;
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
</style>
