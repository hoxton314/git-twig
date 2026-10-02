<script lang="ts">
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { open as openUrl } from "@tauri-apps/plugin-shell";
  import { X } from "lucide-svelte";
  import {
    updater,
    checkForUpdates,
    installUpdate,
    dismissUpdateBanner,
    skipUpdateVersion,
    ensureUpdaterSupport,
  } from "../../lib/stores/updater";
  import { settings, settingsReady } from "../../lib/stores/settings";
  import { renderMarkdown } from "../../lib/markdown";
  import { toastError } from "../../lib/stores/toasts";

  const u = $derived($updater);
  const progressPercent = $derived(u.total > 0 ? Math.round((u.downloaded / u.total) * 100) : 0);
  const notesHtml = $derived(u.notes ? renderMarkdown(u.notes) : "");
  let showNotes = $state(false);

  onMount(() => {
    // Wait for settings so the "check on startup" preference is honored.
    let done = false;
    const unsub = settingsReady.subscribe((ready) => {
      if (!ready || done) return;
      done = true;
      queueMicrotask(() => unsub());
      ensureUpdaterSupport().then((ok) => {
        if (ok && get(settings).check_updates_on_startup) checkForUpdates(false);
      });
    });
    return () => {
      if (!done) unsub();
    };
  });

  /** Open release-note links in the system browser, never in the webview. */
  function onNotesClick(e: MouseEvent) {
    const a = (e.target as HTMLElement | null)?.closest?.("a[data-external]") as HTMLAnchorElement | null;
    if (!a) return;
    e.preventDefault();
    const href = a.getAttribute("href") ?? "";
    if (/^https?:\/\//i.test(href)) openUrl(href).catch((err) => toastError("Could not open link", err));
  }
</script>

{#if u.bannerVisible && (u.status === "available" || u.status === "downloading" || u.status === "error")}
  <div class="update-banner" role="region" aria-label="Application update">
    <div class="update-header">
      <span class="update-title">
        {#if u.status === "error"}
          Update failed
        {:else}
          Twig {u.version} is available
        {/if}
      </span>
      {#if u.status !== "downloading"}
        <button class="icon-btn" onclick={dismissUpdateBanner} aria-label="Dismiss update notification" title="Dismiss">
          <X size={14} />
        </button>
      {/if}
    </div>

    {#if u.status === "error"}
      <p class="update-error">{u.error}</p>
    {/if}

    {#if notesHtml && u.status !== "error"}
      <button class="notes-toggle" onclick={() => (showNotes = !showNotes)} aria-expanded={showNotes}>
        {showNotes ? "Hide release notes" : "Show release notes"}
      </button>
      {#if showNotes}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="release-notes" onclick={onNotesClick}>
          <!-- Safe: renderMarkdown escapes all input and only emits whitelisted tags. -->
          {@html notesHtml}
        </div>
      {/if}
    {/if}

    <div class="update-actions">
      {#if u.status === "downloading"}
        <div
          class="progress-bar"
          role="progressbar"
          aria-label="Downloading update"
          aria-valuemin="0"
          aria-valuemax="100"
          aria-valuenow={u.total > 0 ? progressPercent : undefined}
        >
          <div class="progress-fill" class:indeterminate={u.total === 0} style="width: {u.total > 0 ? progressPercent : 30}%"></div>
        </div>
        <span class="progress-label" aria-live="polite">{u.total > 0 ? `${progressPercent}%` : "Downloading…"}</span>
      {:else if u.status === "error"}
        <button class="btn-update" onclick={installUpdate}>Retry</button>
        <button class="btn-secondary" onclick={dismissUpdateBanner}>Close</button>
      {:else}
        <button class="btn-update" onclick={installUpdate}>Update &amp; Restart</button>
        <button class="btn-secondary" onclick={skipUpdateVersion}>Skip this version</button>
        <button class="btn-secondary" onclick={dismissUpdateBanner}>Later</button>
      {/if}
    </div>
  </div>
{/if}

<style>
  .update-banner {
    position: fixed;
    bottom: 32px;
    right: 1rem;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: 8px;
    padding: 0.75rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    z-index: 900;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4);
    width: 380px;
    max-width: calc(100vw - 2rem);
  }
  .update-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }
  .update-title {
    color: var(--color-text-primary);
    font-weight: 600;
    font-size: 0.875rem;
  }
  .update-error {
    margin: 0;
    color: var(--color-diff-del-text);
    font-size: 0.75rem;
    word-break: break-word;
  }
  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    padding: 0;
    border: none;
    border-radius: 3px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
  }
  .icon-btn:hover {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }
  .notes-toggle {
    align-self: flex-start;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--color-accent);
    font-size: 0.75rem;
    cursor: pointer;
  }
  .notes-toggle:hover {
    text-decoration: underline;
  }
  .release-notes {
    max-height: 240px;
    overflow-y: auto;
    padding: 0.5rem 0.75rem;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    color: var(--color-text-primary);
    font-size: 0.75rem;
    line-height: 1.5;
  }
  .release-notes :global(h4),
  .release-notes :global(h5),
  .release-notes :global(h6) {
    margin: 0.5rem 0 0.25rem;
    font-size: 0.8rem;
  }
  .release-notes :global(p) {
    margin: 0.25rem 0;
  }
  .release-notes :global(ul),
  .release-notes :global(ol) {
    margin: 0.25rem 0;
    padding-left: 1.25rem;
  }
  .release-notes :global(code) {
    font-family: var(--font-mono);
    font-size: 0.7rem;
    padding: 0 3px;
    border-radius: 3px;
    background: var(--color-surface-elevated);
  }
  .release-notes :global(pre) {
    overflow-x: auto;
    padding: 0.5rem;
    border-radius: 4px;
    background: var(--color-surface-elevated);
  }
  .release-notes :global(pre code) {
    padding: 0;
    background: transparent;
  }
  .release-notes :global(a) {
    color: var(--color-accent);
  }
  .update-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  .btn-update {
    background: var(--color-accent);
    color: var(--color-bg);
    border: none;
    border-radius: 4px;
    padding: 0.375rem 0.75rem;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-update:hover {
    filter: brightness(1.1);
  }
  .btn-secondary {
    background: transparent;
    color: var(--color-text-muted);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    padding: 0.375rem 0.75rem;
    font-size: 0.8rem;
    cursor: pointer;
  }
  .btn-secondary:hover {
    color: var(--color-text-primary);
    border-color: var(--color-text-muted);
  }
  .progress-bar {
    flex: 1;
    height: 6px;
    background: var(--color-border);
    border-radius: 3px;
    overflow: hidden;
  }
  .progress-fill {
    height: 100%;
    background: var(--color-accent);
    transition: width 0.2s ease;
  }
  .progress-fill.indeterminate {
    animation: slide 1.2s ease-in-out infinite;
  }
  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }
  .progress-label {
    color: var(--color-text-muted);
    font-size: 0.75rem;
    min-width: 2.5rem;
  }
</style>
