<script lang="ts">
  import { Info, Square, SquareCheck, Trash2, TriangleAlert } from "@lucide/svelte";
  import { session } from "../lib/session.svelte";
  import { fmtSize } from "../lib/format";
  import Button from "./Button.svelte";
  import Kbd from "./Kbd.svelte";

  // Under a findings table: a hint while nothing is ticked, the dark action bar once something is.
  const sel = $derived(session.selection);
</script>

{#if sel.count > 0}
  <div class="bar">
    <b>{sel.count} selected</b>
    <strong>{fmtSize(sel.bytes)}</strong>
    {#if sel.review > 0}
      <span class="review"><TriangleAlert size={15} /> {sel.review} to review</span>
    {/if}
    <span class="grow"></span>
    <Button label="Clear" variant="dark" onclick={() => session.selectNone()} />
    <Button
      label="Recycle Bin"
      icon={session.useTrash ? SquareCheck : Square}
      variant="dark"
      title="Move to the Recycle Bin instead of deleting. Recoverable, but slower - the space is only freed after you empty the bin."
      onclick={() => (session.useTrash = !session.useTrash)}
    />
    <Button
      label="Delete {fmtSize(sel.bytes)}"
      icon={Trash2}
      variant="danger"
      disabled={session.busy}
      onclick={() => session.requestClean()}
    />
  </div>
{:else}
  <p class="hint">
    <Info size={14} /> Tick rows to clean them
    <Kbd keys="Space" /> tick
    <Kbd keys="Del" /> clean
    <span>Double-click to open</span>
  </p>
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 10px 10px 18px;
    border-radius: var(--r-card);
    background: #0f172a;
    color: #fff;
    box-shadow: 0 6px 18px rgb(0 0 0 / 0.16);
    animation: rise 0.15s ease-out;
  }
  b {
    font-size: 15px;
    font-weight: 600;
  }
  strong {
    font-size: 16px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .review {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: #fcd34d;
  }
  .grow {
    flex: 1;
  }
  .hint {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: var(--mut);
  }
  .hint span {
    margin-left: 6px;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
</style>
