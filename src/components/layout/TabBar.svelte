<script lang="ts">
  import { X, Plus, GitBranch, House, Settings, FolderOpen, GitFork, Pin, History, Search, Link, FolderGit2 } from "lucide-svelte";
  import { tick, untrack } from "svelte";
  import { onAction } from "../../lib/keybindings";
  import { scannedRepos, refreshScannedRepos } from "../../lib/stores/scannedRepos";
  import { openNewRepoDialog } from "../../lib/newRepo";
  import {
    buildEntries,
    filterEntries,
    flatRows,
    stepSelection,
    firstSelectable,
    highlightRuns,
    type RepoMenuRow,
  } from "../../lib/repoMenu";
  import { openRepos, activeRepoPath, removeRepo, addRepo, moveRepo } from "../../lib/stores/repos";
  import { currentView } from "../../lib/stores/ui";
  import { shortcutLabels, withShortcut } from "../../lib/keybindings";
  import { repoHistory, favoritePaths, missingRepoPaths, toggleFavoriteRepo, baseName, repoGroups } from "../../lib/stores/repoHistory";
  import { GROUP_PREFIX } from "../../lib/repoMenu";
  import { openRepoGroup } from "../../lib/groupActions";
  import { groupMenuItems } from "../../lib/groupMenu";
  import { toast, toastError } from "../../lib/stores/toasts";
  import { openRepoWithDialog } from "../../lib/appActions";
  import CloneFromGitHub from "../github/CloneFromGitHub.svelte";
  import CreateRepoOnGitHub from "../github/CreateRepoOnGitHub.svelte";
  import ContextMenu, { type MenuItem } from "../shared/ContextMenu.svelte";
  import * as tauri from "../../lib/tauri";
  import type { RepoInfo } from "../../lib/types/git";

  const repos = $derived([...$openRepos.entries()]);
  const openPaths = $derived(new Set(repos.map(([p]) => p)));
  const active = $derived($activeRepoPath);
  const view = $derived($currentView);
  const homeActive = $derived(active === null && view !== "settings");
  const keys = $derived($shortcutLabels);

  let showMenu = $state(false);
  let showCloneModal = $state(false);
  let showCreateRepoModal = $state(false);
  let plusBtnEl: HTMLButtonElement | undefined = $state(undefined);
  let tabBarEl: HTMLDivElement | undefined = $state(undefined);
  let menuLeft = $state(0);
  let menuTop = $state(0);
  let query = $state("");
  /** Selected repository (by path, so it survives the list being rebuilt). */
  let selectedPath = $state<string | null>(null);
  let searchEl: HTMLInputElement | undefined = $state(undefined);
  let menuListEl: HTMLDivElement | undefined = $state(undefined);

  /** Favorites, then recently opened repos that aren't open as tabs. */
  const quickRepos = $derived.by(() => {
    const out: { path: string; name: string; favorite: boolean; missing: boolean }[] = [];
    const seen = new Set<string>();
    const push = (path: string, name: string, favorite: boolean) => {
      if (seen.has(path) || openPaths.has(path)) return;
      seen.add(path);
      out.push({ path, name, favorite, missing: $missingRepoPaths.has(path) });
    };
    for (const path of $repoHistory.favorites) {
      push(path, $repoHistory.recent.find((r) => r.path === path)?.name ?? baseName(path), true);
    }
    for (const r of $repoHistory.recent) {
      if (out.length >= 8) break;
      if (r.last_opened > 0) push(r.path, r.name, false);
    }
    return out;
  });

  // Searchable menu: favorites/recent + default-folder repositories.
  const menuSections = $derived(filterEntries(buildEntries(quickRepos, $scannedRepos, openPaths, $repoGroups), query));
  const menuRows = $derived(flatRows(menuSections));
  /** Index in `menuRows` of each section's first row. */
  const sectionStart = $derived(
    menuSections.reduce<number[]>((acc, _, i) => {
      acc.push(i === 0 ? 0 : acc[i - 1] + menuSections[i - 1].rows.length);
      return acc;
    }, []),
  );

  // The selection follows the repository; if it's gone (or nothing is
  // selected yet) the best selectable match is used.
  const selectedIdx = $derived.by(() => {
    const i = selectedPath === null ? -1 : menuRows.findIndex((r) => r.path === selectedPath && !r.missing);
    return i >= 0 ? i : firstSelectable(menuRows);
  });

  // Only a new query moves the selection back to the best match; a late
  // scan result or history change keeps the user's choice.
  $effect(() => {
    void query;
    untrack(() => (selectedPath = null));
  });

  async function revealSelected() {
    await tick();
    menuListEl?.querySelector<HTMLElement>(`[data-row="${selectedIdx}"]`)?.scrollIntoView({ block: "nearest" });
  }

  function activateRow(row: RepoMenuRow) {
    if (row.missing) return;
    if (row.kind === "group") {
      showMenu = false;
      void openRepoGroup(row.path.slice(GROUP_PREFIX.length));
      return;
    }
    if (row.open) {
      showMenu = false;
      $activeRepoPath = row.path;
      $currentView = "repos";
    } else {
      openSuggestedRepo(row.path);
    }
  }

  function onSearchKeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const next = stepSelection(menuRows, selectedIdx, e.key === "ArrowDown" ? 1 : -1);
      if (next >= 0) selectedPath = menuRows[next].path;
      void revealSelected();
    } else if (e.key === "Enter") {
      e.preventDefault();
      const row = menuRows[selectedIdx];
      if (row) activateRow(row);
    } else if (e.key === "Escape") {
      // First Esc clears the query; the second closes the menu.
      if (query) {
        e.preventDefault();
        e.stopPropagation();
        query = "";
      }
    }
  }

  // Bindable action: open this menu from the keyboard.
  $effect(() =>
    onAction("open_repo_menu", () => {
      if (!showMenu) toggleMenu();
    }),
  );

  const MENU_WIDTH = 280;

  function toggleMenu() {
    showMenu = !showMenu;
    if (showMenu && plusBtnEl) {
      // Keep the menu on-screen when the "+" button sits near the right edge.
      const rect = plusBtnEl.getBoundingClientRect();
      const left = rect.left;
      // Anchor below the button (the custom title bar may sit above the tabs).
      menuTop = rect.bottom;
      menuLeft = Math.max(4, Math.min(left, window.innerWidth - MENU_WIDTH - 4));
      query = "";
      selectedPath = null;
      // Always rescan on open: folders may have been added or removed.
      void refreshScannedRepos(true);
      tick().then(() => searchEl?.focus());
    }
  }

  async function handleOpenRepo() {
    showMenu = false;
    await openRepoWithDialog();
  }

  async function openSuggestedRepo(path: string) {
    showMenu = false;
    try {
      const info = await tauri.openRepo(path);
      addRepo(info);
    } catch (err) {
      toastError("Open repository failed", err);
    }
  }

  function handleCloned(info: RepoInfo) {
    addRepo(info);
    $currentView = "repos";
  }

  function handleRepoCreated(info: RepoInfo) {
    addRepo(info);
    $currentView = "repos";
  }

  function handleMenuKeydown(e: KeyboardEvent) {
    if (showMenu && e.key === "Escape") {
      showMenu = false;
      plusBtnEl?.focus();
    }
  }

  async function closeTab(path: string) {
    removeRepo(path);
    try {
      await tauri.closeRepo(path);
    } catch {
      // Already removed from UI — ignore backend errors
    }
  }

  async function handleCloseTab(e: MouseEvent, path: string) {
    e.stopPropagation();
    await closeTab(path);
  }

  function handleSelectTab(path: string) {
    if (suppressClick) return;
    $activeRepoPath = path;
    $currentView = "repos";
  }

  function handleOpenSettings() {
    $currentView = "settings";
    $activeRepoPath = null;
  }

  // ── Drag to reorder ────────────────────────────────────────────────
  // Pointer-based (not HTML5 DnD, which the native file-drop handler can
  // swallow in the webview). Tabs reorder live while dragging; the session
  // persists the new order automatically.

  let draggingPath = $state<string | null>(null);
  let suppressClick = false;

  function onTabPointerDown(e: PointerEvent, path: string) {
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest(".tab-close")) return;
    const startX = e.clientX;
    let started = false;

    const onMove = (ev: PointerEvent) => {
      if (!started) {
        if (Math.abs(ev.clientX - startX) < 5) return;
        started = true;
        draggingPath = path;
        document.body.style.cursor = "grabbing";
      }
      const tabs = Array.from(tabBarEl?.querySelectorAll<HTMLElement>(".tab[data-path]") ?? []);
      let target = 0;
      for (const el of tabs) {
        if (el.dataset.path === path) continue;
        const r = el.getBoundingClientRect();
        if (ev.clientX > r.left + r.width / 2) target++;
      }
      moveRepo(path, target);
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      window.removeEventListener("blur", onUp);
      if (started) {
        // The click that ends a drag must not also select the tab.
        suppressClick = true;
        setTimeout(() => (suppressClick = false), 0);
      }
      draggingPath = null;
      document.body.style.cursor = "";
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
    window.addEventListener("blur", onUp);
  }

  function onTabKeydown(e: KeyboardEvent, path: string) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      handleSelectTab(path);
    } else if (e.altKey && e.shiftKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
      // Keyboard reordering of the focused tab.
      e.preventDefault();
      const idx = repos.findIndex(([p]) => p === path);
      moveRepo(path, idx + (e.key === "ArrowLeft" ? -1 : 1));
      queueMicrotask(() =>
        tabBarEl?.querySelector<HTMLElement>(`.tab[data-path="${CSS.escape(path)}"]`)?.focus(),
      );
    }
  }

  // ── Context menu ───────────────────────────────────────────────────

  let ctxMenu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  function copyPath(path: string) {
    navigator.clipboard
      .writeText(path)
      .then(() => toast("success", "Path copied"))
      .catch((err) => toastError("Copy failed", err));
  }

  function onTabContextMenu(e: MouseEvent, path: string, info: RepoInfo) {
    e.preventDefault();
    const idx = repos.findIndex(([p]) => p === path);
    const favorite = $favoritePaths.has(path);
    ctxMenu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: "Close", shortcut: path === active ? keys["close_tab"] : undefined, action: () => closeTab(path) },
        {
          label: "Close other tabs",
          disabled: repos.length < 2,
          action: () => repos.filter(([p]) => p !== path).forEach(([p]) => closeTab(p)),
        },
        {
          label: "Close tabs to the right",
          disabled: idx === repos.length - 1,
          action: () => repos.slice(idx + 1).forEach(([p]) => closeTab(p)),
        },
        { separator: true },
        {
          label: favorite ? "Unpin from favorites" : "Pin to favorites",
          action: () => {
            toggleFavoriteRepo(path, info.name);
            toast("info", favorite ? `Unpinned ${info.name}` : `Pinned ${info.name} to favorites`);
          },
        },
        {
          label: "Add to group…",
          action: () => {
            const items = groupMenuItems(path, info.name);
            const { clientX: x, clientY: y } = e;
            // Opened after this menu has closed.
            setTimeout(() => (ctxMenu = { x, y, items }), 0);
          },
        },
        {
          label: "Open folder",
          action: () => tauri.openInFileManager(path).catch((err) => toastError("Could not open folder", err)),
        },
        { label: "Copy path", action: () => copyPath(path) },
        { separator: true },
        { label: "Move left", disabled: idx === 0, action: () => moveRepo(path, idx - 1) },
        { label: "Move right", disabled: idx === repos.length - 1, action: () => moveRepo(path, idx + 1) },
      ],
    };
  }
