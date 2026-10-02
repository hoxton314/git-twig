<script lang="ts">
  import { onMount, tick } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import {
    selectedCommitOid,
    selectedDiff,
    diffLoading,
    selectedWorkingFile,
    workingFileDiff,
    commitGraph,
    refreshStatus,
  } from "../../lib/stores/graph";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { diffViewMode } from "../../lib/stores/ui";
  import { settings, updateSettings } from "../../lib/stores/settings";
  import { toast, toastError } from "../../lib/stores/toasts";
  import { onAction } from "../../lib/keybindings";
  import { ensureLanguage, isLanguageReady, languageForPath } from "../../lib/diff/highlight";
  import * as tauri from "../../lib/tauri";
  import DiffHunk from "./DiffHunk.svelte";
  import ImageDiff from "./ImageDiff.svelte";
  import AudioPreview from "./AudioPreview.svelte";
  import {
    FileText,
    Binary,
    Package,
    Loader2,
    Columns2,
    AlignJustify,
    Image,
    Music,
    ChevronUp,
    ChevronDown,
    Search,
    X,
    Pilcrow,
  } from "lucide-svelte";
  import type {
    DiffFile,
    DiffHunk as DiffHunkType,
    DiffLine,
    HunkAction,
    SelectedLine,
  } from "../../lib/types/git";

  const IMAGE_EXTENSIONS = new Set([
    "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico", "avif", "tiff", "tif",
  ]);

  const AUDIO_EXTENSIONS = new Set([
    "mp3", "wav", "ogg", "oga", "opus", "flac", "m4a", "aac", "weba",
  ]);

  const repoPath = $derived($activeRepoPath);
  const commitOid = $derived($selectedCommitOid);
  const commitDiff = $derived($selectedDiff);
  const workingFile = $derived($selectedWorkingFile);
  const workingDiff = $derived($workingFileDiff);
  const loading = $derived($diffLoading);
  const viewMode = $derived($diffViewMode);
  const tabSize = $derived($settings.tab_size || 4);
  const wordWrap = $derived($settings.word_wrap_in_diffs);
  const syntaxOn = $derived($settings.syntax_highlighting ?? true);
  const showWhitespace = $derived($settings.show_whitespace_changes);

  // Files with more diff lines than this are collapsed behind a "show anyway"
  // button so a huge generated/minified file can't freeze the UI.
  const LARGE_DIFF_LINES = 2000;

  // Show working file diff when a working file is selected or WIP row clicked, otherwise commit diff
  const isWipMode = $derived(commitOid === "__wip__");
  const isWorkingMode = $derived(workingFile !== null || isWipMode);
  const diff = $derived(isWorkingMode ? workingDiff : commitDiff);

  // Hunk/line actions only make sense for a single staged/unstaged file diff
  // (the WIP view concatenates both areas).
  const actionArea = $derived<"staged" | "unstaged" | null>(
    isWorkingMode && !isWipMode && workingFile ? workingFile.area : null,
  );

  let expandedFiles = $state<Set<string>>(new Set());

  // Large files the user explicitly asked to render.
  let forceShown = $state<Set<string>>(new Set());

  // Image blob cache: fileKey -> { old, new, loading }
  let imageBlobs = $state<Record<string, { old: string | null; new: string | null; loading: boolean }>>({});
  // Bumped whenever the displayed diff changes so in-flight blob loads for the
  // previous diff (other area / commit / file contents) are discarded.
  let blobGen = 0;

  // Load diff when selected commit (or repo) changes. The guard keys on both
  // path and oid so switching repos reloads even if the same oid is selected.
  let lastLoaded: string | null = null;
  let diffRequest = 0;
  $effect(() => {
    const oid = commitOid;
    const path = repoPath;
    const key = path && oid ? `${path} ${oid}` : null;
    if (!oid || oid === "__wip__") {
      // Forget the last commit so re-selecting it later reloads and
      // re-expands instead of showing stale expansion state.
      lastLoaded = null;
      return;
    }
    if (path && key !== lastLoaded) {
      lastLoaded = key;
      loadDiff(path, oid);
    }
  });

  // Any new diff (different file/area/commit, or refreshed contents)
  // invalidates cached image/audio blobs and expanded context.
  $effect(() => {
    void diff;
    blobGen++;
    imageBlobs = {};
    forceShown = new Set();
    contexts = {};
  });

  // Auto-expand when working file diff loads
  $effect(() => {
    if (isWorkingMode && workingDiff.length > 0) {
      expandedFiles = new Set(workingDiff.map(fileKey));
    }
  });

  // Context lines / whitespace settings changed: reload what's displayed.
  // The tauri wrappers already picked up the new defaults from the store.
  let lastDiffOpts: string | null = null;
  $effect(() => {
    const opts = `${$settings.context_lines}|${$settings.show_whitespace_changes}`;
    if (lastDiffOpts === null || lastDiffOpts === opts) {
      lastDiffOpts = opts;
      return;
    }
    lastDiffOpts = opts;
    const path = repoPath;
    if (!path) return;
    if (isWorkingMode) {
      refreshStatus(path);
    } else if (commitOid) {
      loadDiff(path, commitOid, true);
    }
  });

  async function loadDiff(path: string, oid: string, keepExpanded = false) {
    const req = ++diffRequest;
    $diffLoading = !keepExpanded;
    if (!keepExpanded) expandedFiles = new Set();
    try {
      const result = await tauri.getCommitDiff(path, oid);
      // Drop the result if another commit/repo was selected while loading,
      // so a slow earlier request can't overwrite the current diff.
      if (req !== diffRequest || repoPath !== path) return;
      $selectedDiff = result;
      if (result.length > 0 && !keepExpanded) {
        expandedFiles = new Set([fileKey(result[0])]);
      }
    } catch (err) {
      if (req !== diffRequest) return;
      console.error("Failed to load diff:", err);
      $selectedDiff = [];
    } finally {
      if (req === diffRequest) $diffLoading = false;
    }
  }

  function lineCount(f: DiffFile): number {
    let n = 0;
    for (const h of f.hunks) n += h.lines.length;
    return n;
  }

  function showLarge(f: DiffFile) {
    const next = new Set(forceShown);
    next.add(fileKey(f));
    forceShown = next;
  }

  function displayPath(f: DiffFile): string {
    if (f.old_path && f.new_path && f.old_path !== f.new_path) {
      return `${f.old_path} → ${f.new_path}`;
    }
    return f.new_path ?? f.old_path ?? "unknown";
  }

  function fileKey(f: DiffFile): string {
    return f.new_path ?? f.old_path ?? "unknown";
  }

  function fileExt(f: DiffFile): string {
    const path = f.new_path ?? f.old_path ?? "";
    return path.split(".").pop()?.toLowerCase() ?? "";
  }

  function isImageFile(f: DiffFile): boolean {
    return IMAGE_EXTENSIONS.has(fileExt(f));
  }

  function isAudioFile(f: DiffFile): boolean {
    return AUDIO_EXTENSIONS.has(fileExt(f));
  }

  // Files whose preview needs raw blob contents rather than text hunks.
  function isBlobPreviewFile(f: DiffFile): boolean {
    return isImageFile(f) || isAudioFile(f);
  }

  function toggleFile(f: DiffFile) {
    const key = fileKey(f);
    const next = new Set(expandedFiles);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
      // Image blobs are loaded by the effect that watches expandedFiles, so we
      // don't also kick off a load here (which could double-fetch).
    }
    expandedFiles = next;
  }

  function parentOidOf(oid: string): string | null {
    const g = $commitGraph;
    const entry = g?.entries.find((e) => e.commit.oid === oid);
    return entry && entry.commit.parent_oids.length > 0 ? entry.commit.parent_oids[0] : null;
  }

  async function loadImageBlobs(f: DiffFile) {
    const path = repoPath;
    if (!path) return;

    const key = fileKey(f);
    const gen = blobGen;
    imageBlobs[key] = { old: null, new: null, loading: true };

    try {
      const filePath = f.new_path ?? f.old_path ?? "";
      const oldFilePath = f.old_path ?? f.new_path ?? "";

      let oldSource: string | null = null;
      let newSource: string | null = null;

      if (isWipMode) {
        // WIP row clicked — combined working changes: old=HEAD, new=workdir.
        // Checked first: WIP mode also sets a placeholder selectedWorkingFile.
        oldSource = f.status !== "added" ? "head" : null;
        newSource = f.status !== "deleted" ? "workdir" : null;
      } else if (isWorkingMode && workingFile) {
        // Single file selected in staging area
        if (workingFile.area === "staged") {
          // staged: old=HEAD, new=index
          oldSource = f.status !== "added" ? "head" : null;
          newSource = f.status !== "deleted" ? "index" : null;
        } else {
          // unstaged: old=index, new=workdir
          oldSource = f.status !== "added" ? "index" : null;
          newSource = f.status !== "deleted" ? "workdir" : null;
        }
      } else if (commitOid && commitOid !== "__wip__") {
        // Commit diff: old=parent, new=commit
        const parentOid = parentOidOf(commitOid);
        oldSource = f.status !== "added" && parentOid ? parentOid : null;
        newSource = f.status !== "deleted" ? commitOid : null;
      }

      const [oldData, newData] = await Promise.all([
        oldSource ? tauri.getFileBlob(path, oldFilePath, oldSource) : Promise.resolve(null),
        newSource ? tauri.getFileBlob(path, filePath, newSource) : Promise.resolve(null),
      ]);

      // Drop stale results if the repo or displayed diff changed mid-load.
      if (repoPath !== path || gen !== blobGen) return;
      imageBlobs[key] = { old: oldData, new: newData, loading: false };
    } catch (err) {
      if (gen !== blobGen) return;
      console.error("Failed to load image blobs:", err);
      imageBlobs[key] = { old: null, new: null, loading: false };
    }
  }

  // Auto-load blobs for any expanded image/audio file (works for all modes)
  $effect(() => {
    for (const f of diff) {
      const key = fileKey(f);
      if (isBlobPreviewFile(f) && expandedFiles.has(key) && !imageBlobs[key]) {
        loadImageBlobs(f);
      }
    }
  });

  function statusBadgeClass(status: string): string {
    switch (status) {
      case "added":
      case "untracked":
        return "badge-added";
      case "deleted":
        return "badge-deleted";
      case "modified":
        return "badge-modified";
      case "renamed":
      case "copied":
        return "badge-renamed";
      default:
        return "";
    }
  }

  function statusLabel(status: string): string {
    return status.charAt(0).toUpperCase();
  }

  // ── Syntax highlighting (grammars load lazily per language) ─────────
  let langVersion = $state(0);

  $effect(() => {
    if (!syntaxOn) return;
    for (const f of diff) {
      if (!expandedFiles.has(fileKey(f))) continue;
      const lang = languageForPath(f.new_path ?? f.old_path);
      if (lang && !isLanguageReady(lang)) {
        ensureLanguage(lang).then((ok) => {
          if (ok) langVersion++;
        });
      }
    }
  });

  function fileLanguage(f: DiffFile): string | null {
    void langVersion;
    if (!syntaxOn) return null;
    const lang = languageForPath(f.new_path ?? f.old_path);
    return lang && isLanguageReady(lang) ? lang : null;
  }

  // ── Hunk / line staging ─────────────────────────────────────────────
  let actionBusy = $state(false);

  function toSelected(lines: DiffLine[]): SelectedLine[] {
    return lines
      .filter((l) => l.origin === "+" || l.origin === "-")
      .map((l) => ({
        origin: l.origin,
        old_lineno: l.old_lineno,
        new_lineno: l.new_lineno,
        content: l.content,
      }));
  }

  async function runHunkAction(
    file: DiffFile,
    hunk: DiffHunkType,
    action: HunkAction,
    lines: DiffLine[] | null,
  ) {
    const path = repoPath;
    const area = actionArea;
    const filePath = file.new_path ?? file.old_path;
    if (!path || !area || !filePath || actionBusy) return;

    const whole = lines === null;
    const selected = toSelected(whole ? hunk.lines : lines);
    if (selected.length === 0) return;

    if (action === "discard" && $settings.confirm_destructive_ops) {
      const what = whole ? "this hunk" : `${selected.length} selected line${selected.length === 1 ? "" : "s"}`;
      const ok = await ask(`Discard ${what} in ${filePath}? This cannot be undone.`, {
        title: "Discard changes",
        kind: "warning",
      });
      if (!ok) return;
    }

    actionBusy = true;
    try {
      const ranges = whole
        ? [{
            old_start: hunk.old_start,
            old_lines: hunk.old_lines,
            new_start: hunk.new_start,
            new_lines: hunk.new_lines,
          }]
        : [];
      const result = await tauri.applyDiffSelection(path, filePath, area, action, selected, ranges);
      if (!result.success) {
        const verb = action === "stage" ? "stage" : action === "unstage" ? "unstage" : "discard";
        toast("error", result.message.trim() || "git apply failed", { title: `Could not ${verb} changes` });
      }
    } catch (err) {
      toastError("Could not apply selection", err);
    } finally {
      actionBusy = false;
      // Refresh staging lists and this diff even after a failure, since a
      // stale view is the most likely cause.
      if (repoPath === path) await refreshStatus(path);
    }
  }

  // ── Expandable context ──────────────────────────────────────────────
  interface FileContext {
    /** New-side file lines (null until loaded). */
    lines: string[] | null;
    loading: boolean;
    failed: boolean;
    /** Per gap index: lines revealed below the previous hunk / above the next. */
    gaps: Record<number, { top: number; bottom: number }>;
  }
  let contexts = $state<Record<string, FileContext>>({});
  const EXPAND_STEP = 20;

  function newSideSource(): string | null {
    if (isWipMode) return null;
    if (isWorkingMode && workingFile) return workingFile.area === "staged" ? "index" : "workdir";
    if (commitOid && commitOid !== "__wip__") return commitOid;
    return null;
  }

  function canExpand(f: DiffFile): boolean {
    return (
      newSideSource() !== null &&
      !f.is_binary &&
      !f.is_lfs &&
      f.status !== "added" &&
      f.status !== "deleted" &&
      f.new_path !== null &&
      f.hunks.length > 0
    );
  }

  function hunkNewEnd(h: DiffHunkType): number {
    return h.new_lines === 0 ? h.new_start : h.new_start + h.new_lines - 1;
  }

  function hunkOldEnd(h: DiffHunkType): number {
    return h.old_lines === 0 ? h.old_start : h.old_start + h.old_lines - 1;
  }

  /** New-side [start, end] of gap `i` (before hunk i; i === length is after the last). */
  function gapBounds(f: DiffFile, i: number, total: number | null): { start: number; end: number; offset: number } {
    const hunks = f.hunks;
    const start = i === 0 ? 1 : hunkNewEnd(hunks[i - 1]) + 1;
    if (i < hunks.length) {
      const h = hunks[i];
      const end = h.new_lines === 0 ? h.new_start : h.new_start - 1;
      const newFirst = h.new_lines === 0 ? h.new_start + 1 : h.new_start;
      const oldFirst = h.old_lines === 0 ? h.old_start + 1 : h.old_start;
      return { start, end, offset: oldFirst - newFirst };
    }
    const last = hunks[hunks.length - 1];
    return {
      start,
      end: total ?? Number.MAX_SAFE_INTEGER,
      offset: hunkOldEnd(last) - hunkNewEnd(last),
    };
  }

  function contextHunk(
    lines: string[],
    from: number,
    to: number,
    offset: number,
  ): DiffHunkType {
    const out: DiffLine[] = [];
    for (let n = from; n <= to && n <= lines.length; n++) {
      out.push({ origin: " ", old_lineno: n + offset, new_lineno: n, content: lines[n - 1] });
    }
    return {
      header: "",
      old_start: from + offset,
      old_lines: out.length,
      new_start: from,
      new_lines: out.length,
      lines: out,
    };
  }

  function splitFileLines(text: string): string[] {
    const parts = text.split("\n");
    if (parts.length > 0 && parts[parts.length - 1] === "") parts.pop();
    // Keep terminators like the diff lines do, so rendering is identical.
    return parts.map((p, i) => (i < parts.length - 1 || text.endsWith("\n") ? `${p}\n` : p));
  }

  async function expandGap(f: DiffFile, gap: number, dir: "up" | "down" | "all") {
    const key = fileKey(f);
    const path = repoPath;
    const source = newSideSource();
    if (!path || !source || !f.new_path) return;
    const gen = blobGen;

    let ctx = contexts[key];
    if (!ctx) {
      ctx = { lines: null, loading: false, failed: false, gaps: {} };
      contexts[key] = ctx;
    }
    if (!ctx.lines) {
      if (contexts[key].loading) return;
      contexts[key].loading = true;
      try {
        const b64 = await tauri.getFileBlob(path, f.new_path, source);
        if (gen !== blobGen || repoPath !== path) return;
        if (b64 === null) throw new Error("file contents unavailable");
        const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
        contexts[key].lines = splitFileLines(new TextDecoder().decode(bytes));
      } catch (err) {
        if (gen !== blobGen) return;
        contexts[key].failed = true;
        toastError("Could not load file contents", err);
        return;
      } finally {
        if (gen === blobGen && contexts[key]) contexts[key].loading = false;
      }
    }
    const c = contexts[key];
    const g = c.gaps[gap] ?? { top: 0, bottom: 0 };
    if (dir === "down") g.top += EXPAND_STEP;
    else if (dir === "up") g.bottom += EXPAND_STEP;
    else g.top = Number.MAX_SAFE_INTEGER;
    c.gaps[gap] = { ...g };
  }

  interface GapView {
    hidden: number;
    /** Lines to show in one block when the whole gap is revealed. */
    full: DiffHunkType | null;
    top: DiffHunkType | null;
    bottom: DiffHunkType | null;
    canUp: boolean;
    canDown: boolean;
    unknownEnd: boolean;
  }

  function gapView(f: DiffFile, gap: number): GapView | null {
    const ctx = contexts[fileKey(f)];
    const lines = ctx?.lines ?? null;
    const isLast = gap === f.hunks.length;
    const { start, end, offset } = gapBounds(f, gap, lines ? lines.length : null);
    if (lines === null) {
      // Before loading we only know the size of gaps between hunks.
      if (isLast) {
        return { hidden: 0, full: null, top: null, bottom: null, canUp: false, canDown: true, unknownEnd: true };
      }
      const size = end - start + 1;
      if (size <= 0) return null;
      return { hidden: size, full: null, top: null, bottom: null, canUp: true, canDown: gap > 0, unknownEnd: false };
    }
    const size = end - start + 1;
    if (size <= 0) return null;
    const g = ctx?.gaps[gap] ?? { top: 0, bottom: 0 };
    if (g.top + g.bottom >= size) {
      return { hidden: 0, full: contextHunk(lines, start, end, offset), top: null, bottom: null, canUp: false, canDown: false, unknownEnd: false };
    }
    return {
      hidden: size - g.top - g.bottom,
      full: null,
      top: g.top > 0 ? contextHunk(lines, start, start + g.top - 1, offset) : null,
      bottom: g.bottom > 0 ? contextHunk(lines, end - g.bottom + 1, end, offset) : null,
      canUp: !isLast,
      canDown: gap > 0,
      unknownEnd: false,
    };
  }

  // ── Hunk navigation ─────────────────────────────────────────────────
  let rootEl = $state<HTMLDivElement | null>(null);
  let filesEl = $state<HTMLDivElement | null>(null);

  function gotoHunk(dir: 1 | -1) {
    const container = filesEl;
    if (!container) return;
    const anchors = Array.from(container.querySelectorAll<HTMLElement>(".hunk-anchor"));
    if (anchors.length === 0) return;
    const cTop = container.getBoundingClientRect().top;
    const tops = anchors.map((a) => a.getBoundingClientRect().top - cTop);
    let target: number;
    if (dir === 1) {
      target = tops.findIndex((t) => t > 8);
      if (target < 0) return;
    } else {
      target = -1;
      for (let i = 0; i < tops.length; i++) if (tops[i] < -8) target = i;
      if (target < 0) target = 0;
    }
    container.scrollTo({ top: container.scrollTop + tops[target], behavior: "smooth" });
  }

  // ── In-diff search ──────────────────────────────────────────────────
  let searchOpen = $state(false);
  let searchInput = $state<HTMLInputElement | null>(null);
  let searchText = $state("");
  let searchQuery = $state("");
  let matchCount = $state(0);
  let activeMatch = $state(0);
  let searchTimer: ReturnType<typeof setTimeout> | null = null;

  function openSearch() {
    searchOpen = true;
    tick().then(() => {
      searchInput?.focus();
      searchInput?.select();
    });
  }

  function closeSearch() {
    searchOpen = false;
    searchText = "";
    searchQuery = "";
    matchCount = 0;
    activeMatch = 0;
    rootEl?.focus();
  }

  function onSearchInput() {
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      searchQuery = searchText.toLowerCase();
      activeMatch = 0;
      // Open every file that has a hit so its matches are rendered.
      if (searchQuery) {
        const next = new Set(expandedFiles);
        for (const f of diff) {
          if (f.hunks.some((h) => h.lines.some((l) => l.content.toLowerCase().includes(searchQuery)))) {
            next.add(fileKey(f));
          }
        }
        expandedFiles = next;
      }
    }, 120);
  }

  function stepMatch(dir: 1 | -1) {
    if (matchCount === 0) return;
    activeMatch = (activeMatch + dir + matchCount) % matchCount;
  }

  function onSearchKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      stepMatch(e.shiftKey ? -1 : 1);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      closeSearch();
    }
  }

  // After every render that can change matches, recount and mark the active one.
  $effect(() => {
    void [searchQuery, diff, expandedFiles, viewMode, contexts, forceShown, langVersion, activeMatch];
    const container = filesEl;
    tick().then(() => {
      if (!container) return;
      for (const el of container.querySelectorAll(".search-active")) el.classList.remove("search-active");
      if (!searchQuery) {
        matchCount = 0;
        return;
      }
      const starts = Array.from(container.querySelectorAll<HTMLElement>("[data-match-start]"));
      matchCount = starts.length;
      if (starts.length === 0) return;
      if (activeMatch >= starts.length) activeMatch = 0;
      const first = starts[activeMatch];
      const id = first.dataset.m;
      if (id) {
        for (const el of container.querySelectorAll<HTMLElement>(`[data-m="${CSS.escape(id)}"]`)) {
          el.classList.add("search-active");
        }
      }
      first.scrollIntoView({ block: "nearest", inline: "nearest" });
    });
  });

  function onRootKeydown(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (mod && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "f") {
      e.preventDefault();
      openSearch();
    } else if (e.key === "F3" || (mod && e.key.toLowerCase() === "g")) {
      if (!searchOpen) return;
      e.preventDefault();
      stepMatch(e.shiftKey ? -1 : 1);
    }
  }

  function toggleWhitespace() {
    updateSettings({ show_whitespace_changes: !showWhitespace });
  }

  // Panel-scoped shortcuts (attached directly: the region isn't a widget).
  $effect(() => {
    const el = rootEl;
    if (!el) return;
    el.addEventListener("keydown", onRootKeydown);
    return () => el.removeEventListener("keydown", onRootKeydown);
  });

  onMount(() => {
    const unsubs = [
      onAction("diff_next_hunk", () => gotoHunk(1)),
      onAction("diff_prev_hunk", () => gotoHunk(-1)),
      onAction("diff_find", openSearch),
      onAction("diff_toggle_whitespace", toggleWhitespace),
    ];
    return () => {
      for (const u of unsubs) u();
      if (searchTimer) clearTimeout(searchTimer);
    };
  });
