<script lang="ts">
  import type { Snippet } from "svelte";
  import type { IconComponent } from "../lib/types";

  let {
    icon: Icon,
    title,
    meta = "",
    desc = "",
    actions,
  }: {
    icon: IconComponent;
    title: string;
    meta?: string;
    desc?: string;
    actions?: Snippet;
  } = $props();
</script>

<header>
  <div class="tile"><Icon size={22} /></div>
  <div class="titles">
    <h1>{title}</h1>
    {#if meta}<p class="meta">{meta}</p>{/if}
  </div>
  {#if actions}<div class="actions">{@render actions()}</div>{/if}
</header>
{#if desc}<p class="desc">{desc}</p>{/if}

<style>
  header {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .tile {
    width: 44px;
    height: 44px;
    flex: none;
    display: grid;
    place-items: center;
    border-radius: var(--r-card);
    background: var(--acc-bg);
    color: var(--acc);
  }
  .titles {
    flex: 1;
    min-width: 0;
  }
  h1 {
    font-size: 21px;
    font-weight: 700;
    line-height: 1.3;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    color: var(--mut);
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .desc {
    max-width: 820px;
    margin-top: 10px;
    color: var(--mut);
  }
</style>
