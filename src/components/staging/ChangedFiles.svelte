<script lang="ts">
  import { showFileHistory, showBlame } from "../../lib/stores/fileviews";
  import { openConflictResolver } from "../../lib/stores/operation";
  import {
    Plus,
    Minus,
    ChevronDown,
    ChevronRight,
    Trash2,
    ListTree,
    List,
    Search,
    X,
  } from "lucide-svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import FileList, { type MenuTarget, type RowAction } from "./FileList.svelte";
  import ContextMenu, { type MenuItem } from "../shared/ContextMenu.svelte";
  import { activeRepoPath } from "../../lib/stores/repos";
  import {
    workingStatus,
    selectedWorkingFile,
    workingFileDiff,
    selectedCommitOid,
    refreshStatus,
  } from "../../lib/stores/graph";
  import { settings, updateSettings } from "../../lib/stores/settings";
  import { onAction } from "../../lib/keybindings";
  import { toast, toastError } from "../../lib/stores/toasts";
  import * as tauri from "../../lib/tauri";
  import type { FileStatus, IgnoreKind } from "../../lib/types/git";
  import { runWrite, confirmDestructive } from "./writeQueue";
  import { filterFiles, parentDir, extension } from "./fileTree";

  type Area = "staged" | "unstaged";

  const repoPath = $derived($activeRepoPath);
  const status = $derived($workingStatus);
  const selectedFile = $derived($selectedWorkingFile);
  const treeView = $derived($settings.staging_tree_view);

  let unstagedExpanded = $state(true);
  let stagedExpanded = $state(true);
  let filter = $state("");
  let filterInput = $state<HTMLInputElement | null>(null);

  const filtering = $derived(filter.trim().length > 0);
  const unstaged = $derived(filterFiles(status.unstaged, filter));
  const staged = $derived(filterFiles(status.staged, filter));

  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  // Keyboard activation for div[role=button] headers. Ignores keys from
  // nested buttons so pressing Enter on "Stage all" doesn't also collapse.
  function activateOnKey(e: KeyboardEvent, fn: () => void) {
    if (e.target !== e.currentTarget) return;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      fn();
    }
  }

  let selectRequest = 0;
  async function selectFile(file: FileStatus, area: Area) {
    if (!repoPath) return;
    const path = repoPath;
    const req = ++selectRequest;
    // Clear commit selection so the diff viewer shows the working file diff.
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

  const plural = (n: number, word: string) => `${n} ${word}${n !== 1 ? "s" : ""}`;

  async function stage(files: FileStatus[]) {
    if (!repoPath || files.length === 0) return;
    const path = repoPath;
    const conflicted = files.filter((f) => f.status === "conflicted");
    if (conflicted.length > 0 && files.length > 1) {
      const ok = await ask(
        `${plural(conflicted.length, "conflicted file")} will be marked as resolved. Make sure all conflict markers have been removed. Continue?`,
        { title: "Stage Conflicted Files", kind: "warning" },
      );
      if (!ok) return;
    }
    const paths = files.map((f) => f.path);
    await runWrite(() => tauri.stageFiles(path, paths), "Stage Failed");
  }

  async function unstage(files: FileStatus[]) {
    if (!repoPath || files.length === 0) return;
    const path = repoPath;
    const paths = files.map((f) => f.path);
    await runWrite(() => tauri.unstageFiles(path, paths), "Unstage Failed");
  }

  async function discard(files: FileStatus[]) {
    if (!repoPath || files.length === 0) return;
    const path = repoPath;
    const untracked = files.filter((f) => f.is_new).map((f) => f.path);
    const tracked = files.filter((f) => !f.is_new).map((f) => f.path);
    let text: string;
    if (files.length === 1) {
      const f = files[0];
      text = f.is_new
        ? `Delete untracked file "${f.path}"? This cannot be undone.`
        : `Discard changes to "${f.path}"? This cannot be undone.`;
    } else {
      text = `Discard changes to ${plural(tracked.length, "file")}${
        untracked.length > 0 ? ` and delete ${plural(untracked.length, "untracked file")}` : ""
      }? This cannot be undone.`;
    }
    if (!(await confirmDestructive(text, "Discard Changes"))) return;
    await runWrite(() => tauri.discardFiles(path, tracked, untracked), "Discard Failed");
  }

  const unstagedActions: RowAction[] = [
    { icon: Trash2, title: "Discard changes", folderTitle: "Discard changes in folder", tone: "danger", run: discard },
    { icon: Plus, title: "Stage file", folderTitle: "Stage folder", tone: "stage", run: stage },
  ];
  const stagedActions: RowAction[] = [
    { icon: Minus, title: "Unstage file", folderTitle: "Unstage folder", tone: "danger", run: unstage },
  ];

  // ── Context menu ─────────────────────────────────────────────────────

  async function copyText(text: string) {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // Fallback for webviews without async clipboard permission.
      const ta = document.createElement("textarea");
      ta.value = text;
      ta.style.position = "fixed";
      ta.style.opacity = "0";
      document.body.appendChild(ta);
      ta.select();
      const ok = document.execCommand("copy");
      ta.remove();
      if (!ok) {
        toast("error", "Could not access the clipboard", { title: "Copy Failed" });
        return;
      }
    }
    toast("success", `Copied ${text}`, { duration: 2000 });
  }

  function absolutePath(rel: string): string {
    const root = repoPath ?? "";
    const sep = root.includes("\\") && !root.includes("/") ? "\\" : "/";
    const clean = rel.replace(/\/+$/, "");
    const joined = clean ? `${root.replace(/[\\/]+$/, "")}${sep}${clean}` : root;
    return sep === "\\" ? joined.replace(/\//g, "\\") : joined;
  }

  async function openFile(rel: string) {
    if (!repoPath) return;
    try {
      await tauri.openRepoFile(repoPath, rel);
    } catch (err) {
      toastError("Open Failed", err);
    }
  }

  async function revealFile(rel: string) {
    if (!repoPath) return;
    try {
      await tauri.revealRepoFile(repoPath, rel);
    } catch (err) {
      toastError("Reveal Failed", err);
    }
  }

  async function externalDiff(file: FileStatus, area: Area) {
    if (!repoPath) return;
    try {
      await tauri.openExternalDiff(repoPath, file.path, area === "staged", $settings.external_diff_tool);
    } catch (err) {
      toastError("External Diff Failed", err);
    }
  }

  async function ignore(rel: string, kind: IgnoreKind) {
    if (!repoPath) return;
    const path = repoPath;
    try {
      const result = await tauri.addToGitignore(path, rel, kind);
      if (result.already_present) {
        toast("info", `${result.pattern} is already in .gitignore`);
      } else if (result.tracked) {
        toast(
          "warning",
          `Added ${result.pattern} to .gitignore, but matching files are already tracked. Ignore rules only apply to untracked files.`,
          { title: "Already Tracked", duration: 8000 },
        );
      } else {
        toast("success", `Added ${result.pattern} to .gitignore`);
      }
    } catch (err) {
      toastError("Add to .gitignore Failed", err);
    }
    await refreshStatus(path);
  }

  function ignoreItems(rel: string, isFolder: boolean): MenuItem[] {
    const items: MenuItem[] = [];
    if (isFolder) {
      items.push({ label: `Ignore folder "${rel}/"`, action: () => ignore(rel, "folder") });
      return items;
    }
    const name = rel.replace(/\/+$/, "").split("/").pop() ?? rel;
    items.push({ label: `Ignore "${name}"`, action: () => ignore(rel, "path") });
    const ext = rel.endsWith("/") ? null : extension(rel);
    if (ext) items.push({ label: `Ignore all *.${ext} files`, action: () => ignore(rel, "extension") });
    const dir = parentDir(rel);
    if (dir) items.push({ label: `Ignore folder "${dir}/"`, action: () => ignore(dir, "folder") });
    return items;
  }

  function fileMenu(file: FileStatus, area: Area): MenuItem[] {
    const deleted = file.status === "deleted";
    // Untracked files have nothing to compare against.
    const canDiff = !(area === "unstaged" && file.status === "untracked");
    const items: MenuItem[] = [];
    if (area === "unstaged") {
      items.push({ label: "Stage", action: () => stage([file]) });
      items.push({
        label: file.is_new ? "Delete untracked file…" : "Discard changes…",
        danger: true,
        action: () => discard([file]),
      });
    } else {
      items.push({ label: "Unstage", action: () => unstage([file]) });
    }
    if (file.status === "conflicted") {
      items.push({ label: "Resolve conflict…", action: () => openConflictResolver(file.path) });
    }
    items.push({ separator: true });
    // New files have no history yet.
    const tracked = !file.is_new && file.status !== "untracked";
    items.push({ label: "File history", disabled: !tracked, action: () => showFileHistory(file.path) });
    items.push({ label: "Blame", disabled: !tracked || deleted, action: () => showBlame(file.path) });
    items.push({ separator: true });
    items.push({ label: "Open in default app", disabled: deleted, action: () => openFile(file.path) });
    items.push({
      label: "Open in editor",
      disabled: deleted,
      action: () => {
        if (repoPath) tauri.openInEditor(repoPath, file.path).catch((err) => toastError("Could not open editor", err));
      },
    });
    items.push({ label: "Reveal in file manager", action: () => revealFile(file.path) });
    items.push({ label: "Open in external diff tool", disabled: !canDiff, action: () => externalDiff(file, area) });
    items.push({ separator: true });
    items.push({ label: "Copy path", action: () => copyText(file.path.replace(/\/+$/, "")) });
    items.push({ label: "Copy absolute path", action: () => copyText(absolutePath(file.path)) });
    if (area === "unstaged") {
      items.push({ separator: true });
      items.push(...ignoreItems(file.path, false));
    }
    return items;
  }

  function folderMenu(path: string, files: FileStatus[], area: Area): MenuItem[] {
    const items: MenuItem[] = [];
    if (area === "unstaged") {
      items.push({ label: `Stage folder (${files.length})`, action: () => stage(files) });
      items.push({ label: "Discard changes in folder…", danger: true, action: () => discard(files) });
    } else {
      items.push({ label: `Unstage folder (${files.length})`, action: () => unstage(files) });
    }
    items.push({ separator: true });
    items.push({ label: "Reveal in file manager", action: () => revealFile(path) });
    items.push({ label: "Copy path", action: () => copyText(path) });
    items.push({ label: "Copy absolute path", action: () => copyText(absolutePath(path)) });
    if (area === "unstaged") {
      items.push({ separator: true });
      items.push(...ignoreItems(path, true));
    }
    return items;
  }

  function openMenu(area: Area, target: MenuTarget, x: number, y: number) {
    const items =
      target.kind === "file" ? fileMenu(target.file, area) : folderMenu(target.path, target.files, area);
    menu = { x, y, items };
  }

  // ── Section-level actions (respect the filter) ───────────────────────

  function toggleTree() {
    updateSettings({ staging_tree_view: !$settings.staging_tree_view });
  }

  function focusFilter() {
    unstagedExpanded = true;
    stagedExpanded = true;
    filterInput?.focus();
    filterInput?.select();
  }

  function onFilterKey(e: KeyboardEvent) {
    if (e.key === "Escape" && filter) {
      e.preventDefault();
      e.stopPropagation();
      filter = "";
    }
  }

  onMount(() => {
    const offs = [
      onAction("staging_toggle_tree", toggleTree),
      onAction("staging_filter_files", focusFilter),
      onAction("staging_stage_all", () => stage(unstaged)),
      onAction("staging_unstage_all", () => unstage(staged)),
    ];
    return () => offs.forEach((off) => off());
  });
</script>

<div class="files-toolbar">
  <div class="filter-box">
    <Search size={12} />
    <input
      bind:this={filterInput}
      bind:value={filter}
      class="filter-input"
      type="text"
      placeholder="Filter files…"
      spellcheck="false"
      aria-label="Filter changed files by path"
      onkeydown={onFilterKey}
    />
    {#if filter}
      <button class="clear-btn" onclick={() => (filter = "")} title="Clear filter" aria-label="Clear filter">
        <X size={12} />
      </button>
    {/if}
  </div>
  <button
    class="view-btn"
    onclick={toggleTree}
    title={treeView ? "Show as flat list" : "Show as folder tree"}
    aria-label={treeView ? "Show as flat list" : "Show as folder tree"}
    aria-pressed={treeView}
  >
    {#if treeView}
      <ListTree size={14} />
    {:else}
      <List size={14} />
    {/if}
  </button>
</div>

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
    <span class="section-count">
      {filtering ? `${unstaged.length} / ${status.unstaged.length}` : status.unstaged.length}
    </span>
    {#if unstaged.length > 0}
      <button
        class="stage-all-btn danger"
        onclick={(e) => { e.stopPropagation(); discard(unstaged); }}
        title={filtering ? `Discard ${unstaged.length} shown` : "Discard all changes"}
      >
        <Trash2 size={12} />
      </button>
      <button
        class="stage-all-btn"
        onclick={(e) => { e.stopPropagation(); stage(unstaged); }}
        title={filtering ? `Stage ${unstaged.length} shown` : "Stage all"}
      >
        <Plus size={12} />
      </button>
    {/if}
  </div>

  {#if unstagedExpanded}
    <FileList
      files={unstaged}
      tree={treeView}
      selectedPath={selectedFile?.area === "unstaged" ? selectedFile.path : null}
      emptyText={filtering && status.unstaged.length > 0 ? "No matching files" : "No unstaged changes"}
      actions={unstagedActions}
      onselect={(f) => selectFile(f, "unstaged")}
      onmenu={(t, x, y) => openMenu("unstaged", t, x, y)}
    />
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
    <span class="section-count">
      {filtering ? `${staged.length} / ${status.staged.length}` : status.staged.length}
    </span>
    {#if staged.length > 0}
      <button
        class="stage-all-btn danger"
        onclick={(e) => { e.stopPropagation(); unstage(staged); }}
        title={filtering ? `Unstage ${staged.length} shown` : "Unstage all"}
      >
        <Minus size={12} />
      </button>
    {/if}
  </div>

  {#if stagedExpanded}
    <FileList
      files={staged}
      tree={treeView}
      selectedPath={selectedFile?.area === "staged" ? selectedFile.path : null}
      emptyText={filtering && status.staged.length > 0 ? "No matching files" : "No staged changes"}
      actions={stagedActions}
      onselect={(f) => selectFile(f, "staged")}
      onmenu={(t, x, y) => openMenu("staged", t, x, y)}
    />
  {/if}
</div>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<style>
  .files-toolbar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 8px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .filter-box {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 6px;
    height: 24px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-muted);
  }

  .filter-box:focus-within {
    border-color: var(--color-accent);
  }

  .filter-input {
    flex: 1;
    min-width: 0;
    border: none;
    outline: none;
    background: transparent;
    color: var(--color-text-primary);
    font-size: 11px;
    font-family: var(--font-mono);
  }

  .filter-input::placeholder {
    color: var(--color-text-muted);
    font-family: var(--font-sans);
  }

  .clear-btn,
  .view-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 0;
    border-radius: 3px;
  }

  .clear-btn {
    width: 16px;
    height: 16px;
  }

  .view-btn {
    width: 24px;
    height: 24px;
    border: 1px solid var(--color-border);
    background: var(--color-surface);
  }

  .clear-btn:hover,
  .view-btn:hover {
    color: var(--color-text-primary);
    background: var(--color-surface-elevated);
  }

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

  .section-header:focus-visible {
    outline: 1px solid var(--color-accent);
    outline-offset: -1px;
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

  .stage-all-btn.danger:hover {
    background: var(--color-diff-del-bg);
    color: var(--color-diff-del-text);
  }
</style>
