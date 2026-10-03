<script lang="ts">
  /** Sidebar section listing submodules (hidden when the repo has none). */
  import { Boxes, ChevronDown, ChevronRight, Download, RefreshCw } from "lucide-svelte";
  import ContextMenu, { type MenuItem } from "../shared/ContextMenu.svelte";
  import { submodules, openAsTab, updateSubmodules, syncSubmodules } from "../../lib/stores/repotools";
  import { toast, toastError } from "../../lib/stores/toasts";
  import type { SubmoduleInfo } from "../../lib/types/git";
  import { t, tr, type MessageKey } from "../../lib/i18n";

  const list = $derived($submodules);
  let expanded = $state(true);
  let busy = $state<string | null>(null);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  const STATUS_LABEL: Record<SubmoduleInfo["status"], MessageKey> = {
    uninitialized: "submodules.statusUninitialized",
    out_of_date: "submodules.statusOutOfDate",
    dirty: "submodules.statusDirty",
    up_to_date: "submodules.statusUpToDate",
  };

  const needsUpdate = $derived(list.some((s) => s.status === "uninitialized" || s.status === "out_of_date"));

  function short(oid: string | null): string {
    return oid ? oid.slice(0, 7) : "—";
  }

  function tooltip(s: SubmoduleInfo): string {
    return [
      s.path,
      s.url ? $t("submodules.tipUrl", { url: s.url }) : null,
      s.branch ? $t("submodules.tipBranch", { branch: s.branch }) : null,
      $t("submodules.tipOids", { recorded: short(s.head_oid), checkedOut: short(s.workdir_oid) }),
      $t("submodules.tipStatus", { status: $t(STATUS_LABEL[s.status]) }),
      s.status === "uninitialized" ? $t("submodules.tipInitialize") : $t("submodules.tipOpen"),
    ]
      .filter(Boolean)
      .join("\n");
  }

  async function run(key: string, fn: () => Promise<void>) {
    if (busy) return;
    busy = key;
    try {
      await fn();
    } finally {
      busy = null;
    }
  }

  function activate(s: SubmoduleInfo) {
    if (s.status === "uninitialized") run(s.path, () => updateSubmodules(s.path));
    else openAsTab(s.abs_path);
  }

  function openMenu(e: MouseEvent, s: SubmoduleInfo) {
    e.preventDefault();
    menu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: tr("submodules.openInNewTab"), action: () => openAsTab(s.abs_path), disabled: s.status === "uninitialized" },
        {
          label: s.status === "uninitialized" ? tr("submodules.initUpdate") : tr("submodules.updateToRecorded"),
          action: () => run(s.path, () => updateSubmodules(s.path)),
          disabled: busy !== null,
        },
        { label: tr("submodules.syncUrl"), action: () => run(s.path, () => syncSubmodules(s.path)) },
        { separator: true },
        {
          label: tr("submodules.copyUrl"),
          disabled: !s.url,
          action: async () => {
            try {
              await navigator.clipboard.writeText(s.url ?? "");
              toast("success", tr("submodules.urlCopied"));
            } catch (err) {
              toastError(tr("submodules.copyFailed"), err);
            }
          },
        },
      ],
    };
  }
</script>

{#if list.length > 0}
  <div class="section">
    <div class="section-header">
      <button class="title-btn" onclick={() => (expanded = !expanded)} aria-expanded={expanded}>
        {#if expanded}<ChevronDown size={12} />{:else}<ChevronRight size={12} />{/if}
        <span class="section-title">{$t("submodules.title")}</span>
        <span class="count">{list.length}</span>
      </button>
      <div class="actions">
        <button
          class="icon-btn"
          class:attention={needsUpdate}
          onclick={() => run("__all__", () => updateSubmodules())}
          disabled={busy !== null}
          title={$t("submodules.updateAllTitle")}
          aria-label={$t("submodules.updateAll")}
        >
          <Download size={13} />
        </button>
        <button
          class="icon-btn"
          onclick={() => run("__sync__", () => syncSubmodules())}
          disabled={busy !== null}
          title={$t("submodules.syncUrlsTitle")}
          aria-label={$t("submodules.syncUrls")}
        >
          <RefreshCw size={13} class={busy === "__sync__" ? "spinner" : ""} />
        </button>
      </div>
    </div>

    {#if expanded}
      {#each list as s (s.path)}
        <div
          class="item"
          role="button"
          tabindex="0"
          title={tooltip(s)}
          onclick={() => activate(s)}
          onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), activate(s))}
          oncontextmenu={(e) => openMenu(e, s)}
        >
          <Boxes size={13} />
          <span class="name">{s.path}</span>
          {#if busy === s.path || busy === "__all__"}
            <RefreshCw size={11} class="spinner" />
          {/if}
          <span class="status status-{s.status}">{$t(STATUS_LABEL[s.status])}</span>
        </div>
      {/each}
    {/if}
  </div>
{/if}

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<style>
  .section {
    display: flex;
    flex-direction: column;
    font-size: 12px;
    border-top: 1px solid var(--color-border);
    padding-bottom: 6px;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px 4px 8px;
  }

  .title-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    border: none;
    background: none;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 0;
  }

  .section-title {
    font-weight: 600;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .count {
    opacity: 0.6;
    margin-left: 4px;
  }

  .actions {
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

  .icon-btn:hover:not(:disabled) {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .icon-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .icon-btn.attention {
    color: var(--color-lane-2);
  }

  .item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px 5px 24px;
    color: var(--color-text-primary);
    cursor: pointer;
    min-width: 0;
  }

  .item:hover {
    background: var(--color-surface-elevated);
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .status {
    margin-left: auto;
    font-size: 10px;
    padding: 0 5px;
    border-radius: 3px;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .status-up_to_date {
    color: var(--color-text-muted);
  }

  .status-uninitialized {
    color: var(--color-text-muted);
    background: var(--color-surface-elevated);
  }

  .status-out_of_date {
    color: var(--color-lane-2);
    background: color-mix(in srgb, var(--color-lane-2) 15%, transparent);
  }

  .status-dirty {
    color: var(--color-accent-secondary);
    background: color-mix(in srgb, var(--color-accent-secondary) 15%, transparent);
  }

  .section :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
