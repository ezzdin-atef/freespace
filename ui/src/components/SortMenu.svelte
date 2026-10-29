<script lang="ts">
  import { ArrowDownAZ, ArrowDownWideNarrow, Check, Layers } from "@lucide/svelte";
  import type { Sort } from "../lib/session.svelte";
  import Select from "./Select.svelte";

  /** `groupLabel` null leaves out the third option (pages with nothing to group by). */
  let {
    value,
    groupLabel = "Category",
    onchange,
  }: { value: Sort; groupLabel?: string | null; onchange: (s: Sort) => void } = $props();

  let open = $state(false);
  let root: HTMLDivElement;

  const options = $derived([
    { key: "size" as const, icon: ArrowDownWideNarrow, label: "Size", detail: "largest first" },
    { key: "name" as const, icon: ArrowDownAZ, label: "Name", detail: "A to Z" },
    ...(groupLabel ? [{ key: "group" as const, icon: Layers, label: groupLabel, detail: "" }] : []),
  ]);
  const current = $derived(options.find((o) => o.key === value) ?? options[0]);

  function pick(s: Sort) {
    open = false;
    onchange(s);
  }
</script>

<svelte:window
  onpointerdown={(e) => open && !root.contains(e.target as Node) && (open = false)}
  onkeydown={(e) => open && e.key === "Escape" && (open = false)}
/>

<div class="menu" bind:this={root}>
  <Select icon={current.icon} value={current.label} width={150} title="Sort the list" onclick={() => (open = !open)} />
  {#if open}
    <div class="popup" role="menu">
      <div class="heading">Sort by</div>
      {#each options as o (o.key)}
        <button role="menuitemradio" aria-checked={o.key === value} class:on={o.key === value} onclick={() => pick(o.key)}>
          <o.icon size={16} />
          <span class="label">{o.label}</span>
          <span class="detail">{o.detail}</span>
          {#if o.key === value}<Check size={14} />{/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .menu {
    position: relative;
    flex: none;
  }
  .popup {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 20;
    width: 220px;
    padding: 6px;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--r-card);
    box-shadow: 0 8px 24px rgb(15 23 42 / 0.12);
  }
  .heading {
    padding: 6px 10px 4px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--mut2);
  }
  button {
    width: 100%;
    height: 36px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 10px;
    border: 0;
    border-radius: var(--r-ctl);
    background: transparent;
    text-align: left;
  }
  button:hover {
    background: var(--pane);
  }
  button :global(svg) {
    color: var(--mut);
    flex: none;
  }
  .on,
  .on :global(svg) {
    color: var(--acc);
  }
  .on {
    font-weight: 600;
  }
  .label {
    flex: 1;
  }
  .detail {
    font-size: 13px;
    font-weight: 400;
    color: var(--mut);
  }
</style>
