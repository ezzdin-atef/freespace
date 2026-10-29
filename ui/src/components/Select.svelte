<script lang="ts">
  import { CaretDown } from "../lib/icons";
  import type { IconComponent } from "../lib/types";

  // The trigger button of a dropdown; the popup belongs to the caller (DrivePicker, SortMenu).
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
  <CaretDown size={16} />
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
