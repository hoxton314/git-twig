<script lang="ts">
  import {
    GitBranch,
    GitMerge,
    Globe,
    Check,
    Plus,
    RefreshCw,
    Trash2,
    ChevronDown,
    ChevronRight,
    ArrowUp,
    ArrowDown,
    Search,
    Folder,
    FolderOpen,
    Server,
  } from "lucide-svelte";
  import { tick } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { activeRepoPath, activeRepo } from "../../lib/stores/repos";
  import { branches, refreshAll } from "../../lib/stores/graph";
  import { settings } from "../../lib/stores/settings";
  import { remotes, remotesDialogOpen, branchListRequest, loadRemotes } from "../../lib/stores/remotes";
  import { toast, toastError } from "../../lib/stores/toasts";
  import {
    buildBranchTree,
    loadCollapsed,
    saveCollapsed,
    copyText,
    type BranchTreeNode,
  } from "../../lib/branchTree";
  import * as tauri from "../../lib/tauri";
  import type { BranchInfo, CommandResult } from "../../lib/types/git";
  import Modal from "../shared/Modal.svelte";
  import ContextMenu, { type MenuItem } from "../shared/ContextMenu.svelte";
  import BranchNameDialog from "./BranchNameDialog.svelte";
  import SetUpstreamDialog from "./SetUpstreamDialog.svelte";
  import CompareDialog from "./CompareDialog.svelte";
  import { shortcutLabels, withShortcut } from "../../lib/keybindings";
  import { trackOperation } from "../../lib/stores/operations";

  const repoPath = $derived($activeRepoPath);
  const allBranches = $derived($branches);

  let filter = $state("");
  let filterEl: HTMLInputElement | undefined = $state(undefined);
  const filterLc = $derived(filter.trim().toLowerCase());
  const matches = (b: BranchInfo) => !filterLc || b.name.toLowerCase().includes(filterLc);

  const allLocal = $derived(allBranches.filter((b) => !b.is_remote));
  const allRemote = $derived(allBranches.filter((b) => b.is_remote));
  const localBranches = $derived(allLocal.filter(matches));
  const remoteBranches = $derived(allRemote.filter(matches));

  const currentBranch = $derived(allLocal.find((b) => b.is_head) ?? null);
  const defaultBranch = $derived(
    allLocal.find((b) => b.name === "main")?.name ??
    allLocal.find((b) => b.name === "master")?.name ??
    null
  );
  /** Detached HEAD: repo has commits but no local branch is checked out. */
  const detachedAt = $derived(
    allBranches.length > 0 && !currentBranch && $activeRepo && !$activeRepo.is_empty
      ? $activeRepo.head_name
      : null
  );

  // ── Folder tree ───────────────────────────────────────────────────────

  const localTree = $derived(buildBranchTree(localBranches, (b) => b.name, "local"));

  /** Remote branches grouped by the remote name reported by the backend. */
  const remoteGroups = $derived.by(() => {
    const names = new Set($remotes.map((r) => r.name));
    for (const b of remoteBranches) names.add(b.remote_name ?? "");
    return [...names]
      .sort((a, b) => a.localeCompare(b))
      .map((name) => {
        const list = remoteBranches.filter((b) => (b.remote_name ?? "") === name);
        const key = `remote:${name}`;
        return { name, key, count: list.length, tree: buildBranchTree(list, (b) => b.short_name, key) };
      })
      // While filtering, hide remotes without matches.
      .filter((g) => !filterLc || g.count > 0);
  });

  const LOCAL_KEY = "section:local";
  const REMOTE_KEY = "section:remote";

  let collapsed = $state<Set<string>>(new Set());
  let collapsedFor: string | null = null;
  $effect(() => {
    const path = repoPath;
    if (path && path !== collapsedFor) {
      collapsedFor = path;
      collapsed = loadCollapsed(path);
    }
  });

  /** Filtering expands everything so matches inside collapsed folders show. */
  function isCollapsed(key: string): boolean {
    return !filterLc && collapsed.has(key);
  }

  function toggle(key: string) {
    if (filterLc) return;
    const next = new Set(collapsed);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    collapsed = next;
    if (repoPath) saveCollapsed(repoPath, next);
  }

  const rowPad = (depth: number) => 24 + depth * 12;

  // ── State ─────────────────────────────────────────────────────────────

  let createInputEl: HTMLInputElement | undefined = $state(undefined);
  $effect(() => {
    if (showCreateInput) createInputEl?.focus();
  });

  let newBranchName = $state("");
  let showCreateInput = $state(false);
  let loading = $state(false);

  // Drag-and-drop state
  let dragSource = $state<BranchInfo | null>(null);
  let dragTarget = $state<string | null>(null);

  // Merge dialog state
  let mergeDialog = $state<{ source: string; target: string; targetIsHead: boolean } | null>(null);
  let merging = $state(false);

  // Context / drop menu
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  // Dialogs
  let nameDialog = $state<
    | { mode: "rename"; branch: BranchInfo }
    | { mode: "create"; startPoint: string }
    | null
  >(null);
  let upstreamFor = $state<BranchInfo | null>(null);
  let comparePair = $state<{ base: string; other: string } | null>(null);

  // Load branches when repo changes
  let lastLoadedPath: string | null = null;
  $effect(() => {
    const path = repoPath;
    if (path && path !== lastLoadedPath) {
      lastLoadedPath = path;
      loadBranches(path);
    }
  });

  async function loadBranches(path: string) {
    try {
      const result = await tauri.getBranches(path);
      // Drop stale results if the user switched repos meanwhile.
      if ($activeRepoPath !== path) return;
      $branches = result;
    } catch (err) {
      console.error("Failed to load branches:", err);
    }
  }

  // Requests from global actions (command palette / shortcuts).
  let lastRequestSeq = 0;
  $effect(() => {
    const req = $branchListRequest;
    if (!req || req.seq === lastRequestSeq) return;
    lastRequestSeq = req.seq;
    if (req.kind === "create") {
      showCreateInput = true;
      tick().then(() => createInputEl?.focus());
    } else if (req.kind === "filter") {
      tick().then(() => {
        filterEl?.focus();
        filterEl?.select();
      });
    } else if (req.kind === "rename_current") {
      if (currentBranch) nameDialog = { mode: "rename", branch: currentBranch };
      else toast("info", "No branch is checked out.");
    }
  });

  // ── Helpers ───────────────────────────────────────────────────────────

  function confirmDestructive(text: string, title: string): Promise<boolean> {
    if (!$settings.confirm_destructive_ops) return Promise.resolve(true);
    return ask(text, { title, kind: "warning" });
  }

  /** Tooltip with tracking info for a branch row. */
  function branchTitle(b: BranchInfo): string {
    const tracking = b.upstream ? `\nTracking ${b.upstream}` : b.is_remote ? "" : "\nNo upstream";
    return `${b.name} — ${b.last_commit_summary}${tracking}`;
  }

  /**
   * Run a write op against the active repo, refresh, and toast the outcome.
   * `success: null` suppresses the success toast (the UI change is enough).
   */
  async function runOp(
    title: string,
    op: (path: string) => Promise<CommandResult>,
    opts: { success?: string | null } = {},
  ): Promise<CommandResult | null> {
    const path = repoPath;
    if (!path || loading) return null;
    loading = true;
    try {
      const result = await op(path);
      await refreshAll(path);
      if (!result.success) toast("error", result.message, { title });
      else if (opts.success !== null) toast("success", opts.success ?? result.message);
      return result;
    } catch (err) {
      toastError(title, err);
      return null;
    } finally {
      loading = false;
    }
  }

  async function copyName(name: string) {
    try {
      await copyText(name);
      toast("info", `Copied "${name}"`, { duration: 1500 });
    } catch (err) {
      toastError("Copy Failed", err);
    }
  }

  // ── Operations ────────────────────────────────────────────────────────

  function handleCheckout(branch: BranchInfo) {
    if (branch.is_head) return;
    // Remote branches get a local tracking branch instead of a detached HEAD.
    runOp(
      "Checkout Failed",
      (p) =>
        branch.is_remote && branch.remote_name
          ? tauri.checkoutRemoteTracking(p, branch.remote_name, branch.short_name)
          : branch.is_remote
            ? tauri.checkoutRemoteBranch(p, branch.name)
            : tauri.checkoutBranch(p, branch.name),
      { success: null },
    );
  }

  async function handleCreateBranch() {
    const name = newBranchName.trim();
    if (!name) return;
    const result = await runOp("Create Branch Failed", (p) => tauri.createBranch(p, name), { success: null });
    if (result?.success) {
      newBranchName = "";
      showCreateInput = false;
    }
  }

  async function restoreBranch(path: string, branch: BranchInfo) {
    try {
      const result = await tauri.createBranchAt(path, branch.name, branch.oid);
      if (!result.success) {
        toast("error", result.message, { title: "Undo Failed" });
        return;
      }
      if (branch.upstream) {
        // Best effort: the upstream may be gone by now.
        await tauri.setBranchUpstream(path, branch.name, branch.upstream).catch(() => null);
      }
      if ($activeRepoPath === path) await refreshAll(path);
      toast("success", `Restored branch "${branch.name}"`);
    } catch (err) {
      toastError("Undo Failed", err);
    }
  }

  async function deleteLocal(branch: BranchInfo) {
    const path = repoPath;
    if (!path || branch.is_head || loading) return;
    const ok = await confirmDestructive(`Delete branch "${branch.name}"?`, "Delete Branch");
    if (!ok) return;
    loading = true;
    try {
      let result = await tauri.deleteBranch(path, branch.name, false);
      // `git branch -d` refuses to delete a branch it can't confirm is merged
      // (common when the upstream was deleted). Offer a force delete (`-D`).
      if (!result.success && /not fully merged/i.test(result.message)) {
        const force = await ask(
          `Branch "${branch.name}" is not fully merged. Deleting it may discard commits that exist only on this branch.\n\nForce delete anyway? (You can undo right after.)`,
          { title: "Branch not fully merged", kind: "warning" },
        );
        if (!force) return;
        result = await tauri.deleteBranch(path, branch.name, true);
      }
      await refreshAll(path);
      if (!result.success) {
        toast("error", result.message, { title: "Delete Failed" });
        return;
      }
      toast("success", `Deleted branch "${branch.name}" (was ${branch.short_oid})`, {
        duration: 10000,
        action: { label: "Undo", run: () => restoreBranch(path, branch) },
      });
    } catch (err) {
      toastError("Delete Failed", err);
    } finally {
      loading = false;
    }
  }

  async function deleteRemote(branch: BranchInfo) {
    if (!branch.remote_name) return;
    const remote = branch.remote_name;
    const name = branch.short_name;
    // Always confirm: this changes shared state other people depend on.
    const ok = await ask(
      `This will permanently delete "${name}" from the remote "${remote}". Anyone using this branch will lose it on their next fetch.\n\nDelete remote branch?`,
      { title: "Delete Remote Branch", kind: "warning" },
    );
    if (!ok) return;
    runOp("Delete Failed", (p) => tauri.deleteRemoteBranch(p, remote, name), {
      success: `Deleted "${name}" on ${remote}`,
    });
  }

  /** Rebase `branch` (current branch when omitted) onto `onto`. */
  async function rebase(onto: string, branch?: string) {
    const path = repoPath;
    if (!path || loading) return;
    const subject = branch ?? currentBranch?.name ?? "HEAD";
    const switching = branch && branch !== currentBranch?.name;
    const ok = await confirmDestructive(
      `Rebase "${subject}" onto "${onto}"?\n\nThis rewrites the commits of "${subject}". If they were already pushed, you will need to force-push.${switching ? `\n\n"${subject}" will be checked out.` : ""}`,
      "Rebase Branch",
    );
    if (!ok) return;
    loading = true;
    try {
      const result = await tauri.rebaseBranch(path, onto, branch);
      await refreshAll(path);
      if (result.success) {
        toast("success", `Rebased "${subject}" onto "${onto}"`);
      } else if (/conflict|could not apply|resolve all conflicts/i.test(result.message)) {
        // The operation banner (conflicts/) offers resolve, continue and abort.
        toast("warning", "Resolve the conflicts, then continue or abort from the banner.", {
          title: "Rebase stopped on conflicts",
        });
      } else {
        toast("error", result.message, { title: "Rebase Failed" });
      }
    } catch (err) {
      toastError("Rebase Failed", err);
    } finally {
      loading = false;
    }
  }

  async function pullBranch(b: BranchInfo) {
    if (!b.is_head) {
      runOp("Fast-forward Failed", (p) => tauri.fastForwardBranch(p, b.name));
      return;
    }
    const up = allRemote.find((r) => r.name === b.upstream);
    const result = await runOp(
      "Pull Failed",
      (p) => (up?.remote_name ? tauri.pull(p, up.remote_name, up.short_name) : tauri.pull(p)),
      { success: null },
    );
    if (result?.success) {
      if (result.message.includes("conflicts")) toast("warning", result.message, { title: "Pull — Stash Conflicts" });
      else toast("success", `Pulled "${b.name}"`);
    }
  }

  function push(b: BranchInfo, remote?: string) {
    runOp("Push Failed", (p) => tauri.pushLocalBranch(p, b.name, remote));
  }

  async function confirmNameDialog(name: string, checkout: boolean) {
    const d = nameDialog;
    if (!d) return;
    if (d.mode === "rename") {
      const old = d.branch.name;
      const result = await runOp("Rename Failed", (p) => tauri.renameBranch(p, old, name), {
        success: `Renamed "${old}" to "${name}"`,
      });
      if (result?.success) nameDialog = null;
    } else {
      const start = d.startPoint;
      const result = await runOp(
        "Create Branch Failed",
        (p) => (checkout ? tauri.createBranch(p, name, start) : tauri.createBranchAt(p, name, start)),
        { success: `Created "${name}" from "${start}"` },
      );
      if (result?.success) nameDialog = null;
    }
  }

  async function confirmUpstream(upstream: string) {
    const b = upstreamFor;
    if (!b) return;
    const result = await runOp("Set Upstream Failed", (p) => tauri.setBranchUpstream(p, b.name, upstream));
    if (result?.success) upstreamFor = null;
  }

  function remoteRun(title: string, op: (p: string) => Promise<CommandResult>) {
    runOp(title, op).then(() => loadRemotes());
  }

  // ── Menus ─────────────────────────────────────────────────────────────

  /** Open the merge confirmation for merging `source` into the current branch. */
  function openMerge(source: string) {
    if (currentBranch) mergeDialog = { source, target: currentBranch.name, targetIsHead: true };
  }

  function openCompare(other: string) {
    if (currentBranch) comparePair = { base: currentBranch.name, other };
  }

  function pushItems(b: BranchInfo): MenuItem[] {
    if (b.upstream) return [{ label: `Push to ${b.upstream}`, action: () => push(b) }];
    if ($remotes.length === 0) return [{ label: "Push (no remotes)", disabled: true }];
    return $remotes.map((r) => ({
      label: `Push to ${r.name} (set upstream)`,
      action: () => push(b, r.name),
    }));
  }

  function localMenu(b: BranchInfo): MenuItem[] {
    const cur = currentBranch;
    const other = !b.is_head && !!cur;
    return [
      { label: "Checkout", action: () => handleCheckout(b), disabled: b.is_head },
      { separator: true },
      { label: cur ? `Merge "${b.name}" into "${cur.name}"` : "Merge into current", disabled: !other,
        action: () => openMerge(b.name) },
      { label: cur ? `Rebase "${cur.name}" onto "${b.name}"` : "Rebase current onto this", disabled: !other,
        action: () => rebase(b.name) },
      { separator: true },
      { label: b.is_head ? "Pull" : "Pull (fast-forward)", disabled: !b.upstream, action: () => pullBranch(b) },
      ...pushItems(b),
      { label: "Set upstream…", action: () => { upstreamFor = b; } },
      { label: "Unset upstream", disabled: !b.upstream,
        action: () => void runOp("Unset Upstream Failed", (p) => tauri.unsetBranchUpstream(p, b.name)) },
      { separator: true },
      { label: "Create branch from here…", action: () => { nameDialog = { mode: "create", startPoint: b.name }; } },
      { label: "Rename…", shortcut: "F2", action: () => { nameDialog = { mode: "rename", branch: b }; } },
      { label: cur ? `Compare with "${cur.name}"` : "Compare with current", disabled: !other,
        action: () => openCompare(b.name) },
      { label: "Copy name", action: () => copyName(b.name) },
      { separator: true },
      { label: "Delete…", shortcut: "Del", danger: true, disabled: b.is_head, action: () => deleteLocal(b) },
    ];
  }

  function remoteMenu(b: BranchInfo): MenuItem[] {
    const cur = currentBranch;
    return [
      { label: `Checkout as "${b.short_name}"`, action: () => handleCheckout(b) },
      { separator: true },
      { label: cur ? `Merge "${b.name}" into "${cur.name}"` : "Merge into current", disabled: !cur,
        action: () => openMerge(b.name) },
      { label: cur ? `Rebase "${cur.name}" onto "${b.name}"` : "Rebase current onto this", disabled: !cur,
        action: () => rebase(b.name) },
      { separator: true },
      { label: "Create branch from here…", action: () => { nameDialog = { mode: "create", startPoint: b.name }; } },
      { label: cur ? `Compare with "${cur.name}"` : "Compare with current", disabled: !cur,
        action: () => openCompare(b.name) },
      { label: "Copy name", action: () => copyName(b.name) },
      { separator: true },
      { label: `Delete from ${b.remote_name ?? "remote"}…`, danger: true, disabled: !b.remote_name,
        action: () => deleteRemote(b) },
    ];
  }

  function remoteGroupMenu(name: string): MenuItem[] {
    const info = $remotes.find((r) => r.name === name);
    return [
      { label: `Fetch ${name}`, disabled: !info, action: () => remoteRun("Fetch Failed", (p) => tauri.fetchRemote(p, name)) },
      { label: "Prune stale branches", disabled: !info,
        action: () => remoteRun("Prune Failed", (p) => tauri.pruneRemote(p, name)) },
      { label: "Copy URL", disabled: !info?.fetch_url, action: () => info?.fetch_url && copyName(info.fetch_url) },
      { separator: true },
      { label: "Manage remotes…", action: () => remotesDialogOpen.set(true) },
    ];
  }

  /** Mouse position, or the element's corner for keyboard-invoked menus. */
  function menuPoint(e: MouseEvent | KeyboardEvent): { x: number; y: number } {
    if (e instanceof MouseEvent && (e.clientX !== 0 || e.clientY !== 0)) return { x: e.clientX, y: e.clientY };
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    return { x: rect.left + 24, y: rect.bottom };
  }

  function openMenu(e: MouseEvent | KeyboardEvent, items: MenuItem[]) {
    e.preventDefault();
    e.stopPropagation();
    menu = { ...menuPoint(e), items };
  }

  function branchMenu(e: MouseEvent | KeyboardEvent, b: BranchInfo) {
    openMenu(e, b.is_remote ? remoteMenu(b) : localMenu(b));
  }

  /** Row keys: Enter/Space checkout, Shift+F10/ContextMenu menu, F2 rename, Del delete. */
  function handleRowKeydown(e: KeyboardEvent, branch: BranchInfo) {
    if (e.target !== e.currentTarget) return;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      handleCheckout(branch);
    } else if (e.key === "ContextMenu" || (e.key === "F10" && e.shiftKey)) {
      branchMenu(e, branch);
    } else if (e.key === "F2" && !branch.is_remote) {
      e.preventDefault();
      nameDialog = { mode: "rename", branch };
    } else if (e.key === "Delete") {
      e.preventDefault();
      if (branch.is_remote) deleteRemote(branch);
      else deleteLocal(branch);
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") handleCreateBranch();
    if (e.key === "Escape") {
      showCreateInput = false;
      newBranchName = "";
    }
  }

  // ── Merge default branch into current ─────────────────────────────────

  function handleMergeDefault(e: MouseEvent) {
    e.stopPropagation();
    if (!defaultBranch || !currentBranch) return;
    mergeDialog = {
      source: defaultBranch,
      target: currentBranch.name,
      targetIsHead: true,
    };
  }

  // ── Drag and drop ─────────────────────────────────────────────────────

  function handleDragStart(e: DragEvent, branch: BranchInfo) {
    if (!e.dataTransfer) return;
    dragSource = branch;
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", branch.name);
  }

  function handleDragOver(e: DragEvent, branch: BranchInfo) {
    if (!dragSource || dragSource.name === branch.name || branch.is_remote) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    dragTarget = branch.name;
  }

  function handleDragLeave(e: DragEvent, branch: BranchInfo) {
    // dragleave also fires when moving onto a child (icon, name, badge);
    // only clear when the pointer really left the row, to avoid flicker.
    const related = e.relatedTarget as Node | null;
    if (related && (e.currentTarget as HTMLElement).contains(related)) return;
    if (dragTarget === branch.name) dragTarget = null;
  }

  function handleDrop(e: DragEvent, target: BranchInfo) {
    e.preventDefault();
    const source = dragSource;
    dragSource = null;
    dragTarget = null;
    if (!source || source.name === target.name || target.is_remote) return;

    const items: MenuItem[] = [
      { label: `Merge "${source.name}" into "${target.name}"`,
        action: () => { mergeDialog = { source: source.name, target: target.name, targetIsHead: target.is_head }; } },
      { label: `Rebase "${source.name}" onto "${target.name}"`, disabled: source.is_remote,
        action: () => rebase(target.name, source.name) },
      { label: `Rebase "${target.name}" onto "${source.name}"`,
        action: () => rebase(source.name, target.name) },
    ];
    menu = { x: e.clientX, y: e.clientY, items };
  }

  function handleDragEnd() {
    dragSource = null;
    dragTarget = null;
  }

  // ── Execute merge ─────────────────────────────────────────────────────

  async function executeMerge() {
    if (!repoPath || !mergeDialog) return;
    const path = repoPath;
    const { source, target, targetIsHead } = mergeDialog;
    merging = true;

    try {
      // Checkout target first if it's not the current HEAD
      if (!targetIsHead) {
        const checkout = await tauri.checkoutBranch(path, target);
        if (!checkout.success) {
          mergeDialog = null;
          toast("error", checkout.message, { title: "Checkout Failed" });
          return;
        }
      }

      const result = await tauri.mergeBranch(path, source);
      mergeDialog = null;

      // Refresh first so the UI reflects any conflict markers, then report.
      await refreshAll(path);

      if (!result.success) toast("error", result.message, { title: "Merge Failed" });
      else toast("success", `Merged "${source}" into "${target}"`);
    } catch (err) {
      mergeDialog = null;
      toastError("Merge Failed", err);
    } finally {
      merging = false;
    }
  }

  async function handleFetch() {
    const result = await runOp(
      "Fetch Failed",
      (p) => trackOperation(p, "fetch", "Fetching…", () => tauri.fetchAll(p)),
      { success: null },
    );
    if (result?.success) toast("success", "Fetched all remotes", { duration: 2000 });
  }
