<script lang="ts">
  import { onMount } from "svelte";
  import {
    ChevronDown,
    ChevronRight,
    Archive,
    ArchiveRestore,
    Copy,
    Trash2,
    Loader2,
    Files,
    X,
  } from "lucide-svelte";
  import { activeRepoPath } from "../../lib/stores/repos";
  import {
    stashEntries,
    workingStatus,
    refreshStash,
    refreshAll,
  } from "../../lib/stores/graph";
  import { settings } from "../../lib/stores/settings";
  import { toast, toastError } from "../../lib/stores/toasts";
  import { onAction } from "../../lib/keybindings";
  import * as tauri from "../../lib/tauri";
  import type { StashDetail } from "../../lib/types/git";
  import { ask } from "@tauri-apps/plugin-dialog";
  import Modal from "../shared/Modal.svelte";
  import ContextMenu, { type MenuItem } from "../shared/ContextMenu.svelte";
  import FilePicker from "../history/FilePicker.svelte";
  import StashViewer, { type StashAction } from "./StashViewer.svelte";

  const repoPath = $derived($activeRepoPath);

  let expanded = $state(false);
  let stashMessage = $state("");
  let keepIndex = $state(false);
  let includeUntracked = $state(true);
  /** Paths to stash; empty = everything. */
  let onlyFiles = $state<string[]>([]);
  let pickerOpen = $state(false);

  let loading = $state(false);
  /** Commit SHA of the entry an operation is running on. */
  let actionOid = $state<string | null>(null);

  /** Stash entries with commit SHAs. Operations address entries by SHA. */
  let details = $state<StashDetail[]>([]);
  let viewing = $state<StashDetail | null>(null);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  let prompt = $state<{ kind: "rename" | "branch"; stash: StashDetail; value: string } | null>(null);

  // Load the stash list when the repo changes (refreshAll keeps it current).
  let lastLoadedPath: string | null = null;
  $effect(() => {
    const path = repoPath;
    if (path && path !== lastLoadedPath) {
      lastLoadedPath = path;
      refreshStash(path);
    }
  });

  // `stashEntries` is refreshed by every refreshAll(); re-read the detailed
  // list (with SHAs) whenever it changes or the repo switches.
  let detailReq = 0;
  $effect(() => {
    void $stashEntries;
    const path = repoPath;
    if (!path) {
      details = [];
      return;
    }
    const id = ++detailReq;
    tauri
      .stashListDetailed(path)
      .then((list) => {
        if (id !== detailReq || $activeRepoPath !== path) return;
        details = list;
        // Keep the viewer pointing at the same stash (its index may shift).
        if (viewing) viewing = list.find((d) => d.oid === viewing?.oid) ?? null;
      })
      .catch(() => {
        if (id === detailReq) details = [];
      });
  });

  // Drop a file selection that no longer has changes.
  const changedFiles = $derived.by(() => {
    const set = new Set<string>();
    for (const f of $workingStatus.staged) set.add(f.path);
    for (const f of $workingStatus.unstaged) set.add(f.path);
    return [...set].sort();
  });
  const fileTags = $derived.by(() => {
    const m = new Map<string, string>();
    for (const f of $workingStatus.unstaged) m.set(f.path, f.is_new ? "untracked" : "unstaged");
    for (const f of $workingStatus.staged) m.set(f.path, m.has(f.path) ? "partly staged" : "staged");
    return m;
  });
  $effect(() => {
    const avail = new Set(changedFiles);
    if (onlyFiles.some((f) => !avail.has(f))) onlyFiles = onlyFiles.filter((f) => avail.has(f));
  });

  // Any stash operation renumbers stash@{N}, so only one may run at a time.
  const busy = $derived(loading || actionOid !== null);

  onMount(() =>
    onAction("stash_files", () => {
      if (!$activeRepoPath) return;
      expanded = true;
      if (changedFiles.length === 0) {
        toast("info", "There are no changes to stash.");
        return;
      }
      pickerOpen = true;
    }),
  );

  async function handleStashPush() {
    if (!repoPath || busy) return;
    loading = true;
    try {
      const result = await tauri.stashPushExt(repoPath, {
        message: stashMessage.trim() || undefined,
        files: onlyFiles.map((f) => f.replace(/\/$/, "")),
        keepIndex,
        includeUntracked,
      });
      if (result.success) {
        const n = onlyFiles.length;
        stashMessage = "";
        onlyFiles = [];
        if (/No local changes to save/i.test(result.message)) {
          toast("info", "No local changes to save");
        } else {
          toast("success", n > 0 ? `Stashed ${n} file${n === 1 ? "" : "s"}` : "Changes stashed");
        }
        await refreshAll();
      } else {
        toast("error", result.message.trim(), { title: "Stash failed" });
      }
    } catch (err) {
      toastError("Stash failed", err);
    } finally {
      loading = false;
    }
  }

  async function act(action: "apply" | "pop" | "drop", s: StashDetail) {
    if (!repoPath || busy) return;
    if (action === "drop" && $settings.confirm_destructive_ops) {
      const ok = await ask(`Drop stash "${s.message}"? This cannot be undone.`, {
        title: "Drop Stash",
        kind: "warning",
      });
      if (!ok) return;
    }
    actionOid = s.oid;
    const title = action === "pop" ? "Stash pop failed" : action === "apply" ? "Stash apply failed" : "Stash drop failed";
    try {
      const result = await tauri.stashAct(repoPath, s.oid, action);
      // Refresh regardless: a conflicting pop/apply still changes the tree.
      await refreshAll();
      if (!result.success) {
        toast("error", result.message.trim(), { title });
      } else {
        if (action !== "apply" && viewing?.oid === s.oid) viewing = null;
        toast("success", action === "drop" ? "Stash dropped" : action === "pop" ? "Stash popped" : "Stash applied");
      }
    } catch (err) {
      toastError(title, err);
    } finally {
      actionOid = null;
    }
  }

  async function submitPrompt(e?: Event) {
    e?.preventDefault();
    const p = prompt;
    if (!p || !repoPath || busy) return;
    const value = p.value.trim();
    if (!value) return;
    actionOid = p.stash.oid;
    try {
      const result =
        p.kind === "rename"
          ? await tauri.stashRename(repoPath, p.stash.oid, value)
          : await tauri.stashBranch(repoPath, p.stash.oid, value);
      await refreshAll();
      if (result.success) {
        prompt = null;
        if (p.kind === "branch") viewing = null;
        toast("success", p.kind === "rename" ? "Stash renamed" : `Created branch ${value} from stash`);
      } else {
        toast("error", result.message.trim(), {
          title: p.kind === "rename" ? "Rename failed" : "Create branch failed",
        });
      }
    } catch (err) {
      toastError(p.kind === "rename" ? "Rename failed" : "Create branch failed", err);
    } finally {
      actionOid = null;
    }
  }

  /** Strip git's "On <branch>: " prefix for editing. */
  function bareMessage(msg: string): string {
    return msg.replace(/^(WIP )?[Oo]n [^:]+:\s*/, "");
  }

  function handleAction(action: StashAction, s: StashDetail) {
    if (action === "rename") prompt = { kind: "rename", stash: s, value: bareMessage(s.message) };
    else if (action === "branch") prompt = { kind: "branch", stash: s, value: "" };
    else act(action, s);
  }

  function openMenu(e: MouseEvent, s: StashDetail) {
    e.preventDefault();
    menu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: "View changes", action: () => { viewing = s; } },
        { separator: true },
        { label: "Pop", action: () => handleAction("pop", s), disabled: busy },
        { label: "Apply", action: () => handleAction("apply", s), disabled: busy },
        { label: "Create branch from stash…", action: () => handleAction("branch", s), disabled: busy },
        { label: "Rename…", action: () => handleAction("rename", s), disabled: busy },
        { separator: true },
        { label: "Drop…", action: () => handleAction("drop", s), danger: true, disabled: busy },
      ],
    };
  }

  function formatTime(timestamp: string): string {
    try {
      const date = new Date(timestamp);
      const now = new Date();
      const diffMs = now.getTime() - date.getTime();
      const diffMins = Math.floor(diffMs / 60000);
      if (diffMins < 1) return "just now";
      if (diffMins < 60) return `${diffMins}m ago`;
      const diffHours = Math.floor(diffMins / 60);
      if (diffHours < 24) return `${diffHours}h ago`;
      const diffDays = Math.floor(diffHours / 24);
      if (diffDays < 30) return `${diffDays}d ago`;
      return date.toLocaleDateString();
    } catch {
      return "";
    }
  }
