<script lang="ts">
  import { RefreshCw, Scissors, Pencil, Trash2, Plus, Copy } from "lucide-svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { untrack } from "svelte";
  import { get } from "svelte/store";
  import Modal from "../shared/Modal.svelte";
  import { t, tr } from "../../lib/i18n";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { refreshAll } from "../../lib/stores/graph";
  import { settings } from "../../lib/stores/settings";
  import { remotes, remotesDialogOpen, loadRemotes } from "../../lib/stores/remotes";
  import { toast, toastError } from "../../lib/stores/toasts";
  import { copyText } from "../../lib/branchTree";
  import * as tauri from "../../lib/tauri";
  import type { CommandResult, RemoteInfo } from "../../lib/types/git";

  const repoPath = $derived($activeRepoPath);
  const list = $derived($remotes);

  /** Name of the remote an operation is running on ("" = add form). */
  let busy = $state<string | null>(null);

  // Edit form (one remote at a time)
  let editing = $state<string | null>(null);
  let editName = $state("");
  let editFetch = $state("");
  let editPush = $state("");

  // Add form
  let newName = $state("");
  let newUrl = $state("");
  // Reset forms and reload when the dialog opens. Only `open` and the repo are
  // tracked: reacting to `list` would re-run this after every reload.
  $effect(() => {
    const open = $remotesDialogOpen;
    const path = repoPath;
    if (!open || !path) return;
    untrack(() => {
      editing = null;
      loadRemotes(path).then(() => {
        if (!newName && get(remotes).length === 0) newName = "origin";
      });
    });
  });

  function close() {
    remotesDialogOpen.set(false);
    editing = null;
  }

  /** Run an op; toast its outcome; reload remotes + refs. Returns success. */
  async function run(
    target: string,
    title: string,
    op: (path: string) => Promise<CommandResult>,
    refreshRefs = true,
  ): Promise<boolean> {
    const path = repoPath;
    if (!path || busy !== null) return false;
    busy = target;
    try {
      const result = await op(path);
      await loadRemotes(path);
      if (refreshRefs) await refreshAll(path);
      if (result.success) {
        toast("success", result.message);
        return true;
      }
      toast("error", result.message, { title });
      return false;
    } catch (err) {
      toastError(title, err);
      return false;
    } finally {
      busy = null;
    }
  }

  function startEdit(r: RemoteInfo) {
    editing = r.name;
    editName = r.name;
    editFetch = r.fetch_url ?? "";
    editPush = r.has_separate_push_url ? (r.push_url ?? "") : "";
  }

  async function saveEdit(r: RemoteInfo) {
    const name = editName.trim();
    const fetchUrl = editFetch.trim();
    const pushUrl = editPush.trim();
    if (!name || !fetchUrl) {
      toast("warning", tr("branches.nameUrlRequired"));
      return;
    }
    let current = r.name;
    if (name !== r.name) {
      const ok = await run(r.name, tr("branches.renameRemoteFailed"), (p) => tauri.renameRemote(p, r.name, name));
      if (!ok) return;
      current = name;
    }
    const oldPush = r.has_separate_push_url ? (r.push_url ?? "") : "";
    if (fetchUrl !== (r.fetch_url ?? "") || pushUrl !== oldPush) {
      const ok = await run(
        current,
        tr("branches.updateRemoteFailed"),
        (p) => tauri.setRemoteUrls(p, current, fetchUrl, pushUrl || null),
        false,
      );
      if (!ok) return;
    }
    editing = null;
  }

  async function removeRemote(r: RemoteInfo) {
    if ($settings.confirm_destructive_ops) {
      const ok = await ask(
        tr("branches.removeRemoteConfirm", { name: r.name }),
        { title: tr("branches.removeRemoteTitle"), kind: "warning" },
      );
      if (!ok) return;
    }
    await run(r.name, tr("branches.removeRemoteFailed"), (p) => tauri.removeRemote(p, r.name));
  }

  async function addRemote() {
    const name = newName.trim();
    const url = newUrl.trim();
    if (!name || !url) return;
    const ok = await run("", tr("branches.addRemoteFailed"), (p) => tauri.addRemote(p, name, url), false);
    if (!ok) return;
    newName = "";
    newUrl = "";
    // Fetch right away so the new remote's branches show up.
    await run(name, tr("branches.fetchFailed"), (p) => tauri.fetchRemote(p, name));
  }

  async function copyUrl(url: string | null) {
    if (!url) return;
    try {
      await copyText(url);
      toast("info", tr("branches.urlCopied"), { duration: 1500 });
    } catch (err) {
      toastError(tr("branches.copyFailed"), err);
    }
  }

  function onAddKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      addRemote();
    }
  }

  function onEditKeydown(e: KeyboardEvent, r: RemoteInfo) {
    if (e.key === "Enter") {
      e.preventDefault();
      saveEdit(r);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      editing = null;
    }
  }
