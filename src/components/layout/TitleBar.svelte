<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { Minus, Square, X } from "lucide-svelte";

  const win = getCurrentWindow();

  function minimize() {
    win.minimize();
  }

  function toggleMaximize() {
    win.toggleMaximize();
  }

  function close() {
    win.close();
  }
</script>

<div class="title-bar" data-tauri-drag-region>
  <span class="title">Twig</span>
  <span class="spacer"></span>
  <button class="win-btn" onclick={minimize} title="Minimize">
    <Minus size={14} />
  </button>
  <button class="win-btn" onclick={toggleMaximize} title="Maximize">
    <Square size={11} />
  </button>
  <button class="win-btn win-close" onclick={close} title="Close">
    <X size={14} />
  </button>
</div>

<style>
  .title-bar {
    display: flex;
    align-items: center;
    height: 32px;
    padding: 0 8px;
    background: var(--color-bg);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
    user-select: none;
  }

  .title {
    font-size: 12px;
    font-weight: 600;
    color: var(--color-text-muted);
    padding-left: 4px;
  }

  .spacer {
    flex: 1;
  }

  .win-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 100%;
    border: none;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .win-btn:hover {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
  }

  .win-close:hover {
    background: rgba(247, 118, 142, 0.3);
    color: #f7768e;
  }
</style>
