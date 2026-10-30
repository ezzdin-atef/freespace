<script lang="ts" module>
  export type Tone = "safe" | "review" | "danger";
</script>

<script lang="ts">
  /** `wrap` lets a long label (a leftover's note) break onto more lines instead of widening its container. */
  let { tone, label, wrap = false }: { tone: Tone; label: string; wrap?: boolean } = $props();
</script>

<span class="pill {tone}" class:wrap title={wrap ? undefined : label}>{label}</span>

<style>
  .pill {
    display: inline-block;
    justify-self: start;
    align-self: start;
    max-width: 100%;
    padding: 1px 9px;
    border-radius: var(--r-sm);
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .wrap {
    white-space: normal;
    overflow-wrap: anywhere;
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
