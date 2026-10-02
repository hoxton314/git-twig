<script lang="ts">
  /**
   * "Search code" panel: `git grep` over the working tree or a commit, with
   * results grouped by file. A result opens blame at that line. Mounted once
   * in AppShell; open with `openCodeSearch()` or the `search_code` action.
   */
  import { onMount, tick } from "svelte";
  import Modal from "../shared/Modal.svelte";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { onAction } from "../../lib/keybindings";
  import { codeSearch, matchRanges, openCodeSearch, parsePaths, splitRuns, workingArea } from "../../lib/codeSearch";
  import { selectedCommitOid, selectedWorkingFile, workingFileDiff, workingStatus } from "../../lib/stores/graph";
  import { showBlame } from "../../lib/stores/fileviews";
  import { toastError } from "../../lib/stores/toasts";
  import * as tauri from "../../lib/tauri";
  import type { GrepResult } from "../../lib/types/git";
  import { ChevronDown, ChevronRight, ExternalLink, Loader2, X } from "lucide-svelte";

  const target = $derived($codeSearch);

  let pattern = $state("");
  let regex = $state(false);
  let matchCase = $state(false);
  let wholeWord = $state(false);
  let pathFilter = $state("");
  let result = $state<GrepResult | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let collapsed = $state<Set<string>>(new Set());
  let input = $state<HTMLInputElement | null>(null);

  const opts = $derived({ regex, ignoreCase: !matchCase, wholeWord });

  onMount(() =>
    onAction("search_code", () => {
      if ($activeRepoPath) openCodeSearch();
    }),
  );

  // Focus the box whenever the panel opens; results stay until the next search.
  $effect(() => {
    if (target) tick().then(() => input?.select());
  });

  let req = 0;

  // A different repo: old results (and a commit to search) don't apply,
  // and a search still running for the old one is dropped.
  let lastRepo: string | null = null;
  $effect(() => {
    const p = $activeRepoPath;
    if (p !== lastRepo) {
      lastRepo = p;
      req++;
      result = null;
      error = null;
      loading = false;
      $codeSearch = null;
    }
  });

  let timer: ReturnType<typeof setTimeout> | null = null;
  // Search as you type (debounced) once the query is long enough.
  $effect(() => {
    const t = target;
    const query = { pattern, regex, matchCase, wholeWord, pathFilter };
    if (!t) return;
    if (timer) clearTimeout(timer);
    if (query.pattern.length < 2) {
      // Too short: drop any running search and its stale results.
      req++;
      result = null;
      loading = false;
      return;
    }
    timer = setTimeout(() => run(), 350);
    return () => {
      if (timer) clearTimeout(timer);
    };
  });

  async function run() {
    const path = $activeRepoPath;
    const t = target;
    if (!path || !t || !pattern) return;
    const id = ++req;
    loading = true;
    error = null;
    try {
      const res = await tauri.searchCode(path, {
        pattern,
        regex,
        ignore_case: !matchCase,
        whole_word: wholeWord,
        rev: t.rev,
        paths: parsePaths(pathFilter),
      });
      if (id !== req) return;
      result = res;
      collapsed = new Set();
    } catch (err) {
      if (id !== req) return;
      result = null;
      error = err instanceof Error ? err.message : String(err);
    } finally {
      if (id === req) loading = false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      if (timer) clearTimeout(timer);
      run();
    }
  }

  function close() {
    $codeSearch = null;
  }

  async function openMatch(file: string, line: number) {
    const rev = target?.rev ?? undefined;
    const path = $activeRepoPath;
    const area = rev ? null : workingArea(file, $workingStatus);
    close();
    if (!area || !path) {
      showBlame(file, rev, line);
      return;
    }
    // Uncommitted changes: HEAD's lines differ, so show the file's diff.
    $selectedCommitOid = null;
    $selectedWorkingFile = { path: file, area };
    try {
      const diff = area === "staged" ? await tauri.getStagedDiff(path, file) : await tauri.getUnstagedDiff(path, file);
      if ($activeRepoPath === path && $selectedWorkingFile?.path === file) $workingFileDiff = diff;
    } catch (err) {
      toastError("Could not show the diff", err);
    }
  }

  async function openInEditor(file: string) {
    const path = $activeRepoPath;
    if (!path) return;
    try {
      await tauri.openInEditor(path, file);
    } catch (err) {
      toastError("Open in editor failed", err);
    }
  }

  function toggle(file: string) {
    const next = new Set(collapsed);
    if (next.has(file)) next.delete(file);
    else next.add(file);
    collapsed = next;
  }

  const fileCount = $derived(result?.files.length ?? 0);
</script>

