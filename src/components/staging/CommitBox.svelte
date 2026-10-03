<script lang="ts" module>
  // Unsent commit messages per repo path. Module-level so drafts survive both
  // tab switches (this component is shared across repos) and remounts.
  const commitDrafts = new Map<string, string>();
</script>

<script lang="ts">
  import { Send, Loader2, Undo2, Users, FileText, History, PenLine, X } from "lucide-svelte";
  import { onMount, tick, untrack } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import ContextMenu, { type MenuItem } from "../shared/ContextMenu.svelte";
  import CoAuthorPicker from "./CoAuthorPicker.svelte";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { workingStatus, commitGraph, branches, refreshAll } from "../../lib/stores/graph";
  import { onAction } from "../../lib/keybindings";
  import { toast, toastError } from "../../lib/stores/toasts";
  import { t, tr } from "../../lib/i18n";
  import * as tauri from "../../lib/tauri";
  import { waitForWrites, confirmDestructive } from "./writeQueue";
  import {
    CONVENTIONAL_TYPES,
    parseConventional,
    applyConventional,
    appendTrailer,
    loadHistory,
    pushHistory,
    loadSignoff,
    saveSignoff,
  } from "./commitHelpers";

  const repoPath = $derived($activeRepoPath);
  const status = $derived($workingStatus);
  const currentHead = $derived($branches.find((b) => b.is_head && !b.is_remote));
  const hasUnpushed = $derived(
    ($commitGraph?.unpushed_oids?.length ?? 0) > 0 ||
      (currentHead != null && (currentHead.upstream == null || currentHead.ahead > 0)),
  );

  let commitMessage = $state("");
  let loading = $state(false);
  let amend = $state(false);
  let amendBusy = $state(false);
  /** The draft that was in the box before "Amend" replaced it with HEAD's message. */
  let preAmendDraft = "";
  let signoff = $state(false);
  /** `--no-verify` for the next commit (deliberately not persisted). */
  let skipHooks = $state(false);
  /** What the last successful commit printed while hooks ran, until dismissed. */
  let hookOutput = $state<{ hooks: string[]; text: string } | null>(null);
  let template = $state<string | null>(null);
  let history = $state<string[]>([]);
  let historyIndex = -1;
  let historyStash = "";
  let showCoAuthor = $state(false);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  let textarea = $state<HTMLTextAreaElement | null>(null);
  let historyBtn = $state<HTMLButtonElement | null>(null);

  // First line of the commit message, for the summary length hint.
  const summaryLength = $derived(commitMessage.split("\n", 1)[0].length);
  const conventional = $derived(parseConventional(commitMessage));
  const canCommit = $derived(
    !loading && !amendBusy && commitMessage.trim().length > 0 && (amend || status.staged.length > 0),
  );

  /** The draft to keep for a repo when leaving it (not HEAD's amend message). */
  function currentDraft(): string {
    return amend ? preAmendDraft : commitMessage;
  }

  // Swap the draft (and per-repo helpers) when the active repo changes, so a
  // half-written message doesn't follow the user into another repo.
  let lastLoadedPath: string | null = null;
  $effect(() => {
    const path = repoPath;
    if (path && path !== lastLoadedPath) {
      untrack(() => {
        if (lastLoadedPath) commitDrafts.set(lastLoadedPath, currentDraft());
        amend = false;
        skipHooks = false;
        hookOutput = null;
        preAmendDraft = "";
        commitMessage = commitDrafts.get(path) ?? "";
        historyIndex = -1;
        signoff = loadSignoff(path);
        history = loadHistory(path);
        template = null;
      });
      lastLoadedPath = path;
      tauri
        .getCommitTemplate(path)
        .then((t) => {
          if (repoPath === path) template = t;
        })
        .catch((err) => console.warn("commit.template:", err));
    }
  });

  onMount(() => () => {
    if (lastLoadedPath) commitDrafts.set(lastLoadedPath, currentDraft());
  });

  // ── Amend ────────────────────────────────────────────────────────────

  async function setAmend(on: boolean) {
    if (!repoPath || on === amend || amendBusy) return;
    if (!on) {
      amend = false;
      commitMessage = preAmendDraft;
      preAmendDraft = "";
      return;
    }
    const path = repoPath;
    amendBusy = true;
    try {
      const head = await tauri.getHeadCommitInfo(path);
      if (repoPath !== path) return;
      if (head.pushed) {
        const ok = await confirmDestructive(
          head.pushed_to ? tr("commit.amendPushedTo", { remote: head.pushed_to }) : tr("commit.amendPushed"),
          tr("commit.amendPushedTitle"),
        );
        if (!ok || repoPath !== path) return;
      }
      preAmendDraft = commitMessage;
      commitMessage = head.message;
      historyIndex = -1;
      amend = true;
    } catch (err) {
      toastError(tr("commit.cannotAmend"), err);
    } finally {
      amendBusy = false;
    }
  }

  function toggleSignoff() {
    signoff = !signoff;
    if (repoPath) saveSignoff(repoPath, signoff);
  }

  // ── Message helpers ──────────────────────────────────────────────────

  function setType(type: string) {
    commitMessage = applyConventional(commitMessage, type, conventional?.scope ?? "");
    textarea?.focus();
  }

  function setScope(scope: string) {
    if (!conventional) return;
    commitMessage = applyConventional(commitMessage, conventional.type, scope);
  }

  function addCoAuthor(ident: string) {
    commitMessage = appendTrailer(commitMessage, `Co-authored-by: ${ident}`);
    tick().then(() => {
      if (!textarea) return;
      textarea.focus();
      // Leave the caret on the summary line when it's still empty.
      const pos = commitMessage.startsWith("\n") ? 0 : commitMessage.length;
      textarea.setSelectionRange(pos, pos);
    });
  }

  async function insertTemplate() {
    if (!template) {
      toast("info", tr("commit.noTemplate"));
      return;
    }
    if (commitMessage.trim() && commitMessage.trim() !== template.trim()) {
      const ok = await ask(tr("commit.replaceWithTemplate"), {
        title: tr("commit.insertTemplateTitle"),
        kind: "warning",
      });
      if (!ok) return;
    }
    commitMessage = template;
    textarea?.focus();
  }

  function recall(delta: 1 | -1) {
    if (history.length === 0) return;
    const next = Math.max(-1, Math.min(history.length - 1, historyIndex + delta));
    if (next === historyIndex) return;
    if (historyIndex === -1) historyStash = commitMessage;
    historyIndex = next;
    commitMessage = next === -1 ? historyStash : history[next];
  }

  function onMessageKey(e: KeyboardEvent) {
    if (!e.ctrlKey || e.shiftKey || e.altKey) return;
    if (e.key === "ArrowUp") {
      e.preventDefault();
      recall(1);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      recall(-1);
    }
  }

  function summaryOf(msg: string): string {
    const first = msg.split("\n", 1)[0];
    return first.length > 60 ? `${first.slice(0, 57)}…` : first || "(empty summary)";
  }

  function openHistory() {
    if (!historyBtn) return;
    const rect = historyBtn.getBoundingClientRect();
    const items: MenuItem[] =
      history.length === 0
        ? [{ label: tr("commit.noRecent"), disabled: true }]
        : history.map((msg, i) => ({
            label: summaryOf(msg),
            shortcut: i === 0 ? "Ctrl+↑" : undefined,
            action: () => {
              if (historyIndex === -1) historyStash = commitMessage;
              historyIndex = i;
              commitMessage = msg;
              textarea?.focus();
            },
          }));
    // Anchor above the button; ContextMenu clamps into the viewport.
    menu = { x: rect.left, y: rect.top - Math.min(items.length * 26 + 8, 400), items };
  }

  // ── Commit / undo ────────────────────────────────────────────────────

  async function handleCommit() {
    // `canCommit` guard: Ctrl+Enter bypasses the disabled button.
    if (!repoPath || !canCommit) return;
    const path = repoPath;
    const msg = commitMessage.trim();
    const wasAmend = amend;
    loading = true;
    try {
      // Wait for any queued stage/unstage so the commit sees the final index.
      await waitForWrites();
      hookOutput = null;
      const result = await tauri.createCommitWithOptions(path, msg, wasAmend, signoff, skipHooks);
      // Failures already show git's full message in the error toast.
      if (repoPath === path && result.success && result.hook_output) {
        hookOutput = { hooks: result.hooks, text: result.hook_output };
      }
      if (result.success) {
        const nextHistory = pushHistory(path, msg);
        commitDrafts.delete(path);
        if (repoPath === path) {
          history = nextHistory;
          historyIndex = -1;
          commitMessage = wasAmend ? preAmendDraft : "";
          preAmendDraft = "";
          amend = false;
          skipHooks = false;
        }
        if (wasAmend) toast("success", tr("commit.amended"));
        await refreshAll();
      } else {
        toast("error", result.message.trim() || tr("commit.gitFailed"), {
          title: wasAmend ? tr("commit.amendFailed") : tr("commit.failed"),
        });
      }
    } catch (err) {
      toastError(wasAmend ? tr("commit.amendFailed") : tr("commit.failed"), err);
    } finally {
      loading = false;
    }
  }

  async function handleUndoCommit() {
    if (!repoPath) return;
    const path = repoPath;
    const ok = await ask(tr("commit.undoConfirm"), {
      title: tr("commit.undoTitleDialog"),
      kind: "warning",
    });
    if (!ok) return;
    try {
      const result = await tauri.undoCommit(path);
      if (result.success) {
        await refreshAll();
      } else {
        toast("error", result.message.trim() || tr("commit.undoFailed"), { title: tr("commit.undoFailed") });
      }
    } catch (err) {
      toastError(tr("commit.undoFailed"), err);
    }
  }

  onMount(() => {
    const offs = [
      onAction("commit", handleCommit),
      onAction("commit_toggle_amend", () => setAmend(!amend)),
      onAction("commit_toggle_signoff", toggleSignoff),
      onAction("commit_add_coauthor", () => (showCoAuthor = true)),
      onAction("commit_insert_template", insertTemplate),
      onAction("commit_message_history", openHistory),
    ];
    return () => offs.forEach((off) => off());
  });