</script>

<div class="stash-section">
  <div
    class="section-header"
    onclick={() => (expanded = !expanded)}
    onkeydown={(e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        expanded = !expanded;
      }
    }}
    role="button"
    tabindex="0"
    aria-expanded={expanded}
  >
    {#if expanded}
      <ChevronDown size={14} />
    {:else}
      <ChevronRight size={14} />
    {/if}
    <Archive size={12} />
    <span class="section-title">Stashes</span>
    <span class="section-count">{details.length}</span>
  </div>

  {#if expanded}
    <div class="stash-content">
      <!-- Stash push input -->
      <div class="stash-push">
        <input
          class="stash-input"
          type="text"
          placeholder="Stash message (optional)..."
          bind:value={stashMessage}
          onkeydown={(e) => e.key === "Enter" && !e.isComposing && handleStashPush()}
        />
        <button
          class="stash-push-btn"
          onclick={handleStashPush}
          disabled={busy}
          title={onlyFiles.length > 0 ? `Stash ${onlyFiles.length} selected file(s)` : "Stash changes"}
        >
          {#if loading}
            <Loader2 size={12} class="spinner" />
          {:else}
            <Archive size={12} />
          {/if}
        </button>
      </div>
      <div class="stash-options">
        <label title="Leave staged changes in place (they are still saved in the stash)">
          <input type="checkbox" bind:checked={keepIndex} /> Keep staged
        </label>
        <label title="Also stash untracked files">
          <input type="checkbox" bind:checked={includeUntracked} /> Untracked
        </label>
        {#if onlyFiles.length > 0}
          <span class="file-chip" title={onlyFiles.join("\n")}>
            <button class="chip-main" onclick={() => (pickerOpen = true)}>{onlyFiles.length} file{onlyFiles.length === 1 ? "" : "s"}</button>
            <button class="chip-x" onclick={() => (onlyFiles = [])} title="Stash everything" aria-label="Clear file selection">
              <X size={10} />
            </button>
          </span>
        {:else}
          <button
            class="files-btn"
            onclick={() => (pickerOpen = true)}
            disabled={changedFiles.length === 0}
            title="Stash only selected files"
          >
            <Files size={11} /> Files…
          </button>
        {/if}
      </div>

      <!-- Stash entries -->
      <div class="stash-list">
        {#each details as entry (entry.oid)}
          <div
            class="stash-item"
            role="button"
            tabindex="0"
            onclick={() => (viewing = entry)}
            onkeydown={(e) => (e.key === "Enter" || e.key === " ") && e.target === e.currentTarget && (e.preventDefault(), (viewing = entry))}
            oncontextmenu={(e) => openMenu(e, entry)}
            title="Click to view changes · right-click for more"
          >
            <div class="stash-info">
              <span class="stash-msg" title={entry.message}>{entry.message}</span>
              <span class="stash-time">
                {formatTime(entry.timestamp)}{entry.has_untracked ? " · +untracked" : ""}
              </span>
            </div>
            <div class="stash-actions">
              {#if actionOid === entry.oid}
                <Loader2 size={12} class="spinner" />
              {:else}
                <button
                  class="stash-action-btn"
                  onclick={(e) => { e.stopPropagation(); act("pop", entry); }}
                  disabled={busy}
                  title="Pop (apply & remove)"
                >
                  <ArchiveRestore size={12} />
                </button>
                <button
                  class="stash-action-btn"
                  onclick={(e) => { e.stopPropagation(); act("apply", entry); }}
                  disabled={busy}
                  title="Apply (keep stash)"
                >
                  <Copy size={12} />
                </button>
                <button
                  class="stash-action-btn delete"
                  onclick={(e) => { e.stopPropagation(); act("drop", entry); }}
                  disabled={busy}
                  title="Drop"
                >
                  <Trash2 size={12} />
                </button>
              {/if}
            </div>
          </div>
        {:else}
          <div class="empty-msg">No stashes</div>
        {/each}
      </div>
    </div>
  {/if}
</div>

{#if repoPath}
  <StashViewer stash={viewing} {repoPath} {busy} onclose={() => (viewing = null)} onaction={handleAction} />
{/if}

<FilePicker
  open={pickerOpen}
  title="Stash selected files"
  items={changedFiles}
  multiple
  initialSelected={onlyFiles}
  tag={(p) => fileTags.get(p) ?? ""}
  confirmLabel="Use selection"
  onconfirm={(paths) => {
    onlyFiles = paths;
    pickerOpen = false;
  }}
  onclose={() => (pickerOpen = false)}
/>

<Modal
  open={prompt !== null}
  title={prompt?.kind === "branch" ? "Create branch from stash" : "Rename stash"}
  onclose={() => (prompt = null)}
  width="420px"
>
  {#if prompt}
    <form class="prompt-form" onsubmit={submitPrompt}>
      <label>
        <span>{prompt.kind === "branch" ? "New branch name" : "Message"}</span>
        <input type="text" bind:value={prompt.value} spellcheck="false" />
      </label>
      <p class="prompt-hint">
        {prompt.kind === "branch"
          ? "Checks out a new branch at the commit the stash was made on, applies the stash and drops it on success."
          : "The stash keeps its contents; it moves to the top of the list."}
      </p>
      <div class="prompt-buttons">
        <button type="button" class="prompt-btn" onclick={() => (prompt = null)}>Cancel</button>
        <button type="submit" class="prompt-btn primary" disabled={!prompt.value.trim() || busy}>
          {prompt.kind === "branch" ? "Create branch" : "Rename"}
        </button>
      </div>
    </form>
  {/if}
</Modal>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<style>
  .stash-section {
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

  .stash-content {
    border-bottom: 1px solid var(--color-border);
  }

  .stash-push {
    display: flex;
    gap: 4px;
    padding: 6px 8px;
  }

  .stash-input {
    flex: 1;
    padding: 3px 6px;
    border: 1px solid var(--color-border);
    border-radius: 3px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 11px;
    font-family: inherit;
    outline: none;
    min-width: 0;
  }

  .stash-input:focus {
    border-color: var(--color-accent);
  }

  .stash-input::placeholder {
    color: var(--color-text-muted);
  }

  .stash-push-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border: 1px solid var(--color-border);
    border-radius: 3px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    cursor: pointer;
    padding: 0;
    flex-shrink: 0;
  }

  .stash-push-btn:hover {
    background: var(--color-surface-elevated);
  }

  .stash-push-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .stash-list {
    max-height: 150px;
    overflow-y: auto;
  }

  .stash-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px 4px 16px;
    transition: background 0.1s;
  }

  .stash-item:hover {
    background: var(--color-surface);
  }

  .stash-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .stash-msg {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .stash-time {
    font-size: 10px;
    color: var(--color-text-muted);
  }

  .stash-actions {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
    align-items: center;
  }

  .stash-action-btn {
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
  }

  .stash-action-btn:hover:not(:disabled) {
    background: color-mix(in srgb, var(--color-accent) 20%, transparent);
    color: var(--color-accent);
  }

  .stash-action-btn.delete:hover:not(:disabled) {
    background: var(--color-diff-del-bg);
    color: var(--color-diff-del-text);
  }

  .stash-action-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .stash-actions :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  .stash-push-btn :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .empty-msg {
    padding: 8px 16px;
    color: var(--color-text-muted);
    font-style: italic;
    font-size: 11px;
  }

  .stash-item {
    cursor: pointer;
  }

  .stash-options {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 8px 6px;
    font-size: 11px;
    color: var(--color-text-muted);
  }

  .stash-options label {
    display: flex;
    align-items: center;
    gap: 4px;
    cursor: pointer;
  }

  .files-btn,
  .chip-main {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-left: auto;
    padding: 1px 6px;
    border: 1px solid var(--color-border);
    border-radius: 3px;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 11px;
    cursor: pointer;
  }

  .files-btn:hover:not(:disabled) {
    color: var(--color-text-primary);
    border-color: var(--color-accent);
  }

  .files-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .file-chip {
    display: inline-flex;
    align-items: center;
    margin-left: auto;
  }

  .chip-main {
    margin-left: 0;
    border-radius: 3px 0 0 3px;
    border-color: var(--color-accent);
    color: var(--color-accent);
  }

  .chip-x {
    display: inline-flex;
    align-items: center;
    padding: 2px 4px;
    border: 1px solid var(--color-accent);
    border-left: none;
    border-radius: 0 3px 3px 0;
    background: transparent;
    color: var(--color-accent);
    cursor: pointer;
  }

  .prompt-form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .prompt-form label {
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: 12px;
    color: var(--color-text-muted);
  }

  .prompt-form input {
    padding: 6px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 13px;
    outline: none;
  }

  .prompt-form input:focus {
    border-color: var(--color-accent);
  }

  .prompt-hint {
    margin: 0;
    font-size: 11px;
    color: var(--color-text-muted);
  }

  .prompt-buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .prompt-btn {
    padding: 6px 14px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
  }

  .prompt-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .prompt-btn.primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: var(--color-bg);
  }
</style>
