<script lang="ts">
  import { Broom, Square, SquareCheck, Trash2 } from "@lucide/svelte";
  import { session } from "../lib/session.svelte";
  import { fmtCount, fmtSize } from "../lib/format";
  import Button from "./Button.svelte";
  import Callout from "./Callout.svelte";
  import Modal from "./Modal.svelte";
  import ProgressBar from "./ProgressBar.svelte";

  // The confirm dialog for a clean and the progress dialog while one runs. Both block the page behind.
  const c = $derived(session.confirm);
  const p = $derived(session.cleaning);
</script>

{#if c}
  {@const plan = c.plan}
  <Modal icon={Trash2} tone="danger" title="Delete {fmtSize(plan.bytes)}?" onclose={() => session.cancelConfirm()}>
    <p>
      {fmtCount(plan.count)}
      {plan.count === 1 ? "item" : "items"} will be {session.useTrash ? "moved to the Recycle Bin" : "permanently deleted"}.
      Files that are in use are skipped.
    </p>
    {#if plan.warn}
      <Callout tone="danger" title="The app is still installed"><p>{plan.warn}</p></Callout>
    {/if}
    {#if plan.caution.length > 0}
      <Callout tone="review" title="{plan.caution.length} {plan.caution.length === 1 ? 'item needs' : 'items need'} a look">
        <ul class="list">
          {#each plan.caution as line, i (i)}<li>{line}</li>{/each}
        </ul>
      </Callout>
    {/if}
    {#if plan.blocked.length > 0}
      <Callout tone="danger" title="{plan.blocked.length} protected {plan.blocked.length === 1 ? 'item' : 'items'} will be skipped">
        <ul class="list">
          {#each plan.blocked.slice(0, 6) as b (b.path)}<li>{b.path} - {b.reason}</li>{/each}
          {#if plan.blocked.length > 6}<li>... and {plan.blocked.length - 6} more</li>{/if}
        </ul>
      </Callout>
    {/if}
    {#if plan.dism}<p>Then: Windows component store cleanup (several minutes).</p>{/if}
    {#if plan.hibernate_off}<p>Then: turn off hibernation.</p>{/if}

    {#snippet footer()}
      <button class="trash" onclick={() => (session.useTrash = !session.useTrash)}>
        {#if session.useTrash}<SquareCheck size={16} />{:else}<Square size={16} />{/if}
        Move to the Recycle Bin instead
      </button>
      <span class="grow"></span>
      <Button label="Cancel" onclick={() => session.cancelConfirm()} />
      <!-- No autofocus on Delete: a stray Enter must never delete. -->
      <Button label="Delete" icon={Trash2} variant="danger" onclick={() => session.confirmGo()} />
    {/snippet}
  </Modal>
{/if}

{#if p}
  {@const frac = p.total > 0 ? p.done / p.total : null}
  <Modal icon={Broom} title="Cleaning... {fmtSize(p.freed)} freed" width={460}>
    <ProgressBar {frac} />
    <p>
      {fmtCount(p.done)} of {fmtCount(p.total)} items · {fmtCount(p.files)} files · {fmtCount(p.failed)} failed
    </p>
    <p class="current" title={p.current}>{p.current || " "}</p>
    {#snippet footer()}
      <Button label="Stop" icon={Square} onclick={() => session.stopClean()} />
    {/snippet}
  </Modal>
{/if}

<style>
  .list {
    max-height: 140px;
    overflow-y: auto;
    margin: 0;
    padding: 0;
    list-style: none;
    font: 12px var(--mono);
    color: var(--fg2);
    user-select: text;
  }
  .list li {
    padding: 1px 0;
    overflow-wrap: anywhere;
  }
  .trash {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 0 4px;
    border: 0;
    background: transparent;
    color: var(--fg2);
  }
  .trash :global(svg) {
    color: var(--acc);
  }
  .grow {
    flex: 1;
  }
  .current {
    font: 12px var(--mono);
    color: var(--mut);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
