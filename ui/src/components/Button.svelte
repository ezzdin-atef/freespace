<script lang="ts">
  import type { IconComponent } from "../lib/types";

  let {
    label,
    icon: Icon,
    primary = false,
    disabled = false,
    width,
    title,
    onclick,
  }: {
    label: string;
    icon?: IconComponent;
    primary?: boolean;
    disabled?: boolean;
    width?: number;
    title?: string;
    onclick?: () => void;
  } = $props();
</script>

<button
  class="btn"
  class:primary
  {disabled}
  {title}
  style:width={width ? `${width}px` : undefined}
  {onclick}
>
  {#if Icon}<Icon size={14} strokeWidth={2.25} />{/if}
  {label}
</button>

<style>
  .btn {
    height: var(--ctl-h);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 0 14px;
    flex: none;
    font-weight: 600;
    white-space: nowrap;
    border-radius: var(--r-ctl);
    border: 1px solid var(--line);
    background: var(--surface);
    color: var(--fg);
    box-shadow: var(--shadow-ctl);
    transition:
      background 0.12s,
      border-color 0.12s;
  }
  .btn :global(svg) {
    color: var(--fg2);
  }
  .btn:hover:not(:disabled) {
    background: var(--row-hover);
    border-color: var(--line-strong);
  }
  .primary {
    background: var(--acc);
    border-color: var(--acc);
    color: #fff;
    box-shadow: 0 1px 2px rgb(79 70 229 / 0.3);
  }
  .primary :global(svg) {
    color: #fff;
  }
  .primary:hover:not(:disabled) {
    background: var(--acc-hover);
    border-color: var(--acc-hover);
  }
  .btn:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
