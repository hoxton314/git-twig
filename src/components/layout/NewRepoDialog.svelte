<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { FolderOpen } from "lucide-svelte";
  import { untrack } from "svelte";
  import Modal from "../shared/Modal.svelte";
  import * as tauri from "../../lib/tauri";
  import { addRepo } from "../../lib/stores/repos";
  import { currentView } from "../../lib/stores/ui";
  import { settings } from "../../lib/stores/settings";
  import { toast } from "../../lib/stores/toasts";
  import { newRepoDialog, cloneFolderName, joinPath } from "../../lib/newRepo";
  import { t, tr, type MessageKey } from "../../lib/i18n";

  /** Example remotes shown in the URL field (not translated). */
  const URL_EXAMPLES = "https://… , git@host:owner/repo.git , ssh://… , /path/to/repo";
  const DEFAULT_BRANCH = "main";

  const mode = $derived($newRepoDialog);

  // Clone
  let url = $state("");
  let parentDir = $state("");
  let folderName = $state("");
  let nameEdited = $state(false);
  let progress = $state("");
  // Init
  let initDir = $state("");
  let initialBranch = $state("main");

  let busy = $state(false);
  let error = $state<string | null>(null);
  let nextOp = 1;

  // Reset the form whenever a dialog opens.
  $effect(() => {
    if (!mode) return;
    const base = untrack(() => $settings.default_repo_dir) ?? "";
    url = "";
    parentDir = base;
    folderName = "";
    nameEdited = false;
    progress = "";
    initDir = "";
    initialBranch = "main";
    error = null;
  });

  // Suggest the folder name from the URL until the user types their own.
  $effect(() => {
    if (!nameEdited) folderName = cloneFolderName(url);
  });

  const destination = $derived(parentDir && folderName ? joinPath(parentDir, folderName) : "");

  function close() {
    if (busy) return;
    newRepoDialog.set(null);
  }

  async function pickFolder(title: string): Promise<string | null> {
    const picked = await openDialog({
      directory: true,
      multiple: false,
      title,
      defaultPath: $settings.default_repo_dir ?? undefined,
    });
    return typeof picked === "string" ? picked : null;
  }

  function opened(info: Awaited<ReturnType<typeof tauri.openRepo>>, doneKey: MessageKey) {
    addRepo(info);
    $currentView = "repos";
    toast("success", tr(doneKey, { name: info.name }));
    busy = false;
    newRepoDialog.set(null);
  }

  async function submitClone(e: Event) {
    e.preventDefault();
    if (busy || !url.trim() || !destination) return;
    busy = true;
    error = null;
    progress = tr("app.cloneStarting");
    const opId = nextOp++;
    let unlisten: (() => void) | null = null;
    try {
      unlisten = await tauri.onCloneProgress((p) => {
        if (p.op_id === opId) progress = p.line;
      });
      opened(await tauri.cloneRepository(url.trim(), destination, opId), "app.cloned");
    } catch (err) {
      error = String(err);
      busy = false;
    } finally {
      unlisten?.();
    }
  }

  async function submitInit(e: Event) {
    e.preventDefault();
    if (busy || !initDir.trim()) return;
    busy = true;
    error = null;
    try {
      opened(await tauri.initRepository(initDir.trim(), initialBranch.trim() || null), "app.initialized");
    } catch (err) {
      error = String(err);
      busy = false;
    }
  }
</script>

<Modal
  open={mode !== null}
  title={mode === "clone" ? $t("app.cloneTitle") : $t("app.initTitle")}
  onclose={close}
  width="520px"
