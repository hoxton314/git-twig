<script lang="ts">
  import { Search, Lock, Star, GitFork, Loader2, FolderOpen } from "lucide-svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import Modal from "../shared/Modal.svelte";
  import { settings } from "../../lib/stores/settings";
  import * as tauri from "../../lib/tauri";
  import type { GitHubRepo } from "../../lib/types/github";
  import type { RepoInfo } from "../../lib/types/git";

  interface Props {
    open_: boolean;
    onclose: () => void;
    oncloned: (info: RepoInfo) => void;
  }

  let { open_: isOpen, onclose, oncloned }: Props = $props();

  let repos = $state<GitHubRepo[]>([]);
  let loading = $state(false);
  let loadingMore = $state(false);
  let cloning = $state(false);
  let error = $state("");
  let searchQuery = $state("");
  let page = $state(1);
  let hasNextPage = $state(false);
  let selectedRepo = $state<GitHubRepo | null>(null);
  let destination = $state("");

  const s = $derived($settings);

  const filteredRepos = $derived(
    searchQuery.trim()
      ? repos.filter((r) =>
          r.full_name.toLowerCase().includes(searchQuery.toLowerCase()) ||
          (r.description?.toLowerCase().includes(searchQuery.toLowerCase()) ?? false)
        )
      : repos,
  );

  $effect(() => {
    if (isOpen && repos.length === 0) {
      loadRepos();
    }
  });

  async function loadRepos() {
    loading = true;
    error = "";
    try {
      const result = await tauri.githubListRepos(1, 30, "updated");
      repos = result.repos;
      hasNextPage = result.has_next_page;
      page = 1;
    } catch (err) {
      error = String(err);
    } finally {
      loading = false;
    }
  }

  async function loadMore() {
    loadingMore = true;
    try {
      const nextPage = page + 1;
      const result = await tauri.githubListRepos(nextPage, 30, "updated");
      repos = [...repos, ...result.repos];
      hasNextPage = result.has_next_page;
      page = nextPage;
    } catch (err) {
      error = String(err);
    } finally {
      loadingMore = false;
    }
  }

  function selectRepo(repo: GitHubRepo) {
    selectedRepo = repo;
    const baseDir = s.default_repo_dir ?? "";
    destination = baseDir ? `${baseDir}/${repo.name}` : repo.name;
  }

  async function pickDestination() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Clone destination",
    });
    if (selected) {
      destination = selectedRepo
        ? `${selected}/${selectedRepo.name}`
        : (selected as string);
    }
  }

  async function handleClone() {
    if (!selectedRepo || !destination.trim()) return;
    cloning = true;
    error = "";
    try {
      const info = await tauri.githubCloneRepo(
        selectedRepo.clone_url,
        destination.trim(),
      );
      oncloned(info);
      handleClose();
    } catch (err) {
      error = String(err);
    } finally {
      cloning = false;
    }
  }

  function handleClose() {
    selectedRepo = null;
    searchQuery = "";
    error = "";
    onclose();
  }

  function formatDate(iso: string): string {
    const d = new Date(iso);
    const now = new Date();
    const diffMs = now.getTime() - d.getTime();
    const days = Math.floor(diffMs / (1000 * 60 * 60 * 24));
    if (days === 0) return "today";
    if (days === 1) return "yesterday";
    if (days < 30) return `${days}d ago`;
    if (days < 365) return `${Math.floor(days / 30)}mo ago`;
    return `${Math.floor(days / 365)}y ago`;
  }
</script>

