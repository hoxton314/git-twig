<script lang="ts">
  import { Image, Loader2 } from "lucide-svelte";

  interface Props {
    oldData: string | null;
    newData: string | null;
    filePath: string;
    loading?: boolean;
  }

  let { oldData, newData, filePath, loading = false }: Props = $props();

  type CompareMode = "side-by-side" | "swipe" | "fade";
  let mode = $state<CompareMode>("side-by-side");
  let fadeOpacity = $state(1);
  let swipePosition = $state(50);
  let dragging = $state(false);

  const hasOld = $derived(oldData !== null);
  const hasNew = $derived(newData !== null);
  const isModified = $derived(hasOld && hasNew);

  function mimeType(path: string): string {
    const ext = path.split(".").pop()?.toLowerCase() ?? "";
    const map: Record<string, string> = {
      png: "image/png",
      jpg: "image/jpeg",
      jpeg: "image/jpeg",
      gif: "image/gif",
      webp: "image/webp",
      svg: "image/svg+xml",
      bmp: "image/bmp",
      ico: "image/x-icon",
      avif: "image/avif",
      tiff: "image/tiff",
      tif: "image/tiff",
    };
    return map[ext] ?? "image/png";
  }

  const mime = $derived(mimeType(filePath));
  const oldSrc = $derived(oldData ? `data:${mime};base64,${oldData}` : null);
  const newSrc = $derived(newData ? `data:${mime};base64,${newData}` : null);

  // Track image dimensions
  let oldDims = $state<{ w: number; h: number } | null>(null);
  let newDims = $state<{ w: number; h: number } | null>(null);

  function onOldLoad(e: Event) {
    const img = e.target as HTMLImageElement;
    oldDims = { w: img.naturalWidth, h: img.naturalHeight };
  }

  function onNewLoad(e: Event) {
    const img = e.target as HTMLImageElement;
    newDims = { w: img.naturalWidth, h: img.naturalHeight };
  }

  function formatSize(data: string): string {
    const bytes = Math.ceil((data.length * 3) / 4);
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${bytes} B`;
  }

  function handleSwipeMove(e: PointerEvent) {
    if (!dragging) return;
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const x = Math.max(0, Math.min(e.clientX - rect.left, rect.width));
    swipePosition = (x / rect.width) * 100;
  }

  function handleSwipeStart(e: PointerEvent) {
    dragging = true;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function handleSwipeEnd() {
    dragging = false;
  }
</script>

<div class="image-diff">
  {#if loading}
    <div class="image-loading">
      <Loader2 size={18} class="spinner" />
      <span>Loading preview...</span>
    </div>
  {:else if isModified}
    <div class="compare-controls">
      <button
        class="mode-btn"
        class:active={mode === "side-by-side"}
        onclick={() => (mode = "side-by-side")}
      >
        Side by side
      </button>
      <button
        class="mode-btn"
        class:active={mode === "swipe"}
        onclick={() => (mode = "swipe")}
      >
        Swipe
      </button>
      <button
        class="mode-btn"
        class:active={mode === "fade"}
        onclick={() => (mode = "fade")}
      >
        Onion skin
      </button>
    </div>

    {#if mode === "side-by-side"}
      <div class="side-by-side">
        <div class="image-panel">
          <div class="panel-label deleted">Before</div>
          <div class="image-container">
            {#if oldSrc}
              <img src={oldSrc} alt="Before" onload={onOldLoad} />
            {/if}
          </div>
          <div class="image-meta">
            {#if oldDims}{oldDims.w} x {oldDims.h}{/if}
            {#if oldData} &middot; {formatSize(oldData)}{/if}
          </div>
        </div>
        <div class="image-panel">
          <div class="panel-label added">After</div>
          <div class="image-container">
            {#if newSrc}
              <img src={newSrc} alt="After" onload={onNewLoad} />
            {/if}
          </div>
          <div class="image-meta">
            {#if newDims}{newDims.w} x {newDims.h}{/if}
            {#if newData} &middot; {formatSize(newData)}{/if}
          </div>
        </div>
      </div>
    {:else if mode === "swipe"}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="swipe-container"
        onpointermove={handleSwipeMove}
        onpointerdown={handleSwipeStart}
        onpointerup={handleSwipeEnd}
        onpointercancel={handleSwipeEnd}
      >
        <div class="swipe-layer swipe-new">
          {#if newSrc}
            <img src={newSrc} alt="After" />
          {/if}
        </div>
        <div class="swipe-layer swipe-old" style="clip-path: inset(0 {100 - swipePosition}% 0 0);">
          {#if oldSrc}
            <img src={oldSrc} alt="Before" />
          {/if}
        </div>
        <div class="swipe-handle" style="left: {swipePosition}%;">
          <div class="swipe-line"></div>
          <div class="swipe-grip"></div>
        </div>
        <div class="swipe-labels">
          <span class="swipe-label deleted">Before</span>
          <span class="swipe-label added">After</span>
        </div>
      </div>
    {:else if mode === "fade"}
      <div class="fade-container">
        <div class="fade-slider-row">
          <span class="fade-label deleted">Before</span>
          <input
            type="range"
            min="0"
            max="1"
            step="0.01"
            bind:value={fadeOpacity}
            class="fade-slider"
          />
          <span class="fade-label added">After</span>
        </div>
        <div class="fade-images">
          {#if oldSrc}
            <img
              src={oldSrc}
              alt="Before"
              class="fade-img"
              style="opacity: {1 - fadeOpacity};"
            />
          {/if}
          {#if newSrc}
            <img
              src={newSrc}
              alt="After"
              class="fade-img"
              style="opacity: {fadeOpacity};"
            />
          {/if}
        </div>
      </div>
    {/if}
  {:else}
    <div class="single-image">
      <div class="panel-label {hasNew ? 'added' : 'deleted'}">
        {hasNew ? "Added" : "Deleted"}
      </div>
      <div class="image-container">
        {#if newSrc}
          <img src={newSrc} alt="Added file" onload={onNewLoad} />
        {:else if oldSrc}
          <img src={oldSrc} alt="Deleted file" onload={onOldLoad} />
        {/if}
      </div>
      <div class="image-meta">
        <Image size={12} />
        {#if hasNew && newDims}{newDims.w} x {newDims.h}{/if}
        {#if hasOld && oldDims}{oldDims.w} x {oldDims.h}{/if}
        {#if hasNew && newData} &middot; {formatSize(newData)}{/if}
        {#if hasOld && oldData} &middot; {formatSize(oldData)}{/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .image-diff {
    padding: 12px;
  }

  .image-loading {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 16px 8px;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .image-loading :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .compare-controls {
    display: flex;
    gap: 2px;
    margin-bottom: 10px;
  }

  .mode-btn {
    padding: 4px 10px;
    font-size: 11px;
    border: 1px solid var(--color-border);
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    border-radius: 4px;
  }

  .mode-btn.active {
    background: var(--color-surface-elevated);
    color: var(--color-accent);
    border-color: var(--color-accent);
  }

  .mode-btn:hover:not(.active) {
    background: var(--color-surface);
  }

  /* Side by side */
  .side-by-side {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .image-panel {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .panel-label {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .panel-label.deleted {
    color: #f7768e;
  }

  .panel-label.added {
    color: #9ece6a;
  }

  .image-container {
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: 6px;
    padding: 12px;
    min-height: 80px;
    overflow: hidden;
  }

  .image-container img {
    max-width: 100%;
    max-height: 400px;
    object-fit: contain;
    image-rendering: auto;
    border-radius: 2px;
  }

  .image-meta {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
  }

  /* Single image (added/deleted) */
  .single-image {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-width: 600px;
  }

  /* Swipe mode */
  .swipe-container {
    position: relative;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    overflow: hidden;
    cursor: ew-resize;
    user-select: none;
    touch-action: none;
  }

  .swipe-layer {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 12px;
  }

  .swipe-layer img {
    max-width: 100%;
    max-height: 400px;
    object-fit: contain;
  }

  .swipe-new {
    background: var(--color-surface);
  }

  .swipe-old {
    position: absolute;
    inset: 0;
    background: var(--color-surface);
    z-index: 1;
  }

  .swipe-handle {
    position: absolute;
    top: 0;
    bottom: 0;
    z-index: 2;
    transform: translateX(-50%);
    pointer-events: none;
  }

  .swipe-line {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 50%;
    width: 2px;
    background: var(--color-accent);
    transform: translateX(-50%);
  }

  .swipe-grip {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 28px;
    height: 28px;
    background: var(--color-accent);
    border-radius: 50%;
    border: 2px solid var(--color-bg);
    pointer-events: auto;
  }

  .swipe-labels {
    position: absolute;
    top: 8px;
    left: 8px;
    right: 8px;
    display: flex;
    justify-content: space-between;
    z-index: 3;
    pointer-events: none;
  }

  .swipe-label {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    padding: 2px 6px;
    background: rgba(0, 0, 0, 0.5);
    border-radius: 3px;
  }

  .swipe-label.deleted {
    color: #f7768e;
  }

  .swipe-label.added {
    color: #9ece6a;
  }

  /* Fade / onion skin mode */
  .fade-container {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .fade-slider-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .fade-label {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    white-space: nowrap;
  }

  .fade-label.deleted {
    color: #f7768e;
  }

  .fade-label.added {
    color: #9ece6a;
  }

  .fade-slider {
    flex: 1;
    accent-color: var(--color-accent);
    cursor: pointer;
  }

  .fade-images {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: 6px;
    padding: 12px;
    min-height: 80px;
  }

  .fade-img {
    max-width: 100%;
    max-height: 400px;
    object-fit: contain;
  }

  .fade-img:first-child {
    position: absolute;
  }
</style>
