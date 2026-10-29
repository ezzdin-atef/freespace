<script lang="ts">
  import type { IconComponent } from "../lib/types";

  let {
    icon: Icon,
    label,
    count = "",
    active = false,
    onclick,
  }: {
    icon: IconComponent;
    label: string;
    count?: string;
    active?: boolean;
    onclick?: () => void;
  } = $props();
</script>

<button class="nav" class:active {onclick} aria-current={active ? "page" : undefined}>
  <Icon size={16} />
  <span class="label">{label}</span>
  {#if count}<span class="count">{count}</span>{/if}
</button>

<style>
  .nav {
    width: 100%;
    height: 36px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 8px 0 12px;
    border: 0;
    border-radius: var(--r-ctl);
    background: transparent;
    color: var(--fg2);
    text-align: left;
  }
  .nav :global(svg) {
    color: var(--mut);
    flex: none;
  }
  .nav:hover {
    background: var(--pane);
  }
  .active,
  .active:hover {
    background: var(--acc-bg);
    color: var(--acc);
    font-weight: 600;
  }
  .active :global(svg) {
    color: var(--acc);
  }
  .label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    padding: 1px 7px;
    border-radius: var(--r-sm);
    background: var(--pane);
    color: var(--mut);
    font-size: 12px;
    font-weight: 600;
  }
  .active .count {
    background: var(--surface);
    color: var(--acc);
  }
</style>
