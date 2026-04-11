<script lang="ts">
  import { onMount } from "svelte";
  import * as tauri from "../../lib/tauri";
  import type { GitConfig } from "../../lib/types/git";

  let userName = $state("");
  let userEmail = $state("");
  let pullRebase = $state<"false" | "true" | "ff-only">("false");
  let fetchPrune = $state(false);
  let gpgSign = $state(false);
  let signingKey = $state("");
  let loading = $state(true);
  let lfsInstalled = $state(false);
  let saving = $state(false);
  let saveError = $state<string | null>(null);

  onMount(async () => {
    try {
      const cfg = await tauri.getGitConfig();
      userName = cfg.user_name;
      userEmail = cfg.user_email;
      pullRebase = cfg.pull_rebase;
      fetchPrune = cfg.fetch_prune;
      gpgSign = cfg.gpg_sign;
      signingKey = cfg.signing_key;
      lfsInstalled = cfg.lfs_installed;
    } catch (e) {
      console.error("Failed to load git config:", e);
    }
    loading = false;
  });

  let saveTimeout: ReturnType<typeof setTimeout> | null = null;

  function scheduleGitConfigSave() {
    saveError = null;
    if (saveTimeout) clearTimeout(saveTimeout);
    saveTimeout = setTimeout(async () => {
      saving = true;
      try {
        const config: GitConfig = {
          user_name: userName,
          user_email: userEmail,
          pull_rebase: pullRebase,
          fetch_prune: fetchPrune,
          gpg_sign: gpgSign,
          signing_key: signingKey,
          lfs_installed: lfsInstalled,
        };
        await tauri.setGitConfig(config);
      } catch (e) {
        saveError = String(e);
        console.error("Failed to save git config:", e);
      }
      saving = false;
    }, 500);
  }

  const pullOptions = [
    { value: "false", label: "Merge (default)" },
    { value: "true", label: "Rebase" },
    { value: "ff-only", label: "Fast-forward only" },
  ];
</script>