</script>

<svelte:window onkeydown={handleMenuKeydown} />

<div class="tab-bar" data-tauri-drag-region role="tablist" aria-label="Open repositories" bind:this={tabBarEl}>
  <button
    class="home-btn"
    class:active={homeActive}
    onclick={() => { $activeRepoPath = null; $currentView = "repos"; }}
    title={withShortcut("Home", keys["go_home"])}
    aria-label="Home"
  >
    <House size={15} />
  </button>

  {#each repos as [path, info] (path)}
    <div
      class="tab"
      class:active={active === path}
      class:dragging={draggingPath === path}
      data-path={path}
      onclick={() => handleSelectTab(path)}
      onauxclick={(e) => e.button === 1 && handleCloseTab(e, path)}
      onpointerdown={(e) => onTabPointerDown(e, path)}
      oncontextmenu={(e) => onTabContextMenu(e, path, info)}
      onkeydown={(e) => onTabKeydown(e, path)}
      role="tab"
      aria-selected={active === path}
      tabindex="0"
      title={`${path}\nDrag (or Alt+Shift+←/→) to reorder · right-click for more`}
    >
      {#if $favoritePaths.has(path)}
        <Pin size={12} class="tab-pin" aria-label="Pinned" />
      {:else}
        <GitBranch size={14} />
      {/if}
      <span class="tab-name">{info.name}</span>
      {#if info.head_name}
        <span class="tab-branch">{info.head_name}</span>
      {/if}
      <button
        class="tab-close"
        onclick={(e) => handleCloseTab(e, path)}
        title={withShortcut("Close tab", active === path ? keys["close_tab"] : undefined)}
        aria-label="Close {info.name}"
      >
        <X size={12} />
      </button>
    </div>
  {/each}

  <button
    class="tab-new"
    bind:this={plusBtnEl}
    onclick={toggleMenu}
    title={withShortcut("Add repository", keys["open_repo"])}
    aria-label="Add repository"
    aria-haspopup="menu"
    aria-expanded={showMenu}
  >
    <Plus size={16} />
  </button>

  <div class="tab-spacer" data-tauri-drag-region></div>

  <button
    class="settings-btn"
    class:active={view === "settings"}
    onclick={handleOpenSettings}
    title={withShortcut("Settings", keys["go_settings"])}
    aria-label="Settings"
  >
    <Settings size={15} />
  </button>
</div>

{#if ctxMenu}
  <ContextMenu x={ctxMenu.x} y={ctxMenu.y} items={ctxMenu.items} onclose={() => (ctxMenu = null)} />
{/if}

{#if showMenu}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="tab-menu-backdrop" onclick={() => (showMenu = false)}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="tab-menu" role="menu" tabindex="-1" style="left: {menuLeft}px; top: {menuTop}px; max-width: {MENU_WIDTH}px" onclick={(e) => e.stopPropagation()}>
      <div class="tab-menu-search">
        <Search size={13} />
        <input
          bind:this={searchEl}
          bind:value={query}
          onkeydown={onSearchKeydown}
          placeholder="Search repositories…"
          aria-label="Search repositories"
          aria-controls="tab-menu-results"
          aria-activedescendant={selectedIdx >= 0 ? `tab-menu-row-${selectedIdx}` : undefined}
          spellcheck="false"
          autocomplete="off"
        />
      </div>
      <div class="tab-menu-results" id="tab-menu-results" role="listbox" aria-label="Repositories" bind:this={menuListEl}>
        {#each menuSections as section, si (section.title)}
          <div class="tab-menu-section">{section.title}</div>
          {#each section.rows as row, ri (row.path)}
            {@const idx = sectionStart[si] + ri}
            <button
              class="tab-menu-item"
              class:tab-menu-item-open={row.open || row.missing}
              class:tab-menu-item-selected={idx === selectedIdx}
              id="tab-menu-row-{idx}"
              data-row={idx}
              role="option"
              aria-selected={idx === selectedIdx}
              disabled={row.missing}
              title={row.kind === "group" ? `Open the “${row.name}” group (${row.branch})` : row.missing ? `${row.path} (folder not found)` : row.path}
              onclick={() => activateRow(row)}
              onmousemove={() => { if (!row.missing) selectedPath = row.path; }}
            >
              {#if row.kind === "favorite"}<Pin size={14} />{:else if row.kind === "recent"}<History size={14} />{:else}<GitBranch size={14} />{/if}
              <span class="tab-menu-repo-name">{#each highlightRuns(row.name, row.nameHits) as run, k (k)}{#if run.hit}<mark>{run.text}</mark>{:else}{run.text}{/if}{/each}</span>
              {#if row.kind === "group"}<span class="tab-menu-repo-branch">{row.branch}</span>{:else if row.missing}<span class="tab-menu-repo-branch">missing</span>{:else if row.open}<span class="tab-menu-repo-branch">open</span>{:else if row.branch}<span class="tab-menu-repo-branch">{row.branch}</span>{/if}
            </button>
          {/each}
        {/each}
        {#if menuRows.length === 0}
          <div class="tab-menu-empty">
            {query.trim() ? `No repositories match "${query.trim()}"` : "No recent repositories"}
          </div>
        {/if}
      </div>
      <div class="tab-menu-divider"></div>
      <button class="tab-menu-item" onclick={handleOpenRepo}>
        <FolderOpen size={14} />
        Open local...
        {#if keys["open_repo"]}<span class="tab-menu-shortcut">{keys["open_repo"]}</span>{/if}
      </button>
      <button class="tab-menu-item" onclick={() => { showMenu = false; openNewRepoDialog("clone"); }}>
        <Link size={14} />
        Clone from URL...
      </button>
      <button class="tab-menu-item" onclick={() => { showMenu = false; showCloneModal = true; }}>
        <GitFork size={14} />
        Clone from GitHub...
      </button>
      <button class="tab-menu-item" onclick={() => { showMenu = false; openNewRepoDialog("init"); }}>
        <FolderGit2 size={14} />
        New repository...
      </button>
      <button class="tab-menu-item" onclick={() => { showMenu = false; showCreateRepoModal = true; }}>
        <Plus size={14} />
        New GitHub repo...
      </button>
    </div>
  </div>
{/if}

<CloneFromGitHub
  open_={showCloneModal}
  onclose={() => (showCloneModal = false)}
  oncloned={handleCloned}
/>
<CreateRepoOnGitHub
  open_={showCreateRepoModal}
  onclose={() => (showCreateRepoModal = false)}
  oncreated={handleRepoCreated}
/>

<style>
  .tab-bar {
    display: flex;
    align-items: center;
    height: var(--tab-height);
    background: var(--color-bg);
    border-bottom: 1px solid var(--color-border);
    overflow-x: auto;
    overflow-y: hidden;
    flex-shrink: 0;
  }

  .home-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--tab-height);
    height: 100%;
    border: none;
    border-right: 1px solid var(--color-border);
    background: var(--color-surface);
    color: var(--color-text-muted);
    cursor: pointer;
    flex-shrink: 0;
    transition: background 0.1s, color 0.1s;
  }

  .home-btn:hover {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .home-btn.active {
    background: var(--color-surface-elevated);
    color: var(--color-accent);
    border-bottom: 2px solid var(--color-accent);
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 100%;
    padding: 0 12px;
    border: none;
    border-right: 1px solid var(--color-border);
    background: var(--color-surface);
    color: var(--color-text-muted);
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
    user-select: none;
    transition: background 0.1s, color 0.1s;
  }

  .tab:hover {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .tab.active {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    border-bottom: 2px solid var(--color-accent);
  }

  .tab.dragging {
    opacity: 0.6;
    cursor: grabbing;
  }

  .tab :global(.tab-pin) {
    color: var(--color-accent);
    flex-shrink: 0;
  }

  .tab-name {
    font-weight: 500;
  }

  .tab-branch {
    color: var(--color-accent);
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .tab-close {
    display: flex;
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
    margin-left: 4px;
  }

  .tab-close:hover {
    background: rgba(247, 118, 142, 0.2);
    color: #f7768e;
  }

  .tab-new {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    padding: 0 10px;
    border: none;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
  }

  .tab-new:hover {
    color: var(--color-accent);
    background: var(--color-surface);
  }

  .tab-menu-backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
  }

  .tab-menu {
    position: fixed;
    top: var(--tab-height);
    z-index: 101;
    min-width: 200px;
    padding: 4px;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
  }

  .tab-menu-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 8px 10px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
    text-align: left;
    transition: background 0.1s;
  }

  .tab-menu-item:hover {
    background: var(--color-surface-elevated);
  }

  .tab-menu-item-open {
    opacity: 0.5;
  }

  .tab-menu-item:disabled {
    cursor: default;
  }

  .tab-menu-shortcut {
    margin-left: auto;
    font-size: 10px;
    font-family: var(--font-mono);
    color: var(--color-text-muted);
  }

  .tab-menu-section {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--color-text-muted);
    padding: 6px 10px 4px;
  }

  .tab-menu-search {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 2px 2px 4px;
    padding: 5px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-muted);
  }

  .tab-menu-search:focus-within {
    border-color: var(--color-accent);
  }

  .tab-menu-search input {
    flex: 1;
    min-width: 0;
    border: none;
    outline: none;
    background: transparent;
    color: var(--color-text-primary);
    font-size: 12px;
  }

  .tab-menu-results {
    max-height: 320px;
    overflow-y: auto;
  }

  .tab-menu-item-selected {
    background: var(--color-surface-elevated);
    box-shadow: inset 2px 0 0 var(--color-accent);
  }

  .tab-menu-repo-name :global(mark) {
    background: transparent;
    color: var(--color-accent);
    font-weight: 600;
  }

  .tab-menu-empty {
    padding: 10px;
    font-size: 12px;
    color: var(--color-text-muted);
  }

  .tab-menu-repo-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tab-menu-repo-branch {
    font-size: 10px;
    font-family: var(--font-mono);
    color: var(--color-accent);
    flex-shrink: 0;
  }

  .tab-menu-divider {
    height: 1px;
    background: var(--color-border);
    margin: 4px 0;
  }

  .tab-spacer {
    flex: 1;
  }

  .settings-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--tab-height);
    height: 100%;
    border: none;
    border-left: 1px solid var(--color-border);
    background: var(--color-surface);
    color: var(--color-text-muted);
    cursor: pointer;
    flex-shrink: 0;
    transition: background 0.1s, color 0.1s;
  }

  .settings-btn:hover {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .settings-btn.active {
    background: var(--color-surface-elevated);
    color: var(--color-accent);
    border-bottom: 2px solid var(--color-accent);
  }
</style>
