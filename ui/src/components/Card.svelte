<script lang="ts">
  import type { Snippet } from "svelte";

  let { pad = 0, children }: { pad?: number; children: Snippet } = $props();
</script>

<div class="card" style:padding="{pad}px">
  {@render children()}
</div>

<style>
  .card {
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--r-card);
    box-shadow: var(--shadow-card);
    overflow: hidden;
  }
</style>
