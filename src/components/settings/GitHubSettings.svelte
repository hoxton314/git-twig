<script lang="ts">
  import { Check, AlertCircle, Loader2 } from "lucide-svelte";
  import { open as openUrl } from "@tauri-apps/plugin-shell";
  import { settings, updateSettings } from "../../lib/stores/settings";
  import * as tauri from "../../lib/tauri";
  import type { GitHubUser } from "../../lib/types/github";
  import { onMount } from "svelte";

  let tokenInput = $state("");
  let status = $state<"idle" | "loading" | "connected" | "error">("idle");
  let user = $state<GitHubUser | null>(null);
  let errorMsg = $state("");

  const s = $derived($settings);

  onMount(() => {
    tokenInput = s.github_token ?? "";
    if (tokenInput) {
      validateToken();
    }
  });

  async function validateToken() {
    if (!tokenInput.trim()) {
      status = "idle";
      return;
    }
    status = "loading";
    // Save first so the backend can read it
    updateSettings({ github_token: tokenInput.trim() });
    try {
      user = await tauri.githubValidateToken();
      status = "connected";
      errorMsg = "";
    } catch (err) {
      status = "error";
      errorMsg = String(err);
      user = null;
    }
  }

  function handleSave() {
    updateSettings({ github_token: tokenInput.trim() || null });
    validateToken();
  }

  function handleClear() {
    tokenInput = "";
    updateSettings({ github_token: null });
    status = "idle";
    user = null;
    errorMsg = "";
  }
</script>

<div class="section">
  <h1 class="section-heading">GitHub</h1>

  <div class="setting-group">
    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Personal Access Token</span>
        <span class="label-hint">
          Create a token at
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <span class="link" onclick={() => openUrl("https://github.com/settings/tokens")}>github.com/settings/tokens</span>
          with <code>repo</code> scope
        </span>
      </div>
      <div class="setting-control token-control">
        <input
          type="password"
          class="token-input"
          placeholder="ghp_..."
          bind:value={tokenInput}
          onkeydown={(e) => e.key === "Enter" && handleSave()}
        />
        <button class="btn-secondary" onclick={handleSave}>Save</button>
        {#if s.github_token}
          <button class="btn-ghost" onclick={handleClear}>Clear</button>
        {/if}
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">Connection status</span>
      </div>
      <div class="setting-control">
        {#if status === "loading"}
          <div class="status-badge loading">
            <Loader2 size={14} class="spinner" />
            <span>Verifying...</span>
          </div>
        {:else if status === "connected" && user}
          <div class="status-badge connected">
            <Check size={14} />
            <span>Connected as <strong>@{user.login}</strong></span>
          </div>
        {:else if status === "error"}
          <div class="status-badge error">
            <AlertCircle size={14} />
            <span>{errorMsg}</span>
          </div>
        {:else}
          <div class="status-badge idle">
            <span>Not configured</span>
          </div>
        {/if}
      </div>
    </div>
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

  .label-hint code {
    font-family: var(--font-mono);
    background: var(--color-surface-elevated);
    padding: 1px 4px;
    border-radius: 3px;
    font-size: 10px;
  }

  .link {
    color: var(--color-accent);
    cursor: pointer;
    text-decoration: underline;
    text-decoration-style: dotted;
    text-underline-offset: 2px;
  }

  .link:hover {
    text-decoration-style: solid;
  }

  .setting-control {
    flex-shrink: 0;
  }

  .token-control {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .token-input {
    width: 180px;
    padding: 6px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    font-family: var(--font-mono);
  }

  .token-input:hover {
    border-color: var(--color-text-muted);
  }

  .token-input:focus {
    outline: none;
    border-color: var(--color-accent);
  }

  .btn-secondary {
    padding: 5px 12px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    font-size: 12px;
    cursor: pointer;
    transition: border-color 0.1s;
  }

  .btn-secondary:hover {
    border-color: var(--color-accent);
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
  }

  .status-badge.idle {
    color: var(--color-text-muted);
  }

  .status-badge.loading {
    color: var(--color-text-muted);
  }

  .status-badge.connected {
    color: #9ece6a;
    background: rgba(158, 206, 106, 0.1);
  }

  .status-badge.error {
    color: #f7768e;
    background: rgba(247, 118, 142, 0.1);
    max-width: 300px;
  }

  .status-badge.error span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .status-badge :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
