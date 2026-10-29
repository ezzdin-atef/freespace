<script lang="ts">
  import { CaretRight, Folder, FolderOpen, HardDrive, Search, SpaceUsage } from "../lib/icons";
  import type { UsageRow } from "../lib/api";
  import { session, type Show, type Sort } from "../lib/session.svelte";
  import { fmtCompact, fmtSize } from "../lib/format";
  import { handleListKeys } from "../lib/listKeys";
  import { SAFETY_LABEL, SAFETY_TONE } from "../lib/safety";
  import BulkBar from "../components/BulkBar.svelte";
  import Card from "../components/Card.svelte";
  import Field from "../components/Field.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import Pill from "../components/Pill.svelte";
  import Segmented from "../components/Segmented.svelte";
  import SortMenu from "../components/SortMenu.svelte";
  import Table, { type Column } from "../components/Table.svelte";
  import UsageDetail from "./UsageDetail.svelte";

  let table: Table<UsageRow, Sort> | undefined = $state();
  let search: Field | undefined = $state();

  // The scan root's level loads the first time the page opens.
  $effect(() => {
    if (session.scan && !session.usage) session.loadUsage(null);
  });

  const level = $derived(session.usage);
  const visible = $derived.by(() => {
    const q = session.filter.trim().toLowerCase();
    const rows = (level?.children ?? []).filter(
      (c) => (session.show === "all" || c.safety === session.show) && (!q || c.name.toLowerCase().includes(q)),
    );
    return session.sort === "name"
      ? rows.sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }))
      : rows.sort((a, b) => b.size - a.size);
  });
  const focused = $derived(visible.find((c) => c.path === session.usageFocus) ?? null);

  const columns: Column<Sort>[] = [
    { key: "name", title: "Folder", width: "minmax(200px, 1fr)", sort: "name" },
    { key: "share", title: "Share", width: "160px" },
    { key: "safety", title: "Safety", width: "130px" },
    { key: "size", title: "Size", width: "100px", right: true, sort: "size" },
  ];
  const showTabs: { key: Show; label: string }[] = [
    { key: "all", label: "All" },
    { key: "safe", label: "Safe" },
    { key: "review", label: "Review" },
    { key: "protected", label: "Protected" },
  ];

  const crumbName = (p: string) => p.split("\\").filter(Boolean).at(-1) ?? p;

  function open(row: UsageRow) {
    if (row.has_children) session.openFolder(row.path);
    else session.revealPath(row.path);
  }

  // A new level, a new scroll position.
  $effect(() => {
    void level?.path;
    table?.scrollToTop();
  });

  function onkeydown(e: KeyboardEvent) {
    handleListKeys(e, {
      keys: visible.map((c) => c.path),
      focused: session.usageFocus,
      setFocus: (p, i) => {
        session.usageFocus = p;
        table?.scrollToIndex(i);
      },
      focusSearch: () => search?.focus(),
      clearFilter: () => (session.filter = ""),
      toggle: (p) => {
        const row = visible.find((c) => c.path === p);
        if (row) session.toggleFolder(row);
      },
      open: (p) => {
        const row = visible.find((c) => c.path === p);
        if (row?.has_children) session.openFolder(row.path);
      },
      back: () => session.usageUp(session.usagePath.length - 1),
      clean: () => session.requestClean(),
    });
  }
</script>

<svelte:window {onkeydown} />

