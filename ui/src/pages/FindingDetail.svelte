<script lang="ts">
  import { Check, FolderOpen, X } from "../lib/icons";
  import type { FindingRow } from "../lib/api";
  import { session } from "../lib/session.svelte";
  import { categoryIcon } from "../lib/icons";
  import { fmtCount, fmtSize } from "../lib/format";
  import { SAFETY_LABEL, SAFETY_TONE } from "../lib/safety";
  import Button from "../components/Button.svelte";
  import Callout, { type CalloutTone } from "../components/Callout.svelte";
  import Pill from "../components/Pill.svelte";

  // Right-hand panel of a findings page: the focused row, or what the page's category is about.
  let { finding, category }: { finding: FindingRow | null; category: string | null } = $props();

  const group = $derived(session.groupByCategory.get(finding?.category ?? category ?? ""));
  const Icon = $derived(categoryIcon(finding?.category ?? category ?? ""));

  const WHY: Record<string, [CalloutTone, string]> = {
    safe: ["safe", "Why it's safe"],
    review: ["review", "Check before deleting"],
    protected: ["danger", "Why it's protected"],
  };

  function whatHappens(f: FindingRow): string {
    if (f.kind === "recycle_bin") return "Empty the Recycle Bin";
    if (f.mode === "contents") return "Empty the folder, keep it";
    return f.kind === "file" ? "Delete the file" : "Delete the folder";
  }
</script>

{#if finding}
  {@const [tone, why] = WHY[finding.safety]}
  {@const on = session.selected.has(finding.id)}
  <div class="detail">
    <Pill tone={SAFETY_TONE[finding.safety]} label={SAFETY_LABEL[finding.safety]} />
    <h2 title={finding.name}>{finding.name}</h2>
    <p class="path">{finding.path}</p>

    <dl>
      <dt>Size</dt>
      <dd>{fmtSize(finding.size)}</dd>
      <dt>Files</dt>
      <dd>{fmtCount(finding.files)}</dd>
      <dt>Category</dt>
      <dd>{group?.label ?? finding.category}</dd>
      <dt>What happens</dt>
      <dd>{whatHappens(finding)}</dd>
    </dl>

    <Callout {tone} title={why}>
      <p class="note">{finding.note}</p>
      {#if group}<p class="small">{group.description}</p>{/if}
    </Callout>
    {#if group}
      <Callout tone="info" title="How to get it back"><p>{group.restore}</p></Callout>
    {/if}

    <div class="actions">
      <Button
        label={on ? "Untick" : "Tick for cleaning"}
        icon={on ? X : Check}
        primary={!on}
        wide
        disabled={finding.safety === "protected"}
        onclick={() => session.toggle(finding.id)}
      />
      {#if finding.kind !== "recycle_bin"}
        <Button label="Show in Explorer" icon={FolderOpen} wide onclick={() => session.reveal(finding)} />
      {/if}
    </div>
  </div>
{:else}
  <div class="detail">
    <div class="empty">
      <span class="tile"><Icon size={26} /></span>
      <b>Select a row</b>
      <p>Click any row to see what it is, why it is safe or not, and how to get it back.</p>
    </div>
    {#if category && group}
      <Callout tone="info" title="How to get it back"><p>{group.restore}</p></Callout>
    {/if}
  </div>
{/if}

<style>
  .detail {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .detail > :global(.pill) {
    align-self: flex-start;
  }
  h2 {
    margin-top: -4px;
    font-size: 18px;
    font-weight: 700;
    line-height: 1.3;
    overflow-wrap: anywhere;
  }
  .path {
    margin-top: -8px;
    font: 13px var(--mono);
    color: var(--mut);
    overflow-wrap: anywhere;
    user-select: text;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 12px;
  }
  dt {
    color: var(--mut);
  }
  dd {
    margin: 0;
    font-weight: 600;
    text-align: right;
  }
  .note {
    color: var(--fg);
  }
  .small {
    font-size: 13px;
  }
  .actions {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 4px;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 40px 8px 12px;
    text-align: center;
  }
  .empty b {
    font-size: 16px;
    font-weight: 600;
  }
  .empty p {
    color: var(--mut);
  }
  .tile {
    width: 56px;
    height: 56px;
    display: grid;
    place-items: center;
    margin-bottom: 6px;
    border-radius: var(--r-card);
    background: var(--pane);
    color: var(--mut);
  }
</style>
