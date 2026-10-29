<script lang="ts">
  import { Broom, CircleCheck, Search, SquareCheck } from "@lucide/svelte";
  import type { FindingRow } from "../lib/api";
  import { session, type Show, type Sort } from "../lib/session.svelte";
  import { categoryIcon } from "../lib/icons";
  import { fmtCount, fmtSize } from "../lib/format";
  import { handleListKeys } from "../lib/listKeys";
  import { SAFETY_LABEL, SAFETY_TONE } from "../lib/safety";
  import BulkBar from "../components/BulkBar.svelte";
  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import Field from "../components/Field.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import Pill from "../components/Pill.svelte";
  import Segmented from "../components/Segmented.svelte";
  import SortMenu from "../components/SortMenu.svelte";
  import Table, { type Column } from "../components/Table.svelte";
  import FindingDetail from "./FindingDetail.svelte";

  const LARGE = "LargeFiles";

  /** `category` null with `large` false is "All cleanable". */
  let { category, large = false }: { category: string | null; large?: boolean } = $props();

  let table: Table<FindingRow, Sort> | undefined = $state();
  let search: Field | undefined = $state();

  const cat = $derived(large ? LARGE : category);
  const group = $derived(cat ? session.groupByCategory.get(cat) : undefined);

  /** Rows of this page before the Show tab and filter. */
  const base = $derived(
    session.findings.filter((f) => (cat ? f.category === cat : f.category !== LARGE)),
  );
  const sums = $derived.by(() => {
    let safe = 0;
    let review = 0;
    for (const f of base) {
      if (f.safety === "safe") safe += f.size;
      else if (f.safety === "review") review += f.size;
    }
    return { safe, review };
  });

  const visible = $derived.by(() => {
    const q = session.filter.trim().toLowerCase();
    const show = session.show;
    const rows = base.filter((f) => (show === "all" || f.safety === show) && (!q || f.path.toLowerCase().includes(q)));
    const label = (f: FindingRow) => session.groupByCategory.get(f.category)?.label ?? f.category;
    switch (session.sort) {
      case "size":
        return rows.sort((a, b) => b.size - a.size);
      case "name":
        return rows.sort((a, b) => a.path.localeCompare(b.path, undefined, { sensitivity: "base" }));
      case "group":
        return rows.sort((a, b) => label(a).localeCompare(label(b)) || b.size - a.size);
    }
  });

  const total = $derived(visible.reduce((n, f) => n + f.size, 0));
  const focused = $derived(session.focusId === null ? null : (session.findingById.get(session.focusId) ?? null));

  const columns: Column<Sort>[] = [
    { key: "path", title: "Path", width: "minmax(200px, 1fr)", sort: "name" },
    { key: "category", title: "Category", width: "minmax(120px, 200px)", sort: "group" },
    { key: "safety", title: "Safety", width: "130px" },
    { key: "size", title: "Size", width: "100px", right: true, sort: "size" },
  ];

  const showTabs = $derived<{ key: Show; label: string; count?: string }[]>([
    { key: "all", label: "All" },
    { key: "safe", label: "Safe", count: fmtSize(sums.safe) },
    { key: "review", label: "Review", count: fmtSize(sums.review) },
    { key: "protected", label: "Protected" },
  ]);

  /** Path relative to the scan root, which every row shares. */
  function rel(f: FindingRow): string {
    const root = session.scan?.root ?? "";
    if (f.kind === "recycle_bin" || !root || !f.path.toLowerCase().startsWith(root.toLowerCase())) return f.path;
    const r = f.path.slice(root.length).replace(/^\\/, "");
    return r || f.path;
  }

  // New page, new scroll position.
  $effect(() => {
    void cat;
    table?.scrollToTop();
  });

  function onkeydown(e: KeyboardEvent) {
    handleListKeys(e, {
      keys: visible.map((f) => f.id),
      focused: session.focusId,
      setFocus: (id, i) => {
        session.focusId = id;
        table?.scrollToIndex(i);
      },
      focusSearch: () => search?.focus(),
      clearFilter: () => (session.filter = ""),
      toggle: (id) => session.toggle(id),
      clean: () => session.requestClean(),
    });
  }