</script>

<Modal open={$remotesDialogOpen} title={$t("branches.remotesTitle")} onclose={close} width="600px">
  <div class="remotes">
    {#if list.length === 0}
      <p class="empty">{$t("branches.noRemotesConfigured")}</p>
    {:else}
      <ul class="remote-list">
        {#each list as r (r.name)}
          <li class="remote" class:busy={busy === r.name}>
            {#if editing === r.name}
              <div class="edit-form">
                <label>
                  <span>{$t("branches.remoteNameField")}</span>
                  <input bind:value={editName} onkeydown={(e) => onEditKeydown(e, r)} spellcheck="false" />
                </label>
                <label>
                  <span>{$t("branches.fetchUrl")}</span>
                  <input bind:value={editFetch} onkeydown={(e) => onEditKeydown(e, r)} spellcheck="false" />
                </label>
                <label>
                  <span>{$t("branches.pushUrl")}</span>
                  <input
                    bind:value={editPush}
                    onkeydown={(e) => onEditKeydown(e, r)}
                    placeholder={$t("branches.sameAsFetch")}
                    spellcheck="false"
                  />
                </label>
                <div class="form-actions">
                  <button class="btn-primary" onclick={() => saveEdit(r)} disabled={busy !== null}>{$t("common.save")}</button>
                  <button class="btn-secondary" onclick={() => (editing = null)} disabled={busy !== null}>{$t("common.cancel")}</button>
                </div>
              </div>
            {:else}
              <div class="remote-info">
                <span class="remote-name">{r.name}</span>
                <button class="url" onclick={() => copyUrl(r.fetch_url)} title={$t("branches.copyFetchUrl")}>
                  <span class="url-text">{r.fetch_url ?? $t("branches.noUrl")}</span>
                  <Copy size={11} />
                </button>
                {#if r.has_separate_push_url}
                  <button class="url" onclick={() => copyUrl(r.push_url)} title={$t("branches.copyPushUrl")}>
                    <span class="url-kind">{$t("branches.pushKind")}</span>
                    <span class="url-text">{r.push_url}</span>
                    <Copy size={11} />
                  </button>
                {/if}
              </div>
              <div class="remote-actions">
                <button
                  class="icon-btn"
                  onclick={() => run(r.name, $t("branches.fetchFailed"), (p) => tauri.fetchRemote(p, r.name))}
                  disabled={busy !== null}
                  title={$t("branches.fetchRemote", { name: r.name })}
                  aria-label={$t("branches.fetchRemote", { name: r.name })}
                >
                  <RefreshCw size={13} class={busy === r.name ? "spin" : ""} />
                </button>
                <button
                  class="icon-btn"
                  onclick={() => run(r.name, $t("branches.pruneFailed"), (p) => tauri.pruneRemote(p, r.name))}
                  disabled={busy !== null}
                  title={$t("branches.pruneStaleTracking")}
                  aria-label={$t("branches.pruneNamed", { name: r.name })}
                >
                  <Scissors size={13} />
                </button>
                <button
                  class="icon-btn"
                  onclick={() => startEdit(r)}
                  disabled={busy !== null}
                  title={$t("branches.editRemoteTitle")}
                  aria-label={$t("branches.editNamed", { name: r.name })}
                >
                  <Pencil size={13} />
                </button>
                <button
                  class="icon-btn danger"
                  onclick={() => removeRemote(r)}
                  disabled={busy !== null}
                  title={$t("branches.removeRemote")}
                  aria-label={$t("branches.removeNamed", { name: r.name })}
                >
                  <Trash2 size={13} />
                </button>
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}

    <div class="add-form">
      <div class="add-title">{$t("branches.addRemote")}</div>
      <div class="add-row">
        <input
          class="name-input"
          bind:value={newName}
          onkeydown={onAddKeydown}
          placeholder={$t("branches.remoteNameField")}
          aria-label={$t("branches.remoteName")}
          spellcheck="false"
        />
        <input
          class="url-input"
          bind:value={newUrl}
          onkeydown={onAddKeydown}
          placeholder={$t("branches.remoteUrlPlaceholder")}
          aria-label={$t("branches.remoteUrl")}
          spellcheck="false"
        />
        <button
          class="btn-primary"
          onclick={addRemote}
          disabled={busy !== null || !newName.trim() || !newUrl.trim()}
        >
          <Plus size={13} />
          {busy === "" ? $t("branches.adding") : $t("common.add")}
        </button>
      </div>
    </div>
  </div>
</Modal>

<style>
  .remotes {
    display: flex;
    flex-direction: column;
    gap: 14px;
    font-size: 12px;
  }

  .empty {
    margin: 0;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .remote-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    overflow: hidden;
  }

  .remote {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--color-border);
  }

  .remote:last-child {
    border-bottom: none;
  }

  .remote.busy {
    opacity: 0.7;
  }

  .remote-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }

  .remote-name {
    font-weight: 600;
    color: var(--color-text-primary);
  }

  .url {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0;
    border: none;
    background: none;
    color: var(--color-text-muted);
    font-family: var(--font-mono, monospace);
    font-size: 11px;
    text-align: left;
    cursor: pointer;
    min-width: 0;
  }

  .url:hover {
    color: var(--color-text-primary);
  }

  .url-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .url-kind {
    flex-shrink: 0;
    padding: 0 4px;
    border: 1px solid var(--color-border);
    border-radius: 3px;
    font-family: inherit;
  }

  .remote-actions {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
  }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 0;
  }

  .icon-btn:hover:not(:disabled) {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .icon-btn.danger:hover:not(:disabled) {
    color: var(--color-diff-del-text);
  }

  .icon-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .icon-btn :global(.spin) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .edit-form {
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex: 1;
  }

  .edit-form label {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .edit-form label span {
    width: 70px;
    flex-shrink: 0;
    color: var(--color-text-muted);
  }

  input {
    flex: 1;
    min-width: 0;
    padding: 5px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 12px;
    outline: none;
  }

  input:focus {
    border-color: var(--color-accent);
  }

  .form-actions {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
  }

  .add-form {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .add-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--color-text-muted);
  }

  .add-row {
    display: flex;
    gap: 6px;
  }

  .name-input {
    flex: 0 0 120px;
  }

  .btn-primary,
  .btn-secondary {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border-radius: 4px;
    padding: 5px 12px;
    font-size: 12px;
    cursor: pointer;
    flex-shrink: 0;
  }

  .btn-primary {
    background: var(--color-accent);
    color: var(--color-on-accent);
    border: none;
    font-weight: 600;
  }

  .btn-secondary {
    background: transparent;
    color: var(--color-text-muted);
    border: 1px solid var(--color-border);
  }

  .btn-primary:disabled,
  .btn-secondary:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
