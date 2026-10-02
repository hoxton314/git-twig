<script lang="ts">
  import { Loader2, ExternalLink, AlertCircle, AlertTriangle, ArrowUpFromLine } from "lucide-svelte";
  import { open as openUrl } from "@tauri-apps/plugin-shell";
  import { untrack } from "svelte";
  import Modal from "../shared/Modal.svelte";
  import * as tauri from "../../lib/tauri";
  import type { GitHubPullRequest } from "../../lib/types/github";
  import type { HostedRemote } from "../../lib/types/hosting";
  import type { BranchInfo } from "../../lib/types/git";
  import { refreshAll } from "../../lib/stores/graph";
  import { toastError } from "../../lib/stores/toasts";

  interface Props {
    open_: boolean;
    onclose: () => void;
    repoPath: string;
  }

  let { open_: isOpen, onclose, repoPath }: Props = $props();

  /** Hosted remotes (GitHub / GitLab / Gitea), preferred target first. */
  let remotes = $state<HostedRemote[]>([]);
  let targetName = $state("");
  let headRemoteName = $state("");
  let detecting = $state(false);
  let noRemote = $state(false);

  let branchInfos = $state<BranchInfo[]>([]);
  let remoteBranches = $state<string[]>([]);
  let currentBranch = $state("");

  let head = $state("");
  let base = $state("");
  let title = $state("");
  let body = $state("");

  let creating = $state(false);
  let pushing = $state(false);
  let error = $state("");
  let createdPr = $state<GitHubPullRequest | null>(null);

  const target = $derived(remotes.find((r) => r.remote_name === targetName) ?? null);
  const headRemote = $derived(remotes.find((r) => r.remote_name === headRemoteName) ?? target);
  /** Remotes a PR head can come from: same provider + host as the target. */
  const headCandidates = $derived(
    target ? remotes.filter((r) => r.provider === target.provider && r.host === target.host) : [],
  );
  const isFork = $derived(
    !!target && !!headRemote && headRemote.project_path.toLowerCase() !== target.project_path.toLowerCase(),
  );
  /** Head as sent to the API: `branch`, or `owner:branch` for a fork. */
  const headSpec = $derived(isFork && headRemote ? `${headRemote.owner}:${head}` : head);
  const localBranches = $derived(branchInfos.filter((b) => !b.is_remote).map((b) => b.name));
  const prWord = $derived(target?.provider === "gitlab" ? "Merge Request" : "Pull Request");

  /** Whether `head` exists on the head remote and is up to date there. */
  const pushState = $derived.by((): { kind: "ok" } | { kind: "missing" } | { kind: "ahead"; count: number } => {
    if (!headRemote || !head) return { kind: "ok" };
    const tracking = `${headRemote.remote_name}/${head}`;
    if (!branchInfos.some((b) => b.is_remote && b.name === tracking)) return { kind: "missing" };
    const local = branchInfos.find((b) => !b.is_remote && b.name === head);
    if (local && local.upstream === tracking && local.ahead > 0) return { kind: "ahead", count: local.ahead };
    return { kind: "ok" };
  });

  // Re-detect only when the dialog opens or the repo changes. detectRemote()
  // synchronously reads `detecting`; without untrack, flipping it back to
  // false at the end would re-trigger this effect in an endless API loop.
  $effect(() => {
    if (isOpen && repoPath) {
      untrack(() => detectRemote());
    }
  });

  async function detectRemote() {
    if (detecting) return; // guard against overlapping detections
    detecting = true;
    noRemote = false;
    error = "";
    createdPr = null;

    try {
      const [list, branches, info] = await Promise.all([
        tauri.hostingListRemotes(repoPath),
        tauri.getBranches(repoPath),
        tauri.getRepoInfo(repoPath),
      ]);
      remotes = list;
      branchInfos = branches;
      if (!list.length) {
        noRemote = true;
        return;
      }
      targetName = list[0].remote_name;
      // A fork workflow usually pushes to `origin` and targets `upstream`.
      headRemoteName = list.find((r) => r.remote_name === "origin")?.remote_name ?? targetName;
      currentBranch = info.head_name ?? "";
      head = currentBranch;
      title = branchToTitle(currentBranch);
      await loadBaseBranches();
    } catch (err) {
      error = String(err);
    } finally {
      detecting = false;
    }
  }

  async function loadBaseBranches() {
    if (!targetName) return;
    try {
      remoteBranches = await tauri.hostingListBranches(repoPath, targetName);
      if (!remoteBranches.includes(base)) {
        base = remoteBranches.includes("main")
          ? "main"
          : remoteBranches.includes("master")
            ? "master"
            : remoteBranches[0] ?? "";
      }
    } catch (err) {
      error = String(err);
      remoteBranches = [];
    }
  }

  function onTargetChange(name: string) {
    targetName = name;
    const t = remotes.find((r) => r.remote_name === name);
    const h = remotes.find((r) => r.remote_name === headRemoteName);
    if (!t || !h || h.provider !== t.provider || h.host !== t.host) headRemoteName = name;
    loadBaseBranches();
  }

  function branchToTitle(branch: string): string {
    return branch
      .replace(/^(feature|fix|bugfix|hotfix|chore|docs)\//i, "")
      .replace(/[-_]/g, " ")
      .replace(/^\w/, (c) => c.toUpperCase());
  }

  async function pushHead() {
    if (!headRemote || !head || pushing) return;
    pushing = true;
    try {
      const res = await tauri.pushBranch(repoPath, head, headRemote.remote_name, true);
      if (!res.success) {
        error = res.message;
      } else {
        error = "";
        branchInfos = await tauri.getBranches(repoPath);
        refreshAll(repoPath);
      }
    } catch (err) {
      toastError("Push failed", err);
    } finally {
      pushing = false;
    }
  }

  async function handleCreate() {
    if (!target || !title.trim() || !head || !base || creating) return;
    creating = true;
    error = "";
    try {
      createdPr = await tauri.hostingCreatePr(
        repoPath,
        target.remote_name,
        title.trim(),
        body.trim(),
        headSpec,
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
    remotes = [];
    onclose();
  }
</script>

<Modal open={isOpen} title="Create {prWord}" onclose={handleClose} width="520px">
  {#if error}
    <div class="error-banner">{error}</div>
  {/if}

  {#if detecting}
    <div class="loading-state">
      <Loader2 size={20} class="spinner" />
      <span>Detecting remote...</span>
    </div>
  {:else if noRemote}
    <div class="no-remote">
      <AlertCircle size={20} />
      <span>No GitHub, GitLab or Gitea remote detected for this repository.</span>
    </div>
  {:else if createdPr}
    <!-- Success -->
    <div class="success-state">
      <div class="success-msg">
        {prWord} <strong>{target?.provider === "gitlab" ? "!" : "#"}{createdPr.number}</strong> created: {createdPr.title}
      </div>
      <div class="success-actions">
        <button class="btn-primary" onclick={handleOpenInBrowser}>
          <ExternalLink size={14} />
          <span>Open in browser</span>
        </button>
        <button class="btn-ghost" onclick={handleClose}>Close</button>
      </div>
    </div>
  {:else if target}
    <!-- PR form -->
    <div class="pr-form">
      {#if remotes.length > 1}
        <div class="branch-row">
          <label class="branch-field">
            <span class="branch-label">Target repository</span>
            <select class="branch-select" value={targetName} onchange={(e) => onTargetChange(e.currentTarget.value)}>
              {#each remotes as r (r.remote_name)}
                <option value={r.remote_name}>{r.project_path} ({r.remote_name})</option>
              {/each}
            </select>
          </label>
          {#if headCandidates.length > 1}
            <label class="branch-field">
              <span class="branch-label">Head repository</span>
              <select class="branch-select" bind:value={headRemoteName}>
                {#each headCandidates as r (r.remote_name)}
                  <option value={r.remote_name}>{r.project_path} ({r.remote_name})</option>
                {/each}
              </select>
            </label>
          {/if}
        </div>
      {/if}

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
          <span class="branch-label">Head{isFork && headRemote ? ` (${headRemote.owner}:)` : ""}</span>
          <select class="branch-select" bind:value={head}>
            {#each localBranches as b (b)}
              <option value={b}>{b}</option>
            {/each}
          </select>
        </label>
      </div>

      {#if pushState.kind !== "ok" && headRemote}
        <div class="push-warning">
          <AlertTriangle size={14} />
          <span>
            {#if pushState.kind === "missing"}
              <code>{head}</code> hasn't been pushed to <code>{headRemote.remote_name}</code> yet.
            {:else}
              {pushState.count} local commit{pushState.count === 1 ? "" : "s"} on <code>{head}</code> not pushed to <code>{headRemote.remote_name}</code>.
            {/if}
          </span>
          <button class="btn-push" onclick={pushHead} disabled={pushing}>
            {#if pushing}<Loader2 size={12} class="spinner" />{:else}<ArrowUpFromLine size={12} />{/if}
            Push
          </button>
        </div>
      {/if}

      <label class="field-label">
        Title
        <input
          type="text"
          class="field-input"
          bind:value={title}
          placeholder="{prWord} title"
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
        {target.project_path} via <code>{target.remote_name}</code>{#if isFork} · head <code>{headSpec}</code>{/if}
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
          <span>Create {prWord}</span>
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

  .push-warning {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-radius: 4px;
    border: 1px solid var(--color-lane-2);
    color: var(--color-text-primary);
    font-size: 12px;
  }

  .push-warning :global(svg:first-child) {
    color: var(--color-lane-2);
    flex-shrink: 0;
  }

  .push-warning span {
    flex: 1;
  }

  .push-warning code {
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .btn-push {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    font-size: 11px;
    cursor: pointer;
    flex-shrink: 0;
  }

  .btn-push:hover:not(:disabled) {
    border-color: var(--color-accent);
  }
</style>
