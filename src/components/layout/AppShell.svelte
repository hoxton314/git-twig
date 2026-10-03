<script lang="ts">
  import TabBar from "./TabBar.svelte";
  import TitleBar from "./TitleBar.svelte";
  import Sidebar from "./Sidebar.svelte";
  import CommitGraph from "../graph/CommitGraph.svelte";
  import DiffViewer from "../diff/DiffViewer.svelte";
  import StagingArea from "../staging/StagingArea.svelte";
  import HomeScreen from "./HomeScreen.svelte";
  import Toaster from "../shared/Toaster.svelte";
  import UndoHistory from "../reflog/UndoHistory.svelte";
  import OperationBanner from "../conflicts/OperationBanner.svelte";
  import OperationHost from "../conflicts/OperationHost.svelte";
  import FileViewHost from "../history/FileViewHost.svelte";
  import CodeSearch from "../search/CodeSearch.svelte";
  import { lfsPanelOpen } from "../../lib/stores/lfs";
  import { dashboardScope, openDashboard } from "../../lib/dashboard";
  import RepoToolsHost from "../worktrees/RepoToolsHost.svelte";
  import StatusBar from "./StatusBar.svelte";
  import CommandPalette from "./CommandPalette.svelte";
  import { activeRepo, restoreSession, activeRepoPath, openRepos, removeRepo, addRepo, moveRepo } from "../../lib/stores/repos";
  import { selectedCommitOid, selectedWorkingFile, refreshAll } from "../../lib/stores/graph";
  import { diffPanelRatio, sidebarWidth, sidebarOpen, stagingWidth, currentView } from "../../lib/stores/ui";
  import { loadSettings, settings, flushSettings, applyRemoteSettings } from "../../lib/stores/settings";
  import { initAutoFetch } from "../../lib/stores/autofetch";
  import { initCiWatch } from "../../lib/ciWatch";
  import { onWindowClosing, openNewWindow, restoreSavedWindows } from "../../lib/windows";
  import { installKeybindings, onAction } from "../../lib/keybindings";
  import { togglePalette } from "../../lib/palette";
  import { installBuiltinPaletteProviders } from "../../lib/paletteProviders";
  import { loadRepoHistory, toggleFavoriteRepo, isFavoriteRepo, applyRemoteHistory } from "../../lib/stores/repoHistory";
  import { trackOperation } from "../../lib/stores/operations";
  import { toast, toastError } from "../../lib/stores/toasts";
  import { updater, checkForUpdates, ensureUpdaterSupport } from "../../lib/stores/updater";
  import {
    openRepoWithDialog,
    openRepoInTerminal,
    openPathsAsTabs,
    openRepoInEditor,
    openSettingsFolder,
    exportSettingsToFile,
    importSettingsFromFile,
  } from "../../lib/appActions";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { getVersion } from "@tauri-apps/api/app";
  import * as tauri from "../../lib/tauri";
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  // Hosting integrations
  import PullRequestsPanel from "../github/PullRequestsPanel.svelte";
  import NewRepoDialog from "./NewRepoDialog.svelte";
  import { openNewRepoDialog } from "../../lib/newRepo";
  import { invalidateCi } from "../../lib/stores/ci";
  import { tr } from "../../lib/i18n";

  let showTitleBar = $state(false);
  let appVersion = $state("");

  /** Move the active tab one position left/right. */
  function shiftActiveTab(delta: number) {
    const path = get(activeRepoPath);
    if (!path) return;
    const keys = [...get(openRepos).keys()];
    const idx = keys.indexOf(path);
    if (idx !== -1) moveRepo(path, idx + delta);
  }

  async function manualUpdateCheck() {
    if (!(await ensureUpdaterSupport())) {
      toast("info", tr("app.updatesManaged"));
      return;
    }
    await checkForUpdates(true);
    const u = get(updater);
    if (u.status === "up-to-date") toast("success", tr("app.upToDate"));
    else if (u.status === "error") toastError(tr("app.updateCheckFailed"), u.error);
  }

  onMount(() => {
    // Settings first so restore honors "restore tabs on startup".
    // Command-line paths (startup args and later `twig <path>` launches) are
    // queued in Rust and only drained after the session is restored, so they
    // open last and end up active, and none are lost while loading.
    const drainPaths = () => tauri.takePendingPaths().then(openPathsAsTabs);
    const stopOpenPaths = loadSettings()
      .then(() => restoreSession(get(settings).restore_tabs_on_startup))
      .then(() => tauri.onOpenPaths(() => void drainPaths()))
      .then(async (unlisten) => {
        await drainPaths();
        // Main window: reopen the other windows from last time.
        void restoreSavedWindows(get(settings).restore_tabs_on_startup);
        return unlisten;
      })
      .catch((err) => {
        console.error("command-line paths:", err);
        return () => {};
      });
    loadRepoHistory();
    const stopAutoFetch = initAutoFetch();
    const stopCiWatch = initCiWatch();
    installKeybindings();
    const uninstallPalette = installBuiltinPaletteProviders();

    // ── Keybinding action handlers ─────────────────────────────────
    const unsubs = [
      onAction("open_repo", openRepoWithDialog),
      onAction("repo_dashboard", () => openDashboard(null)),
      onAction("new_window", () => void openNewWindow()),
      onAction("lfs_manage", () => {
        if ($activeRepoPath) lfsPanelOpen.set(true);
      }),
      onAction("clone_from_url", () => openNewRepoDialog("clone")),
      onAction("init_repository", () => openNewRepoDialog("init")),
      onAction("close_tab", () => {
        const path = get(activeRepoPath);
        if (path) {
          removeRepo(path);
          tauri.closeRepo(path).catch(() => {});
        }
      }),
      onAction("next_tab", () => {
        const repos = [...get(openRepos).keys()];
        if (repos.length === 0) return;
        const current = get(activeRepoPath);
        const idx = current ? repos.indexOf(current) : -1;
        const next = repos[(idx + 1) % repos.length];
        $activeRepoPath = next;
        $currentView = "repos";
      }),
      onAction("prev_tab", () => {
        const repos = [...get(openRepos).keys()];
        if (repos.length === 0) return;
        const current = get(activeRepoPath);
        const idx = current ? repos.indexOf(current) : repos.length;
        const prev = repos[(idx - 1 + repos.length) % repos.length];
        $activeRepoPath = prev;
        $currentView = "repos";
      }),
      onAction("go_home", () => {
        $activeRepoPath = null;
        $currentView = "repos";
      }),
      onAction("go_settings", () => {
        $currentView = "settings";
        $activeRepoPath = null;
      }),
      onAction("toggle_sidebar", () => {
        $sidebarOpen = !get(sidebarOpen);
      }),
      onAction("fetch", async () => {
        const path = get(activeRepoPath);
        if (!path) return;
        try {
          const result = await trackOperation(path, "fetch", tr("app.fetching"), () => tauri.fetchAll(path));
          refreshAll(path);
          invalidateCi(path);
          if (!result.success) toastError(tr("app.fetchFailed"), result.message);
        } catch (err) {
          toastError(tr("app.fetchFailed"), err);
        }
      }),
      onAction("pull", async () => {
        const path = get(activeRepoPath);
        if (!path) return;
        try {
          const result = await trackOperation(path, "pull", tr("app.pulling"), () => tauri.pull(path));
          refreshAll(path);
          if (!result.success) toastError(tr("app.pullFailed"), result.message);
          else if (result.message.includes("conflicts")) {
            toast("warning", result.message, { title: tr("app.pullStashConflicts"), duration: 0 });
          }
        } catch (err) {
          toastError(tr("app.pullFailed"), err);
        }
      }),
      onAction("push", async () => {
        const path = get(activeRepoPath);
        if (!path) return;
        try {
          const info = await tauri.getRepoInfo(path);
          const branch = info.head_name ?? "HEAD";
          // Same behavior as the Push button in StagingArea: set upstream so
          // first pushes of new branches work.
          const result = await trackOperation(path, "push", tr("app.pushing", { branch }), () =>
            tauri.pushBranch(path, branch, undefined, true),
          );
          refreshAll(path);
          invalidateCi(path);
          if (!result.success) toastError(tr("app.pushFailed"), result.message);
        } catch (err) {
          toastError(tr("app.pushFailed"), err);
        }
      }),
      // commit is handled inside StagingArea via the textarea exception in keybindings.ts

      // ── App shell ──
      onAction("command_palette", togglePalette),
      onAction("move_tab_left", () => shiftActiveTab(-1)),
      onAction("move_tab_right", () => shiftActiveTab(1)),
      onAction("toggle_favorite_repo", () => {
        const info = get(activeRepo);
        if (!info) return;
        const pinned = !isFavoriteRepo(info.path);
        toggleFavoriteRepo(info.path, info.name);
        toast("info", pinned ? tr("tabs.pinned", { name: info.name }) : tr("tabs.unpinned", { name: info.name }));
      }),
      onAction("reveal_repo", () => {
        const path = get(activeRepoPath);
        if (path) tauri.openInFileManager(path).catch((err) => toastError(tr("tabs.openFolderFailed"), err));
      }),
      onAction("open_terminal", () => {
        const path = get(activeRepoPath);
        if (path) openRepoInTerminal(path);
      }),
      onAction("open_editor", () => {
        const path = get(activeRepoPath);
        if (path) openRepoInEditor(path);
      }),
      onAction("copy_repo_path", () => {
        const path = get(activeRepoPath);
        if (!path) return;
        navigator.clipboard
          .writeText(path)
          .then(() => toast("success", tr("app.repoPathCopied")))
          .catch((err) => toastError(tr("tabs.copyFailed"), err));
      }),
      onAction("check_for_updates", manualUpdateCheck),
      onAction("open_settings_folder", openSettingsFolder),
      onAction("export_settings", exportSettingsToFile),
      onAction("import_settings", importSettingsFromFile),
    ];

    tauri.isTilingWm().then((tiling) => {
      showTitleBar = !tiling;
    });

    getVersion().then((v) => (appVersion = v));

    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused && $activeRepoPath) {
        refreshAll();
      }
    });

    // Settings saves are debounced; don't lose a change made right before quit.
    // Other windows' saved settings / history.
    const unlistenSync = tauri.onSync((kind, payload) => {
      if (kind === "repo-history") applyRemoteHistory(payload);
      else applyRemoteSettings(kind, payload);
    });
    const unlistenClose = getCurrentWindow().onCloseRequested(async () => {
      await flushSettings().catch(() => {});
      await onWindowClosing();
    });

    return () => {
      unsubs.forEach((fn) => fn());
      uninstallPalette();
      unlisten.then((fn) => fn());
      unlistenClose.then((fn) => fn());
      unlistenSync.then((fn) => fn());
      stopAutoFetch();
      stopCiWatch();
      stopOpenPaths.then((unlisten) => unlisten());
      stopDrag?.();
    };
  });

  const repo = $derived($activeRepo);
  const view = $derived($currentView);
  const hasSelectedCommit = $derived($selectedCommitOid !== null);
  const hasSelectedWorkingFile = $derived($selectedWorkingFile !== null);
  const showDiff = $derived(hasSelectedCommit || hasSelectedWorkingFile);
  const panelRatio = $derived($diffPanelRatio);
  const sbWidth = $derived($sidebarWidth);
  const stWidth = $derived($stagingWidth);

  // Clear working file selection when commit is selected, and vice versa
  $effect(() => {
    if ($selectedCommitOid !== null) {
      $selectedWorkingFile = null;
    }
  });

  // ── Drag resize ────────────────────────────────────────────────────

  let mainAreaEl: HTMLElement | undefined = $state(undefined);
  let contentEl: HTMLElement | undefined = $state(undefined);
  let dragging = $state<"sidebar" | "diff" | "staging" | null>(null);
  /** Ends an in-progress resize drag (also called on unmount). */
  let stopDrag: (() => void) | null = null;

  function onDragStart(kind: "sidebar" | "diff" | "staging") {
    return (e: MouseEvent) => {
      e.preventDefault();
      dragging = kind;
      document.body.style.userSelect = "none";
      document.body.style.cursor =
        kind === "diff" ? "row-resize" : "col-resize";

      const onMove = (ev: MouseEvent) => {
        if (kind === "diff" && mainAreaEl) {
          const rect = mainAreaEl.getBoundingClientRect();
          const ratio = 1 - (ev.clientY - rect.top) / rect.height;
          $diffPanelRatio = Math.max(0.1, Math.min(0.8, ratio));
        } else if (kind === "sidebar") {
          $sidebarWidth = Math.max(160, Math.min(500, ev.clientX));
        } else if (kind === "staging" && contentEl) {
          const rect = contentEl.getBoundingClientRect();
          $stagingWidth = Math.max(200, Math.min(600, rect.right - ev.clientX));
        }
      };

      const onUp = () => {
        dragging = null;
        stopDrag = null;
        document.body.style.userSelect = "";
        document.body.style.cursor = "";
        document.removeEventListener("mousemove", onMove);
        document.removeEventListener("mouseup", onUp);
        window.removeEventListener("blur", onUp);
      };
      stopDrag = onUp;

      document.addEventListener("mousemove", onMove);
      document.addEventListener("mouseup", onUp);
      // Releasing the mouse outside the window never fires mouseup here.
      window.addEventListener("blur", onUp);
    };
  }

