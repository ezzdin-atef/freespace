<script lang="ts">
  import type { IconComponent } from "../lib/types";

  /** secondary (default), primary, danger, ghost (no border), dark (on the dark bulk bar). */
  type Variant = "secondary" | "primary" | "danger" | "ghost" | "dark";

  let {
    label,
    icon: Icon,
    primary = false,
    variant,
    disabled = false,
    width,
    wide = false,
    title,
    onclick,
  }: {
    label: string;
    icon?: IconComponent;
    /** Shorthand for variant="primary". */
    primary?: boolean;
    variant?: Variant;
    disabled?: boolean;
    width?: number;
    /** Fill the container's width. */
    wide?: boolean;
    title?: string;
    onclick?: () => void;
  } = $props();

  const look = $derived(variant ?? (primary ? "primary" : "secondary"));
</script>

<button
  class="btn {look}"
  class:wide
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
  .wide {
    width: 100%;
  }
  .secondary:hover:not(:disabled) {
    background: var(--row-hover);
    border-color: var(--line-strong);
  }
  .primary,
  .danger {
    color: #fff;
  }
  .primary :global(svg),
  .danger :global(svg) {
    color: #fff;
  }
  .primary {
    background: var(--acc);
    border-color: var(--acc);
    box-shadow: 0 1px 2px rgb(79 70 229 / 0.3);
  }
  .primary:hover:not(:disabled) {
    background: var(--acc-hover);
    border-color: var(--acc-hover);
  }
  .danger {
    background: var(--danger);
    border-color: var(--danger);
    box-shadow: 0 1px 2px rgb(220 38 38 / 0.3);
  }
  .danger:hover:not(:disabled) {
    background: #b91c1c;
    border-color: #b91c1c;
  }
  .ghost {
    border-color: transparent;
    background: transparent;
    box-shadow: none;
    color: var(--mut);
  }
  .ghost:hover:not(:disabled) {
    background: var(--pane);
    color: var(--fg);
  }
  .dark {
    border-color: #334155;
    background: #1e293b;
    color: #e2e8f0;
    box-shadow: none;
  }
  .dark :global(svg) {
    color: #cbd5e1;
  }
  .dark:hover:not(:disabled) {
    background: #334155;
  }
  .btn:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
