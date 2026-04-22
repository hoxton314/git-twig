<script lang="ts">
  import AppShell from "./components/layout/AppShell.svelte";
  import { onMount } from "svelte";
  import type { Component } from "svelte";

  let UpdateChecker = $state<Component | null>(null);

  onMount(async () => {
    const mod = await import("./components/updater/UpdateChecker.svelte");
    UpdateChecker = mod.default;
  });
</script>

<AppShell />
{#if UpdateChecker}
  <UpdateChecker />
{/if}
