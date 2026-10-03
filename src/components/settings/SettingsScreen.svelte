<script lang="ts">
  import { ArrowLeft } from "lucide-svelte";
  import GeneralSettings from "./GeneralSettings.svelte";
  import AppearanceSettings from "./AppearanceSettings.svelte";
  import EditorDiffSettings from "./EditorDiffSettings.svelte";
  import GitConfigSettings from "./GitConfigSettings.svelte";
  import KeybindingsSettings from "./KeybindingsSettings.svelte";
  import GitHubSettings from "./GitHubSettings.svelte";
  import AboutSettings from "./AboutSettings.svelte";
  import RepoSettings from "./RepoSettings.svelte";
  import { currentView } from "../../lib/stores/ui";
  import { t, type MessageKey } from "../../lib/i18n";

  type Section = "general" | "repo" | "appearance" | "editor" | "git" | "github" | "keybindings" | "about";

  const sections: { id: Section; key: MessageKey }[] = [
    { id: "general", key: "settings.nav.general" },
    { id: "repo", key: "settings.nav.repo" },
    { id: "appearance", key: "settings.nav.appearance" },
    { id: "editor", key: "settings.nav.editor" },
    { id: "git", key: "settings.nav.git" },
    { id: "github", key: "settings.nav.github" },
    { id: "keybindings", key: "settings.nav.keybindings" },
    { id: "about", key: "settings.nav.about" },
  ];

  let activeSection = $state<Section>("general");

  function goBack() {
    $currentView = "repos";
  }
</script>

<div class="settings-screen">
  <nav class="settings-nav" aria-label={$t("settings.sectionsLabel")}>
    <button class="back-btn" onclick={goBack}>
      <ArrowLeft size={16} />
      <span>{$t("settings.back")}</span>
    </button>

    <h2 class="nav-title">{$t("settings.title")}</h2>

    <div class="nav-sections">
      {#each sections as section (section.id)}
        <button
          class="nav-item"
          class:active={activeSection === section.id}
          aria-current={activeSection === section.id ? "page" : undefined}
          onclick={() => (activeSection = section.id)}
        >
          {$t(section.key)}
        </button>
      {/each}
    </div>
  </nav>

  <div class="settings-content">
    {#if activeSection === "general"}
      <GeneralSettings />
    {:else if activeSection === "repo"}
      <RepoSettings />
    {:else if activeSection === "appearance"}
      <AppearanceSettings />
    {:else if activeSection === "editor"}
      <EditorDiffSettings />
    {:else if activeSection === "git"}
      <GitConfigSettings />
    {:else if activeSection === "github"}
      <GitHubSettings />
    {:else if activeSection === "keybindings"}
      <KeybindingsSettings />
    {:else if activeSection === "about"}
      <AboutSettings />
    {/if}
  </div>
</div>

<style>
  .settings-screen {
    display: flex;
    flex: 1;
    overflow: hidden;
  }

  .settings-nav {
    width: 200px;
    flex-shrink: 0;
    background: var(--color-surface);
    border-right: 1px solid var(--color-border);
    display: flex;
    flex-direction: column;
    padding: 16px 0;
  }

  .back-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    margin: 0 8px 12px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 12px;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .back-btn:hover {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .nav-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--color-text-muted);
    margin: 0 0 8px;
    padding: 0 16px;
  }

  .nav-sections {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .nav-item {
    padding: 8px 16px;
    border: none;
    background: transparent;
    color: var(--color-text-primary);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
    transition: background 0.1s;
  }

  .nav-item:hover {
    background: var(--color-surface-elevated);
  }

  .nav-item.active {
    background: var(--color-surface-elevated);
    color: var(--color-accent);
    box-shadow: inset 2px 0 0 var(--color-accent);
  }

  .settings-content {
    flex: 1;
    overflow-y: auto;
    padding: 32px 40px;
  }
</style>