<Modal open={isOpen} title="Clone from GitHub" onclose={handleClose} width="560px">
  {#if error}
    <div class="error-banner">{error}</div>
  {/if}

  {#if selectedRepo}
    <!-- Clone form -->
    <div class="clone-form">
      <div class="selected-repo">
        <span class="repo-full-name">{selectedRepo.full_name}</span>
        <button class="btn-link" onclick={() => (selectedRepo = null)}>Change</button>
      </div>

      <label class="field-label">
        Destination
        <div class="dest-row">
          <input
            type="text"
            class="dest-input"
            bind:value={destination}
            placeholder="/path/to/clone"
          />
          <button class="btn-secondary" onclick={pickDestination}>
            <FolderOpen size={14} />
          </button>
        </div>
      </label>

      <button
        class="btn-primary"
        onclick={handleClone}
        disabled={cloning || !destination.trim()}
      >
        {#if cloning}
          <Loader2 size={14} class="spinner" />
          <span>Cloning...</span>
        {:else}
          <span>Clone</span>
        {/if}
      </button>
    </div>
  {:else}
    <!-- Repo list -->
    <div class="search-bar">
      <Search size={14} />
      <input
        type="text"
        class="search-input"
        placeholder="Filter repositories..."
        bind:value={searchQuery}
      />
    </div>

    {#if loading}
      <div class="loading-state">
        <Loader2 size={20} class="spinner" />
        <span>Loading repositories...</span>
      </div>
    {:else}
      <div class="repo-list">
        {#each filteredRepos as repo (repo.id)}
          <button class="repo-item" onclick={() => selectRepo(repo)}>
            <div class="repo-top">
              <span class="repo-name">{repo.full_name}</span>
              <div class="repo-badges">
                {#if repo.private}
                  <span class="badge private"><Lock size={10} /> Private</span>
                {/if}
                {#if repo.fork}
                  <span class="badge fork"><GitFork size={10} /> Fork</span>
                {/if}
              </div>
            </div>
            {#if repo.description}
              <span class="repo-desc">{repo.description}</span>
            {/if}
            <div class="repo-meta">
              {#if repo.stargazers_count > 0}
                <span class="meta-item"><Star size={11} /> {repo.stargazers_count}</span>
              {/if}
              <span class="meta-item">Updated {formatDate(repo.updated_at)}</span>
            </div>
          </button>
        {/each}

        {#if filteredRepos.length === 0 && !loading}
          <div class="empty-state">No repositories found</div>
        {/if}
      </div>

      {#if hasNextPage && !searchQuery}
        <button
          class="load-more-btn"
          onclick={loadMore}
          disabled={loadingMore}
        >
          {#if loadingMore}
            <Loader2 size={14} class="spinner" />
            Loading...
          {:else}
            Load more
          {/if}
        </button>
      {/if}
    {/if}
  {/if}
</Modal>

<style>
  .error-banner {
    padding: 8px 12px;
    margin-bottom: 12px;
    border-radius: 4px;
    background: rgba(247, 118, 142, 0.1);
    color: #f7768e;
    font-size: 12px;
    word-break: break-word;
  }

  .search-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-bg);
    margin-bottom: 12px;
    color: var(--color-text-muted);
  }

  .search-input {
    flex: 1;
    border: none;
    background: transparent;
    color: var(--color-text-primary);
    font-size: 13px;
    outline: none;
  }

  .search-input::placeholder {
    color: var(--color-text-muted);
  }

  .loading-state {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 32px;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .repo-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 360px;
    overflow-y: auto;
  }

  .repo-item {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px 12px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: transparent;
    color: var(--color-text-primary);
    cursor: pointer;
    text-align: left;
    transition: background 0.1s, border-color 0.1s;
    width: 100%;
  }

  .repo-item:hover {
    background: var(--color-surface-elevated);
    border-color: var(--color-border);
  }

  .repo-top {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .repo-name {
    font-size: 13px;
    font-weight: 500;
    color: var(--color-accent);
  }

  .repo-badges {
    display: flex;
    gap: 4px;
  }

  .badge {
    display: flex;
    align-items: center;
    gap: 3px;
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 3px;
    background: var(--color-surface-elevated);
    color: var(--color-text-muted);
  }

  .repo-desc {
    font-size: 12px;
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .repo-meta {
    display: flex;
    gap: 12px;
  }

  .meta-item {
    display: flex;
    align-items: center;
    gap: 3px;
    font-size: 11px;
    color: var(--color-text-muted);
  }

  .empty-state {
    padding: 24px;
    text-align: center;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .load-more-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: 100%;
    padding: 8px;
    margin-top: 8px;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 12px;
    cursor: pointer;
    transition: background 0.1s;
  }

  .load-more-btn:hover {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .load-more-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  /* Clone form */
  .clone-form {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .selected-repo {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    border-radius: 6px;
    background: var(--color-surface-elevated);
  }

  .repo-full-name {
    font-size: 13px;
    font-weight: 500;
    color: var(--color-accent);
    font-family: var(--font-mono);
  }

  .btn-link {
    border: none;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 12px;
    cursor: pointer;
    text-decoration: underline;
  }

  .btn-link:hover {
    color: var(--color-text-primary);
  }

  .field-label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    font-weight: 500;
    color: var(--color-text-primary);
  }

  .dest-row {
    display: flex;
    gap: 6px;
  }

  .dest-input {
    flex: 1;
    padding: 8px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 12px;
    font-family: var(--font-mono);
  }

  .dest-input:focus {
    outline: none;
    border-color: var(--color-accent);
  }

  .btn-secondary {
    display: flex;
    align-items: center;
    padding: 8px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    cursor: pointer;
    transition: border-color 0.1s;
  }

  .btn-secondary:hover {
    border-color: var(--color-accent);
  }

  .btn-primary {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 10px;
    border: none;
    border-radius: 6px;
    background: var(--color-accent);
    color: var(--color-bg);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: opacity 0.15s;
  }

  .btn-primary:hover {
    opacity: 0.9;
  }

  .btn-primary:disabled {
    opacity: 0.4;
    cursor: default;
  }

  :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