<div class="section">
  <h1 class="section-heading">Git Configuration</h1>

  {#if loading}
    <p class="loading-text">Loading git config...</p>
  {:else}
    <p class="section-desc">
      These settings map to your global <code>~/.gitconfig</code>. Changes are written directly to git config.
    </p>

    <div class="setting-group">
      <h2 class="group-heading">Identity</h2>

      <div class="setting-row">
        <div class="setting-label">
          <span class="label-text">User name</span>
          <span class="label-hint">git config --global user.name</span>
        </div>
        <div class="setting-control">
          <input
            type="text"
            class="text-input"
            placeholder="Your Name"
            value={userName}
            onchange={(e) => { userName = e.currentTarget.value; scheduleGitConfigSave(); }}
          />
        </div>
      </div>

      <div class="setting-row">
        <div class="setting-label">
          <span class="label-text">Email</span>
          <span class="label-hint">git config --global user.email</span>
        </div>
        <div class="setting-control">
          <input
            type="text"
            class="text-input"
            placeholder="you@example.com"
            value={userEmail}
            onchange={(e) => { userEmail = e.currentTarget.value; scheduleGitConfigSave(); }}
          />
        </div>
      </div>
    </div>

    <div class="setting-group">
      <h2 class="group-heading">Pull & Fetch</h2>

      <div class="setting-row">
        <div class="setting-label">
          <span class="label-text">Pull strategy</span>
          <span class="label-hint">How to reconcile divergent branches on pull</span>
        </div>
        <div class="setting-control">
          <select
            value={pullRebase}
            onchange={(e) => { pullRebase = e.currentTarget.value as typeof pullRebase; scheduleGitConfigSave(); }}
          >
            {#each pullOptions as opt (opt.value)}
              <option value={opt.value}>{opt.label}</option>
            {/each}
          </select>
        </div>
      </div>

      <div class="setting-row">
        <div class="setting-label">
          <span class="label-text">Auto-prune on fetch</span>
          <span class="label-hint">Remove stale remote-tracking branches when fetching</span>
        </div>
        <div class="setting-control">
          <label class="toggle">
            <input
              type="checkbox"
              checked={fetchPrune}
              onchange={() => { fetchPrune = !fetchPrune; scheduleGitConfigSave(); }}
            />
            <span class="toggle-slider"></span>
          </label>
        </div>
      </div>
    </div>

    <div class="setting-group">
      <h2 class="group-heading">Signing</h2>

      <div class="setting-row">
        <div class="setting-label">
          <span class="label-text">GPG sign commits</span>
          <span class="label-hint">Automatically sign all commits with your GPG key</span>
        </div>
        <div class="setting-control">
          <label class="toggle">
            <input
              type="checkbox"
              checked={gpgSign}
              onchange={() => { gpgSign = !gpgSign; scheduleGitConfigSave(); }}
            />
            <span class="toggle-slider"></span>
          </label>
        </div>
      </div>

      {#if gpgSign}
        <div class="setting-row">
          <div class="setting-label">
            <span class="label-text">Signing key</span>
            <span class="label-hint">GPG key ID for commit signing</span>
          </div>
          <div class="setting-control">
            <input
              type="text"
              class="text-input"
              placeholder="Key ID"
              value={signingKey}
              onchange={(e) => { signingKey = e.currentTarget.value; scheduleGitConfigSave(); }}
            />
          </div>
        </div>
      {/if}
    </div>

    <div class="setting-group">
      <h2 class="group-heading">LFS</h2>

      <div class="setting-row">
        <div class="setting-label">
          <span class="label-text">Git LFS</span>
          <span class="label-hint">Large File Storage support</span>
        </div>
        <div class="setting-control">
          <span class="status-badge" class:installed={lfsInstalled}>
            {lfsInstalled ? "Installed" : "Not detected"}
          </span>
        </div>
      </div>
    </div>

    {#if saveError}
      <div class="notice error">{saveError}</div>
    {:else if saving}
      <div class="notice saving">Saving to git config...</div>
    {/if}
  {/if}
</div>

<style>
  .section {
    max-width: 640px;
  }

  .section-heading {
    font-size: 20px;
    font-weight: 600;
    color: var(--color-text-primary);
    margin: 0 0 8px;
  }

  .section-desc {
    font-size: 12px;
    color: var(--color-text-muted);
    margin: 0 0 24px;
  }

  .section-desc code {
    font-family: var(--font-mono);
    font-size: 11px;
    padding: 1px 5px;
    background: var(--color-surface-elevated);
    border-radius: 3px;
  }

  .loading-text {
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .setting-group {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-bottom: 24px;
  }

  .group-heading {
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--color-text-muted);
    margin: 0 0 8px;
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
    font-family: var(--font-mono);
  }

  .setting-control {
    flex-shrink: 0;
  }

  .text-input {
    padding: 6px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    font-family: inherit;
  }

  .text-input:hover {
    border-color: var(--color-text-muted);
  }

  .text-input:focus {
    outline: none;
    border-color: var(--color-accent);
  }

  .text-input {
    width: 200px;
  }

  .text-input::placeholder {
    color: var(--color-text-muted);
  }

  .status-badge {
    font-size: 11px;
    padding: 3px 10px;
    border-radius: 3px;
    background: rgba(247, 118, 142, 0.1);
    color: var(--color-diff-del-text);
  }

  .status-badge.installed {
    background: rgba(158, 206, 106, 0.1);
    color: var(--color-diff-add-text);
  }

  .notice {
    margin-top: 16px;
    padding: 10px 14px;
    border-radius: 4px;
    font-size: 12px;
  }

  .notice.saving {
    background: rgba(122, 162, 247, 0.1);
    color: var(--color-accent);
    border: 1px solid rgba(122, 162, 247, 0.2);
  }

  .notice.error {
    background: rgba(247, 118, 142, 0.1);
    color: var(--color-diff-del-text);
    border: 1px solid rgba(247, 118, 142, 0.2);
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