</script>

{#snippet branchRow(branch: BranchInfo, label: string, depth: number)}
  <div
    class="branch-item"
    class:remote={branch.is_remote}
    class:active={branch.is_head}
    class:drag-over={dragTarget === branch.name}
    class:dragging={dragSource?.name === branch.name}
    style="padding-left: {rowPad(depth)}px"
    draggable="true"
    ondragstart={(e) => handleDragStart(e, branch)}
    ondragover={(e) => handleDragOver(e, branch)}
    ondragleave={(e) => handleDragLeave(e, branch)}
    ondrop={(e) => handleDrop(e, branch)}
    ondragend={() => handleDragEnd()}
    onclick={() => handleCheckout(branch)}
    oncontextmenu={(e) => branchMenu(e, branch)}
    onkeydown={(e) => handleRowKeydown(e, branch)}
    role="button"
    tabindex="0"
    aria-haspopup="menu"
    title={branchTitle(branch)}
  >
    {#if branch.is_head}
      <Check size={12} class="head-icon" />
    {:else}
      <span class="dot"></span>
    {/if}
    <span class="branch-name">{label}</span>
    {#if branch.ahead > 0 || branch.behind > 0}
      <span class="ahead-behind">
        {#if branch.ahead > 0}
          <span class="ab-badge ab-ahead" title="{branch.ahead} ahead of remote">
            <ArrowUp size={10} />{branch.ahead}
          </span>
        {/if}
        {#if branch.behind > 0}
          <span class="ab-badge ab-behind" title="{branch.behind} behind remote">
            <ArrowDown size={10} />{branch.behind}
          </span>
        {/if}
      </span>
    {/if}
    {#if branch.is_head && defaultBranch && branch.name !== defaultBranch}
      <button
        class="merge-btn"
        onclick={(e) => handleMergeDefault(e)}
        title="Merge {defaultBranch} into {branch.name}"
        aria-label="Merge {defaultBranch} into {branch.name}"
      >
        <GitMerge size={12} />
      </button>
    {/if}
    {#if branch.is_remote}
      <button
        class="delete-btn"
        onclick={(e) => {
          e.stopPropagation();
          deleteRemote(branch);
        }}
        title="Delete branch on remote"
        aria-label="Delete remote branch {branch.name}"
      >
        <Trash2 size={12} />
      </button>
    {:else if !branch.is_head}
      <button
        class="delete-btn"
        onclick={(e) => {
          e.stopPropagation();
          deleteLocal(branch);
        }}
        title="Delete branch"
        aria-label="Delete branch {branch.name}"
      >
        <Trash2 size={12} />
      </button>
    {/if}
  </div>
{/snippet}

{#snippet tree(nodes: BranchTreeNode[], depth: number)}
  {#each nodes as node (node.key)}
    {#if node.kind === "folder"}
      {@const open = !isCollapsed(node.key)}
      <button
        class="folder-row"
        style="padding-left: {rowPad(depth) - 4}px"
        onclick={() => toggle(node.key)}
        aria-expanded={open}
        title={node.key.slice(node.key.indexOf(":") + 1)}
      >
        {#if open}
          <ChevronDown size={12} />
          <FolderOpen size={12} />
        {:else}
          <ChevronRight size={12} />
          <Folder size={12} />
        {/if}
        <span class="branch-name">{node.label}</span>
        <span class="count">{node.count}</span>
      </button>
      {#if open}
        {@render tree(node.children, depth + 1)}
      {/if}
    {:else}
      {@render branchRow(node.branch, node.label, depth)}
    {/if}
  {/each}
{/snippet}

<div class="branch-list">
  <div class="section-header">
    <span class="section-title">Branches</span>
    <div class="header-actions">
      <button class="icon-btn" onclick={() => (showCreateInput = !showCreateInput)} title="New branch" aria-label="New branch">
        <Plus size={14} />
      </button>
      <button class="icon-btn" onclick={() => remotesDialogOpen.set(true)} title="Manage remotes" aria-label="Manage remotes">
        <Server size={14} />
      </button>
      <button class="icon-btn" onclick={handleFetch} title={withShortcut("Fetch all", $shortcutLabels["fetch"])} aria-label="Fetch all" disabled={loading}>
        <RefreshCw size={14} class={loading ? "spin" : ""} />
      </button>
    </div>
  </div>

  {#if showCreateInput}
    <div class="create-input">
      <input
        type="text"
        placeholder="New branch name..."
        aria-label="New branch name"
        bind:this={createInputEl}
        bind:value={newBranchName}
        onkeydown={handleKeydown}
      />
    </div>
  {/if}

  <div class="filter-input">
    <Search size={12} />
    <input
      type="text"
      placeholder="Filter branches..."
      aria-label="Filter branches"
      bind:this={filterEl}
      bind:value={filter}
      onkeydown={(e) => e.key === "Escape" && (filter = "")}
    />
  </div>

  {#if detachedAt}
    <div class="detached" title="HEAD is not on a branch. Create a branch to keep new commits.">
      <span class="dot"></span>
      <span class="branch-name">HEAD detached at {detachedAt}</span>
    </div>
  {/if}

  <!-- Local branches -->
  <button class="group-header" onclick={() => toggle(LOCAL_KEY)} aria-expanded={!isCollapsed(LOCAL_KEY)}>
    {#if !isCollapsed(LOCAL_KEY)}
      <ChevronDown size={14} />
    {:else}
      <ChevronRight size={14} />
    {/if}
    <GitBranch size={14} />
    <span>Local</span>
    <span class="count">{localBranches.length}</span>
  </button>

  {#if !isCollapsed(LOCAL_KEY)}
    {#if localBranches.length === 0}
      <div class="empty">{filterLc ? "No matching branches" : "No branches yet"}</div>
    {:else}
      {@render tree(localTree, 0)}
    {/if}
  {/if}

  <!-- Remote branches, grouped by remote -->
  <button class="group-header" onclick={() => toggle(REMOTE_KEY)} aria-expanded={!isCollapsed(REMOTE_KEY)}>
    {#if !isCollapsed(REMOTE_KEY)}
      <ChevronDown size={14} />
    {:else}
      <ChevronRight size={14} />
    {/if}
    <Globe size={14} />
    <span>Remote</span>
    <span class="count">{remoteBranches.length}</span>
  </button>

  {#if !isCollapsed(REMOTE_KEY)}
    {#each remoteGroups as group (group.key)}
      {@const open = !isCollapsed(group.key)}
      <button
        class="folder-row remote-root"
        style="padding-left: {rowPad(0) - 4}px"
        onclick={() => toggle(group.key)}
        oncontextmenu={(e) => openMenu(e, remoteGroupMenu(group.name))}
        aria-expanded={open}
        aria-haspopup="menu"
        title={$remotes.find((r) => r.name === group.name)?.fetch_url ?? group.name}
      >
        {#if open}
          <ChevronDown size={12} />
        {:else}
          <ChevronRight size={12} />
        {/if}
        <Server size={12} />
        <span class="branch-name">{group.name || "(unknown remote)"}</span>
        <span class="count">{group.count}</span>
      </button>
      {#if open}
        {#if group.count === 0}
          <div class="empty" style="padding-left: {rowPad(1)}px">No branches — fetch to update</div>
        {:else}
          {@render tree(group.tree, 1)}
        {/if}
      {/if}
    {:else}
      <div class="empty">
        {#if filterLc}
          No matching branches
        {:else}
          No remotes. <button class="link" onclick={() => remotesDialogOpen.set(true)}>Add a remote…</button>
        {/if}
      </div>
    {/each}
  {/if}
</div>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<BranchNameDialog
  open={!!nameDialog}
  title={nameDialog?.mode === "rename" ? "Rename Branch" : "Create Branch"}
  description={nameDialog?.mode === "rename"
    ? `Rename "${nameDialog.branch.name}". Its upstream configuration is kept.`
    : nameDialog
      ? `Create a branch starting at "${nameDialog.startPoint}".`
      : ""}
  initial={nameDialog?.mode === "rename" ? nameDialog.branch.name : ""}
  confirmLabel={nameDialog?.mode === "rename" ? "Rename" : "Create"}
  showCheckout={nameDialog?.mode === "create"}
  existing={allLocal.map((b) => b.name)}
  busy={loading}
  onconfirm={confirmNameDialog}
  onclose={() => (nameDialog = null)}
/>

<SetUpstreamDialog
  branch={upstreamFor}
  remoteBranches={allRemote}
  busy={loading}
  onconfirm={confirmUpstream}
  onclose={() => (upstreamFor = null)}
/>

<CompareDialog pair={comparePair} onclose={() => (comparePair = null)} />

<!-- Merge confirmation modal -->
<Modal open={!!mergeDialog} title="Merge Branch" onclose={() => (mergeDialog = null)} width="360px">
  {#if mergeDialog}
    <div class="merge-dialog">
      <p class="merge-desc">
        Merge <strong>{mergeDialog.source}</strong> into <strong>{mergeDialog.target}</strong>?
      </p>
      {#if !mergeDialog.targetIsHead}
        <p class="merge-warning">This will checkout <strong>{mergeDialog.target}</strong> first.</p>
      {/if}
      <div class="merge-actions">
        <button class="btn-merge" onclick={executeMerge} disabled={merging}>
          {#if merging}Merging...{:else}Merge{/if}
        </button>
        <button class="btn-cancel" onclick={() => (mergeDialog = null)} disabled={merging}>
          Cancel
        </button>
      </div>
    </div>
  {/if}
</Modal>

<style>
  .branch-list {
    display: flex;
    flex-direction: column;
    padding: 0;
    font-size: 12px;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px 6px;
  }

  .section-title {
    font-weight: 600;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--color-text-muted);
  }

  .header-actions {
    display: flex;
    gap: 2px;
  }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 0;
  }

  .icon-btn:hover {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .icon-btn :global(.spin) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .create-input {
    padding: 4px 12px 8px;
  }

  .create-input input {
    width: 100%;
    padding: 4px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 12px;
    outline: none;
    box-sizing: border-box;
  }

  .create-input input:focus {
    border-color: var(--color-accent);
  }

  .filter-input {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 12px 6px;
    padding: 0 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-muted);
  }

  .filter-input:focus-within {
    border-color: var(--color-accent);
  }

  .filter-input input {
    flex: 1;
    min-width: 0;
    padding: 4px 0;
    border: none;
    background: transparent;
    color: var(--color-text-primary);
    font-size: 12px;
    outline: none;
  }

  .detached {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 12px 4px 32px;
    color: var(--color-lane-2);
    font-style: italic;
  }

  .group-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    color: var(--color-text-muted);
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    border: none;
    background: none;
    text-align: left;
    width: 100%;
  }

  .group-header:hover {
    color: var(--color-text-primary);
    background: var(--color-surface-elevated);
  }

  .folder-row {
    display: flex;
    align-items: center;
    gap: 5px;
    width: 100%;
    padding-top: 4px;
    padding-bottom: 4px;
    padding-right: 12px;
    border: none;
    background: none;
    color: var(--color-text-muted);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .folder-row:hover {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .folder-row.remote-root {
    font-weight: 500;
  }

  .count {
    margin-left: auto;
    opacity: 0.5;
  }

  .empty {
    padding: 4px 12px 4px 32px;
    color: var(--color-text-muted);
    font-style: italic;
  }

  .link {
    padding: 0;
    border: none;
    background: none;
    color: var(--color-accent);
    font: inherit;
    font-style: normal;
    cursor: pointer;
  }

  .link:hover {
    text-decoration: underline;
  }

  .branch-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px 5px 24px;
    color: var(--color-text-primary);
    cursor: pointer;
    border: none;
    background: none;
    text-align: left;
    width: 100%;
    transition: background 0.1s;
  }

  .branch-item:hover {
    background: var(--color-surface-elevated);
  }

  .branch-item.active {
    color: var(--color-accent);
    font-weight: 500;
  }

  .branch-item.remote {
    color: var(--color-text-muted);
  }

  .branch-item.drag-over {
    background: color-mix(in srgb, var(--color-accent) 15%, transparent);
    outline: 1px dashed var(--color-accent);
    outline-offset: -1px;
  }

  .branch-item.dragging {
    opacity: 0.4;
  }

  .branch-item :global(.head-icon) {
    color: var(--color-accent);
    flex-shrink: 0;
  }

  .branch-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-text-muted);
    flex-shrink: 0;
    opacity: 0.4;
  }

  .ahead-behind {
    display: flex;
    gap: 3px;
    flex-shrink: 0;
    margin-left: auto;
  }

  .ab-badge {
    display: inline-flex;
    align-items: center;
    gap: 1px;
    font-size: 10px;
    font-weight: 500;
    padding: 0 3px;
    border-radius: 3px;
    line-height: 16px;
  }

  .ab-ahead {
    color: var(--color-lane-2);
    background: color-mix(in srgb, var(--color-lane-2) 15%, transparent);
  }

  .ab-behind {
    color: var(--color-accent);
    background: color-mix(in srgb, var(--color-accent) 15%, transparent);
  }

  .delete-btn,
  .merge-btn {
    display: none;
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
    flex-shrink: 0;
  }

  .branch-item:hover .delete-btn,
  .branch-item:hover .merge-btn,
  .branch-item:focus-within .delete-btn,
  .branch-item:focus-within .merge-btn {
    display: flex;
  }

  .delete-btn:hover {
    background: var(--color-diff-del-bg);
    color: var(--color-diff-del-text);
  }

  .merge-btn:hover {
    background: color-mix(in srgb, var(--color-accent) 20%, transparent);
    color: var(--color-accent);
  }

  /* ── Merge dialog ─────────────────────────────────────────────────── */

  .merge-dialog {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .merge-desc {
    color: var(--color-text-primary);
    font-size: 13px;
    margin: 0;
    line-height: 1.5;
  }

  .merge-desc strong {
    color: var(--color-accent);
  }

  .merge-warning {
    color: var(--color-lane-2);
    font-size: 12px;
    margin: 0;
    line-height: 1.5;
  }

  .merge-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    margin-top: 4px;
  }

  .btn-merge {
    background: var(--color-accent);
    color: var(--color-bg);
    border: none;
    border-radius: 4px;
    padding: 6px 16px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }

  .btn-merge:hover:not(:disabled) {
    filter: brightness(1.1);
  }

  .btn-merge:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .btn-cancel {
    background: transparent;
    color: var(--color-text-muted);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    padding: 6px 16px;
    font-size: 13px;
    cursor: pointer;
  }

  .btn-cancel:hover:not(:disabled) {
    color: var(--color-text-primary);
  }

  .btn-cancel:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
