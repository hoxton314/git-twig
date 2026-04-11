<script lang="ts">
  import { onMount } from 'svelte';
  import { check } from '@tauri-apps/plugin-updater';
  import { relaunch } from '@tauri-apps/plugin-process';
  import { message } from '@tauri-apps/plugin-dialog';

  let updateAvailable = $state(false);
  let updateVersion = $state('');
  let updateBody = $state('');
  let downloading = $state(false);
  let downloadProgress = $state(0);
  let contentLength = $state(0);

  let progressPercent = $derived(
    contentLength > 0 ? Math.round((downloadProgress / contentLength) * 100) : 0
  );

  onMount(async () => {
    try {
      const update = await check();
      if (update?.available) {
        updateAvailable = true;
        updateVersion = update.version;
        updateBody = update.body ?? '';
      }
    } catch (e) {
      console.error('Update check failed:', e);
    }
  });

  async function installUpdate() {
    try {
      const update = await check();
      if (!update?.available) return;

      downloading = true;
      downloadProgress = 0;

      await update.downloadAndInstall((event) => {
        if (event.event === 'Started') {
          contentLength = event.data.contentLength ?? 0;
        } else if (event.event === 'Progress') {
          downloadProgress += event.data.chunkLength;
        }
      });

      await relaunch();
    } catch (e) {
      downloading = false;
      await message(`Update failed: ${e}`, { title: 'Update Error', kind: 'error' });
    }
  }

  function dismiss() {
    updateAvailable = false;
  }
</script>

{#if updateAvailable}
  <div class="update-banner">
    <div class="update-info">
      <span class="update-title">Twig {updateVersion} available</span>
      {#if updateBody}
        <span class="update-body">{updateBody}</span>
      {/if}
    </div>
    <div class="update-actions">
      {#if downloading}
        <div class="progress-bar">
          <div class="progress-fill" style="width: {progressPercent}%"></div>
        </div>
        <span class="progress-label">{progressPercent}%</span>
      {:else}
        <button class="btn-update" onclick={installUpdate}>Update & Restart</button>
        <button class="btn-dismiss" onclick={dismiss}>Later</button>
      {/if}
    </div>
  </div>
{/if}

<style>
  .update-banner {
    position: fixed;
    bottom: 1rem;
    right: 1rem;
    background: #1f2335;
    border: 1px solid #292e42;
    border-radius: 8px;
    padding: 0.75rem 1rem;
    display: flex;
    align-items: center;
    gap: 1rem;
    z-index: 9999;
    box-shadow: 0 4px 20px rgba(0,0,0,0.4);
    max-width: 360px;
  }
  .update-info {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .update-title {
    color: #c0caf5;
    font-weight: 600;
    font-size: 0.875rem;
  }
  .update-body {
    color: #565f89;
    font-size: 0.75rem;
  }
  .update-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }
  .btn-update {
    background: #7aa2f7;
    color: #1a1b26;
    border: none;
    border-radius: 4px;
    padding: 0.375rem 0.75rem;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-update:hover { background: #89b4fa; }
  .btn-dismiss {
    background: transparent;
    color: #565f89;
    border: 1px solid #292e42;
    border-radius: 4px;
    padding: 0.375rem 0.75rem;
    font-size: 0.8rem;
    cursor: pointer;
  }
  .btn-dismiss:hover { color: #c0caf5; }
  .progress-bar {
    width: 100px;
    height: 6px;
    background: #292e42;
    border-radius: 3px;
    overflow: hidden;
  }
  .progress-fill {
    height: 100%;
    background: #7aa2f7;
    transition: width 0.2s ease;
  }
  .progress-label {
    color: #565f89;
    font-size: 0.75rem;
    width: 2.5rem;
  }
</style>
