<script lang="ts">
  import { fmtSize } from "../lib/format";
  import { squarify } from "../lib/squarify";

  /** Small treemap of labelled sizes; clicking a block calls `onpick` with its index. */
  let {
    items,
    height = 190,
    onpick,
  }: { items: { name: string; size: number }[]; height?: number; onpick?: (i: number) => void } = $props();

  // Soft indigo/blue/cyan tints.
  const TINTS = ["#c7d2fe", "#bfdbfe", "#ddd6fe", "#a5f3fc", "#e0e7ff", "#dbeafe", "#ede9fe", "#cffafe"];
  const GAP = 1.5;

  let width = $state(0);
  const blocks = $derived(
    width > 0 ? squarify(items.map((i) => i.size), { x: 0, y: 0, w: width, h: height }) : [],
  );
</script>

<div class="map" style:height="{height}px" bind:clientWidth={width}>
  {#each blocks as b, i (i)}
    {#if b.w > 2 * GAP + 2 && b.h > 2 * GAP + 2}
      <button
        class="block"
        style:left="{b.x + GAP}px"
        style:top="{b.y + GAP}px"
        style:width="{b.w - 2 * GAP}px"
        style:height="{b.h - 2 * GAP}px"
        style:background={TINTS[i % TINTS.length]}
        title="{items[i].name}&#10;{fmtSize(items[i].size)}"
        tabindex="-1"
        onclick={() => onpick?.(i)}
      >
        {#if b.w > 46 && b.h > 22}<span>{items[i].name}</span>{/if}
      </button>
    {/if}
  {/each}
</div>

<style>
  .map {
    position: relative;
  }
  .block {
    position: absolute;
    padding: 5px 7px;
    border: 0;
    border-radius: 6px;
    text-align: left;
    vertical-align: top;
    display: flex;
    align-items: flex-start;
    overflow: hidden;
    transition: filter 0.1s;
  }
  .block:hover {
    filter: brightness(0.93) saturate(1.3);
  }
  span {
    font-size: 12px;
    font-weight: 600;
    color: var(--fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