</script>

{#snippet expander(file: DiffFile, gi: number, view: GapView, busy: boolean)}
  <div class="expander">
    {#if view.canDown}
      <button class="exp-btn" disabled={busy} onclick={() => expandGap(file, gi, "down")} title="Show {EXPAND_STEP} more lines below">
        <ChevronDown size={13} />
      </button>
    {/if}
    {#if view.canUp}
      <button class="exp-btn" disabled={busy} onclick={() => expandGap(file, gi, "up")} title="Show {EXPAND_STEP} more lines above">
        <ChevronUp size={13} />
      </button>
    {/if}
    <button class="exp-label" disabled={busy} onclick={() => expandGap(file, gi, "all")}>
      {#if busy}
        Loading…
      {:else if view.unknownEnd}
        Show rest of file
      {:else}
        Show {view.hidden} hidden line{view.hidden === 1 ? "" : "s"}
      {/if}
    </button>
  </div>
{/snippet}

{#snippet gapBlock(file: DiffFile, gi: number, lang: string | null)}
  {@const view = gapView(file, gi)}
  {#if view}
    {@const busy = contexts[fileKey(file)]?.loading ?? false}
    {#if view.full}
      {#if view.full.lines.length > 0}
        <DiffHunk hunk={view.full} mode={viewMode} {tabSize} wrap={wordWrap} language={lang} showHeader={false} search={searchQuery} matchPrefix="{fileKey(file)}:g{gi}" />
      {/if}
    {:else}
      {#if view.top}
        <DiffHunk hunk={view.top} mode={viewMode} {tabSize} wrap={wordWrap} language={lang} showHeader={false} search={searchQuery} matchPrefix="{fileKey(file)}:gt{gi}" />
      {/if}
      {#if view.hidden > 0 || view.unknownEnd}
        {@render expander(file, gi, view, busy)}
      {/if}
      {#if view.bottom}
        <DiffHunk hunk={view.bottom} mode={viewMode} {tabSize} wrap={wordWrap} language={lang} showHeader={false} search={searchQuery} matchPrefix="{fileKey(file)}:gb{gi}" />
      {/if}
    {/if}
  {/if}
{/snippet}

<!-- Focusable so Ctrl+F / F3 can be scoped to the diff panel. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div class="diff-viewer" bind:this={rootEl} tabindex="-1" role="region" aria-label="Diff">
  <div class="diff-header">
    <span class="diff-title">
      {#if isWipMode}
        <span class="label">Working changes</span>
        — {diff.length} file{diff.length !== 1 ? "s" : ""} changed
      {:else if isWorkingMode && workingFile}
        <span class="label">{workingFile.area === "staged" ? "Staged" : "Unstaged"}</span>
        — {workingFile.path}
      {:else if commitOid}
        <span class="oid">{commitOid?.slice(0, 7)}</span>
        — {diff.length} file{diff.length !== 1 ? "s" : ""} changed
      {/if}
    </span>
    <div class="view-toggle">
      <button class="toggle-btn" onclick={() => gotoHunk(-1)} title="Previous change (Alt+Up)" aria-label="Previous change">
        <ChevronUp size={14} />
      </button>
      <button class="toggle-btn" onclick={() => gotoHunk(1)} title="Next change (Alt+Down)" aria-label="Next change">
        <ChevronDown size={14} />
      </button>
      <button
        class="toggle-btn"
        class:active={searchOpen}
        onclick={() => (searchOpen ? closeSearch() : openSearch())}
        title="Find in diff (Ctrl+F)"
        aria-label="Find in diff"
      >
        <Search size={14} />
      </button>
      <button
        class="toggle-btn"
        class:active={showWhitespace}
        onclick={toggleWhitespace}
        title={showWhitespace ? "Hide whitespace-only changes" : "Show whitespace-only changes"}
        aria-label="Toggle whitespace changes"
        aria-pressed={showWhitespace}
      >
        <Pilcrow size={14} />
      </button>
      <span class="sep"></span>
      <button
        class="toggle-btn"
        class:active={viewMode === "unified"}
        onclick={() => ($diffViewMode = "unified")}
        title="Unified view"
      >
        <AlignJustify size={14} />
      </button>
      <button
        class="toggle-btn"
        class:active={viewMode === "split"}
        onclick={() => ($diffViewMode = "split")}
        title="Split view"
      >
        <Columns2 size={14} />
      </button>
    </div>
  </div>

  {#if searchOpen}
    <div class="search-bar">
      <Search size={13} />
      <input
        bind:this={searchInput}
        bind:value={searchText}
        oninput={onSearchInput}
        onkeydown={onSearchKeydown}
        placeholder="Find in diff"
        aria-label="Find in diff"
        spellcheck="false"
      />
      <span class="match-count">
        {#if searchQuery}
          {matchCount === 0 ? "No results" : `${activeMatch + 1} of ${matchCount}`}
        {/if}
      </span>
      <button class="toggle-btn" disabled={matchCount === 0} onclick={() => stepMatch(-1)} title="Previous match (Shift+Enter)" aria-label="Previous match">
        <ChevronUp size={14} />
      </button>
      <button class="toggle-btn" disabled={matchCount === 0} onclick={() => stepMatch(1)} title="Next match (Enter)" aria-label="Next match">
        <ChevronDown size={14} />
      </button>
      <button class="toggle-btn" onclick={closeSearch} title="Close (Esc)" aria-label="Close search">
        <X size={14} />
      </button>
    </div>
  {/if}

  <div class="diff-files" bind:this={filesEl}>
    {#if loading}
      <div class="loading">
        <Loader2 size={20} class="spinner" />
        <span>Loading diff...</span>
      </div>
    {:else}
      <!-- Keyed by index too: the WIP diff concatenates staged + unstaged, so the
           same path can legitimately appear twice. -->
      {#each diff as file, fi (`${fi}:${fileKey(file)}`)}
        {@const expanded = expandedFiles.has(fileKey(file))}
        {@const lines = lineCount(file)}
        <div class="file-section">
          <button
            class="file-header"
            onclick={() => toggleFile(file)}
            aria-expanded={expanded}
            title={displayPath(file)}
          >
            <span class="status-badge {statusBadgeClass(file.status)}">{statusLabel(file.status)}</span>
            {#if isImageFile(file)}
              <Image size={14} />
            {:else if isAudioFile(file)}
              <Music size={14} />
            {:else if file.is_binary}
              <Binary size={14} />
            {:else if file.is_lfs}
              <Package size={14} />
            {:else}
              <FileText size={14} />
            {/if}
            <span class="file-path">{displayPath(file)}</span>
          </button>

          {#if expanded}
            <div class="file-diff">
              {#if file.is_lfs}
                <div class="lfs-notice">
                  <Package size={16} />
                  LFS object — {file.lfs_size ?? "unknown size"}
                </div>
              {:else if isImageFile(file)}
                {@const blob = imageBlobs[fileKey(file)]}
                <ImageDiff
                  oldData={blob?.old ?? null}
                  newData={blob?.new ?? null}
                  filePath={file.new_path ?? file.old_path ?? ""}
                  loading={blob?.loading ?? true}
                />
              {:else if isAudioFile(file)}
                {@const blob = imageBlobs[fileKey(file)]}
                <AudioPreview
                  oldData={blob?.old ?? null}
                  newData={blob?.new ?? null}
                  filePath={file.new_path ?? file.old_path ?? ""}
                  loading={blob?.loading ?? true}
                />
              {:else if file.is_binary}
                <div class="binary-notice">
                  <Binary size={16} />
                  Binary file
                </div>
              {:else if file.hunks.length === 0}
                <div class="binary-notice">
                  {#if file.status === "renamed" || file.status === "copied"}
                    File renamed without content changes
                  {:else if file.status === "added"}
                    New empty file
                  {:else if file.status === "deleted"}
                    Deleted empty file
                  {:else if !showWhitespace}
                    Only whitespace changes (hidden)
                    <button class="show-large-btn" onclick={toggleWhitespace}>Show whitespace</button>
                  {:else}
                    No content changes
                  {/if}
                </div>
              {:else if lines > LARGE_DIFF_LINES && !forceShown.has(fileKey(file))}
                <div class="binary-notice">
                  Large diff ({lines.toLocaleString()} lines) hidden
                  <button class="show-large-btn" onclick={() => showLarge(file)}>Show anyway</button>
                </div>
              {:else}
                {@const lang = fileLanguage(file)}
                {@const expandable = canExpand(file)}
                {#each file.hunks as hunk, hi (hi)}
                  {#if expandable}
                    {@render gapBlock(file, hi, lang)}
                  {/if}
                  <DiffHunk
                    {hunk}
                    mode={viewMode}
                    {tabSize}
                    wrap={wordWrap}
                    language={lang}
                    actions={actionArea}
                    busy={actionBusy}
                    search={searchQuery}
                    matchPrefix="{fi}:{hi}"
                    onaction={(action, sel) => runHunkAction(file, hunk, action, sel)}
                  />
                {/each}
                {#if expandable}
                  {@render gapBlock(file, file.hunks.length, lang)}
                {/if}
              {/if}
            </div>
          {/if}
        </div>
      {/each}
      {#if diff.length === 0 && !loading}
        <div class="empty-diff">No changes to display</div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .diff-viewer {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    outline: none;
  }

  .diff-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 12px;
    background: var(--color-surface);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .diff-title {
    font-size: 12px;
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .oid {
    font-family: var(--font-mono);
    color: var(--color-accent);
  }

  .label {
    color: var(--color-accent-secondary);
    font-weight: 500;
  }

  .view-toggle {
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
  }

  .sep {
    width: 1px;
    height: 16px;
    margin: 0 4px;
    background: var(--color-border);
  }

  .toggle-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 24px;
    border: 1px solid var(--color-border);
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    border-radius: 4px;
    padding: 0;
  }

  .toggle-btn:hover:not(:disabled) {
    color: var(--color-text-primary);
  }

  .toggle-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .toggle-btn.active {
    background: var(--color-surface-elevated);
    color: var(--color-accent);
    border-color: var(--color-accent);
  }

  .search-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 12px;
    background: var(--color-surface);
    border-bottom: 1px solid var(--color-border);
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .search-bar input {
    flex: 1;
    min-width: 0;
    height: 24px;
    padding: 0 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 12px;
    outline: none;
  }

  .search-bar input:focus {
    border-color: var(--color-accent);
  }

  .match-count {
    min-width: 64px;
    font-size: 11px;
    text-align: right;
    white-space: nowrap;
  }

  .diff-files {
    flex: 1;
    overflow-y: auto;
  }

  .loading,
  .empty-diff {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 16px;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .loading :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .file-section {
    border-bottom: 1px solid var(--color-border);
  }

  .file-header {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 12px;
    border: none;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
    text-align: left;
  }

  .file-header:hover {
    background: var(--color-surface-elevated);
  }

  .status-badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border-radius: 3px;
    font-size: 10px;
    font-weight: 700;
    flex-shrink: 0;
  }

  .badge-added {
    background: var(--color-diff-add-bg);
    color: var(--color-diff-add-text);
  }

  .badge-deleted {
    background: var(--color-diff-del-bg);
    color: var(--color-diff-del-text);
  }

  .badge-modified {
    background: color-mix(in srgb, var(--color-lane-2) 20%, transparent);
    color: var(--color-lane-2);
  }

  .badge-renamed {
    background: color-mix(in srgb, var(--color-accent) 20%, transparent);
    color: var(--color-accent);
  }

  .show-large-btn {
    padding: 2px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
  }

  .show-large-btn:hover {
    background: var(--color-surface-elevated);
  }

  .file-path {
    font-family: var(--font-mono);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file-diff {
    background: var(--color-bg);
  }

  .expander {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 1px 8px;
    background: var(--color-surface);
    border-bottom: 1px solid var(--color-border);
    font-size: 11px;
  }

  .exp-btn,
  .exp-label {
    display: flex;
    align-items: center;
    height: 20px;
    padding: 0 6px;
    border: none;
    border-radius: 3px;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 11px;
    cursor: pointer;
  }

  .exp-btn:hover:not(:disabled),
  .exp-label:hover:not(:disabled) {
    background: var(--color-surface-elevated);
    color: var(--color-accent);
  }

  .exp-btn:disabled,
  .exp-label:disabled {
    cursor: default;
    opacity: 0.6;
  }

  .lfs-notice,
  .binary-notice {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 16px 20px;
    color: var(--color-text-muted);
    font-size: 13px;
  }
</style>
