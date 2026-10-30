<script lang="ts">
  import { AppWindow, Store } from "../lib/icons";
  import type { AppRow } from "../lib/api";
  import { session } from "../lib/session.svelte";

  /** The app's own logo when Windows has one, else a tinted tile with a generic desktop or Store icon. */
  let { app, size = 26 }: { app: AppRow; size?: number } = $props();

  const logo = $derived(session.appLogos[app.id]);
  const Icon = $derived(app.store ? Store : AppWindow);
</script>

<span class="logo" class:store={app.store} class:img={!!logo} style:--s="{size}px">
  {#if logo}
    <img src={logo} alt="" draggable="false" />
  {:else}
    <Icon size={Math.round(size * 0.58)} />
  {/if}
</span>

<style>
  .logo {
    width: var(--s);
    height: var(--s);
    flex: none;
    display: grid;
    place-items: center;
    border-radius: calc(var(--s) * 0.27);
    background: var(--pane);
    color: var(--fg2);
  }
  .store {
    background: var(--acc-bg);
    color: var(--acc);
  }
  /* A real logo carries its own shape and colour: no tile behind it. */
  .img {
    background: transparent;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
</style>
