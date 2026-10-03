<script lang="ts">
  import { ChevronDown, ChevronRight, Plus, Search, Tag, Upload } from "lucide-svelte";
  import ContextMenu, { type MenuItem } from "../shared/ContextMenu.svelte";
  import { activeRepoPath } from "../../lib/stores/repos";
  import { commitGraph } from "../../lib/stores/graph";
  import { tags, tagsLoading, refreshTags } from "../../lib/stores/tags";
  import { createTagTarget, revealCommit } from "../../lib/stores/commitUi";
  import { checkoutCommitAction, copyText } from "../../lib/commitActions";
  import { deleteRemoteTagAction, deleteTagAction, pushTagAction } from "../../lib/tagActions";
  import type { TagInfo } from "../../lib/types/git";
  import { t } from "../../lib/i18n";

  const repoPath = $derived($activeRepoPath);
  const allTags = $derived($tags);

  let expanded = $state(true);
  let filter = $state("");
  const filterLc = $derived(filter.trim().toLowerCase());
  const visible = $derived(
    filterLc
      ? allTags.filter(
          (tag) => tag.name.toLowerCase().includes(filterLc) || (tag.message ?? "").toLowerCase().includes(filterLc),
        )
      : allTags,
  );

  let menu = $state<{ tag: TagInfo; x: number; y: number } | null>(null);

  // Reload whenever the graph is (re)loaded: every refreshAll after a write,
  // fetch, focus, or repo switch goes through it, so tags stay in sync.
  $effect(() => {
    const path = repoPath;
    void $commitGraph;
    if (path) refreshTags(path);
  });

  function tooltip(tag: TagInfo): string {
    const lines = [tag.name];
    if (tag.short_target_oid) lines.push(`→ ${tag.short_target_oid} ${tag.commit_summary ?? ""}`.trimEnd());
    if (tag.annotated) {
      const date = new Date(tag.timestamp * 1000).toLocaleString();
      lines.push(
        tag.tagger_name ? $t("tags.annotatedBy", { name: tag.tagger_name, date }) : $t("tags.annotated", { date }),
      );
      if (tag.message) lines.push("", tag.message);
    } else {
      lines.push($t("tags.lightweight"));
    }
    return lines.join("\n");
  }

  function show(tag: TagInfo) {
    if (tag.target_oid) revealCommit(tag.target_oid);
  }

  function items(tag: TagInfo): MenuItem[] {
    const path = repoPath;
    if (!path) return [];
    const target = tag.target_oid;
    return [
      { label: $t("tags.showInGraph"), disabled: !target, action: () => show(tag) },
      {
        label: $t("tags.checkoutDetached"),
        disabled: !target,
        action: () => target && checkoutCommitAction(path, target),
      },
      { separator: true },
      { label: $t("tags.pushToRemote"), action: () => pushTagAction(path, tag.name) },
      { label: $t("tags.copyName"), action: () => copyText(tag.name, $t("tags.nameWhat")) },
      { separator: true },
      { label: $t("tags.delete"), danger: true, action: () => deleteTagAction(path, tag) },
      { label: $t("tags.deleteFromRemote"), danger: true, action: () => deleteRemoteTagAction(path, tag) },
    ];
  }

  function openMenu(e: MouseEvent, tag: TagInfo) {
    e.preventDefault();
    menu = { tag, x: e.clientX, y: e.clientY };
  }

  function onRowKeydown(e: KeyboardEvent, tag: TagInfo) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      show(tag);
    } else if (e.key === "ContextMenu" || (e.key === "F10" && e.shiftKey)) {
      e.preventDefault();
      e.stopPropagation();
      const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
      menu = { tag, x: r.left + 24, y: r.bottom };
    }
  }

  function onFilterKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") filter = "";
  }

  function createAtHead() {
    $createTagTarget = { oid: "HEAD", label: "HEAD" };
  }
</script>

<div class="tag-list">
  <div class="group-header">
    <button class="toggle" onclick={() => (expanded = !expanded)} aria-expanded={expanded}>
      {#if expanded}<ChevronDown size={14} />{:else}<ChevronRight size={14} />{/if}
      <Tag size={14} />
      <span>{$t("tags.title")}</span>
      <span class="count">{filterLc ? `${visible.length}/${allTags.length}` : allTags.length}</span>
    </button>
    <button class="icon-btn" onclick={createAtHead} title={$t("tags.newAtHead")} aria-label={$t("tags.newAtHead")}>
      <Plus size={13} />
    </button>
    <button
      class="icon-btn"
      onclick={() => repoPath && pushTagAction(repoPath)}
      disabled={allTags.length === 0}
      title={$t("tags.pushAll")}
      aria-label={$t("tags.pushAll")}
    >
      <Upload size={13} />
    </button>
  </div>

  {#if expanded}
    {#if allTags.length > 8 || filter}
      <div class="filter-input">
        <Search size={12} />
        <input
          type="text"
          placeholder={$t("tags.filterPlaceholder")}
          aria-label={$t("tags.filterLabel")}
          bind:value={filter}
          onkeydown={onFilterKeydown}
        />
      </div>
    {/if}

    {#if $tagsLoading && allTags.length === 0}
      <div class="empty">{$t("tags.loading")}</div>
    {:else if allTags.length === 0}
      <div class="empty">{$t("tags.none")}</div>
    {:else if visible.length === 0}
      <div class="empty">{$t("tags.noMatch", { filter })}</div>
    {:else}
      {#each visible as tag (tag.name)}
        <div
          class="tag-item"
          role="button"
          tabindex="0"
          title={tooltip(tag)}
          onclick={() => show(tag)}
          onkeydown={(e) => onRowKeydown(e, tag)}
          oncontextmenu={(e) => openMenu(e, tag)}
        >
          <Tag size={11} class={tag.annotated ? "tag-icon annotated" : "tag-icon"} />
          <span class="tag-name">{tag.name}</span>
          {#if tag.short_target_oid}<span class="oid">{tag.short_target_oid}</span>{/if}
        </div>
      {/each}
    {/if}
  {/if}
</div>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={items(menu.tag)} onclose={() => (menu = null)} />
{/if}

<style>
  .tag-list { display: flex; flex-direction: column; font-size: 12px; padding-bottom: 8px; }
  .group-header { display: flex; align-items: center; padding-right: 8px; }
  .toggle {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border: none;
    background: none;
    color: var(--color-text-muted);
    font-size: 11px;
    font-weight: 500;
    text-align: left;
    cursor: pointer;
  }
  .toggle:hover { color: var(--color-text-primary); }
  .count { margin-left: auto; opacity: 0.5; }
  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
  }
  .icon-btn:hover:not(:disabled) { background: var(--color-surface-elevated); color: var(--color-text-primary); }
  .icon-btn:disabled { opacity: 0.4; cursor: default; }
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
  .filter-input:focus-within { border-color: var(--color-accent); }
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
  .empty { padding: 4px 12px 4px 32px; color: var(--color-text-muted); font-style: italic; }
  .tag-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px 5px 24px;
    color: var(--color-text-primary);
    cursor: pointer;
    transition: background 0.1s;
  }
  .tag-item:hover, .tag-item:focus-visible { background: var(--color-surface-elevated); outline: none; }
  .tag-item :global(.tag-icon) { flex-shrink: 0; color: var(--color-text-muted); }
  .tag-item :global(.tag-icon.annotated) { color: var(--color-lane-2); }
  .tag-name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .oid { flex-shrink: 0; font-family: var(--font-mono); font-size: 10px; color: var(--color-text-muted); }
</style>