</script>

<svelte:window {onkeydown} />

<div class="page">
  <section class="content">
    <PageHeader
      icon={cat ? categoryIcon(cat) : Broom}
      title={group?.label ?? (large ? "Large files" : "All cleanable")}
      meta="{fmtSize(total)}  ·  {fmtCount(visible.length)} items"
      desc={group?.description ??
        "Everything Freespace can clean here, biggest first. Green is safe to delete, amber needs a quick look, red is never deleted."}
    >
      {#snippet actions()}
        {#if session.selection.count > 0}
          <Button label="Clear selection" variant="ghost" onclick={() => session.selectNone()} />
        {/if}
        {#if !large}
          <Button label="Select all safe" icon={SquareCheck} onclick={() => session.selectSafe(cat)} />
        {/if}
      {/snippet}
    </PageHeader>

    <div class="toolbar">
      <Field
        bind:this={search}
        bind:value={session.filter}
        icon={Search}
        placeholder="Filter by path  (Ctrl+F)"
        width={360}
        clearable
        title="Ctrl+F jumps here, Esc clears it"
      />
      <span class="tabs"><Segmented items={showTabs} value={session.show} onchange={(s) => (session.show = s)} /></span>
      <SortMenu value={session.sort} onchange={(s) => (session.sort = s)} />
    </div>

    <div class="table">
      <Card>
        <Table
          bind:this={table}
          rows={visible}
          {columns}
          key={(f) => f.id}
          sort={session.sort}
          onsort={(s) => (session.sort = s)}
          checked={(f) => session.selected.has(f.id)}
          checkable={(f) => f.safety !== "protected"}
          ontoggle={(f) => {
            session.focusId = f.id;
            session.toggle(f.id);
          }}
          focused={session.focusId}
          onfocus={(f) => (session.focusId = f.id)}
          onopen={(f) => session.reveal(f)}
        >
          {#snippet cell(f, key)}
            {#if key === "path"}
              <span class="path" title={f.path}>&lrm;{rel(f)}&lrm;</span>
            {:else if key === "category"}
              {@const Icon = categoryIcon(f.category)}
              <span class="cat"><Icon size={15} /><span>{session.groupByCategory.get(f.category)?.label ?? f.category}</span></span>
            {:else if key === "safety"}
              <Pill tone={SAFETY_TONE[f.safety]} label={SAFETY_LABEL[f.safety]} />
            {:else}
              <b class="size">{fmtSize(f.size)}</b>
            {/if}
          {/snippet}
          {#snippet empty()}
            <div class="empty">
              <span class="tile"><CircleCheck size={26} /></span>
              <b>{session.filter ? "No items match your filter" : "Nothing to clean here"}</b>
              <p>Try another category or clear the filter.</p>
            </div>
          {/snippet}
        </Table>
      </Card>
    </div>

    <BulkBar />
  </section>

  <aside>
    <FindingDetail finding={focused} category={cat} />
  </aside>
</div>

<style>
  .page {
    height: 100%;
    display: grid;
    grid-template-columns: minmax(0, 1fr) clamp(300px, 25vw, 380px);
  }
  .content {
    container-type: inline-size;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 24px 28px 20px;
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    margin-top: 4px;
  }
  .tabs {
    flex: 1;
  }
  /* Too narrow for one row: search and sort stay on top, the Show tabs move below. */
  @container (max-width: 860px) {
    .tabs {
      order: 3;
      flex-basis: 100%;
    }
  }
  .table {
    flex: 1;
    min-height: 120px;
    display: flex;
  }
  .table > :global(.card) {
    flex: 1;
    min-height: 0;
  }
  aside {
    overflow-y: auto;
    padding: 20px;
    background: var(--surface);
    border-left: 1px solid var(--line);
  }
  /* Right-to-left so a long path is cut at its start and the useful end stays visible. */
  .path {
    display: block;
    direction: rtl;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .cat {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--mut);
  }
  .cat span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .cat :global(svg) {
    flex: none;
  }
  .size {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 60px 20px;
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
