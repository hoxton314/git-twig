<script lang="ts">
  /** Home screen: repository groups (create, rename, delete, open all, remove repos). */
  import { tick } from "svelte";
  import { FolderGit2, FolderOpen, Layers, LayoutDashboard, Pencil, Plus, Trash2, X } from "lucide-svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { openRepos, activeRepoPath } from "../../lib/stores/repos";
  import {
    repoGroups,
    repoHistory,
    missingRepoPaths,
    createRepoGroup,
    renameRepoGroup,
    deleteRepoGroup,
    removeRepoFromGroup,
    repoDisplayName,
  } from "../../lib/stores/repoHistory";
  import { openRepoGroup } from "../../lib/groupActions";
  import { openDashboard } from "../../lib/dashboard";
  import { MAX_GROUP_NAME } from "../../lib/repoGroups";
  import { settings } from "../../lib/stores/settings";
  import { t, tr } from "../../lib/i18n";

  interface Props {
    /** Open (or switch to) a single repository. */
    onopen: (path: string) => void;
  }
  let { onopen }: Props = $props();

  let editing = $state<string | null>(null);
  let draft = $state("");
  let input = $state<HTMLInputElement | null>(null);

  async function startRename(id: string, name: string) {
    editing = id;
    draft = name;
    await tick();
    input?.select();
  }

  function onRenameKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") commitRename();
    else if (e.key === "Escape") {
      e.stopPropagation();
      editing = null;
    }
  }

  function commitRename() {
    if (editing && draft.trim()) renameRepoGroup(editing, draft);
    editing = null;
  }

  async function newGroup() {
    const id = await createRepoGroup(tr("groups.defaultName"));
    const g = $repoGroups.find((x) => x.id === id);
    if (g) startRename(id, g.name);
  }

  async function remove(id: string, name: string, count: number) {
    if (count > 0 && $settings.confirm_destructive_ops) {
      const ok = await ask(tr("groups.deleteConfirm", { name, count }), {
        title: tr("groups.deleteTitle"),
        kind: "warning",
        okLabel: tr("common.delete"),
      });
      if (!ok) return;
    }
    deleteRepoGroup(id);
  }
</script>

<section class="open-repos groups" aria-labelledby="groups-heading">
  <div class="section-header">
    <h2 class="section-title" id="groups-heading"><Layers size={13} /> {$t("groups.heading")}</h2>
    <span class="header-actions">
      <button class="link-btn" onclick={() => openDashboard(null)} title={$t("groups.dashboardAllTitle")}><LayoutDashboard size={12} /> {$t("groups.dashboard")}</button>
      <button class="link-btn" onclick={newGroup}><Plus size={12} /> {$t("groups.newGroup")}</button>
    </span>
  </div>
  {#if $repoGroups.length === 0}
    <p class="empty">{$t("groups.empty")}</p>
  {/if}
  {#each $repoGroups as g (g.id)}
    {@const openCount = g.paths.filter((p) => $openRepos.has(p)).length}
    <div class="group">
      <div class="group-head">
        {#if editing === g.id}
          <input
            bind:this={input}
            bind:value={draft}
            class="rename"
            maxlength={MAX_GROUP_NAME - 4}
            aria-label={$t("groups.nameLabel")}
            onkeydown={onRenameKeydown}
            onblur={commitRename}
          />
        {:else}
          <span class="group-name" title={g.name}>{g.name}</span>
          <span class="count">{$t("tabs.repoCount", { count: g.paths.length })}{openCount > 0 ? $t("groups.openCount", { count: openCount }) : ""}</span>
        {/if}
        <span class="spacer"></span>
        <button class="link-btn" disabled={g.paths.length === 0} onclick={() => openDashboard(g.id)} title={$t("groups.dashboardGroupTitle")}>
          <LayoutDashboard size={12} /> {$t("groups.dashboard")}
        </button>
        <button class="link-btn" disabled={g.paths.length === 0} onclick={() => openRepoGroup(g.id)} title={$t("groups.openAllTitle")}>
          <FolderOpen size={12} /> {$t("groups.openAll")}
        </button>
        <button class="icon-btn" onclick={() => startRename(g.id, g.name)} title={$t("common.rename")} aria-label={$t("groups.renameNamed", { name: g.name })}><Pencil size={12} /></button>
        <button class="icon-btn" onclick={() => remove(g.id, g.name, g.paths.length)} title={$t("groups.deleteGroup")} aria-label={$t("groups.deleteNamed", { name: g.name })}><Trash2 size={12} /></button>
      </div>
      {#if g.paths.length > 0}
        <div class="members">
          {#each g.paths as path (path)}
            {@const missing = $missingRepoPaths.has(path)}
            <div class="member" class:missing>
              <button
                class="member-open"
                class:active={$activeRepoPath === path}
                disabled={missing}
                onclick={() => onopen(path)}
                title={missing ? $t("groups.folderNotFound", { path }) : path}
              >
                <FolderGit2 size={13} />
                <span class="member-name">{repoDisplayName($repoHistory, path)}</span>
                {#if missing}<span class="badge">{$t("tabs.missing")}</span>{:else if $openRepos.has(path)}<span class="badge">{$t("tabs.open")}</span>{/if}
              </button>
              <button class="icon-btn" onclick={() => removeRepoFromGroup(g.id, path)} title={$t("groups.removeFromGroup")} aria-label={$t("groups.removeNamed", { path, name: g.name })}>
                <X size={12} />
              </button>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/each}
</section>

<style>
  .open-repos { width: 100%; max-width: 480px; }
  .section-header { display: flex; align-items: center; justify-content: space-between; margin: 0 0 2px; }
  .section-title {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--color-text-muted);
  }
  .link-btn { padding: 0; border: none; background: transparent; color: var(--color-text-muted); font-size: 11px; cursor: pointer; }
  .link-btn:hover:not(:disabled) { color: var(--color-text-primary); }
  .groups { display: flex; flex-direction: column; gap: 8px; }
  .header-actions { display: flex; gap: 12px; }
  .empty { margin: 0; font-size: 12px; color: var(--color-text-muted); }
  .group { border: 1px solid var(--color-border); border-radius: 6px; background: var(--color-surface); }
  .group-head { display: flex; align-items: center; gap: 8px; padding: 6px 10px; }
  .group-name { font-size: 13px; font-weight: 600; color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .count { font-size: 11px; color: var(--color-text-muted); white-space: nowrap; }
  .spacer { flex: 1; }
  .rename {
    flex: 1;
    min-width: 0;
    padding: 3px 6px;
    border: 1px solid var(--color-accent);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 13px;
    outline: none;
  }
  .members { display: flex; flex-wrap: wrap; gap: 4px; padding: 0 8px 8px; }
  .member { display: flex; align-items: center; border: 1px solid var(--color-border); border-radius: 4px; background: var(--color-bg); }
  .member-open {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 3px 4px 3px 7px;
    background: none;
    border: none;
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
  }
  .member-open:disabled { cursor: default; color: var(--color-text-muted); }
  .member-open.active .member-name { color: var(--color-accent); }
  .badge { font-size: 10px; color: var(--color-text-muted); }
  .member.missing { opacity: 0.6; }
  .icon-btn { display: flex; background: none; border: none; padding: 3px; color: var(--color-text-muted); cursor: pointer; }
  .icon-btn:hover { color: var(--color-text-primary); }
  .link-btn { display: flex; align-items: center; gap: 3px; }
  .link-btn:disabled { opacity: 0.5; cursor: default; }
</style>
