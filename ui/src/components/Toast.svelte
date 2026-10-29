<script lang="ts">
  import { CheckCircle, Warning, X, XCircle } from "../lib/icons";
  import { session } from "../lib/session.svelte";

  const ICONS = { safe: CheckCircle, review: Warning, danger: XCircle };

  // Each new toast restarts the 8 s timer.
  $effect(() => {
    const t = session.toast;
    if (!t) return;
    const timer = setTimeout(() => session.toast?.id === t.id && session.dismissToast(), 8000);
    return () => clearTimeout(timer);
  });
</script>

{#if session.toast}
  {@const Icon = ICONS[session.toast.tone]}
  <div class="toast {session.toast.tone}" role="status">
    <Icon size={18} />
    <p>{session.toast.text}</p>
    <button aria-label="Dismiss" onclick={() => session.dismissToast()}><X size={14} /></button>
  </div>
{/if}

<style>
  .toast {
    position: fixed;
    right: 24px;
    /* Clear of the bulk action bar at the bottom of list pages. */
    bottom: 84px;
    z-index: 50;
    max-width: 440px;
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 12px 12px 12px 16px;
    border-radius: var(--r-card);
    background: #0f172a;
    color: #fff;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.2);
    animation: rise 0.18s ease-out;
  }
  .toast > :global(svg) {
    flex: none;
    margin-top: 1px;
  }
  .safe > :global(svg) {
    color: #4ade80;
  }
  .review > :global(svg) {
    color: #fcd34d;
  }
  .danger > :global(svg) {
    color: #f87171;
  }
  p {
    flex: 1;
  }
  button {
    flex: none;
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    color: #cbd5e1;
  }
  button:hover {
    background: #1e293b;
    color: #fff;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }
</style>
