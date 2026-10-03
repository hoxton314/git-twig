<script lang="ts">
  import { FolderOpen, GitBranch, GitFork, Plus, FolderGit2, Pin, PinOff, X, History, AlertTriangle, FolderPlus } from "lucide-svelte";
  import { openRepos, addRepo, activeRepoPath } from "../../lib/stores/repos";
  import { currentView } from "../../lib/stores/ui";
  import { settings } from "../../lib/stores/settings";
  import { shortcutLabels, withShortcut } from "../../lib/keybindings";
  import {
    repoHistory,
    favoriteRepos,
    favoritePaths,
    missingRepoPaths,
    refreshMissingPaths,
    removeRecentRepo,
    clearRecentRepos,
    setFavoriteRepo,
  } from "../../lib/stores/repoHistory";
  import { toastError } from "../../lib/stores/toasts";
  import { t, tr } from "../../lib/i18n";
  import { openRepoWithDialog } from "../../lib/appActions";
  import { openNewRepoDialog } from "../../lib/newRepo";
  import CloneFromGitHub from "../github/CloneFromGitHub.svelte";
  import GroupsSection from "./GroupsSection.svelte";
  import ContextMenu, { type MenuItem } from "../shared/ContextMenu.svelte";
  import { groupMenuItems } from "../../lib/groupMenu";
  import CreateRepoOnGitHub from "../github/CreateRepoOnGitHub.svelte";
  import * as tauri from "../../lib/tauri";
  import type { RepoInfo } from "../../lib/types/git";
  import { onMount } from "svelte";

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

  /** Recently opened repos (favorites are listed separately). */
  const recentRepos = $derived(
    $repoHistory.recent.filter((r) => r.last_opened > 0 && !$favoritePaths.has(r.path)).slice(0, 12),
  );

  // Derived so unrelated settings changes (theme, accent...) don't rescan the dir.
  const repoDir = $derived($settings.default_repo_dir);

  $effect(() => {
    const dir = repoDir;
    if (!dir) {
      discoveredRepos = [];
      return;
    }
    // Ignore a slow scan of a previous directory that resolves after a newer one.
    let cancelled = false;
    tauri
      .listReposInDir(dir)
      .then((r) => { if (!cancelled) discoveredRepos = r; })
      .catch(() => { if (!cancelled) discoveredRepos = []; });
    return () => { cancelled = true; };
  });

  // Folders may have been moved/deleted since startup.
  onMount(() => {
    refreshMissingPaths();
  });

  async function openDiscoveredRepo(path: string) {
    try {
      const info = await tauri.openRepo(path);
      addRepo(info);
    } catch (err) {
      toastError(tr("tabs.openFailed"), err);
      refreshMissingPaths();
    }
  }

  function openOrSwitch(path: string) {
    if (openPaths.has(path)) switchToRepo(path);
    else openDiscoveredRepo(path);
  }

  function switchToRepo(path: string) {
    $activeRepoPath = path;
  }

  let groupMenu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  function openGroupMenu(e: MouseEvent, path: string, name: string) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    groupMenu = { x: r.left, y: r.bottom, items: groupMenuItems(path, name) };
  }

  function relativeTime(ms: number): string {
    const s = Math.max(0, (Date.now() - ms) / 1000);
    if (s < 60) return $t("time.justNow");
    if (s < 3600) return $t("time.minutesAgo", { count: Math.round(s / 60) });
    if (s < 86400) return $t("time.hoursAgo", { count: Math.round(s / 3600) });
    const d = Math.round(s / 86400);
    return d < 30 ? $t("time.daysAgo", { count: d }) : new Date(ms).toLocaleDateString();
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
    <p class="tagline">{$t("home.tagline")}</p>
    <button class="open-button" onclick={openRepoWithDialog} title={withShortcut($t("home.openRepo"), $shortcutLabels["open_repo"])}>
      <FolderOpen size={16} />
      {$t("home.openRepo")}
    </button>
    <div class="github-buttons">
      <button class="github-button" onclick={() => openNewRepoDialog("clone")}>
        <GitBranch size={15} />
        {$t("home.cloneUrl")}
      </button>
      <button class="github-button" onclick={() => openNewRepoDialog("init")}>
        <FolderGit2 size={15} />
        {$t("home.newRepo")}
      </button>
    </div>
    <div class="github-buttons">
      <button class="github-button" onclick={() => (showCloneModal = true)}>
        <GitFork size={15} />
        {$t("home.cloneGitHub")}
      </button>
      <button class="github-button" onclick={() => (showCreateRepoModal = true)}>
        <Plus size={15} />
        {$t("home.newGitHubRepo")}
      </button>
    </div>
    {#if $shortcutLabels["command_palette"]}
      <p class="palette-hint">{$t("home.paletteHintBefore")}<kbd>{$shortcutLabels["command_palette"]}</kbd>{$t("home.paletteHintAfter")}</p>
    {/if}
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

  <GroupsSection onopen={openOrSwitch} />

  {#if $favoriteRepos.length > 0}
    <section class="open-repos" aria-labelledby="favorites-heading">
      <h2 class="section-title" id="favorites-heading"><Pin size={13} /> {$t("home.favorites")}</h2>
      <div class="repo-list">
        {#each $favoriteRepos as repo (repo.path)}
          {@const missing = $missingRepoPaths.has(repo.path)}
          <div class="repo-row" class:missing>
            <button
              class="repo-card"
              class:repo-card-open={openPaths.has(repo.path)}
              disabled={missing}
              onclick={() => openOrSwitch(repo.path)}
              title={missing ? `${repo.path} — folder not found` : repo.path}
            >
              {#if missing}
                <AlertTriangle size={16} class="repo-icon missing-icon" aria-label={$t("home.missing")} />
              {:else}
                <GitBranch size={16} class="repo-icon" />
              {/if}
              <div class="repo-info">
                <span class="repo-name">{repo.name}</span>
                <span class="repo-path">{repo.path}</span>
              </div>
              {#if missing}<span class="repo-badge missing-badge">{$t("home.missing")}</span>{/if}
            </button>
            <button
              class="row-action"
              onclick={(e) => openGroupMenu(e, repo.path, repo.name)}
              title={$t("home.addToGroup")}
              aria-label={$t("home.addToGroupNamed", { name: repo.name })}
            >
              <FolderPlus size={14} />
            </button>
            <button
              class="row-action"
              onclick={() => setFavoriteRepo(repo.path, false)}
              title={$t("tabs.unpin")}
              aria-label={$t("home.unpinNamed", { name: repo.name })}
            >
              <PinOff size={14} />
            </button>
          </div>
        {/each}
      </div>
    </section>
  {/if}

  {#if recentRepos.length > 0}
    <section class="open-repos" aria-labelledby="recent-heading">
      <div class="section-header">
        <h2 class="section-title" id="recent-heading"><History size={13} /> {$t("home.recent")}</h2>
        <button class="link-btn" onclick={clearRecentRepos}>{$t("home.clear")}</button>
      </div>
      <div class="repo-list">
        {#each recentRepos as repo (repo.path)}
          {@const missing = $missingRepoPaths.has(repo.path)}
          <div class="repo-row" class:missing>
            <button
              class="repo-card"
              class:repo-card-open={openPaths.has(repo.path)}
              disabled={missing}
              onclick={() => openOrSwitch(repo.path)}
              title={missing ? `${repo.path} — folder not found` : repo.path}
            >
              {#if missing}
                <AlertTriangle size={16} class="repo-icon missing-icon" aria-label={$t("home.missing")} />
              {:else}
                <GitBranch size={16} class="repo-icon" />
              {/if}
              <div class="repo-info">
                <span class="repo-name">{repo.name}</span>
                <span class="repo-path">{repo.path}</span>
              </div>
              {#if missing}
                <span class="repo-badge missing-badge">{$t("home.missing")}</span>
              {:else}
                <span class="repo-time">{relativeTime(repo.last_opened)}</span>
              {/if}
            </button>
            {#if !missing}
              <button
                class="row-action"
                onclick={(e) => openGroupMenu(e, repo.path, repo.name)}
                title={$t("home.addToGroup")}
                aria-label={$t("home.addToGroupNamed", { name: repo.name })}
              >
                <FolderPlus size={14} />
              </button>
              <button
                class="row-action"
                onclick={() => setFavoriteRepo(repo.path, true, repo.name)}
                title={$t("tabs.pin")}
                aria-label={$t("home.pinNamed", { name: repo.name })}
              >
                <Pin size={14} />
              </button>
            {/if}
            <button
              class="row-action"
              onclick={() => removeRecentRepo(repo.path)}
              title={$t("home.removeFromList")}
              aria-label={$t("home.removeNamed", { name: repo.name })}
            >
              <X size={14} />
            </button>
          </div>
        {/each}
      </div>
    </section>
  {/if}

  {#if repos.length > 0 && !$settings.default_repo_dir}
    <div class="open-repos">
      <h2 class="section-title">{$t("home.openRepos")}</h2>
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
          <option value="recent">{$t("home.sortRecent")}</option>
          <option value="name">{$t("home.sortName")}</option>
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

{#if groupMenu}
  <ContextMenu x={groupMenu.x} y={groupMenu.y} items={groupMenu.items} onclose={() => (groupMenu = null)} />
{/if}

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

  .palette-hint {
    margin: 8px 0 0;
    font-size: 11px;
    color: var(--color-text-muted);
  }

  .palette-hint kbd {
    padding: 1px 5px;
    border: 1px solid var(--color-border);
    border-radius: 3px;
    background: var(--color-surface);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .repo-row {
    display: flex;
    align-items: stretch;
    gap: 4px;
  }

  .repo-row .repo-card {
    flex: 1;
    min-width: 0;
  }

  .repo-card:disabled {
    cursor: default;
  }

  .repo-card:disabled:hover {
    background: var(--color-surface);
    border-color: var(--color-border);
  }

  .repo-row.missing .repo-name,
  .repo-row.missing .repo-path {
    text-decoration: line-through;
    opacity: 0.7;
  }

  .repo-card :global(.missing-icon) {
    color: var(--color-diff-del-text);
  }

  .repo-badge {
    font-size: 10px;
    padding: 2px 6px;
    border-radius: 3px;
    flex-shrink: 0;
  }

  .missing-badge {
    color: var(--color-diff-del-text);
    background: var(--color-diff-del-bg);
  }

  .repo-time {
    font-size: 11px;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .row-action {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    flex-shrink: 0;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-surface);
    color: var(--color-text-muted);
    cursor: pointer;
    transition: color 0.1s, border-color 0.1s;
  }

  .row-action:hover {
    color: var(--color-text-primary);
    border-color: var(--color-text-muted);
  }

  .link-btn {
    padding: 0;
    border: none;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 11px;
    cursor: pointer;
  }

  .link-btn:hover {
    color: var(--color-text-primary);
    text-decoration: underline;
  }
</style>
