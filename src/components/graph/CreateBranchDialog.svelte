<script lang="ts">
  import Modal from "../shared/Modal.svelte";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { refreshAll } from "../../lib/stores/graph";
  import { createBranchTarget } from "../../lib/stores/commitUi";
  import { toast, toastError } from "../../lib/stores/toasts";
  import * as tauri from "../../lib/tauri";

  const target = $derived($createBranchTarget);

  let name = $state("");
  let checkout = $state(true);
  let busy = $state(false);
  let error = $state<string | null>(null);

  // Reset the form each time the dialog opens for a new target.
  $effect(() => {
    if (target) {
      name = "";
      checkout = true;
      error = null;
    }
  });

  function close() {
    if (!busy) $createBranchTarget = null;
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    const path = $activeRepoPath;
    const branch = name.trim();
    if (!path || !target || !branch || busy) return;
    if (/\s/.test(branch)) {
      error = "Branch names cannot contain spaces.";
      return;
    }
    busy = true;
    error = null;
    try {
      const created = await tauri.createBranch(path, branch, target.oid);
      if (!created.success) {
        error = created.message.trim() || "Could not create the branch.";
        return;
      }
      if (checkout) {
        const co = await tauri.checkoutBranch(path, branch);
        await refreshAll(path);
        if (!co.success) {
          toast("warning", `Created ${branch}, but checking it out failed: ${co.message.trim()}`, {
            title: "Checkout Failed",
          });
        } else {
          toast("success", `Created and checked out ${branch}`);
        }
      } else {
        await refreshAll(path);
        toast("success", `Created branch ${branch} at ${target.label}`);
      }
      busy = false;
      close();
    } catch (err) {
      toastError("Create Branch Failed", err);
    } finally {
      busy = false;
    }
  }
</script>

<Modal open={!!target} title="Create Branch" onclose={close} width="400px">
  {#if target}
    <form class="dialog" onsubmit={submit}>
      <p class="desc">New branch at <code>{target.label}</code></p>
      <label class="field">
        <span>Name</span>
        <input type="text" bind:value={name} placeholder="feature/my-branch" spellcheck="false" autocomplete="off" />
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={checkout} />
        <span>Check out after creating</span>
      </label>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <div class="actions">
        <button type="button" class="btn-secondary" onclick={close} disabled={busy}>Cancel</button>
        <button type="submit" class="btn-primary" disabled={busy || !name.trim()}>
          {busy ? "Creating…" : "Create Branch"}
        </button>
      </div>
    </form>
  {/if}
</Modal>

<style>
  .dialog { display: flex; flex-direction: column; gap: 12px; font-size: 13px; }
  .desc { margin: 0; color: var(--color-text-muted); }
  code { font-family: var(--font-mono); color: var(--color-accent); }
  .field { display: flex; flex-direction: column; gap: 4px; color: var(--color-text-muted); font-size: 12px; }
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
  .check { display: flex; align-items: center; gap: 6px; color: var(--color-text-primary); font-size: 12px; cursor: pointer; }
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
