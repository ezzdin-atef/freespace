<script lang="ts">
  import { ArrowRight, Check, FolderOpen, X } from "../lib/icons";
  import * as api from "../lib/api";
  import type { UsageLevel, UsageRow } from "../lib/api";
  import { session } from "../lib/session.svelte";
  import { fmtCount, fmtSize } from "../lib/format";
  import { SAFETY_LABEL, SAFETY_TONE } from "../lib/safety";
  import Button from "../components/Button.svelte";
  import Callout, { type CalloutTone } from "../components/Callout.svelte";
  import Pill from "../components/Pill.svelte";
  import Treemap from "../components/Treemap.svelte";

  // Right-hand panel of Space usage: the focused folder, or the folder being browsed when nothing is focused.
  let { level, row }: { level: UsageLevel; row: UsageRow | null } = $props();

  // What's inside the focused folder; Rust sends one tree level at a time, so fetch it on focus.
  let inside = $state.raw<UsageLevel | null>(null);
  $effect(() => {
    const r = row;
    const scan = session.scan;
    inside = null;
    if (!r || !r.has_children || !scan) return;
    let live = true;
    api.usage(scan.scan_id, r.path).then((l) => live && (inside = l)).catch(() => {});
    return () => (live = false);
  });

  const children = $derived((row ? inside?.children : level.children) ?? []);
  const blocks = $derived(children.slice(0, 40).map((c) => ({ name: c.name, size: c.size })));
  const tagGroup = $derived(row?.tag ? session.groups.find((g) => g.label === row.tag) : undefined);
  const TONE: Record<string, CalloutTone> = { safe: "safe", review: "review", protected: "danger" };

  async function pick(i: number) {
    const target = children[i];
    if (!target) return;
    if (row) await session.openFolder(row.path);
    session.usageFocus = target.path;
  }
</script>

<div class="detail">
  {#if row}
    <Pill tone={SAFETY_TONE[row.safety]} label={SAFETY_LABEL[row.safety]} />
  {/if}
  <h2>{row?.name ?? level.name}</h2>
  <p class="path">{row?.path ?? level.path}</p>

  <dl>
    <dt>Size</dt>
    <dd>{fmtSize(row?.size ?? level.size)}</dd>
    <dt>Files</dt>
    <dd>{fmtCount(row?.files ?? level.files)}</dd>
    {#if row}
      <dt>Share of parent</dt>
      <dd>{((row.size / Math.max(level.size, 1)) * 100).toFixed(1)}%</dd>
    {/if}
  </dl>

  {#if row?.tag}
    <Callout tone="safe" title="Cleanable: {row.tag}">
      <p>{tagGroup?.restore ?? "Listed under Clean up."}</p>
    </Callout>
  {:else if row}
    <Callout tone={TONE[row.safety]} title={SAFETY_LABEL[row.safety]}><p>{row.note}</p></Callout>
  {/if}

  {#if blocks.length > 0}
    <h3>What's inside</h3>
    <Treemap items={blocks} onpick={pick} />
  {/if}

  <div class="actions">
    {#if row}
      {@const on = session.selectedFolders.has(row.path)}
      <Button
        label={on ? "Untick" : "Tick for cleaning"}
        icon={on ? X : Check}
        primary={!on}
        wide
        disabled={row.safety === "protected"}
        onclick={() => session.toggleFolder(row)}
      />
      {#if row.has_children}
        <Button label="Open this folder" icon={ArrowRight} wide onclick={() => session.openFolder(row.path)} />
      {/if}
    {/if}
    <Button label="Show in Explorer" icon={FolderOpen} wide onclick={() => session.revealPath(row?.path ?? level.path)} />
  </div>
</div>

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
  h3 {
    margin-top: 4px;
    font-size: 15px;
    font-weight: 600;
  }
  .actions {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 4px;
  }
</style>
