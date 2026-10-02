<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { open as openUrl } from "@tauri-apps/plugin-shell";
  import { RefreshCw, Loader2, CheckCircle2, AlertTriangle, Download, ExternalLink } from "lucide-svelte";
  import { settings, updateSettings } from "../../lib/stores/settings";
  import { updater, checkForUpdates, ensureUpdaterSupport, installUpdate } from "../../lib/stores/updater";
  import { renderMarkdown } from "../../lib/markdown";
  import { toastError } from "../../lib/stores/toasts";

  const RELEASES_URL = "https://github.com/hoxton314/git-twig/releases";

  const s = $derived($settings);
  const u = $derived($updater);
  const busy = $derived(u.status === "checking" || u.status === "downloading");
  const notesHtml = $derived(u.status === "available" && u.notes ? renderMarkdown(u.notes) : "");

  let version = $state("");

  onMount(() => {
    getVersion().then((v) => (version = v)).catch(() => {});
    ensureUpdaterSupport();
  });

  function formatTime(ms: number | null): string {
    return ms ? new Date(ms).toLocaleString() : "";
  }

  function openExternal(url: string) {
    openUrl(url).catch((err) => toastError("Could not open link", err));
  }

  function onNotesClick(e: MouseEvent) {
    const a = (e.target as HTMLElement | null)?.closest?.("a[data-external]") as HTMLAnchorElement | null;
    if (!a) return;
    e.preventDefault();
    const href = a.getAttribute("href") ?? "";
    if (/^https?:\/\//i.test(href)) openExternal(href);
  }
</script>

<div class="section">
  <h1 class="section-heading">About &amp; Updates</h1>

  <div class="about-card">
    <span class="app-name">Twig</span>
    <span class="app-version">Version {version || "…"}</span>
    <button class="link-btn" onclick={() => openExternal(RELEASES_URL)}>
      Release history <ExternalLink size={11} aria-hidden="true" />
    </button>
  </div>

  <div class="setting-group">
    {#if u.status === "unsupported"}
      <div class="setting-row">
        <div class="setting-label">
          <span class="label-text">Updates</span>
          <span class="label-hint">Updates are managed by your package manager (or this is a development build).</span>
        </div>
      </div>
    {:else}
      <div class="setting-row">
        <div class="setting-label">
          <span class="label-text">Check for updates</span>
          <span class="label-hint status-line" role="status" aria-live="polite">
            {#if u.status === "unknown"}
              Determining update support…
            {:else if u.status === "checking"}
              <Loader2 size={12} class="spin" aria-hidden="true" /> Checking…
            {:else if u.status === "up-to-date"}
              <CheckCircle2 size={12} class="ok" aria-hidden="true" /> Twig is up to date{u.lastChecked ? ` (checked ${formatTime(u.lastChecked)})` : ""}.
            {:else if u.status === "available"}
              <Download size={12} class="accent" aria-hidden="true" /> Version {u.version} is available.
            {:else if u.status === "downloading"}
              <Loader2 size={12} class="spin" aria-hidden="true" /> Downloading {u.version}…
            {:else if u.status === "error"}
              <AlertTriangle size={12} class="err" aria-hidden="true" /> Update check failed: {u.error}
            {:else}
              Not checked yet this session.
            {/if}
          </span>
        </div>
        <div class="setting-control actions">
          {#if u.status === "available"}
            <button class="btn-primary" onclick={installUpdate}>Update &amp; Restart</button>
          {/if}
          <button class="btn-secondary" onclick={() => checkForUpdates(true)} disabled={busy || u.status === "unknown"}>
            <RefreshCw size={12} aria-hidden="true" /> Check now
          </button>
        </div>
      </div>

      {#if notesHtml}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="release-notes" onclick={onNotesClick} aria-label="Release notes for {u.version}">
          <!-- Safe: renderMarkdown escapes all input and only emits whitelisted tags. -->
          {@html notesHtml}
        </div>
      {/if}

      <div class="setting-row">
        <div class="setting-label">
          <span class="label-text">Automatically check on startup</span>
          <span class="label-hint">Look for a new release each time Twig starts</span>
        </div>
        <div class="setting-control">
          <label class="toggle">
            <input
              type="checkbox"
              aria-label="Automatically check for updates on startup"
              checked={s.check_updates_on_startup}
              onchange={() => updateSettings({ check_updates_on_startup: !s.check_updates_on_startup })}
            />
            <span class="toggle-slider"></span>
          </label>
        </div>
      </div>

      {#if s.skipped_update_version}
        <div class="setting-row">
          <div class="setting-label">
            <span class="label-text">Skipped version</span>
            <span class="label-hint">You won't be prompted about version {s.skipped_update_version} at startup</span>
          </div>
          <div class="setting-control">
            <button class="btn-secondary" onclick={() => updateSettings({ skipped_update_version: null })}>
              Stop skipping
            </button>
          </div>
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .section {
    max-width: 640px;
  }

  .section-heading {
    font-size: 20px;
    font-weight: 600;
    color: var(--color-text-primary);
    margin: 0 0 24px;
  }

  .about-card {
    display: flex;
    align-items: baseline;
    gap: 12px;
    padding: 14px 16px;
    margin-bottom: 16px;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-surface);
  }

  .app-name {
    font-size: 16px;
    font-weight: 700;
    color: var(--color-text-primary);
  }

  .app-version {
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--color-text-muted);
  }

  .link-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-left: auto;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--color-accent);
    font-size: 12px;
    cursor: pointer;
  }

  .link-btn:hover {
    text-decoration: underline;
  }

  .setting-group {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    padding: 12px 0;
    border-bottom: 1px solid var(--color-border);
  }

  .setting-row:last-child {
    border-bottom: none;
  }

  .setting-label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }

  .label-text {
    font-size: 13px;
    font-weight: 500;
    color: var(--color-text-primary);
  }

  .label-hint {
    font-size: 11px;
    color: var(--color-text-muted);
  }

  .status-line {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex-wrap: wrap;
    word-break: break-word;
  }

  .status-line :global(.ok) {
    color: var(--color-diff-add-text);
  }

  .status-line :global(.err) {
    color: var(--color-diff-del-text);
  }

  .status-line :global(.accent) {
    color: var(--color-accent);
  }

  .status-line :global(.spin) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .setting-control {
    flex-shrink: 0;
  }

  .actions {
    display: flex;
    gap: 8px;
  }

  .btn-secondary,
  .btn-primary {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    border-radius: 4px;
    font-size: 12px;
    cursor: pointer;
  }

  .btn-secondary {
    border: 1px solid var(--color-border);
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .btn-secondary:hover:not(:disabled) {
    border-color: var(--color-accent);
  }

  .btn-secondary:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .btn-primary {
    border: 1px solid var(--color-accent);
    background: var(--color-accent);
    color: var(--color-bg);
    font-weight: 600;
  }

  .release-notes {
    max-height: 260px;
    overflow-y: auto;
    margin: 4px 0 8px;
    padding: 8px 12px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    font-size: 12px;
    line-height: 1.5;
    color: var(--color-text-primary);
  }

  .release-notes :global(h4),
  .release-notes :global(h5),
  .release-notes :global(h6) {
    margin: 8px 0 4px;
    font-size: 13px;
  }

  .release-notes :global(p) {
    margin: 4px 0;
  }

  .release-notes :global(ul),
  .release-notes :global(ol) {
    margin: 4px 0;
    padding-left: 20px;
  }

  .release-notes :global(code) {
    font-family: var(--font-mono);
    font-size: 11px;
    padding: 0 3px;
    border-radius: 3px;
    background: var(--color-surface-elevated);
  }

  .release-notes :global(pre) {
    overflow-x: auto;
    padding: 8px;
    border-radius: 4px;
    background: var(--color-surface-elevated);
  }

  .release-notes :global(a) {
    color: var(--color-accent);
  }

  /* Toggle switch */
  .toggle {
    position: relative;
    display: inline-block;
    width: 36px;
    height: 20px;
    cursor: pointer;
  }

  .toggle input {
    opacity: 0;
    width: 0;
    height: 0;
    position: absolute;
  }

  .toggle-slider {
    position: absolute;
    inset: 0;
    background: var(--color-border);
    border-radius: 10px;
    transition: background 0.15s;
  }

  .toggle-slider::before {
    content: "";
    position: absolute;
    width: 16px;
    height: 16px;
    left: 2px;
    bottom: 2px;
    background: var(--color-text-muted);
    border-radius: 50%;
    transition: transform 0.15s, background 0.15s;
  }

  .toggle input:checked + .toggle-slider {
    background: var(--color-accent);
  }

  .toggle input:checked + .toggle-slider::before {
    transform: translateX(16px);
    background: var(--color-text-primary);
  }
</style>
