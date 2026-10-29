<script lang="ts">
  /** 0..1, or null for an indeterminate sliding bar. */
  let { frac }: { frac: number | null } = $props();
</script>

<div class="track" role="progressbar" aria-valuenow={frac === null ? undefined : Math.round(frac * 100)}>
  {#if frac === null}
    <i class="slide"></i>
  {:else}
    <i style:width="{Math.min(Math.max(frac, 0), 1) * 100}%"></i>
  {/if}
</div>

<style>
  .track {
    position: relative;
    height: 10px;
    border-radius: 5px;
    background: var(--pane);
    overflow: hidden;
  }
  i {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: 5px;
    background: var(--acc);
    transition: width 0.2s ease-out;
  }
  .slide {
    width: 25%;
    animation: slide 2s ease-in-out infinite;
  }
  @keyframes slide {
    from {
      left: 0;
    }
    50% {
      left: 75%;
    }
    to {
      left: 0;
    }
  }
</style>
