<script lang="ts">
  import { Loader2, ExternalLink, FolderOpen } from "lucide-svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { open as openUrl } from "@tauri-apps/plugin-shell";
  import Modal from "../shared/Modal.svelte";
  import { settings } from "../../lib/stores/settings";
  import * as tauri from "../../lib/tauri";
  import type { GitHubRepo } from "../../lib/types/github";
  import type { RepoInfo } from "../../lib/types/git";

  interface Props {
    open_: boolean;
    onclose: () => void;
    oncreated: (info: RepoInfo) => void;
  }

  let { open_: isOpen, onclose, oncreated }: Props = $props();

  let name = $state("");
  let description = $state("");
  let isPrivate = $state(true);
  let autoInit = $state(true);
  let creating = $state(false);
  let cloning = $state(false);
  let error = $state("");
  let createdRepo = $state<GitHubRepo | null>(null);

  const s = $derived($settings);

  async function handleCreate() {
    if (!name.trim()) return;
    creating = true;
    error = "";
    try {
      createdRepo = await tauri.githubCreateRepo(
        name.trim(),
        description.trim() || null,
        isPrivate,
        autoInit,
      );
    } catch (err) {
      error = String(err);
    } finally {
      creating = false;
    }
  }

  async function handleCloneLocally() {
    if (!createdRepo) return;
    cloning = true;
    error = "";
    try {
      const baseDir = s.default_repo_dir ?? "";
      let dest = baseDir
        ? `${baseDir}/${createdRepo.name}`
        : createdRepo.name;

      const selected = await openDialog({
        directory: true,
        multiple: false,
        title: "Clone destination",
      });
      if (selected) {
        dest = `${selected}/${createdRepo.name}`;
      } else {
        cloning = false;
        return;
      }

      const info = await tauri.githubCloneRepo(createdRepo.clone_url, dest);
      oncreated(info);
      handleClose();
    } catch (err) {
      error = String(err);
    } finally {
      cloning = false;
    }
  }

  function handleOpenInBrowser() {
    if (createdRepo) {
      openUrl(createdRepo.html_url);
    }
  }

  function handleClose() {
    name = "";
    description = "";
    isPrivate = true;
    autoInit = true;
    error = "";
    createdRepo = null;
    onclose();
  }
</script>

<Modal open={isOpen} title="New GitHub Repository" onclose={handleClose} width="460px">
  {#if error}
    <div class="error-banner">{error}</div>
  {/if}

  {#if createdRepo}
    <!-- Success state -->
    <div class="success-state">
      <div class="success-msg">
        Repository <strong>{createdRepo.full_name}</strong> created successfully.
      </div>

      <div class="success-actions">
        <button class="btn-primary" onclick={handleCloneLocally} disabled={cloning}>
          {#if cloning}
            <Loader2 size={14} class="spinner" />
            <span>Cloning...</span>
          {:else}
            <FolderOpen size={14} />
            <span>Clone locally</span>
          {/if}
        </button>
        <button class="btn-secondary" onclick={handleOpenInBrowser}>
          <ExternalLink size={14} />
          <span>Open on GitHub</span>
        </button>
        <button class="btn-ghost" onclick={handleClose}>Close</button>
      </div>
    </div>
  {:else}
    <!-- Create form -->
    <div class="create-form">
      <label class="field-label">
        Repository name
        <input
          type="text"
          class="field-input"
          placeholder="my-new-repo"
          bind:value={name}
          onkeydown={(e) => e.key === "Enter" && handleCreate()}
        />
      </label>

      <label class="field-label">
        Description
        <input
          type="text"
          class="field-input"
          placeholder="Optional description"
          bind:value={description}
        />
      </label>

      <div class="visibility-row">
        <span class="field-label-text">Visibility</span>
        <div class="visibility-toggle">
          <button
            class="vis-btn"
            class:active={isPrivate}
            onclick={() => (isPrivate = true)}
          >
            Private
          </button>
          <button
            class="vis-btn"
            class:active={!isPrivate}
            onclick={() => (isPrivate = false)}
          >
            Public
          </button>
        </div>
      </div>

      <label class="checkbox-row">
        <input type="checkbox" bind:checked={autoInit} />
        <span>Initialize with README</span>
      </label>

      <button
        class="btn-primary"
        onclick={handleCreate}
        disabled={creating || !name.trim()}
      >
        {#if creating}
          <Loader2 size={14} class="spinner" />
          <span>Creating...</span>
        {:else}
          <span>Create repository</span>
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

  .create-form {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .field-label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    font-weight: 500;
    color: var(--color-text-primary);
  }

  .field-label-text {
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

  .visibility-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .visibility-toggle {
    display: flex;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    overflow: hidden;
  }

  .vis-btn {
    padding: 6px 14px;
    border: none;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 12px;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .vis-btn:first-child {
    border-right: 1px solid var(--color-border);
  }

  .vis-btn.active {
    background: var(--color-accent);
    color: var(--color-bg);
  }

  .checkbox-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--color-text-primary);
    cursor: pointer;
  }

  .checkbox-row input[type="checkbox"] {
    accent-color: var(--color-accent);
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

  /* Success state */
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

  .btn-secondary {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 10px;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: transparent;
    color: var(--color-text-primary);
    font-size: 13px;
    cursor: pointer;
    transition: border-color 0.1s;
  }

  .btn-secondary:hover {
    border-color: var(--color-accent);
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
