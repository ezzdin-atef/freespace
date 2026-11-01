<script lang="ts">
  /** Fraction filled, 0..1. Turns red above 90%, unless `alarm` is off: being nearly full is not always bad. */
  let { frac, alarm = true }: { frac: number; alarm?: boolean } = $props();
  const pct = $derived(Math.min(Math.max(frac, 0), 1) * 100);
</script>

<div class="bar"><i class:low={alarm && frac > 0.9} style:width="{pct}%"></i></div>

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