<div class="page">
  <section class="content">
    <PageHeader
      icon={SpaceUsage}
      title="Space usage"
      meta={level ? `${fmtSize(level.size)}  ·  ${fmtCompact(level.files)} files` : ""}
      desc="Biggest folders first. Double-click or Enter opens a folder, Backspace goes back up. Tick any folder to delete it; protected folders can't be ticked."
    />

    <nav class="crumbs" aria-label="Folder path">
      <button class:on={session.usagePath.length === 0} onclick={() => session.usageUp(0)}>
        <HardDrive size={14} />{session.scan?.root}
      </button>
      {#each session.usagePath as p, i (p)}
        <CaretRight size={14} />
        <button class:on={i === session.usagePath.length - 1} onclick={() => session.usageUp(i + 1)}>{crumbName(p)}</button>
      {/each}
    </nav>

    <div class="toolbar">
      <Field
        bind:this={search}
        bind:value={session.filter}
        icon={Search}
        placeholder="Filter folders  (Ctrl+F)"
        width={360}
        clearable
        title="Ctrl+F jumps here, Esc clears it"
      />
      <span class="tabs"><Segmented items={showTabs} value={session.show} onchange={(s) => (session.show = s)} /></span>
      <SortMenu value={session.sort === "group" ? "size" : session.sort} groupLabel={null} onchange={(s) => (session.sort = s)} />
    </div>

    <div class="table">
      <Card>
        <Table
          bind:this={table}
          rows={visible}
          {columns}
          key={(c) => c.path}
          sort={session.sort === "name" ? "name" : "size"}
          onsort={(s) => (session.sort = s)}
          checked={(c) => session.selectedFolders.has(c.path)}
          checkable={(c) => c.safety !== "protected"}
          ontoggle={(c) => {
            session.usageFocus = c.path;
            session.toggleFolder(c);
          }}
          onsetall={(rows, on) => session.setFolders(rows, on)}
          focused={session.usageFocus}
          onfocus={(c) => (session.usageFocus = c.path)}
          onopen={open}
        >
          {#snippet cell(c, key)}
            {#if key === "name"}
              {@const Icon = c.has_children ? Folder : FolderOpen}
              <span class="name">
                <Icon size={16} />
                <span class="text" title={c.path}>{c.name}</span>
                {#if c.tag}<span class="tag">{c.tag}</span>{/if}
              </span>
            {:else if key === "share"}
              {@const frac = c.size / Math.max(level?.size ?? 1, 1)}
              <span class="share"><i><b style:width="{frac * 100}%"></b></i>{Math.round(frac * 100)}%</span>
            {:else if key === "safety"}
              <Pill tone={SAFETY_TONE[c.safety]} label={SAFETY_LABEL[c.safety]} />
            {:else}
              <b class="size">{fmtSize(c.size)}</b>
            {/if}
          {/snippet}
          {#snippet empty()}
            <div class="empty">
              <span class="tile"><FolderOpen size={26} /></span>
              <b>{level ? "No sub-folders over 1 MB here" : "Loading..."}</b>
              {#if level}<p>Go back up, or clear the filter.</p>{/if}
            </div>
          {/snippet}
        </Table>
      </Card>
    </div>

    <BulkBar />
  </section>

  <aside>
    {#if level}<UsageDetail {level} row={focused} />{/if}
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
    gap: 12px;
    padding: 24px 28px 20px;
  }
  .crumbs {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px;
    color: var(--mut2);
  }
  .crumbs button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 8px;
    border: 1px solid transparent;
    border-radius: var(--r-ctl);
    background: transparent;
    color: var(--mut);
    font-weight: 600;
  }
  .crumbs button:hover {
    background: var(--pane);
    color: var(--fg);
  }
  .crumbs button.on {
    border-color: var(--line);
    background: var(--surface);
    color: var(--fg);
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }
  .tabs {
    flex: 1;
  }
  @container (max-width: 760px) {
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
  .name {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .name :global(svg) {
    flex: none;
    color: var(--mut);
  }
  .text {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tag {
    flex: none;
    padding: 1px 8px;
    border-radius: var(--r-sm);
    background: var(--acc-bg);
    color: var(--acc);
    font-size: 12px;
    font-weight: 600;
  }
  .share {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    color: var(--mut);
  }
  .share i {
    width: 90px;
    height: 6px;
    border-radius: 3px;
    background: var(--pane);
    overflow: hidden;
  }
  .share b {
    display: block;
    height: 100%;
    background: var(--acc);
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
