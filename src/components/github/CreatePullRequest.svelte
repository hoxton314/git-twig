<script lang="ts">
  import { Loader2, ExternalLink, AlertCircle } from "lucide-svelte";
  import { open as openUrl } from "@tauri-apps/plugin-shell";
  import Modal from "../shared/Modal.svelte";
  import * as tauri from "../../lib/tauri";
  import type { GitHubRemoteInfo } from "../../lib/types/github";
  import type { GitHubPullRequest } from "../../lib/types/github";
  import type { BranchInfo } from "../../lib/types/git";

  interface Props {
    open_: boolean;
    onclose: () => void;
    repoPath: string;
  }

  let { open_: isOpen, onclose, repoPath }: Props = $props();

  let remote = $state<GitHubRemoteInfo | null>(null);
  let detecting = $state(false);
  let noRemote = $state(false);

  let localBranches = $state<string[]>([]);
  let remoteBranches = $state<string[]>([]);
  let currentBranch = $state("");

  let head = $state("");
  let base = $state("");
  let title = $state("");
  let body = $state("");

  let creating = $state(false);
  let error = $state("");
  let createdPr = $state<GitHubPullRequest | null>(null);

  $effect(() => {
    if (isOpen && repoPath) {
      detectRemote();
    }
  });

  async function detectRemote() {
    detecting = true;
    noRemote = false;
    error = "";
    createdPr = null;

    try {
      remote = await tauri.githubDetectRemote(repoPath);
      if (!remote) {
        noRemote = true;
        detecting = false;
        return;
      }

      // Load branches in parallel
      const [branches, info, remoteBranchList] = await Promise.all([
        tauri.getBranches(repoPath),
        tauri.getRepoInfo(repoPath),
        tauri.githubListBranches(remote.owner, remote.repo),
      ]);

      localBranches = branches
        .filter((b: BranchInfo) => !b.is_remote)
        .map((b: BranchInfo) => b.name);
      remoteBranches = remoteBranchList;
      currentBranch = info.head_name ?? "";

      head = currentBranch;
      base = remoteBranches.includes("main")
        ? "main"
        : remoteBranches.includes("master")
          ? "master"
          : remoteBranches[0] ?? "";

      // Auto-fill title from branch name
      title = branchToTitle(currentBranch);
    } catch (err) {
      error = String(err);
    } finally {
      detecting = false;
    }
  }

  function branchToTitle(branch: string): string {
    return branch
      .replace(/^(feature|fix|bugfix|hotfix|chore|docs)\//i, "")
      .replace(/[-_]/g, " ")
      .replace(/^\w/, (c) => c.toUpperCase());
  }

  async function handleCreate() {
    if (!remote || !title.trim() || !head || !base) return;
    creating = true;
    error = "";
    try {
      createdPr = await tauri.githubCreatePullRequest(
        remote.owner,
        remote.repo,
        title.trim(),
        body.trim(),
        head,
        base,
      );
    } catch (err) {
      error = String(err);
    } finally {
      creating = false;
    }
  }

  function handleOpenInBrowser() {
    if (createdPr) {
      openUrl(createdPr.html_url);
    }
  }

  function handleClose() {
    title = "";
    body = "";
    error = "";
    createdPr = null;
    remote = null;
    onclose();
  }
</script>

<Modal open={isOpen} title="Create Pull Request" onclose={handleClose} width="500px">
  {#if error}
    <div class="error-banner">{error}</div>
  {/if}

  {#if detecting}
    <div class="loading-state">
      <Loader2 size={20} class="spinner" />
      <span>Detecting GitHub remote...</span>
    </div>
  {:else if noRemote}
    <div class="no-remote">
      <AlertCircle size={20} />
      <span>No GitHub remote detected for this repository.</span>
    </div>
  {:else if createdPr}
    <!-- Success -->
    <div class="success-state">
      <div class="success-msg">
        Pull Request <strong>#{createdPr.number}</strong> created: {createdPr.title}
      </div>
      <div class="success-actions">
        <button class="btn-primary" onclick={handleOpenInBrowser}>
          <ExternalLink size={14} />
          <span>Open on GitHub</span>
        </button>
        <button class="btn-ghost" onclick={handleClose}>Close</button>
      </div>
    </div>
  {:else if remote}
    <!-- PR form -->
    <div class="pr-form">
      <div class="branch-row">
        <label class="branch-field">
          <span class="branch-label">Base</span>
          <select class="branch-select" bind:value={base}>
            {#each remoteBranches as b (b)}
              <option value={b}>{b}</option>
            {/each}
          </select>
        </label>
        <span class="arrow">←</span>
        <label class="branch-field">
          <span class="branch-label">Head</span>
          <select class="branch-select" bind:value={head}>
            {#each localBranches as b (b)}
              <option value={b}>{b}</option>
            {/each}
          </select>
        </label>
      </div>

      <label class="field-label">
        Title
        <input
          type="text"
          class="field-input"
          bind:value={title}
          placeholder="Pull request title"
        />
      </label>

      <label class="field-label">
        Description
        <textarea
          class="field-textarea"
          bind:value={body}
          placeholder="Optional description..."
          rows="4"
        ></textarea>
      </label>

      <div class="remote-info">
        {remote.owner}/{remote.repo} via <code>{remote.remote_name}</code>
      </div>

      <button
        class="btn-primary"
        onclick={handleCreate}
        disabled={creating || !title.trim() || !head || !base}
      >
        {#if creating}
          <Loader2 size={14} class="spinner" />
          <span>Creating...</span>
        {:else}
          <span>Create Pull Request</span>
        {/if}
      </button>
    </div>
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

  .loading-state {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 32px;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .no-remote {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 20px;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .pr-form {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .branch-row {
    display: flex;
    align-items: flex-end;
    gap: 10px;
  }

  .branch-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1;
  }

  .branch-label {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.3px;
    color: var(--color-text-muted);
  }

  .branch-select {
    padding: 8px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 12px;
    font-family: var(--font-mono);
  }

  .branch-select:focus {
    outline: none;
    border-color: var(--color-accent);
  }

  .arrow {
    color: var(--color-text-muted);
    font-size: 16px;
    padding-bottom: 8px;
  }

  .field-label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    font-weight: 500;
    color: var(--color-text-primary);
  }

  .field-input {
    padding: 8px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 13px;
  }

  .field-input:focus {
    outline: none;
    border-color: var(--color-accent);
  }

  .field-input::placeholder {
    color: var(--color-text-muted);
  }

  .field-textarea {
    padding: 8px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 13px;
    font-family: var(--font-sans);
    resize: vertical;
    min-height: 60px;
  }

  .field-textarea:focus {
    outline: none;
    border-color: var(--color-accent);
  }

  .field-textarea::placeholder {
    color: var(--color-text-muted);
  }

  .remote-info {
    font-size: 11px;
    color: var(--color-text-muted);
  }

  .remote-info code {
    font-family: var(--font-mono);
    background: var(--color-surface-elevated);
    padding: 1px 4px;
    border-radius: 3px;
    font-size: 10px;
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

  .success-state {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .success-msg {
    font-size: 13px;
    color: var(--color-text-primary);
    padding: 12px;
    border-radius: 6px;
    background: rgba(158, 206, 106, 0.1);
    border: 1px solid rgba(158, 206, 106, 0.2);
  }

  .success-msg strong {
    font-family: var(--font-mono);
    color: var(--color-accent);
  }

  .success-actions {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .btn-ghost {
    padding: 8px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 12px;
    cursor: pointer;
    text-align: center;
  }

  .btn-ghost:hover {
    color: var(--color-text-primary);
  }

  :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
