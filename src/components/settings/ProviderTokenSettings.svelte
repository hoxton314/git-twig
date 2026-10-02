<!--
  Instance URL + access token for a self-hostable provider (GitLab, Gitea /
  Forgejo). The token is stored in the OS keyring, keyed by instance host,
  and never comes back to the UI.
-->
<script lang="ts">
  import { Check, AlertCircle, Loader2 } from "lucide-svelte";
  import { open as openUrl } from "@tauri-apps/plugin-shell";
  import { onMount } from "svelte";
  import * as tauri from "../../lib/tauri";
  import { globalSettings, updateGlobalSettings, flushSettings } from "../../lib/stores/settings";
  import { clearCi } from "../../lib/stores/ci";
  import type { GitHubUser } from "../../lib/types/github";

  interface Props {
    provider: "gitlab" | "gitea";
    title: string;
    urlPlaceholder: string;
    /** Path of the token page relative to the instance URL. */
    tokenPath: string;
    scopesHint: string;
  }

  let { provider, title, urlPlaceholder, tokenPath, scopesHint }: Props = $props();

  const urlKey = $derived(provider === "gitlab" ? "gitlab_base_url" : "gitea_base_url");
  const savedUrl = $derived(($globalSettings[urlKey] as string) ?? "");

  let urlInput = $state("");
  let tokenInput = $state("");
  let hasToken = $state(false);
  let status = $state<"idle" | "loading" | "connected" | "error">("idle");
  let user = $state<GitHubUser | null>(null);
  let errorMsg = $state("");
  let seq = 0;

  onMount(() => {
    urlInput = savedUrl;
    refresh();
  });

  const normalizedUrl = $derived(urlInput.trim().replace(/\/+$/, ""));
  const tokenPage = $derived(
    savedUrl ? `${/^https?:\/\//i.test(savedUrl) ? "" : "https://"}${savedUrl.replace(/\/+$/, "")}${tokenPath}` : "",
  );

  async function refresh() {
    const my = ++seq;
    user = null;
    errorMsg = "";
    if (!savedUrl.trim()) {
      hasToken = false;
      status = "idle";
      return;
    }
    status = "loading";
    try {
      hasToken = await tauri.hostingHasToken(provider);
      if (my !== seq) return;
      if (!hasToken) {
        status = "idle";
        return;
      }
      const u = await tauri.hostingValidateToken(provider);
      if (my !== seq) return;
      user = u;
      status = "connected";
    } catch (err) {
      if (my !== seq) return;
      status = "error";
      errorMsg = String(err);
    }
  }

  async function saveUrl() {
    if (normalizedUrl === savedUrl) return;
    updateGlobalSettings({ [urlKey]: normalizedUrl });
    // The backend reads settings.json, so persist before re-checking.
    await flushSettings().catch(() => {});
    clearCi();
    refresh();
  }

  async function saveToken() {
    const token = tokenInput.trim();
    if (!token) return;
    await saveUrl();
    try {
      await tauri.hostingSetToken(provider, token);
      tokenInput = "";
    } catch (err) {
      status = "error";
      errorMsg = String(err);
      return;
    }
    clearCi();
    refresh();
  }

  async function clearToken() {
    try {
      await tauri.hostingSetToken(provider, null);
      tokenInput = "";
      clearCi();
      refresh();
    } catch (err) {
      status = "error";
      errorMsg = String(err);
    }
  }
</script>

<h2 class="sub-heading">{title}</h2>
<div class="setting-group">
  <div class="setting-row">
    <div class="setting-label">
      <span class="label-text">Instance URL</span>
      <span class="label-hint">Remotes on this host get pull request and CI integration.</span>
    </div>
    <div class="setting-control">
      <input
        type="text"
        class="text-input"
        spellcheck="false"
        placeholder={urlPlaceholder}
        aria-label="{title} instance URL"
        bind:value={urlInput}
        onblur={saveUrl}
        onkeydown={(e) => e.key === "Enter" && saveUrl()}
      />
    </div>
  </div>

  <div class="setting-row">
    <div class="setting-label">
      <span class="label-text">Access token</span>
      <span class="label-hint">
        {#if tokenPage}
          Create one at
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <span class="link" role="link" tabindex="0" onclick={() => openUrl(tokenPage)} onkeydown={(e) => e.key === "Enter" && openUrl(tokenPage)}>{tokenPath.replace(/^\//, "")}</span>
          with {scopesHint}.
        {:else}
          Set the instance URL first.
        {/if}
        Stored in your system keyring.
      </span>
    </div>
    <div class="setting-control token-control">
      <input
        type="password"
        class="token-input"
        autocomplete="off"
        spellcheck="false"
        aria-label="{title} access token"
        placeholder={hasToken ? "Saved in system keyring — paste to replace" : "Token"}
        disabled={!normalizedUrl}
        bind:value={tokenInput}
        onkeydown={(e) => e.key === "Enter" && saveToken()}
      />
      <button class="btn-secondary" onclick={saveToken} disabled={!tokenInput.trim() || !normalizedUrl}>Save</button>
      {#if hasToken}
        <button class="btn-ghost" onclick={clearToken}>Clear</button>
      {/if}
    </div>
  </div>

  <div class="setting-row">
    <div class="setting-label">
      <span class="label-text">Connection status</span>
    </div>
    <div class="setting-control">
      {#if status === "loading"}
        <div class="status-badge muted"><Loader2 size={14} class="spinner" /><span>Verifying...</span></div>
      {:else if status === "connected" && user}
        <div class="status-badge connected"><Check size={14} /><span>Connected as <strong>@{user.login}</strong></span></div>
      {:else if status === "error"}
        <div class="status-badge error" title={errorMsg}><AlertCircle size={14} /><span>{errorMsg}</span></div>
      {:else}
        <div class="status-badge muted"><span>Not configured</span></div>
      {/if}
    </div>
  </div>
</div>

<style>
  .sub-heading {
    font-size: 15px;
    font-weight: 600;
    color: var(--color-text-primary);
    margin: 28px 0 8px;
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

  .link {
    color: var(--color-accent);
    cursor: pointer;
    text-decoration: underline;
    text-decoration-style: dotted;
    text-underline-offset: 2px;
  }

  .setting-control {
    flex-shrink: 0;
  }

  .token-control {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .text-input,
  .token-input {
    padding: 6px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    font-family: var(--font-mono);
  }

  .text-input {
    width: 240px;
  }

  .token-input {
    width: 180px;
  }

  .text-input:focus,
  .token-input:focus {
    outline: none;
    border-color: var(--color-accent);
  }

  .token-input:disabled {
    opacity: 0.5;
  }

  .btn-secondary {
    padding: 5px 12px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
  }

  .btn-secondary:hover:not(:disabled) {
    border-color: var(--color-accent);
  }

  .btn-secondary:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .btn-ghost {
    padding: 5px 8px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 11px;
    cursor: pointer;
  }

  .btn-ghost:hover {
    color: var(--color-diff-del-text);
  }

  .status-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: 4px;
    max-width: 300px;
  }

  .status-badge span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .status-badge.muted {
    color: var(--color-text-muted);
  }

  .status-badge.connected {
    color: var(--color-diff-add-text);
    background: var(--color-diff-add-bg);
  }

  .status-badge.error {
    color: var(--color-diff-del-text);
    background: var(--color-diff-del-bg);
  }

  .status-badge :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
