<script lang="ts">
  import { Music, Loader2 } from "lucide-svelte";

  interface Props {
    oldData: string | null;
    newData: string | null;
    filePath: string;
    loading?: boolean;
  }

  let { oldData, newData, filePath, loading = false }: Props = $props();

  const hasOld = $derived(oldData !== null);
  const hasNew = $derived(newData !== null);
  const isModified = $derived(hasOld && hasNew);

  function mimeType(path: string): string {
    const ext = path.split(".").pop()?.toLowerCase() ?? "";
    const map: Record<string, string> = {
      mp3: "audio/mpeg",
      wav: "audio/wav",
      ogg: "audio/ogg",
      oga: "audio/ogg",
      opus: "audio/ogg",
      flac: "audio/flac",
      m4a: "audio/mp4",
      aac: "audio/aac",
      weba: "audio/webm",
    };
    return map[ext] ?? "audio/mpeg";
  }

  const mime = $derived(mimeType(filePath));
  const oldSrc = $derived(oldData ? `data:${mime};base64,${oldData}` : null);
  const newSrc = $derived(newData ? `data:${mime};base64,${newData}` : null);

  function formatSize(data: string): string {
    const base64 = data.includes(",") ? data.slice(data.indexOf(",") + 1) : data;
    const padding = base64.endsWith("==") ? 2 : base64.endsWith("=") ? 1 : 0;
    const bytes = Math.max(0, Math.floor((base64.length * 3) / 4) - padding);
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${bytes} B`;
  }
</script>

<div class="audio-preview">
  {#if loading}
    <div class="audio-loading">
      <Loader2 size={18} class="spinner" />
      <span>Loading preview...</span>
    </div>
  {:else if !hasOld && !hasNew}
    <div class="audio-loading">
      <Music size={16} />
      <span>No audio data available</span>
    </div>
  {:else}
    <div class="players" class:two-up={isModified}>
      {#if oldSrc}
        <div class="player-panel" class:removed={!hasNew}>
          <div class="panel-label">
            <span class="label-text {!hasNew ? 'label-removed' : 'label-old'}">
              {!hasNew ? "Deleted" : "Previous"}
            </span>
            <span class="meta">{formatSize(oldSrc)}</span>
          </div>
          <audio controls preload="metadata" src={oldSrc}></audio>
        </div>
      {/if}
      {#if newSrc}
        <div class="player-panel">
          <div class="panel-label">
            <span class="label-text {!hasOld ? 'label-added' : 'label-new'}">
              {!hasOld ? "Added" : "Current"}
            </span>
            <span class="meta">{formatSize(newSrc)}</span>
          </div>
          <audio controls preload="metadata" src={newSrc}></audio>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .audio-preview {
    padding: 16px 20px;
  }

  .audio-loading {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .audio-loading :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .players {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 520px;
  }

  .player-panel {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: 6px;
  }

  .player-panel.removed {
    opacity: 0.75;
  }

  .panel-label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 11px;
  }

  .label-text {
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .label-old,
  .label-removed {
    color: #f7768e;
  }

  .label-new,
  .label-added {
    color: #9ece6a;
  }

  .meta {
    color: var(--color-text-muted);
    font-family: var(--font-mono);
  }

  audio {
    width: 100%;
    height: 36px;
  }
</style>
