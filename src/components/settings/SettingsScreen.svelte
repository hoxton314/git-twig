<script lang="ts">
  import { ArrowLeft } from "lucide-svelte";
  import GeneralSettings from "./GeneralSettings.svelte";
  import AppearanceSettings from "./AppearanceSettings.svelte";
  import EditorDiffSettings from "./EditorDiffSettings.svelte";
  import GitConfigSettings from "./GitConfigSettings.svelte";
  import KeybindingsSettings from "./KeybindingsSettings.svelte";
  import { currentView } from "../../lib/stores/ui";

  type Section = "general" | "appearance" | "editor" | "git" | "keybindings";

  const sections: { id: Section; label: string }[] = [
    { id: "general", label: "General" },
    { id: "appearance", label: "Appearance" },
    { id: "editor", label: "Editor & Diff" },
    { id: "git", label: "Git Configuration" },
    { id: "keybindings", label: "Keybindings" },
  ];

  let activeSection = $state<Section>("general");

  function goBack() {
    $currentView = "repos";
  }
</script>

<div class="settings-screen">
  <nav class="settings-nav">
    <button class="back-btn" onclick={goBack}>
      <ArrowLeft size={16} />
      <span>Back</span>
    </button>

    <h2 class="nav-title">Settings</h2>

    <div class="nav-sections">
      {#each sections as section (section.id)}
        <button
          class="nav-item"
          class:active={activeSection === section.id}
          onclick={() => (activeSection = section.id)}
        >
          {section.label}
        </button>
      {/each}
    </div>
  </nav>

  <div class="settings-content">
    {#if activeSection === "general"}
      <GeneralSettings />
    {:else if activeSection === "appearance"}
      <AppearanceSettings />
    {:else if activeSection === "editor"}
      <EditorDiffSettings />
    {:else if activeSection === "git"}
      <GitConfigSettings />
    {:else if activeSection === "keybindings"}
      <KeybindingsSettings />
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
    border-left: 2px solid var(--color-accent);
  }

  .settings-content {
    flex: 1;
    overflow-y: auto;
    padding: 32px 40px;
  }
</style>