>
  {#if mode === "clone"}
    <form class="dialog" onsubmit={submitClone}>
      <label class="field">
        <span>{$t("app.repoUrl")}</span>
        <input
          type="text"
          bind:value={url}
          placeholder={URL_EXAMPLES}
          spellcheck="false"
          autocomplete="off"
          disabled={busy}
        />
      </label>
      <div class="field">
        <span>{$t("app.cloneInto")}</span>
        <div class="row">
          <input type="text" bind:value={parentDir} placeholder={$t("app.parentFolder")} spellcheck="false" disabled={busy} aria-label={$t("app.parentFolder")} />
          <button
            type="button"
            class="icon-btn"
            title={$t("app.chooseFolder")}
            aria-label={$t("app.chooseParentFolder")}
            disabled={busy}
            onclick={async () => { const d = await pickFolder(tr("app.cloneIntoFolder")); if (d) parentDir = d; }}
          >
            <FolderOpen size={14} />
          </button>
        </div>
        <input
          type="text"
          bind:value={folderName}
          oninput={() => (nameEdited = true)}
          placeholder={$t("app.folderName")}
          spellcheck="false"
          disabled={busy}
          aria-label={$t("app.folderName")}
        />
        {#if destination}<span class="hint">{destination}</span>{/if}
      </div>
      {#if busy && progress}<p class="progress" aria-live="polite">{progress}</p>{/if}
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <div class="actions">
        <button type="button" class="btn-secondary" onclick={close} disabled={busy}>{$t("common.cancel")}</button>
        <button type="submit" class="btn-primary" disabled={busy || !url.trim() || !destination}>
          {busy ? $t("app.cloning") : $t("app.clone")}
        </button>
      </div>
    </form>
  {:else if mode === "init"}
    <form class="dialog" onsubmit={submitInit}>
      <div class="field">
        <span>{$t("app.initFolder")}</span>
        <div class="row">
          <input type="text" bind:value={initDir} placeholder={$t("app.initFolderPlaceholder")} spellcheck="false" disabled={busy} aria-label={$t("app.repoFolder")} />
          <button
            type="button"
            class="icon-btn"
            title={$t("app.chooseFolder")}
            aria-label={$t("app.chooseFolder")}
            disabled={busy}
            onclick={async () => { const d = await pickFolder(tr("app.initInFolder")); if (d) initDir = d; }}
          >
            <FolderOpen size={14} />
          </button>
        </div>
      </div>
      <label class="field">
        <span>{$t("app.initialBranch")}</span>
        <input type="text" bind:value={initialBranch} placeholder={DEFAULT_BRANCH} spellcheck="false" autocomplete="off" disabled={busy} />
      </label>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <div class="actions">
        <button type="button" class="btn-secondary" onclick={close} disabled={busy}>{$t("common.cancel")}</button>
        <button type="submit" class="btn-primary" disabled={busy || !initDir.trim()}>
          {busy ? $t("app.initializing") : $t("app.initialize")}
        </button>
      </div>
    </form>
  {/if}
</Modal>

<style>
  .dialog { display: flex; flex-direction: column; gap: 12px; font-size: 13px; }
  .field { display: flex; flex-direction: column; gap: 4px; color: var(--color-text-muted); font-size: 12px; }
  .row { display: flex; gap: 6px; }
  .row input { flex: 1; }
  .field input {
    padding: 6px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 13px;
    outline: none;
  }
  .field input:focus { border-color: var(--color-accent); }
  .hint { font-family: var(--font-mono); font-size: 11px; word-break: break-all; }
  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
  }
  .icon-btn:hover:not(:disabled) { color: var(--color-text-primary); }
  .progress {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .error { margin: 0; color: var(--color-diff-del-text); font-size: 12px; white-space: pre-wrap; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; }
  .btn-primary, .btn-secondary {
    border-radius: 4px;
    padding: 6px 16px;
    font-size: 13px;
    cursor: pointer;
  }
  .btn-primary { background: var(--color-accent); color: var(--color-bg); border: none; font-weight: 600; }
  .btn-secondary { background: transparent; color: var(--color-text-muted); border: 1px solid var(--color-border); }
  .btn-secondary:hover:not(:disabled) { color: var(--color-text-primary); }
  .btn-primary:disabled, .btn-secondary:disabled { opacity: 0.6; cursor: not-allowed; }
</style>
