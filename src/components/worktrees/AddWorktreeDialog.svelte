<script lang="ts">
  import Modal from "../shared/Modal.svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { activeRepoPath, activeRepo } from "../../lib/stores/repos";
  import { branches, refreshAll } from "../../lib/stores/graph";
  import { worktrees, refreshRepoTools, openAsTab, addWorktreeOpen } from "../../lib/stores/repotools";
  import { toast, toastError } from "../../lib/stores/toasts";
  import * as tauri from "../../lib/tauri";
  import { t, tr } from "../../lib/i18n";
  import { FolderOpen, Loader2 } from "lucide-svelte";

  const isOpen = $derived($addWorktreeOpen);
  const repoPath = $derived($activeRepoPath);

  let mode = $state<"new" | "existing">("new");
  let newBranch = $state("");
  let base = $state("");
  let existing = $state("");
  let dir = $state("");
  let dirTouched = $state(false);
  let openAfter = $state(true);
  let busy = $state(false);
  let error = $state<string | null>(null);

  const local = $derived($branches.filter((b) => !b.is_remote));
  /** Branches already checked out in some worktree can't be checked out again. */
  const checkedOut = $derived(new Set($worktrees.map((w) => w.branch).filter(Boolean) as string[]));
  const available = $derived(local.filter((b) => !checkedOut.has(b.name)));

  /** Directory containing the main worktree (new worktrees default next to it). */
  const mainDir = $derived($worktrees.find((w) => w.is_main)?.path ?? repoPath ?? "");
  const parentDir = $derived(mainDir.replace(/[\\/][^\\/]*$/, ""));
  const repoName = $derived(mainDir.split(/[\\/]/).pop() ?? $activeRepo?.name ?? "repo");
  const sep = $derived(mainDir.includes("\\") && !mainDir.includes("/") ? "\\" : "/");

  const branchForPath = $derived(mode === "new" ? newBranch.trim() : existing);
  const suggested = $derived(
    branchForPath ? `${parentDir}${sep}${repoName}-${branchForPath.replace(/[\\/]/g, "-")}` : "",
  );

  $effect(() => {
    if (isOpen) {
      mode = "new";
      newBranch = "";
      base = "";
      existing = "";
      dir = "";
      dirTouched = false;
      error = null;
      busy = false;
    }
  });

  // Follow the branch name until the user edits the path.
  $effect(() => {
    const s = suggested;
    if (!dirTouched) dir = s;
  });

  const valid = $derived(
    dir.trim().length > 0 && (mode === "new" ? newBranch.trim().length > 0 : existing.length > 0),
  );

  async function browse() {
    const picked = await openDialog({
      directory: true,
      multiple: false,
      title: $t("worktrees.chooseParent"),
      defaultPath: parentDir || undefined,
    });
    if (typeof picked !== "string") return;
    const leaf = branchForPath ? `${repoName}-${branchForPath.replace(/[\\/]/g, "-")}` : repoName + "-worktree";
    dir = `${picked.replace(/[\\/]$/, "")}${sep}${leaf}`;
    dirTouched = true;
  }

  async function submit(e?: Event) {
    e?.preventDefault();
    if (!repoPath || !valid || busy) return;
    busy = true;
    error = null;
    const path = dir.trim();
    try {
      const res =
        mode === "new"
          ? await tauri.worktreeAdd(repoPath, path, { newBranch: newBranch.trim(), commitish: base || undefined })
          : await tauri.worktreeAdd(repoPath, path, { commitish: existing });
      if (!res.success) {
        error = res.message.trim() || tr("worktrees.addFailedFallback");
        return;
      }
      $addWorktreeOpen = false;
      toast("success", tr("worktrees.created", { path }));
      await refreshRepoTools(repoPath);
      refreshAll(repoPath);
      if (openAfter) await openAsTab(path);
    } catch (err) {
      toastError(tr("worktrees.addFailed"), err);
    } finally {
      busy = false;
    }
  }
</script>

<Modal open={isOpen} title={$t("worktrees.addTitle")} onclose={() => ($addWorktreeOpen = false)} width="520px">
  <form class="form" onsubmit={submit}>
    <div class="seg" role="radiogroup" aria-label={$t("worktrees.branch")}>
      <label class:active={mode === "new"}>
        <input type="radio" bind:group={mode} value="new" /> {$t("worktrees.newBranch")}
      </label>
      <label class:active={mode === "existing"}>
        <input type="radio" bind:group={mode} value="existing" /> {$t("worktrees.existingBranch")}
      </label>
    </div>

    {#if mode === "new"}
      <label class="field">
        <span>{$t("worktrees.branchName")}</span>
        <input type="text" bind:value={newBranch} placeholder={$t("worktrees.branchPlaceholder")} spellcheck="false" />
      </label>
      <label class="field">
        <span>{$t("worktrees.startFrom")}</span>
        <select bind:value={base}>
          <option value="">HEAD ({$activeRepo?.head_name ?? $t("worktrees.current")})</option>
          {#each $branches as b (b.name)}
            <option value={b.name}>{b.name}</option>
          {/each}
        </select>
      </label>
    {:else}
      <label class="field">
        <span>{$t("worktrees.branch")}</span>
        <select bind:value={existing}>
          <option value="" disabled>{$t("worktrees.selectBranch")}</option>
          {#each available as b (b.name)}
            <option value={b.name}>{b.name}</option>
          {/each}
        </select>
        {#if available.length === 0}
          <small>{$t("worktrees.allCheckedOut")}</small>
        {/if}
      </label>
    {/if}

    <label class="field">
      <span>{$t("worktrees.location")}</span>
      <div class="path-row">
        <input
          type="text"
          bind:value={dir}
          oninput={() => (dirTouched = true)}
          placeholder={$t("worktrees.locationPlaceholder")}
          spellcheck="false"
        />
        <button type="button" class="btn" onclick={browse} title={$t("worktrees.chooseParentShort")}>
          <FolderOpen size={14} />
        </button>
      </div>
      <small>{$t("worktrees.locationHint")}</small>
    </label>

    <label class="check">
      <input type="checkbox" bind:checked={openAfter} /> {$t("worktrees.openInTab")}
    </label>

    {#if error}
      <pre class="error">{error}</pre>
    {/if}

    <div class="buttons">
      <button type="button" class="btn" onclick={() => ($addWorktreeOpen = false)}>{$t("common.cancel")}</button>
      <button type="submit" class="btn primary" disabled={!valid || busy}>
        {#if busy}<Loader2 size={13} class="spinner" />{/if}
        {$t("worktrees.create")}
      </button>
    </div>
  </form>
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .seg {
    display: flex;
    gap: 4px;
    padding: 3px;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-bg);
  }

  .seg label {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 6px;
    border-radius: 4px;
    font-size: 12px;
    color: var(--color-text-muted);
    cursor: pointer;
  }

  .seg label.active {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .seg input {
    display: none;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: 12px;
    color: var(--color-text-muted);
  }

  .field input,
  .field select {
    width: 100%;
    padding: 6px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 13px;
    box-sizing: border-box;
  }

  .field input:focus,
  .field select:focus {
    border-color: var(--color-accent);
    outline: none;
  }

  .field small {
    font-size: 11px;
  }

  .path-row {
    display: flex;
    gap: 6px;
  }

  .path-row input {
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--color-text-primary);
  }

  .error {
    margin: 0;
    padding: 8px 10px;
    border-radius: 4px;
    background: var(--color-diff-del-bg);
    color: var(--color-diff-del-text);
    font-size: 12px;
    white-space: pre-wrap;
  }

  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 14px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .btn.primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: var(--color-bg);
  }

  .form :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
