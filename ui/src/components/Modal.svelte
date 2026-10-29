<script lang="ts">
  import type { Snippet } from "svelte";
  import type { IconComponent } from "../lib/types";

  /** Centered dialog over a dimmed backdrop. Esc and the backdrop call `onclose` when it is given. */
  let {
    icon: Icon,
    tone = "accent",
    title,
    width = 540,
    onclose,
    children,
    footer,
  }: {
    icon: IconComponent;
    tone?: "accent" | "danger" | "review";
    title: string;
    width?: number;
    onclose?: () => void;
    children: Snippet;
    footer?: Snippet;
  } = $props();

  let box: HTMLDivElement;

  // Move focus into the dialog so Enter/Space act on it, not on the page behind.
  $effect(() => {
    const target = box.querySelector<HTMLElement>("[data-autofocus]") ?? box;
    target.focus();
  });
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose?.()} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose?.()}>
  <div class="modal" role="dialog" aria-modal="true" aria-label={title} tabindex="-1" bind:this={box} style:width="{width}px">
    <div class="head">
      <span class="tile {tone}"><Icon size={20} /></span>
      <h2>{title}</h2>
    </div>
    <div class="body">{@render children()}</div>
    {#if footer}<div class="footer">{@render footer()}</div>{/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgb(15 23 42 / 0.43);
    animation: fade 0.12s ease-out;
  }
  .modal {
    max-width: 100%;
    max-height: calc(100vh - 48px);
    display: flex;
    flex-direction: column;
    padding: 24px;
    background: var(--surface);
    border-radius: 10px;
    box-shadow: 0 8px 28px rgb(15 23 42 / 0.25);
    outline: none;
    animation: pop 0.14s ease-out;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 12px;
  }
  .tile {
    width: 40px;
    height: 40px;
    flex: none;
    display: grid;
    place-items: center;
    border-radius: var(--r-card);
  }
  .accent {
    background: var(--acc-bg);
    color: var(--acc);
  }
  .danger {
    background: var(--danger-bg);
    color: var(--danger);
  }
  .review {
    background: var(--review-bg);
    color: var(--review);
  }
  h2 {
    font-size: 19px;
    font-weight: 700;
  }
  .body {
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
    color: var(--fg2);
  }
  .footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
    padding-top: 16px;
    border-top: 1px solid var(--line);
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.97);
    }
  }
</style>
