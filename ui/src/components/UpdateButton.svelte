<script lang="ts">
  import { Download } from "../lib/icons";
  import { updater } from "../lib/updater.svelte";
  import Button from "./Button.svelte";

  updater.start();

  const label = $derived.by(() => {
    if (updater.progress === null) return `Update to ${updater.available?.version}`;
    if (updater.progress >= 1) return "Installing...";
    return `Downloading ${Math.round(updater.progress * 100)}%`;
  });
  const title = $derived(
    updater.error ?? "Download the new version, install it and restart Freespace. Running jobs will be stopped.",
  );
</script>

{#if updater.available}
  <Button
    {label}
    icon={Download}
    variant="primary"
    {title}
    disabled={updater.progress !== null}
    onclick={() => updater.install()}
  />
{/if}