<Modal open={!!target} title="Search Code" onclose={close} width="min(960px, 94vw)">
  {#if target}
    <div class="search">
      <div class="controls">
        <input
          bind:this={input}
          bind:value={pattern}
          onkeydown={onKeydown}
          class="query"
          type="text"
          placeholder={regex ? "Regular expression (POSIX extended)" : "Text to find"}
          spellcheck="false"
          autocomplete="off"
          aria-label="Search code"
        />
        <label class="opt" title="Treat the query as a regular expression (git grep -E)">
          <input type="checkbox" bind:checked={regex} /> .*
        </label>
        <label class="opt" title="Match case">
          <input type="checkbox" bind:checked={matchCase} /> Aa
        </label>
        <label class="opt" title="Whole words only (git grep -w)">
          <input type="checkbox" bind:checked={wholeWord} /> \b
        </label>
      </div>
      <div class="controls">
        <input
          bind:value={pathFilter}
          onkeydown={onKeydown}
          class="paths"
          type="text"
          placeholder="Limit to paths, e.g. src/ *.ts (optional)"
          spellcheck="false"
          autocomplete="off"
          aria-label="Limit to paths"
        />
        <span class="scope">
          {#if target.rev}
            In commit <code>{target.label ?? target.rev.slice(0, 7)}</code>
            <button class="clear" onclick={() => openCodeSearch()} title="Search the working tree instead" aria-label="Search the working tree instead">
              <X size={12} />
            </button>
          {:else}
            Tracked files in the working tree
          {/if}
        </span>
      </div>

      <div class="summary" aria-live="polite">
        {#if loading}
          <Loader2 size={13} class="spinner" /> Searching…
        {:else if error}
          <span class="error">{error}</span>
        {:else if result}
          {result.total.toLocaleString()} match{result.total === 1 ? "" : "es"} in {fileCount.toLocaleString()} file{fileCount === 1 ? "" : "s"}
          {#if result.truncated}<span class="muted"> — stopped at {result.total.toLocaleString()}; narrow the search to see the rest</span>{/if}
        {:else}
          <span class="muted">Type at least two characters, or press Enter.</span>
        {/if}
      </div>

      {#if result && result.files.length > 0}
        <div class="results">
          {#each result.files as file (file.path)}
            <div class="file">
              <div class="file-head">
                <button class="file-toggle" onclick={() => toggle(file.path)} aria-expanded={!collapsed.has(file.path)}>
                  {#if collapsed.has(file.path)}<ChevronRight size={13} />{:else}<ChevronDown size={13} />{/if}
                  <span class="file-path">{file.path}</span>
                  <span class="count">{file.matches.length}</span>
                </button>
                {#if !target.rev}
                  <button class="icon-btn" onclick={() => openInEditor(file.path)} title="Open in editor" aria-label="Open {file.path} in editor">
                    <ExternalLink size={12} />
                  </button>
                {/if}
              </div>
              {#if !collapsed.has(file.path)}
                {#each file.matches as m (m.line)}
                  <button class="match" onclick={() => openMatch(file.path, m.line)} title={!target.rev && workingArea(file.path, $workingStatus) ? "Has uncommitted changes: show its diff" : `Show blame at line ${m.line}`}>
                    <span class="ln">{m.line}</span>
                    <span class="text">{#each splitRuns(m.text, matchRanges(m.text, pattern, opts)) as seg, i (i)}{#if seg.hit}<mark>{seg.text}</mark>{:else}{seg.text}{/if}{/each}</span>
                  </button>
                {/each}
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</Modal>

<style>
  .search { display: flex; flex-direction: column; gap: 8px; font-size: 13px; min-height: 0; }
  .controls { display: flex; align-items: center; gap: 8px; }
  .query, .paths {
    flex: 1;
    min-width: 0;
    padding: 6px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-family: var(--font-mono);
    font-size: 12px;
    outline: none;
  }
  .query:focus, .paths:focus { border-color: var(--color-accent); }
  .opt {
    display: flex;
    align-items: center;
    gap: 3px;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-muted);
    cursor: pointer;
    user-select: none;
  }
  .opt input { margin: 0; accent-color: var(--color-accent); }
  .scope { display: flex; align-items: center; gap: 4px; font-size: 11px; color: var(--color-text-muted); white-space: nowrap; }
  .scope code { font-family: var(--font-mono); color: var(--color-accent); }
  .clear, .icon-btn {
    display: flex;
    background: none;
    border: none;
    padding: 2px;
    color: var(--color-text-muted);
    cursor: pointer;
  }
  .clear:hover, .icon-btn:hover { color: var(--color-text-primary); }
  .summary { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--color-text-primary); min-height: 18px; }
  .muted { color: var(--color-text-muted); }
  .error { color: var(--color-diff-del-text); white-space: pre-wrap; }
  .results {
    max-height: min(60vh, 640px);
    overflow: auto;
    border: 1px solid var(--color-border);
    border-radius: 4px;
  }
  .file + .file { border-top: 1px solid var(--color-border); }
  .file-head {
    position: sticky;
    top: 0;
    display: flex;
    align-items: center;
    background: var(--color-surface-elevated);
    padding-right: 6px;
  }
  .file-toggle {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 6px;
    background: none;
    border: none;
    color: var(--color-text-primary);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }
  .file-path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: var(--font-mono); }
  .count { margin-left: auto; font-size: 10px; color: var(--color-text-muted); }
  .match {
    display: flex;
    width: 100%;
    padding: 1px 6px 1px 22px;
    background: none;
    border: none;
    color: var(--color-text-primary);
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 18px;
    text-align: left;
    cursor: pointer;
  }
  .match:hover, .match:focus-visible { background: var(--color-surface); outline: none; }
  .ln { flex-shrink: 0; width: 48px; padding-right: 8px; text-align: right; color: var(--color-text-muted); }
  .text { white-space: pre; overflow: hidden; text-overflow: ellipsis; }
  mark { background: var(--color-search-hit-bg); color: var(--color-search-hit-text); border-radius: 2px; }
  :global(.search .spinner) { animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