</script>

<div class="commit-box">
  <div class="helpers">
    <select
      class="cc-type"
      value={conventional?.type ?? ""}
      onchange={(e) => setType(e.currentTarget.value)}
      title={$t("commit.ccType")}
      aria-label={$t("commit.ccType")}
    >
      <option value="">{$t("commit.ccTypePlaceholder")}</option>
      {#each CONVENTIONAL_TYPES as ty (ty)}
        <option value={ty}>{ty}</option>
      {/each}
    </select>
    <input
      class="cc-scope"
      type="text"
      placeholder={$t("commit.ccScopePlaceholder")}
      value={conventional?.scope ?? ""}
      oninput={(e) => setScope(e.currentTarget.value)}
      disabled={!conventional}
      spellcheck="false"
      title={conventional ? $t("commit.ccScope") : $t("commit.ccScopeNeedsType")}
      aria-label={$t("commit.ccScopeLabel")}
    />
    <div class="helper-btns">
      <button class="icon-btn" onclick={() => (showCoAuthor = true)} title={$t("commit.addCoAuthor")} aria-label={$t("commit.addCoAuthor")}>
        <Users size={13} />
      </button>
      {#if template}
        <button class="icon-btn" onclick={insertTemplate} title={$t("commit.insertTemplate")} aria-label={$t("commit.insertTemplate")}>
          <FileText size={13} />
        </button>
      {/if}
      <button
        class="icon-btn"
        bind:this={historyBtn}
        onclick={openHistory}
        title={$t("commit.recentTitle")}
        aria-label={$t("commit.recentLabel")}
        aria-haspopup="menu"
      >
        <History size={13} />
      </button>
    </div>
  </div>

  <textarea
    bind:this={textarea}
    class="commit-input"
    class:amending={amend}
    placeholder={template ?? (amend ? $t("commit.placeholderAmend") : $t("commit.placeholder"))}
    bind:value={commitMessage}
    oninput={() => (historyIndex = -1)}
    onkeydown={onMessageKey}
    rows="3"
    spellcheck="true"
    aria-label={$t("commit.messageLabel")}
  ></textarea>

  <div class="options">
    <label class="opt" title={$t("commit.amendTitle")}>
      <input type="checkbox" checked={amend} disabled={amendBusy} onchange={(e) => setAmend(e.currentTarget.checked)} />
      <span>{$t("commit.amend")}</span>
    </label>
    <label class="opt" title={$t("commit.signoffTitle")}>
      <input type="checkbox" checked={signoff} onchange={toggleSignoff} />
      <span>{$t("commit.signoff")}</span>
    </label>
    <label class="opt" class:skipping={skipHooks} title={$t("commit.skipHooksTitle")}>
      <input type="checkbox" bind:checked={skipHooks} aria-label={$t("commit.skipHooks")} />
      <span>{$t("commit.skipHooks")}</span>
    </label>
    {#if summaryLength > 0}
      <span
        class="summary-hint"
        class:warn={summaryLength > 50}
        class:over={summaryLength > 72}
        title={$t("commit.summaryLength")}
      >
        {summaryLength}
      </span>
    {/if}
  </div>

  <div class="commit-actions">
    <button class="commit-btn" onclick={handleCommit} disabled={!canCommit}>
      {#if loading || amendBusy}
        <Loader2 size={14} class="spinner" />
      {:else if amend}
        <PenLine size={14} />
      {:else}
        <Send size={14} />
      {/if}
      {#if amend}
        <span>{status.staged.length > 0 ? $t("commit.amendButtonCount", { count: status.staged.length }) : $t("commit.amendButton")}</span>
      {:else}
        <span>{status.staged.length > 0 ? $t("commit.buttonCount", { count: status.staged.length }) : $t("commit.button")}</span>
      {/if}
    </button>
    {#if hasUnpushed && !amend}
      <button class="undo-btn" onclick={handleUndoCommit} title={$t("commit.undoTitle")}>
        <Undo2 size={14} />
      </button>
    {/if}
  </div>

  {#if hookOutput}
    <div class="hook-output" role="status" aria-label={$t("commit.hookOutput")}>
      <div class="hook-head">
        <span title={$t("commit.hookOutputTitle")}>{$t("commit.hookOutputHead", { hooks: hookOutput.hooks.join(", ") })}</span>
        <button class="hook-close" onclick={() => (hookOutput = null)} title={$t("common.dismiss")} aria-label={$t("commit.hookOutputDismiss")}>
          <X size={12} />
        </button>
      </div>
      <pre>{hookOutput.text}</pre>
    </div>
  {/if}
</div>

{#if repoPath}
  <CoAuthorPicker open={showCoAuthor} {repoPath} onclose={() => (showCoAuthor = false)} onpick={addCoAuthor} />
{/if}

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<style>
  .commit-box {
    margin-top: auto;
    padding: 8px;
    border-top: 1px solid var(--color-border);
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex-shrink: 0;
  }

  .helpers {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .cc-type,
  .cc-scope {
    height: 22px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 11px;
    font-family: var(--font-mono);
    outline: none;
    box-sizing: border-box;
  }

  .cc-type {
    width: 76px;
    padding: 0 2px;
  }

  .cc-scope {
    flex: 1;
    min-width: 40px;
    max-width: 110px;
    padding: 0 6px;
  }

  .cc-type:focus,
  .cc-scope:focus {
    border-color: var(--color-accent);
  }

  .cc-scope:disabled {
    opacity: 0.5;
  }

  .cc-scope::placeholder {
    color: var(--color-text-muted);
  }

  .helper-btns {
    display: flex;
    gap: 2px;
    margin-left: auto;
  }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border: none;
    border-radius: 3px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 0;
  }

  .icon-btn:hover {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
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

  .commit-input.amending {
    border-color: var(--color-lane-2);
  }

  .commit-input::placeholder {
    color: var(--color-text-muted);
  }

  .options {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 11px;
    color: var(--color-text-muted);
  }

  .opt {
    display: flex;
    align-items: center;
    gap: 4px;
    cursor: pointer;
    user-select: none;
  }

  .opt input {
    margin: 0;
    accent-color: var(--color-accent);
    cursor: pointer;
  }

  .opt:hover {
    color: var(--color-text-primary);
  }

  .opt.skipping {
    color: var(--color-diff-del-text);
  }

  .hook-output {
    margin: 0;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    font-size: 11px;
  }

  .hook-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 6px 4px 8px;
    color: var(--color-text-muted);
    border-bottom: 1px solid var(--color-border);
  }

  .hook-close {
    display: flex;
    background: none;
    border: none;
    padding: 2px;
    color: var(--color-text-muted);
    cursor: pointer;
  }

  .hook-close:hover {
    color: var(--color-text-primary);
  }

  .hook-output pre {
    margin: 0;
    padding: 6px 8px;
    max-height: 160px;
    overflow: auto;
    font-family: var(--font-mono);
    white-space: pre-wrap;
    word-break: break-word;
    color: var(--color-text-primary);
  }

  .summary-hint {
    margin-left: auto;
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

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
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
</style>
