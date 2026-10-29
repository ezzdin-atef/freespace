<script lang="ts">
  import { Check } from "@lucide/svelte";

  let {
    checked,
    disabled = false,
    label,
    onchange,
  }: { checked: boolean; disabled?: boolean; label: string; onchange?: () => void } = $props();
</script>

<!-- Not in the tab order and never takes focus on click: the list is driven by arrow keys and Space. -->
<button
  class="box"
  class:on={checked}
  role="checkbox"
  aria-checked={checked}
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
  {#if checked}<Check size={13} strokeWidth={3} />{/if}
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
  .box:disabled {
    background: var(--pane);
    border-color: var(--line);
    cursor: not-allowed;
  }
</style>
