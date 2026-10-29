<script lang="ts">
  let { label }: { label: string } = $props();
</script>

<div class="section">{label}</div>

<style>
  .section {
    padding: 14px 12px 4px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--mut2);
  }
</style>
