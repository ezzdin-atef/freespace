<script lang="ts">
  /** A row of equal-width stat cards. */
  let { items }: { items: { label: string; value: string; tone?: "safe" | "review" | "danger" }[] } = $props();
</script>

<div class="stats">
  {#each items as s (s.label)}
    <div class="stat">
      <span class="label">{s.label}</span>
      <span class="value {s.tone ?? ''}">{s.value}</span>
    </div>
  {/each}
</div>

<style>
  .stats {
    display: grid;
    grid-auto-columns: 1fr;
    grid-auto-flow: column;
    gap: 16px;
  }
  .stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 16px;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--r-card);
    box-shadow: var(--shadow-card);
  }
  .label {
    font-size: 13px;
    color: var(--mut);
  }
  .value {
    font-size: 22px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .safe {
    color: var(--safe);
  }
  .review {
    color: var(--review);
  }
  .danger {
    color: var(--danger);
  }
</style>