</script>

<div class="app-shell">
  {#if showTitleBar}
    <TitleBar />
  {/if}
  <TabBar />

  {#if view === "settings"}
    <!-- Loaded on first open to keep the main bundle small. -->
    {#await import("../settings/SettingsScreen.svelte") then { default: SettingsScreen }}
      <SettingsScreen />
    {/await}
  {:else if repo}
    <div class="content" bind:this={contentEl}>
      <Sidebar />
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="resize-handle-v"
        class:active={dragging === "sidebar"}
        onmousedown={onDragStart("sidebar")}
      ></div>
      <main class="main-area" bind:this={mainAreaEl}>
        <OperationBanner />
        <div
          class="graph-panel"
          style="flex: {showDiff ? 1 - panelRatio : 1}"
        >
          <CommitGraph />
        </div>
        {#if showDiff}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="panel-divider"
            class:active={dragging === "diff"}
            onmousedown={onDragStart("diff")}
          ></div>
          <div class="diff-panel" style="flex: {panelRatio}">
            <DiffViewer />
          </div>
        {/if}
      </main>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="resize-handle-v"
        class:active={dragging === "staging"}
        onmousedown={onDragStart("staging")}
      ></div>
      <aside class="staging-panel" style="width: {stWidth}px; min-width: 200px;">
        <StagingArea />
      </aside>
    </div>
    <StatusBar version={showTitleBar ? "" : appVersion} />
  {:else}
    <HomeScreen />
  {/if}

  {#if !showTitleBar && appVersion && (view === "settings" || !repo)}
    <span class="version-badge">v{appVersion}</span>
  {/if}

  <OperationHost />
  <CommandPalette />
  <Toaster />
  <UndoHistory />
  <!-- File history & blame overlay; worktree/submodule actions -->
  <FileViewHost />
  <CodeSearch />
  <!-- Rarely opened: loaded on first use to keep the main bundle small. -->
  {#if $dashboardScope}
    {#await import("../dashboard/Dashboard.svelte") then { default: Dashboard }}
      <Dashboard />
    {/await}
  {/if}
  {#if $lfsPanelOpen}
    {#await import("../lfs/LfsPanel.svelte") then { default: LfsPanel }}
      <LfsPanel />
    {/await}
  {/if}
  <RepoToolsHost />
  <PullRequestsPanel />
  <NewRepoDialog />
</div>

<style>
  .app-shell {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--color-bg);
  }

  .content {
    display: flex;
    flex: 1;
    overflow: hidden;
  }

  .main-area {
    display: flex;
    flex-direction: column;
    flex: 1;
    overflow: hidden;
  }

  .graph-panel {
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  /* Horizontal divider (graph / diff) */
  .panel-divider {
    height: 3px;
    background: var(--color-border);
    cursor: row-resize;
    flex-shrink: 0;
  }

  .panel-divider:hover,
  .panel-divider.active {
    background: var(--color-accent);
  }

  /* Vertical resize handles (sidebar edges) */
  .resize-handle-v {
    width: 3px;
    cursor: col-resize;
    background: var(--color-border);
    flex-shrink: 0;
  }

  .resize-handle-v:hover,
  .resize-handle-v.active {
    background: var(--color-accent);
  }

  .diff-panel {
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .staging-panel {
    border-left: none;
    background: var(--color-surface);
    overflow: hidden;
    flex-shrink: 0;
  }

  .version-badge {
    position: fixed;
    bottom: 4px;
    left: 6px;
    font-size: 12px;
    color: var(--color-text-muted);
    opacity: 0.4;
    pointer-events: none;
    user-select: none;
    z-index: 1;
  }
</style>
