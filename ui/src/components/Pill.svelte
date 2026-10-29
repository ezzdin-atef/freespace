<script lang="ts" module>
  export type Tone = "safe" | "review" | "danger";
</script>

<script lang="ts">
  let { tone, label }: { tone: Tone; label: string } = $props();
</script>

<span class="pill {tone}">{label}</span>

<style>
  .pill {
    display: inline-block;
    justify-self: start;
    align-self: start;
    padding: 1px 9px;
    border-radius: var(--r-sm);
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
  }
  .safe {
    color: var(--safe);
    background: var(--safe-bg);
  }
  .review {
    color: var(--review);
    background: var(--review-bg);
  }
  .danger {
    color: var(--danger);
    background: var(--danger-bg);
  }
</style>
