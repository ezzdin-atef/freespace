<script lang="ts">
  /** Fraction filled, 0..1. Turns red above 90% like the egui bars. */
  let { frac }: { frac: number } = $props();
  const pct = $derived(Math.min(Math.max(frac, 0), 1) * 100);
</script>

<div class="bar"><i class:low={frac > 0.9} style:width="{pct}%"></i></div>

<style>
  .bar {
    height: 6px;
    border-radius: 3px;
    background: var(--pane);
    overflow: hidden;
  }
  i {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--acc);
  }
  i.low {
    background: var(--danger);
  }
</style>
