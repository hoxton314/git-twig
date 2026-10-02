<script lang="ts">
  /**
   * Context menu for commits in the graph, plus the keyboard / command-palette
   * actions that act on the selected commit. Mounted once by CommitGraph.
   */
  import { onMount } from "svelte";
  import ContextMenu, { type MenuItem } from "../shared/ContextMenu.svelte";
  import CreateBranchDialog from "./CreateBranchDialog.svelte";
  import SquashDialog from "./SquashDialog.svelte";
  import { toast } from "../../lib/stores/toasts";
  import CreateTagDialog from "../tags/CreateTagDialog.svelte";
  import { activeRepoPath } from "../../lib/stores/repos";
  import {
    branches,
    commitGraph,
    commitSelection,
    selectedCommitOid,
    selectedCommits,
  } from "../../lib/stores/graph";
  import { comparePair } from "../../lib/graphSelection";
  import {
    commitMenu,
    createBranchTarget,
    createTagTarget,
    squashTarget,
    undoHistoryOpen,
  } from "../../lib/stores/commitUi";
  import { onAction } from "../../lib/keybindings";
  import {
    checkoutCommitAction,
    cherryPickAction,
    cherryPickRangeAction,
    copyText,
    resetAction,
    revertAction,
  } from "../../lib/commitActions";
  import { pushTagAction } from "../../lib/tagActions";
  import { markBisect, startBisect } from "../../lib/bisectActions";
  import { abortOperation, bisectState, operationState } from "../../lib/stores/operation";
  import {
    applyPatchAction,
    patchFileName,
    savePatchesAction,
    saveWorkingPatchAction,
  } from "../../lib/patchActions";
  import { openRebaseDialog, openInteractiveRebase } from "../../lib/stores/operation";
  import type { CommitInfo } from "../../lib/types/git";

  const menu = $derived($commitMenu);
  const currentBranch = $derived($branches.find((b) => !b.is_remote && b.is_head) ?? null);

  function findCommit(oid: string | null): CommitInfo | null {
    if (!oid || oid === "__wip__") return null;
    return $commitGraph?.entries.find((e) => e.commit.oid === oid)?.commit ?? null;
  }

  const menuCommit = $derived(menu ? findCommit(menu.oid) : null);

  function fullMessage(c: CommitInfo): string {
    return c.body.trim() ? `${c.summary}\n\n${c.body.trim()}` : c.summary;
  }

  function itemsFor(c: CommitInfo): MenuItem[] {
    const path = $activeRepoPath;
    if (!path) return [];
    const isHead = currentBranch?.oid === c.oid;
    const branch = currentBranch?.name ?? null;
    const resetTarget = branch ?? "HEAD";
    const label = `${c.short_oid} ${c.summary}`;
    return [
      { label: "Checkout this commit (detached)", action: () => checkoutCommitAction(path, c.oid) },
      {
        label: "Create branch here…",
        action: () => createBranchTarget.set({ oid: c.oid, label }),
      },
      { label: "Create tag here…", action: () => createTagTarget.set({ oid: c.oid, label }) },
      { separator: true },
      { label: "Cherry-pick commit", disabled: isHead, action: () => cherryPickAction(path, c.oid) },
      { label: "Revert commit", action: () => revertAction(path, c.oid) },
      { separator: true },
      {
        label: `Rebase ${branch ?? "HEAD"} onto this commit…`,
        disabled: isHead,
        action: () => openRebaseDialog(c.oid),
      },
      {
        label: "Interactive rebase from here…",
        // Base is exclusive: start at the parent so this commit is included.
        action: () => openInteractiveRebase(c.parent_oids.length > 0 ? `${c.oid}^` : null),
      },
      { separator: true },
      {
        label: `Reset ${resetTarget} here — soft`,
        disabled: isHead,
        action: () => resetAction(path, c.oid, "soft", branch),
      },
      {
        label: `Reset ${resetTarget} here — mixed`,
        disabled: isHead,
        action: () => resetAction(path, c.oid, "mixed", branch),
      },
      {
        label: `Reset ${resetTarget} here — hard`,
        danger: true,
        action: () => resetAction(path, c.oid, "hard", branch),
      },
      { separator: true },
      { label: "Copy SHA", action: () => copyText(c.oid, "SHA") },
      { label: "Copy short SHA", action: () => copyText(c.short_oid, "short SHA") },
      { label: "Copy message", action: () => copyText(fullMessage(c), "commit message") },
      {
        label: "Save as patch…",
        disabled: c.parent_oids.length > 1,
        action: () => savePatchesAction(path, [c.oid], { defaultName: patchFileName(c.short_oid, c.summary) }),
      },
      { separator: true },
      ...bisectItems(c),
      { separator: true },
      { label: "Apply patch file…", action: () => applyPatchAction(path) },
      { label: "Undo history…", shortcut: "Ctrl+Shift+H", action: () => undoHistoryOpen.set(true) },
    ];
  }

  /** Bisect entries for one commit: start from it, or mark it. */
  function bisectItems(c: CommitInfo): MenuItem[] {
    const b = $bisectState;
    if ($operationState?.kind === "bisect" && b) {
      if (b.first_bad) return [];
      return [
        { label: `Bisect: mark as ${b.term_good}`, action: () => markBisect("good", c.oid) },
        { label: `Bisect: mark as ${b.term_bad}`, action: () => markBisect("bad", c.oid) },
        { label: "Bisect: skip", action: () => markBisect("skip", c.oid) },
      ];
    }
    if ($operationState && $operationState.kind !== "none") return [];
    return [
      { label: "Start bisect: this commit is bad", action: () => startBisect(c.oid, null) },
      { label: "Start bisect: this commit is good", action: () => startBisect(null, c.oid) },
    ];
  }

  /** Menu for a right-click inside a multi-selection. */
  function multiItemsFor(c: CommitInfo, oids: string[]): MenuItem[] {
    const pair = comparePair(oids);
    const shorts = oids.map((o) => findCommit(o)?.short_oid ?? o.slice(0, 7));
    const path = $activeRepoPath;
    const items: MenuItem[] = [];
    if (path) {
      items.push(
        {
          label: `Cherry-pick ${oids.length} commits onto ${currentBranch?.name ?? "HEAD"}`,
          action: () => cherryPickRangeAction(path, oids),
        },
        { label: `Squash ${oids.length} commits…`, action: () => squashTarget.set([...oids]) },
        { separator: true },
      );
    }
    if (pair && $operationState?.kind === "none") {
      items.push({
        label: `Start bisect: ${pair.to.slice(0, 7)} bad, ${pair.from.slice(0, 7)} good`,
        action: () => startBisect(pair.to, pair.from),
      });
    }
    if (pair) {
      items.push(
        { label: "Copy compared range (older..newer)", action: () => copyText(`${pair.from}..${pair.to}`, "range") },
        { separator: true },
      );
    }
    if (path) {
      items.push(
        { label: `Save ${oids.length} commits as one patch file…`, action: () => savePatchesAction(path, oids) },
        {
          label: `Save ${oids.length} commits as patch files…`,
          action: () => savePatchesAction(path, oids, { folder: true }),
        },
        { separator: true },
      );
    }
    items.push(
      { label: `Copy ${oids.length} SHAs`, action: () => copyText(oids.join("\n"), "SHAs") },
      { label: `Copy ${oids.length} short SHAs`, action: () => copyText(shorts.join("\n"), "short SHAs") },
      { separator: true },
      {
        label: `Select only ${c.short_oid}`,
        action: () => {
          commitSelection.set({ oids: [], anchor: c.oid });
          selectedCommitOid.set(c.oid);
        },
      },
    );
    return items;
  }

  /** Open the menu for the selected commit, anchored to its row. */
  function openForSelected() {
    // Shift+F10 is global; other lists (files, branches) handle it for their
    // own focused rows, so only act when focus is in the graph or nowhere.
    const active = document.activeElement;
    if (active && active !== document.body && !active.closest(".commit-graph")) return;
    const oid = $selectedCommitOid;
    if (!findCommit(oid)) return;
    const row = document.querySelector<HTMLElement>(".commit-graph .commit-row.selected");
    const area = document.querySelector<HTMLElement>(".commit-graph");
    const rect = row?.getBoundingClientRect() ?? area?.getBoundingClientRect();
    if (!rect) return;
    const x = row ? rect.left + Math.min(240, rect.width / 3) : rect.left + rect.width / 2;
    const y = row ? rect.bottom : rect.top + rect.height / 3;
    commitMenu.set({ oid: oid as string, x, y });
  }

  /** Run `fn` with the active repo path and the selected commit, if any. */
  function withSelected(fn: (path: string, c: CommitInfo) => void) {
    return () => {
      const path = $activeRepoPath;
      const c = findCommit($selectedCommitOid);
      if (path && c) fn(path, c);
    };
  }

  onMount(() => {
    // The dedicated Menu key opens the menu when focus is in the graph.
    function onKeydown(e: KeyboardEvent) {
      if (e.key !== "ContextMenu" || e.defaultPrevented) return;
      const active = document.activeElement;
      if (!active?.closest(".commit-graph")) return;
      e.preventDefault();
      openForSelected();
    }
    window.addEventListener("keydown", onKeydown);

    const unsubs = [
      onAction("commit_context_menu", openForSelected),
      onAction(
        "branch_from_selected",
        withSelected((_, c) => createBranchTarget.set({ oid: c.oid, label: `${c.short_oid} ${c.summary}` })),
      ),
      onAction(
        "create_tag",
        withSelected((_, c) => createTagTarget.set({ oid: c.oid, label: `${c.short_oid} ${c.summary}` })),
      ),
      onAction(
        "cherry_pick_selected",
        withSelected((p, c) => {
          const oids = $selectedCommits;
          if (oids.length > 1) cherryPickRangeAction(p, oids);
          else cherryPickAction(p, c.oid);
        }),
      ),
      onAction(
        "squash_selected",
        withSelected(() => {
          const oids = $selectedCommits;
          if (oids.length > 1) squashTarget.set([...oids]);
          else toast("info", "Select two or more commits (Ctrl/Shift-click) to squash them.");
        }),
      ),
      onAction("revert_selected", withSelected((p, c) => revertAction(p, c.oid))),
      onAction(
        "copy_commit_sha",
        withSelected((_, c) => {
          const oids = $selectedCommits;
          if (oids.length > 1) copyText(oids.join("\n"), "SHAs");
          else copyText(c.oid, "SHA");
        }),
      ),
      onAction(
        "save_patch_selected",
        withSelected((p, c) => {
          const oids = $selectedCommits;
          if (oids.length > 1) savePatchesAction(p, oids);
          else savePatchesAction(p, [c.oid], { defaultName: patchFileName(c.short_oid, c.summary) });
        }),
      ),
      onAction("save_working_patch", () => {
        if ($activeRepoPath) saveWorkingPatchAction($activeRepoPath);
      }),
      onAction("apply_patch", () => {
        if ($activeRepoPath) applyPatchAction($activeRepoPath);
      }),
      onAction("bisect_start", withSelected((_, c) => startBisect(c.oid, null))),
      onAction("bisect_good", () => markBisect("good")),
      onAction("bisect_bad", () => markBisect("bad")),
      onAction("bisect_skip", () => markBisect("skip")),
      onAction("bisect_reset", () => {
        if ($operationState?.kind === "bisect") abortOperation();
      }),
      onAction("push_all_tags", () => {
        if ($activeRepoPath) pushTagAction($activeRepoPath);
      }),
    ];
    return () => {
      window.removeEventListener("keydown", onKeydown);
      unsubs.forEach((fn) => fn());
      commitMenu.set(null);
    };
  });
</script>

{#if menu && menuCommit}
  <ContextMenu
    x={menu.x}
    y={menu.y}
    items={$selectedCommits.length > 1 && $selectedCommits.includes(menuCommit.oid)
      ? multiItemsFor(menuCommit, $selectedCommits)
      : itemsFor(menuCommit)}
    onclose={() => commitMenu.set(null)}
  />
{/if}

<CreateBranchDialog />
<SquashDialog />
<CreateTagDialog />
