<script lang="ts">
  import { onMount } from "svelte";
  import * as tauri from "../../lib/tauri";
  import type { GitConfig, SigningKey } from "../../lib/types/git";
  import { toast } from "../../lib/stores/toasts";
  import { clearSignatureCache } from "../../lib/stores/signatures";
  import { t, tr } from "../../lib/i18n";

  let userName = $state("");
  let userEmail = $state("");
  let pullRebase = $state<"false" | "true" | "ff-only">("false");
  let fetchPrune = $state(false);
  let gpgSign = $state(false);
  let signingKey = $state("");
  /** "openpgp" | "ssh", or another git value (e.g. "x509") kept unchanged. */
  let gpgFormat = $state("openpgp");
  let keys = $state<SigningKey[]>([]);
  let keysLoading = $state(false);
  let testing = $state(false);
  let loading = $state(true);
  let lfsInstalled = $state(false);
  let saving = $state(false);
  let saveError = $state<string | null>(null);
  let loadError = $state<string | null>(null);

  onMount(async () => {
    try {
      const cfg = await tauri.getGitConfig();
      userName = cfg.user_name;
      userEmail = cfg.user_email;
      pullRebase = cfg.pull_rebase;
      fetchPrune = cfg.fetch_prune;
      gpgSign = cfg.gpg_sign;
      signingKey = cfg.signing_key;
      gpgFormat = cfg.gpg_format || "openpgp";
      lfsInstalled = cfg.lfs_installed;
    } catch (e) {
      // Don't show (and later save) placeholder values over the real config.
      loadError = String(e);
      console.error("Failed to load git config:", e);
    }
    loading = false;
  });

  // Keys for the chosen format, refreshed when it changes.
  $effect(() => {
    if (loading || !gpgSign) return;
    const format = gpgFormat;
    if (format !== "openpgp" && format !== "ssh") {
      keys = [];
      return;
    }
    let cancelled = false;
    keysLoading = true;
    tauri
      .listSigningKeys(format)
      .then((k) => { if (!cancelled) keys = k; })
      .catch(() => { if (!cancelled) keys = []; })
      .finally(() => { if (!cancelled) keysLoading = false; });
    return () => { cancelled = true; };
  });

  async function runSigningTest() {
    if (testing) return;
    testing = true;
    try {
      // Make sure the latest choices are saved before testing them.
      if (saveTimeout) {
        clearTimeout(saveTimeout);
        saveTimeout = null;
        void saveNow();
      }
      await savePromise;
      if (saveError) {
        toast("error", tr("gitconfig.saveFailed", { error: saveError }), { title: tr("gitconfig.commitSigning") });
        return;
      }
      const res = await tauri.testSigning();
      toast(res.success ? "success" : "error", res.message, { title: tr("gitconfig.commitSigning") });
    } catch (e) {
      toast("error", String(e), { title: tr("gitconfig.commitSigning") });
    } finally {
      testing = false;
    }
  }

  let saveTimeout: ReturnType<typeof setTimeout> | null = null;

  /** The latest save (awaited by "Test signing"). */
  let savePromise: Promise<void> = Promise.resolve();

  function saveNow(): Promise<void> {
    savePromise = savePromise.then(doSave);
    return savePromise;
  }

  async function doSave() {
    saving = true;
    saveError = null;
    try {
      const config: GitConfig = {
        user_name: userName,
        user_email: userEmail,
        pull_rebase: pullRebase,
        fetch_prune: fetchPrune,
        gpg_sign: gpgSign,
        signing_key: signingKey,
        gpg_format: gpgFormat,
        lfs_installed: lfsInstalled,
      };
      await tauri.setGitConfig(config);
      // Signing / trust settings may have changed: re-verify graph badges.
      clearSignatureCache();
    } catch (e) {
      saveError = String(e);
      console.error("Failed to save git config:", e);
    }
    saving = false;
  }

  function scheduleGitConfigSave() {
    saveError = null;
    if (saveTimeout) clearTimeout(saveTimeout);
    saveTimeout = setTimeout(() => {
      saveTimeout = null;
      void saveNow();
    }, 500);
  }

  const pullOptions = $derived([
    { value: "false", text: $t("gitconfig.pullMerge") },
    { value: "true", text: $t("gitconfig.pullRebase") },
    { value: "ff-only", text: $t("gitconfig.pullFfOnly") },
  ]);
