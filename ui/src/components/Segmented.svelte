<script lang="ts" generics="K extends string">
  let {
    items,
    value,
    onchange,
  }: {
    items: { key: K; label: string; count?: string }[];
    value: K;
    onchange: (key: K) => void;
  } = $props();
</script>

<div class="seg" role="tablist">
  {#each items as it (it.key)}
    <button role="tab" aria-selected={it.key === value} class:on={it.key === value} onclick={() => onchange(it.key)}>
      {it.label}
      {#if it.count}<span>{it.count}</span>{/if}
    </button>
  {/each}
</div>

<style>
  .seg {
    height: var(--ctl-h);
    display: inline-flex;
    flex: none;
    gap: 2px;
    padding: 3px;
    background: var(--pane);
    border-radius: var(--r-ctl);
  }
  button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0 12px;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--mut);
    font-weight: 600;
    white-space: nowrap;
  }
  button:hover {
    color: var(--fg);
  }
  .on {
    background: var(--surface);
    color: var(--fg);
    box-shadow: var(--shadow-ctl);
  }
  span {
    font-size: 13px;
    font-weight: 400;
    color: var(--mut);
  }
</style>
