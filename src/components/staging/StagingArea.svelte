<script lang="ts" module>
  // Unsent commit messages per repo path. Module-level so drafts survive both
  // tab switches (this component is shared across repos) and remounts.
  const commitDrafts = new Map<string, string>();
</script>

<script lang="ts">
  import {
    Plus,
    Minus,
    ChevronDown,
    ChevronRight,
    ArrowUpFromLine,
    ArrowDownToLine,
    RefreshCw,
    Send,
    FileText,
    FilePlus,
    FileX,
    FilePen,
    Loader2,
    GitPullRequest,
    Trash2,
    Undo2,
    AlertTriangle,
  } from "lucide-svelte";
  import StashPanel from "./StashPanel.svelte";
  import CreatePullRequest from "../github/CreatePullRequest.svelte";
  import { activeRepoPath } from "../../lib/stores/repos";
  import {
    workingStatus,
    selectedWorkingFile,
    workingFileDiff,
    selectedCommitOid,
    commitGraph,
    branches,
    refreshStatus,
    refreshAll,
  } from "../../lib/stores/graph";
  import { onAction } from "../../lib/keybindings";
  import { settings } from "../../lib/stores/settings";
  import * as tauri from "../../lib/tauri";
  import type { FileStatus } from "../../lib/types/git";
  import { onMount, untrack } from "svelte";
  import { message, ask } from "@tauri-apps/plugin-dialog";

  const repoPath = $derived($activeRepoPath);
  const status = $derived($workingStatus);
  const selectedFile = $derived($selectedWorkingFile);
  const currentHead = $derived($branches.find((b) => b.is_head && !b.is_remote));
  const hasUnpushed = $derived(
    ($commitGraph?.unpushed_oids?.length ?? 0) > 0 ||
    (currentHead != null && (currentHead.upstream == null || currentHead.ahead > 0))
  );

  let unstagedExpanded = $state(true);
  let stagedExpanded = $state(true);
  let commitMessage = $state("");
  let loading = $state(false);
  let pushLoading = $state(false);
  let pullLoading = $state(false);
  let showPrModal = $state(false);

  // First line of the commit message, for the summary length hint.
  const summaryLength = $derived(commitMessage.split("\n", 1)[0].length);

  // Load working status when repo changes, and swap the commit message draft
  // so a half-written message doesn't follow the user into another repo.
  let lastLoadedPath: string | null = null;
  $effect(() => {
    const path = repoPath;
    if (path && path !== lastLoadedPath) {
      untrack(() => {
        if (lastLoadedPath) commitDrafts.set(lastLoadedPath, commitMessage);
        commitMessage = commitDrafts.get(path) ?? "";
      });
      lastLoadedPath = path;
      refreshStatus(path);
    }
  });

  onMount(() => () => {
    if (lastLoadedPath) commitDrafts.set(lastLoadedPath, commitMessage);
  });

  // Write operations are serialized: concurrent `git add`/`reset`/`checkout`
  // calls race on .git/index.lock and fail, and overlapping status refreshes
  // can resolve out of order and show stale lists.
  let writeQueue: Promise<unknown> = Promise.resolve();

  // Run a write command (then refresh status) and surface any failure to the
  // user instead of swallowing it. Returns true on success.
  function runWrite(
    op: () => Promise<{ success: boolean; message: string }>,
    title: string,
  ): Promise<boolean> {
    const task = writeQueue.then(async () => {
      let ok = false;
      try {
        const result = await op();
        if (!result.success) {
          await message(result.message, { title, kind: "error" });
        } else {
          ok = true;
        }
      } catch (err) {
        await message(String(err), { title, kind: "error" });
      }
      await refreshStatus();
      return ok;
    });
    writeQueue = task;
    return task;
  }

  function confirmDestructive(text: string, title: string): Promise<boolean> {
    if (!$settings.confirm_destructive_ops) return Promise.resolve(true);
    return ask(text, { title, kind: "warning" });
  }

  // Keyboard activation for div[role=button] rows/headers. Ignores keys that
  // originate from nested buttons so pressing Enter on "Stage" doesn't also
  // select the row or collapse the section.
  function activateOnKey(e: KeyboardEvent, fn: () => void) {
    if (e.target !== e.currentTarget) return;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      fn();
    }
  }

  let selectRequest = 0;
  async function selectFile(file: FileStatus, area: "staged" | "unstaged") {
    if (!repoPath) return;
    const path = repoPath;
    const req = ++selectRequest;
    // Clear commit selection so diff viewer shows working file diff
    $selectedCommitOid = null;
    $selectedWorkingFile = { path: file.path, area };
    try {
      const diff =
        area === "staged"
          ? await tauri.getStagedDiff(path, file.path)
          : await tauri.getUnstagedDiff(path, file.path);
      // Drop the result if the user clicked another file meanwhile.
      if (req !== selectRequest) return;
      $workingFileDiff = diff;
    } catch (err) {
      if (req !== selectRequest) return;
      console.error("Failed to load file diff:", err);
      $workingFileDiff = [];
    }
  }

  async function stageFile(e: MouseEvent, file: FileStatus) {
    e.stopPropagation();
    if (!repoPath) return;
    const path = repoPath;
    await runWrite(() => tauri.stageFiles(path, [file.path]), "Stage Failed");
  }

  async function unstageFile(e: MouseEvent, file: FileStatus) {
    e.stopPropagation();
    if (!repoPath) return;
    const path = repoPath;
    await runWrite(() => tauri.unstageFiles(path, [file.path]), "Unstage Failed");
  }

  async function stageAll() {
    if (!repoPath || status.unstaged.length === 0) return;
    const path = repoPath;
    const conflicted = status.unstaged.filter((f) => f.status === "conflicted");
    if (conflicted.length > 0) {
      const ok = await ask(
        `${conflicted.length} conflicted file${conflicted.length !== 1 ? "s" : ""} will be marked as resolved. Make sure all conflict markers have been removed. Continue?`,
        { title: "Stage Conflicted Files", kind: "warning" },
      );
      if (!ok) return;
    }
    const paths = status.unstaged.map((f) => f.path);
    await runWrite(() => tauri.stageFiles(path, paths), "Stage Failed");
  }

  async function unstageAll() {
    if (!repoPath || status.staged.length === 0) return;
    const path = repoPath;
    const paths = status.staged.map((f) => f.path);
    await runWrite(() => tauri.unstageFiles(path, paths), "Unstage Failed");
  }

  async function discardFile(e: MouseEvent, file: FileStatus) {
    e.stopPropagation();
    if (!repoPath) return;
    const path = repoPath;
    const ok = await confirmDestructive(
      file.is_new
        ? `Delete untracked file "${file.path}"? This cannot be undone.`
        : `Discard changes to "${file.path}"? This cannot be undone.`,
      "Discard Changes",
    );
    if (!ok) return;
    const tracked = file.is_new ? [] : [file.path];
    const untracked = file.is_new ? [file.path] : [];
    await runWrite(() => tauri.discardFiles(path, tracked, untracked), "Discard Failed");
  }

  async function discardAll() {
    if (!repoPath || status.unstaged.length === 0) return;
    const path = repoPath;
    const untrackedCount = status.unstaged.filter((f) => f.is_new).length;
    const ok = await confirmDestructive(
      `Discard all unstaged changes${untrackedCount > 0 ? ` and delete ${untrackedCount} untracked file${untrackedCount !== 1 ? "s" : ""}` : ""}? This cannot be undone.`,
      "Discard Changes",
    );
    if (!ok) return;
    const tracked = status.unstaged.filter((f) => !f.is_new).map((f) => f.path);
    const untracked = status.unstaged.filter((f) => f.is_new).map((f) => f.path);
    await runWrite(() => tauri.discardFiles(path, tracked, untracked), "Discard Failed");
  }

  async function handleUndoCommit() {
    if (!repoPath) return;
    const ok = await ask("Undo the last commit? Changes will be kept staged.", {
      title: "Undo Commit",
      kind: "warning",
    });
    if (!ok) return;
    try {
      const result = await tauri.undoCommit(repoPath);
      if (result.success) {
        await refreshAll();
      } else {
        await message(result.message, { title: "Undo Failed", kind: "error" });
      }
    } catch (err) {
      await message(String(err), { title: "Undo Failed", kind: "error" });
    }
  }

  async function handleCommit() {
    // `loading` guard: Ctrl+Enter bypasses the disabled button.
    if (loading || !repoPath || !commitMessage.trim() || status.staged.length === 0)
      return;
    const path = repoPath;
    loading = true;
    try {
      // Wait for any queued stage/unstage so the commit sees the final index.
      await writeQueue;
      const result = await tauri.createCommit(path, commitMessage.trim());
      if (result.success) {
        commitDrafts.delete(path);
        if (repoPath === path) commitMessage = "";
        await refreshAll();
      } else {
        await message(result.message, { title: "Commit Failed", kind: "error" });
      }
    } catch (err) {
      await message(String(err), { title: "Commit Failed", kind: "error" });
    } finally {
      loading = false;
    }
  }

  async function handlePush() {
    if (!repoPath) return;
    pushLoading = true;
    try {
      const info = await tauri.getRepoInfo(repoPath);
      const branch = info.head_name ?? "HEAD";
      const result = await tauri.pushBranch(repoPath, branch, undefined, true);
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
      const result = await tauri.pull(repoPath);
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

  function statusIcon(status: string) {
    switch (status) {
      case "added":
      case "untracked":
        return FilePlus;
      case "deleted":
        return FileX;
      case "modified":
        return FilePen;
      case "conflicted":
        return AlertTriangle;
      default:
        return FileText;
    }
  }

  function statusColor(status: string): string {
    switch (status) {
      case "added":
      case "untracked":
        return "var(--color-diff-add-text)";
      case "deleted":
        return "var(--color-diff-del-text)";
      case "modified":
      case "renamed":
        return "var(--color-lane-2)";
      case "conflicted":
        return "var(--color-diff-del-text)";
      default:
        return "var(--color-text-muted)";
    }
  }

  onMount(() => {
    return onAction("commit", handleCommit);
  });
</script>

<div class="staging-area">
  <!-- Push/Pull toolbar -->
  <div class="toolbar">
    <button
      class="toolbar-btn"
      onclick={handlePull}
      disabled={pullLoading}
      title="Pull"
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
      title="Push"
    >
      {#if pushLoading}
        <Loader2 size={14} class="spinner" />
      {:else}
        <ArrowUpFromLine size={14} />
      {/if}
      <span>Push</span>
    </button>
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

  <!-- Unstaged files -->
  <div class="file-section">
    <div
      class="section-header"
      onclick={() => (unstagedExpanded = !unstagedExpanded)}
      onkeydown={(e) => activateOnKey(e, () => (unstagedExpanded = !unstagedExpanded))}
      role="button"
      tabindex="0"
      aria-expanded={unstagedExpanded}
    >
      {#if unstagedExpanded}
        <ChevronDown size={14} />
      {:else}
        <ChevronRight size={14} />
      {/if}
      <span class="section-title">Unstaged</span>
      <span class="section-count">{status.unstaged.length}</span>
      {#if status.unstaged.length > 0}
        <button
          class="stage-all-btn"
          onclick={(e) => { e.stopPropagation(); discardAll(); }}
          title="Discard all changes"
        >
          <Trash2 size={12} />
        </button>
        <button
          class="stage-all-btn"
          onclick={(e) => { e.stopPropagation(); stageAll(); }}
          title="Stage all"
        >
          <Plus size={12} />
        </button>
      {/if}
    </div>

    {#if unstagedExpanded}
      <div class="file-list">
        {#each status.unstaged as file (file.path)}
          {@const Icon = statusIcon(file.status)}
          <div
            class="file-item"
            class:selected={selectedFile?.path === file.path && selectedFile?.area === "unstaged"}
            onclick={() => selectFile(file, "unstaged")}
            onkeydown={(e) => activateOnKey(e, () => selectFile(file, "unstaged"))}
            role="button"
            tabindex="0"
          >
            <Icon size={13} color={statusColor(file.status)} />
            <span class="file-name" title="{file.path} ({file.status})">{file.path}</span>
            <button
              class="action-btn discard-btn"
              onclick={(e) => discardFile(e, file)}
              title="Discard changes"
            >
              <Trash2 size={12} />
            </button>
            <button
              class="action-btn stage-btn"
              onclick={(e) => stageFile(e, file)}
              title="Stage file"
            >
              <Plus size={12} />
            </button>
          </div>
        {/each}
        {#if status.unstaged.length === 0}
          <div class="empty-msg">No unstaged changes</div>
        {/if}
      </div>
    {/if}
  </div>

  <!-- Staged files -->
  <div class="file-section">
    <div
      class="section-header"
      onclick={() => (stagedExpanded = !stagedExpanded)}
      onkeydown={(e) => activateOnKey(e, () => (stagedExpanded = !stagedExpanded))}
      role="button"
      tabindex="0"
      aria-expanded={stagedExpanded}
    >
      {#if stagedExpanded}
        <ChevronDown size={14} />
      {:else}
        <ChevronRight size={14} />
      {/if}
      <span class="section-title">Staged</span>
      <span class="section-count">{status.staged.length}</span>
      {#if status.staged.length > 0}
        <button
          class="stage-all-btn"
          onclick={(e) => { e.stopPropagation(); unstageAll(); }}
          title="Unstage all"
        >
          <Minus size={12} />
        </button>
      {/if}
    </div>

    {#if stagedExpanded}
      <div class="file-list">
        {#each status.staged as file (file.path)}
          {@const Icon = statusIcon(file.status)}
          <div
            class="file-item"
            class:selected={selectedFile?.path === file.path && selectedFile?.area === "staged"}
            onclick={() => selectFile(file, "staged")}
            onkeydown={(e) => activateOnKey(e, () => selectFile(file, "staged"))}
            role="button"
            tabindex="0"
          >
            <Icon size={13} color={statusColor(file.status)} />
            <span class="file-name" title="{file.path} ({file.status})">{file.path}</span>
            <button
              class="action-btn unstage-btn"
              onclick={(e) => unstageFile(e, file)}
              title="Unstage file"
            >
              <Minus size={12} />
            </button>
          </div>
        {/each}
        {#if status.staged.length === 0}
          <div class="empty-msg">No staged changes</div>
        {/if}
      </div>
    {/if}
  </div>

  <!-- Stash -->
  <StashPanel />

  <!-- Commit box -->
  <div class="commit-box">
    <textarea
      class="commit-input"
      placeholder="Commit message..."
      bind:value={commitMessage}
      rows="3"
      spellcheck="true"
      aria-label="Commit message"
    ></textarea>
    {#if summaryLength > 0}
      <div
        class="summary-hint"
        class:warn={summaryLength > 50}
        class:over={summaryLength > 72}
        title="Summary (first line) length — keep it under 50, at most 72 characters"
      >
        {summaryLength}
      </div>
    {/if}
    <div class="commit-actions">
      <button
        class="commit-btn"
        onclick={handleCommit}
        disabled={loading || !commitMessage.trim() || status.staged.length === 0}
      >
        {#if loading}
          <Loader2 size={14} class="spinner" />
        {:else}
          <Send size={14} />
        {/if}
        <span>Commit{status.staged.length > 0 ? ` (${status.staged.length})` : ""}</span>
      </button>
      {#if hasUnpushed}
        <button
          class="undo-btn"
          onclick={handleUndoCommit}
          title="Undo last commit (keep changes staged)"
        >
          <Undo2 size={14} />
        </button>
      {/if}
    </div>
  </div>
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

  /* File sections */
  .file-section {
    flex-shrink: 0;
  }

  .section-header {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 6px 8px;
    border: none;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-surface);
    color: var(--color-text-muted);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.3px;
    cursor: pointer;
    text-align: left;
  }

  .section-header:hover {
    background: var(--color-surface-elevated);
  }

  .section-count {
    margin-left: auto;
    opacity: 0.6;
    font-weight: 400;
  }

  .stage-all-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border: none;
    border-radius: 3px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 0;
    margin-left: 4px;
  }

  .stage-all-btn:hover {
    background: color-mix(in srgb, var(--color-accent) 20%, transparent);
    color: var(--color-accent);
  }

  /* File list */
  .file-list {
    max-height: 200px;
    overflow-y: auto;
  }

  .file-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 8px 3px 16px;
    cursor: pointer;
    transition: background 0.1s;
    color: var(--color-text-primary);
  }

  .file-item:hover {
    background: var(--color-surface);
  }

  .file-item.selected {
    background: var(--color-surface-elevated);
    border-left: 2px solid var(--color-accent);
    padding-left: 14px;
  }

  .file-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .action-btn {
    display: none;
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
    flex-shrink: 0;
  }

  .file-item:hover .action-btn,
  .file-item:focus-within .action-btn,
  .file-item.selected .action-btn {
    display: flex;
  }

  .file-item:focus-visible,
  .section-header:focus-visible {
    outline: 1px solid var(--color-accent);
    outline-offset: -1px;
  }

  .stage-btn:hover {
    background: var(--color-diff-add-bg);
    color: var(--color-diff-add-text);
  }

  .unstage-btn:hover,
  .discard-btn:hover {
    background: var(--color-diff-del-bg);
    color: var(--color-diff-del-text);
  }

  .summary-hint {
    align-self: flex-end;
    margin-top: -4px;
    font-size: 10px;
    font-family: var(--font-mono);
    color: var(--color-text-muted);
  }

  .summary-hint.warn {
    color: var(--color-lane-2);
  }

  .summary-hint.over {
    color: var(--color-diff-del-text);
  }

  .empty-msg {
    padding: 8px 16px;
    color: var(--color-text-muted);
    font-style: italic;
    font-size: 11px;
  }

  /* Commit box */
  .commit-box {
    margin-top: auto;
    padding: 8px;
    border-top: 1px solid var(--color-border);
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex-shrink: 0;
  }

  .commit-input {
    width: 100%;
    padding: 6px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 12px;
    font-family: var(--font-sans);
    resize: vertical;
    outline: none;
    box-sizing: border-box;
    min-height: 60px;
  }

  .commit-input:focus {
    border-color: var(--color-accent);
  }

  .commit-input::placeholder {
    color: var(--color-text-muted);
  }

  .commit-actions {
    display: flex;
    gap: 4px;
  }

  .commit-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 6px 12px;
    border: none;
    border-radius: 4px;
    background: var(--color-accent);
    color: var(--color-bg);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: opacity 0.15s;
    flex: 1;
  }

  .undo-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 0;
    flex-shrink: 0;
    transition: background 0.1s, color 0.1s;
  }

  .undo-btn:hover {
    background: var(--color-diff-del-bg);
    color: var(--color-diff-del-text);
    border-color: var(--color-diff-del-text);
  }

  .commit-btn:hover {
    opacity: 0.9;
  }

  .commit-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .commit-btn :global(.spinner) {
    animation: spin 1s linear infinite;
  }
</style>
