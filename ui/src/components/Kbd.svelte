<script lang="ts">
  let { keys }: { keys: string } = $props();
</script>

<kbd>{keys}</kbd>

<style>
  kbd {
    justify-self: start;
    padding: 2px 6px;
    font: 12px var(--mono);
    color: var(--fg2);
    background: var(--row-hover);
    border: 1px solid var(--line-strong);
    border-radius: var(--r-sm);
    white-space: nowrap;
  }
</style>