</script>

<div class="section">
  <h1 class="section-heading">{$t("gitconfig.heading")}</h1>

  {#if loading}
    <p class="loading-text">{$t("gitconfig.loading")}</p>
  {:else if loadError}
    <div class="notice error">{$t("gitconfig.loadFailed", { error: loadError })}</div>
  {:else}
    <p class="section-desc">
      {$t("gitconfig.descBefore")}<code>{"~/.gitconfig"}</code>{$t("gitconfig.descAfter")}
    </p>

    <div class="setting-group">
      <h2 class="group-heading">{$t("gitconfig.identity")}</h2>

      <div class="setting-row">
        <div class="setting-label">
          <span class="label-text">{$t("gitconfig.userName")}</span>
          <span class="label-hint">{"git config --global user.name"}</span>
        </div>
        <div class="setting-control">
          <input
            type="text"
            class="text-input"
            placeholder={$t("gitconfig.userNamePlaceholder")}
            value={userName}
            onchange={(e) => { userName = e.currentTarget.value.trim(); scheduleGitConfigSave(); }}
          />
        </div>
      </div>

      <div class="setting-row">
        <div class="setting-label">
          <span class="label-text">{$t("gitconfig.email")}</span>
          <span class="label-hint">{"git config --global user.email"}</span>
        </div>
        <div class="setting-control">
          <input
            type="text"
            class="text-input"
            placeholder={$t("gitconfig.emailPlaceholder")}
            value={userEmail}
            onchange={(e) => { userEmail = e.currentTarget.value.trim(); scheduleGitConfigSave(); }}
          />
        </div>
      </div>
    </div>

    <div class="setting-group">
      <h2 class="group-heading">{$t("gitconfig.pullFetch")}</h2>

      <div class="setting-row">
        <div class="setting-label">
          <span class="label-text">{$t("gitconfig.pullStrategy")}</span>
          <span class="label-hint">{$t("gitconfig.pullStrategyHint")}</span>
        </div>
        <div class="setting-control">
          <select
            value={pullRebase}
            onchange={(e) => { pullRebase = e.currentTarget.value as typeof pullRebase; scheduleGitConfigSave(); }}
          >
            {#each pullOptions as opt (opt.value)}
              <option value={opt.value}>{opt.text}</option>
            {/each}
          </select>
        </div>
      </div>

      <div class="setting-row">
        <div class="setting-label">
          <span class="label-text">{$t("gitconfig.autoPrune")}</span>
          <span class="label-hint">{$t("gitconfig.autoPruneHint")}</span>
        </div>
        <div class="setting-control">
          <label class="toggle">
            <input
              type="checkbox"
              aria-label={$t("gitconfig.autoPrune")}
              checked={fetchPrune}
              onchange={() => { fetchPrune = !fetchPrune; scheduleGitConfigSave(); }}
            />
            <span class="toggle-slider"></span>
          </label>
        </div>
      </div>
    </div>

    <div class="setting-group">
      <h2 class="group-heading">{$t("gitconfig.signing")}</h2>

      <div class="setting-row">
        <div class="setting-label">
          <span class="label-text">{$t("gitconfig.signCommits")}</span>
          <span class="label-hint">{$t("gitconfig.signCommitsHintBefore")}<code>{"commit.gpgsign"}</code>{$t("gitconfig.signCommitsHintAfter")}</span>
        </div>
        <div class="setting-control">
          <label class="toggle">
            <input
              type="checkbox"
              aria-label={$t("gitconfig.signCommits")}
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
            <span class="label-text">{$t("gitconfig.signingFormat")}</span>
            <span class="label-hint">{$t("gitconfig.signingFormatHintBefore")}<code>{"gpg.format"}</code>{$t("gitconfig.signingFormatHintAfter")}</span>
          </div>
          <div class="setting-control">
            <select
              aria-label={$t("gitconfig.signingFormat")}
              value={gpgFormat}
              onchange={(e) => {
                gpgFormat = e.currentTarget.value;
                signingKey = "";
                scheduleGitConfigSave();
              }}
            >
              <option value="openpgp">{$t("gitconfig.formatOpenpgp")}</option>
              <option value="ssh">{$t("gitconfig.formatSsh")}</option>
              {#if gpgFormat !== "openpgp" && gpgFormat !== "ssh"}
                <option value={gpgFormat}>{gpgFormat === "x509" ? "X.509 (gpgsm)" : gpgFormat}</option>
              {/if}
            </select>
          </div>
        </div>

        <div class="setting-row">
          <div class="setting-label">
            <span class="label-text">{$t("gitconfig.signingKey")}</span>
            <span class="label-hint">
              {gpgFormat === "ssh" ? $t("gitconfig.signingKeyHintSsh") : $t("gitconfig.signingKeyHintGpg")}
              {#if !keysLoading && keys.length === 0} {$t("gitconfig.noKeysFound")}{/if}
            </span>
          </div>
          <div class="setting-control signing-key">
            {#if keys.length > 0}
              <select
                aria-label={$t("gitconfig.signingKey")}
                value={keys.some((k) => k.value === signingKey) ? signingKey : ""}
                onchange={(e) => {
                  if (e.currentTarget.value) { signingKey = e.currentTarget.value; scheduleGitConfigSave(); }
                }}
              >
                <option value="">{signingKey && !keys.some((k) => k.value === signingKey) ? $t("gitconfig.keyCustom") : $t("gitconfig.keyChoose")}</option>
                {#each keys as k (k.value)}
                  <option value={k.value}>{k.label}</option>
                {/each}
              </select>
            {/if}
            <input
              type="text"
              class="text-input"
              placeholder={gpgFormat === "ssh" ? $t("gitconfig.keyPlaceholderSsh") : $t("gitconfig.keyPlaceholderGpg")}
              aria-label={$t("gitconfig.keyValueLabel")}
              value={signingKey}
              onchange={(e) => { signingKey = e.currentTarget.value.trim(); scheduleGitConfigSave(); }}
            />
            <button class="test-btn" onclick={runSigningTest} disabled={testing || !signingKey}>
              {testing ? $t("gitconfig.testing") : $t("gitconfig.testSigning")}
            </button>
          </div>
        </div>
      {/if}
    </div>

    <div class="setting-group">
      <h2 class="group-heading">LFS</h2>

      <div class="setting-row">
        <div class="setting-label">
          <span class="label-text">Git LFS</span>
          <span class="label-hint">{$t("gitconfig.lfsHint")}</span>
        </div>
        <div class="setting-control">
          <span class="status-badge" class:installed={lfsInstalled}>
            {lfsInstalled ? $t("gitconfig.lfsInstalled") : $t("gitconfig.lfsNotDetected")}
          </span>
        </div>
      </div>
    </div>

    {#if saveError}
      <div class="notice error">{saveError}</div>
    {:else if saving}
      <div class="notice saving">{$t("gitconfig.saving")}</div>
    {/if}
  {/if}
</div>

<style>
  .signing-key {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 6px;
    min-width: 260px;
  }
  .test-btn {
    align-self: flex-end;
    padding: 4px 12px;
    font-size: 12px;
    border-radius: 4px;
    border: 1px solid var(--color-border);
    background: transparent;
    color: var(--color-text-primary);
    cursor: pointer;
  }
  .test-btn:disabled { opacity: 0.6; cursor: not-allowed; }
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
