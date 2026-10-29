<script lang="ts">
  /** `mixed` shows a dash: some, not all, of what this box stands for is ticked. */
  let {
    checked,
    mixed = false,
    disabled = false,
    label,
    title,
    onchange,
  }: {
    checked: boolean;
    mixed?: boolean;
    disabled?: boolean;
    label: string;
    title?: string;
    onchange?: () => void;
  } = $props();
</script>

<!-- Not in the tab order and never takes focus on click: the list is driven by arrow keys and Space. -->
<button
  class="box"
  class:on={checked || mixed}
  role="checkbox"
  aria-checked={mixed ? "mixed" : checked}
  {title}
  aria-label={label}
  tabindex="-1"
  {disabled}
  onmousedown={(e) => e.preventDefault()}
  onclick={(e) => {
    e.stopPropagation();
    onchange?.();
  }}
  ondblclick={(e) => e.stopPropagation()}
>
  {#if mixed}
    <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M3 6h6" /></svg>
  {:else if checked}
    <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M2.5 6.2 5 8.6l4.6-5" /></svg>
  {/if}
</button>

<style>
  .box {
    width: 18px;
    height: 18px;
    display: grid;
    place-items: center;
    padding: 0;
    border: 1.5px solid var(--line-strong);
    border-radius: var(--r-sm);
    background: var(--surface);
    color: #fff;
    transition:
      background 0.1s,
      border-color 0.1s;
  }
  .box:hover:not(:disabled) {
    border-color: var(--acc);
  }
  .on {
    background: var(--acc);
    border-color: var(--acc);
  }
  svg {
    width: 12px;
    height: 12px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .box:disabled {
    background: var(--pane);
    border-color: var(--line);
    cursor: not-allowed;
  }
</style>
