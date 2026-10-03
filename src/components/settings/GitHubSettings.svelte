<script lang="ts">
  import { Check, AlertCircle, Loader2, LogIn, Copy, ExternalLink } from "lucide-svelte";
  import { open as openUrl } from "@tauri-apps/plugin-shell";
  import * as tauri from "../../lib/tauri";
  import type { GitHubUser } from "../../lib/types/github";
  import type { DeviceFlowStart, HostingInfo } from "../../lib/types/hosting";
  import { onMount } from "svelte";
  import { globalSettings, updateGlobalSettings, flushSettings } from "../../lib/stores/settings";
  import { clearCi } from "../../lib/stores/ci";
  import { toast } from "../../lib/stores/toasts";
  import ProviderTokenSettings from "./ProviderTokenSettings.svelte";
  import { t, tr } from "../../lib/i18n";

  const GITEA_TITLE = "Gitea / Forgejo";

  let tokenInput = $state("");
  // The token lives in the OS keyring and is never sent to the UI; we only
  // know whether one is stored.
  let hasToken = $state(false);
  let status = $state<"idle" | "loading" | "connected" | "error">("idle");
  let user = $state<GitHubUser | null>(null);
  let errorMsg = $state("");

  let info = $state<HostingInfo | null>(null);
  let hostInput = $state("");
  let apiInput = $state("");

  // OAuth device flow
  let device = $state<DeviceFlowStart | null>(null);
  let deviceBusy = $state(false);
  let deviceError = $state("");

  const s = $derived($globalSettings);
  const host = $derived(info?.github_host ?? "github.com");
  const tokensUrl = $derived(`https://${host}/settings/tokens`);

  onMount(() => {
    hostInput = s.github_host;
    apiInput = s.github_api_url;
    loadInfo();
    (async () => {
      try {
        hasToken = await tauri.githubHasToken();
      } catch (err) {
        status = "error";
        errorMsg = String(err);
        return;
      }
      if (hasToken) validateStoredToken();
    })();
    return () => {
      if (device) tauri.githubDeviceCancel(device.flow_id).catch(() => {});
    };
  });

  async function loadInfo() {
    try {
      info = await tauri.hostingInfo();
    } catch {
      info = null;
    }
  }

  let validationSeq = 0;

  async function validateStoredToken() {
    const seq = ++validationSeq;
    status = "loading";
    try {
      const u = await tauri.githubValidateToken();
      if (seq !== validationSeq) return; // superseded by a newer save/clear
      user = u;
      status = "connected";
      errorMsg = "";
    } catch (err) {
      if (seq !== validationSeq) return;
      status = "error";
      errorMsg = String(err);
      user = null;
    }
  }

  async function handleSave() {
    const token = tokenInput.trim();
    if (!token) return;
    const seq = ++validationSeq;
    status = "loading";
    try {
      await tauri.githubSetToken(token);
    } catch (err) {
      if (seq !== validationSeq) return;
      status = "error";
      errorMsg = String(err);
      user = null;
      return;
    }
    if (seq !== validationSeq) return;
    hasToken = true;
    tokenInput = "";
    clearCi();
    await validateStoredToken();
  }

  async function handleClear() {
    validationSeq++;
    tokenInput = "";
    try {
      await tauri.githubSetToken(null);
      hasToken = false;
      status = "idle";
      user = null;
      errorMsg = "";
      clearCi();
    } catch (err) {
      status = "error";
      errorMsg = String(err);
    }
  }

  // ── Sign in with GitHub (device flow) ──────────────────────────────

  let deviceCancelled = false;

  async function startDeviceFlow() {
    deviceError = "";
    deviceBusy = true;
    deviceCancelled = false;
    try {
      const flow = await tauri.githubDeviceStart();
      device = flow;
      navigator.clipboard.writeText(flow.user_code).catch(() => {});
      openUrl(flow.verification_uri);
      const u = await tauri.githubDeviceWait(flow.flow_id);
      if (device?.flow_id !== flow.flow_id) return;
      validationSeq++;
      hasToken = true;
      user = u;
      status = "connected";
      errorMsg = "";
      clearCi();
      toast("success", tr("hosting.signedInAs", { login: u.login }));
    } catch (err) {
      if (!deviceCancelled) deviceError = String(err);
    } finally {
      device = null;
      deviceBusy = false;
    }
  }

  async function cancelDeviceFlow() {
    deviceCancelled = true;
    if (device) await tauri.githubDeviceCancel(device.flow_id).catch(() => {});
  }

  function copyCode() {
    if (!device) return;
    navigator.clipboard.writeText(device.user_code).then(
      () => toast("success", tr("hosting.codeCopied")),
      () => {},
    );
  }

  // ── HTTPS auth + GitHub Enterprise ────────────────────────────────

  async function persistAndRefresh() {
    // The backend reads settings.json, so write it before re-checking.
    await flushSettings().catch(() => {});
    clearCi();
    await loadInfo();
    if (hasToken) validateStoredToken();
  }

  function saveHost() {
    const h = hostInput.trim().replace(/^https?:\/\//i, "").replace(/\/.*$/, "").toLowerCase() || "github.com";
    const api = apiInput.trim().replace(/\/+$/, "");
    hostInput = h;
    apiInput = api;
    if (h === s.github_host && api === s.github_api_url) return;
    updateGlobalSettings({ github_host: h, github_api_url: api });
    persistAndRefresh();
  }

  function toggleHttpsAuth() {
    updateGlobalSettings({ github_https_auth: !s.github_https_auth });
    flushSettings().catch(() => {});
  }
</script>

<div class="section">
  <h1 class="section-heading">GitHub</h1>

  <div class="setting-group">
    {#if info?.github_oauth_available}
      <div class="setting-row">
        <div class="setting-label">
          <span class="label-text">{$t("hosting.signInWithGitHub")}</span>
          <span class="label-hint">{$t("hosting.signInHint")}</span>
        </div>
        <div class="setting-control">
          {#if device}
            <div class="device-box">
              <span class="device-hint">{$t("hosting.enterCode")}</span>
              <div class="device-code-row">
                <code class="device-code">{device.user_code}</code>
                <button class="icon-btn" onclick={copyCode} title={$t("hosting.copyCode")} aria-label={$t("hosting.copyCode")}><Copy size={13} /></button>
              </div>
              <div class="device-actions">
                <button class="btn-secondary" onclick={() => device && openUrl(device.verification_uri)}>
                  <ExternalLink size={12} /> {$t("hosting.openUrl", { url: device.verification_uri.replace(/^https?:\/\//, "") })}
                </button>
                <button class="btn-ghost" onclick={cancelDeviceFlow}>{$t("common.cancel")}</button>
              </div>
              <span class="device-wait"><Loader2 size={12} class="spinner" /> {$t("hosting.waiting")}</span>
            </div>
          {:else}
            <button class="btn-primary" onclick={startDeviceFlow} disabled={deviceBusy}>
              <LogIn size={14} />
              <span>{hasToken ? $t("hosting.signInAgain") : $t("hosting.signInWithGitHub")}</span>
            </button>
          {/if}
          {#if deviceError}
            <div class="device-error">{deviceError}</div>
          {/if}
        </div>
      </div>
    {/if}

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">{$t("hosting.pat")}</span>
        <span class="label-hint">
          {$t("hosting.patHintBefore")}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <span class="link" role="link" tabindex="0" onclick={() => openUrl(tokensUrl)} onkeydown={(e) => e.key === "Enter" && openUrl(tokensUrl)}>{`${host}/settings/tokens`}</span>
          {$t("hosting.patHintMiddle")} <code>{"repo"}</code>{$t("hosting.patHintAfter")}
        </span>
      </div>
      <div class="setting-control token-control">
        <input
          type="password"
          class="token-input"
          autocomplete="off"
          spellcheck="false"
          aria-label={$t("hosting.patLabel")}
          placeholder={hasToken ? $t("hosting.tokenSavedPlaceholder") : "ghp_..."}
          bind:value={tokenInput}
          onkeydown={(e) => e.key === "Enter" && handleSave()}
        />
        <button class="btn-secondary" onclick={handleSave} disabled={!tokenInput.trim()}>{$t("common.save")}</button>
        {#if hasToken}
          <button class="btn-ghost" onclick={handleClear}>{$t("common.clear")}</button>
        {/if}
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">{$t("hosting.connectionStatus")}</span>
      </div>
      <div class="setting-control">
        {#if status === "loading"}
          <div class="status-badge loading">
            <Loader2 size={14} class="spinner" />
            <span>{$t("hosting.verifying")}</span>
          </div>
        {:else if status === "connected" && user}
          <div class="status-badge connected">
            <Check size={14} />
            <span>{$t("hosting.connectedAs")} <strong>@{user.login}</strong></span>
          </div>
        {:else if status === "error"}
          <div class="status-badge error" title={errorMsg}>
            <AlertCircle size={14} />
            <span>{errorMsg}</span>
          </div>
        {:else}
          <div class="status-badge idle">
            <span>{$t("hosting.notConfigured")}</span>
          </div>
        {/if}
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">{$t("hosting.httpsAuth")}</span>
        <span class="label-hint">
          {$t("hosting.httpsAuthHintBefore")} <code>https://{host}/</code>{$t("hosting.httpsAuthHintAfter")}
        </span>
      </div>
      <div class="setting-control">
        <label class="toggle">
          <input type="checkbox" checked={s.github_https_auth} onchange={toggleHttpsAuth} aria-label={$t("hosting.httpsAuthLabel")} />
          <span class="toggle-slider"></span>
        </label>
      </div>
    </div>

    <div class="setting-row">
      <div class="setting-label">
        <span class="label-text">{$t("hosting.githubHost")}</span>
        <span class="label-hint">
          <code>{"github.com"}</code>{$t("hosting.githubHostHint")}
        </span>
      </div>
      <div class="setting-control">
        <input
          type="text"
          class="text-input"
          spellcheck="false"
          placeholder={"github.com"}
          aria-label={$t("hosting.githubHost")}
          bind:value={hostInput}
          onblur={saveHost}
          onkeydown={(e) => e.key === "Enter" && saveHost()}
        />
      </div>
    </div>

    {#if hostInput.trim() && hostInput.trim().toLowerCase() !== "github.com"}
      <div class="setting-row">
        <div class="setting-label">
          <span class="label-text">{$t("hosting.apiUrl")}</span>
          <span class="label-hint">
            {$t("hosting.apiUrlDefaults")} <code>{info?.github_api_base ?? `https://${hostInput.trim()}/api/v3`}</code>.
          </span>
        </div>
        <div class="setting-control">
          <input
            type="text"
            class="text-input"
            spellcheck="false"
            placeholder={`https://${hostInput.trim()}/api/v3`}
            aria-label={$t("hosting.apiUrlLabel")}
            bind:value={apiInput}
            onblur={saveHost}
            onkeydown={(e) => e.key === "Enter" && saveHost()}
          />
        </div>
      </div>
    {/if}
  </div>

  <ProviderTokenSettings
    provider="gitlab"
    title="GitLab"
    urlPlaceholder="https://gitlab.com"
    tokenPath="/-/user_settings/personal_access_tokens"
    scopesHint={$t("hosting.gitlabScopes")}
  />

  <ProviderTokenSettings
    provider="gitea"
    title={GITEA_TITLE}
    urlPlaceholder="https://codeberg.org"
    tokenPath="/user/settings/applications"
    scopesHint={$t("hosting.giteaScopes")}
  />
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
  }

  .status-badge.idle {
    color: var(--color-text-muted);
  }

  .status-badge.loading {
    color: var(--color-text-muted);
  }

  .status-badge.connected {
    color: var(--color-diff-add-text);
    background: var(--color-diff-add-bg);
  }

  .status-badge.error {
    color: var(--color-diff-del-text);
    background: var(--color-diff-del-bg);
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

  /* ── Hosting integrations ─────────────────────────────────────── */
  .text-input {
    width: 220px;
    padding: 6px 10px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 12px;
    font-family: var(--font-mono);
  }

  .text-input:focus {
    outline: none;
    border-color: var(--color-accent);
  }

  .btn-primary {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 14px;
    border: none;
    border-radius: 4px;
    background: var(--color-accent);
    color: var(--color-bg);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
  }

  .btn-primary:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .btn-secondary {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }

  .icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
  }

  .icon-btn:hover {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .device-box {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 6px;
  }

  .device-hint,
  .device-wait {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    color: var(--color-text-muted);
  }

  .device-wait :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  .device-code-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .device-code {
    font-family: var(--font-mono);
    font-size: 18px;
    font-weight: 600;
    letter-spacing: 2px;
    padding: 4px 10px;
    border-radius: 4px;
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .device-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .device-error {
    margin-top: 6px;
    max-width: 300px;
    font-size: 11px;
    color: var(--color-diff-del-text);
  }

  .label-hint code {
    word-break: break-all;
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

  .toggle input:focus-visible + .toggle-slider {
    outline: 1px solid var(--color-accent);
    outline-offset: 2px;
  }
</style>
