<script lang="ts">
  import { ChevronDown } from "@lucide/svelte";
  import type { IconComponent } from "../lib/types";

  // Trigger only for now; the popup (drives + folder) arrives with the scan flow in phase 3.
  let {
    icon: Icon,
    value,
    width = 260,
    disabled = false,
    title,
    onclick,
  }: {
    icon?: IconComponent;
    value: string;
    width?: number;
    disabled?: boolean;
    title?: string;
    onclick?: () => void;
  } = $props();
</script>

<button class="select" style:width="{width}px" {disabled} {title} {onclick}>
  {#if Icon}<Icon size={16} />{/if}
  <span>{value}</span>
  <ChevronDown size={16} />
</button>

<style>
  .select {
    height: var(--ctl-h);
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px;
    flex: none;
    font-weight: 600;
    text-align: left;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--r-ctl);
    transition: border-color 0.12s;
  }
  .select:hover:not(:disabled) {
    border-color: var(--line-strong);
  }
  .select :global(svg) {
    color: var(--mut);
    flex: none;
  }
  span {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .select:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
