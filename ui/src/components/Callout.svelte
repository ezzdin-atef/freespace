<script lang="ts" module>
  export type CalloutTone = "safe" | "review" | "danger" | "info";
</script>

<script lang="ts">
  import { CircleCheck, Info, ShieldAlert, TriangleAlert } from "@lucide/svelte";
  import type { Snippet } from "svelte";

  let { tone, title, children }: { tone: CalloutTone; title: string; children: Snippet } = $props();

  const ICONS = { safe: CircleCheck, review: TriangleAlert, danger: ShieldAlert, info: Info };
  const Icon = $derived(ICONS[tone]);
</script>

<div class="callout {tone}">
  <div class="title"><Icon size={15} />{title}</div>
  <div class="body">{@render children()}</div>
</div>

<style>
  .callout {
    padding: 12px 14px;
    border: 1px solid;
    border-radius: var(--r-ctl);
  }
  .title {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
    font-weight: 600;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 4px;
    color: var(--fg2);
  }
  .safe {
    background: #f0fdf4;
    border-color: #bbf7d0;
  }
  .safe .title {
    color: var(--safe);
  }
  .review {
    background: #fffbeb;
    border-color: #fde68a;
  }
  .review .title {
    color: var(--review);
  }
  .danger {
    background: #fef2f2;
    border-color: #fecaca;
  }
  .danger .title {
    color: var(--danger);
  }
  .info {
    background: var(--row-hover);
    border-color: var(--line);
  }
  .info .title {
    color: var(--fg2);
  }
</style>
