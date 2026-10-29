<script lang="ts">
  import { AppWindow, Search, Store } from "@lucide/svelte";
  import type { AppRow } from "../lib/api";
  import { session, type AppShow, type Sort } from "../lib/session.svelte";
  import { fmtCount, fmtSize } from "../lib/format";
  import { handleListKeys } from "../lib/listKeys";
  import Card from "../components/Card.svelte";
  import Field from "../components/Field.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import Segmented from "../components/Segmented.svelte";
  import SortMenu from "../components/SortMenu.svelte";
  import Table, { type Column } from "../components/Table.svelte";
  import AppDetail from "./AppDetail.svelte";

  let table: Table<AppRow, Sort> | undefined = $state();
  let search: Field | undefined = $state();

  const visible = $derived.by(() => {
    const q = session.filter.trim().toLowerCase();
    const rows = session.apps.filter(
      (a) =>
        (session.appShow === "all" || (session.appShow === "store") === a.store) &&
        (!q || a.name.toLowerCase().includes(q) || a.publisher.toLowerCase().includes(q)),
    );
    const byName = (a: AppRow, b: AppRow) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
    switch (session.sort) {
      case "size":
        return rows.sort((a, b) => b.size - a.size);
      case "name":
        return rows.sort(byName);
      case "group":
        return rows.sort((a, b) => a.publisher.localeCompare(b.publisher, undefined, { sensitivity: "base" }) || byName(a, b));
    }
  });
  const total = $derived(visible.reduce((n, a) => n + a.size, 0));
  const desktop = $derived(session.apps.filter((a) => !a.store).length);
  const focused = $derived(session.apps.find((a) => a.id === session.appFocus) ?? null);

  // The detail panel is never empty: opening the page focuses the first app.
  $effect(() => {
    if (session.appFocus === null && visible.length > 0) session.focusApp(visible[0].id);
  });

  const columns: Column<Sort>[] = [
    { key: "name", title: "App", width: "minmax(200px, 1fr)", sort: "name" },
    { key: "publisher", title: "Publisher", width: "minmax(100px, 220px)", sort: "group" },
    { key: "version", title: "Version", width: "130px" },
    { key: "installed", title: "Installed", width: "110px" },
    { key: "size", title: "Size", width: "96px", right: true, sort: "size" },
  ];
  const tabs = $derived<{ key: AppShow; label: string; count: string }[]>([
    { key: "all", label: "All", count: String(session.apps.length) },
    { key: "desktop", label: "Desktop", count: String(desktop) },
    { key: "store", label: "Store", count: String(session.apps.length - desktop) },
  ]);

  function onkeydown(e: KeyboardEvent) {
    handleListKeys(e, {
      keys: visible.map((a) => a.id),
      focused: session.appFocus,
      setFocus: (id, i) => {
        session.focusApp(id);
        table?.scrollToIndex(i);
      },
      focusSearch: () => search?.focus(),
      clearFilter: () => (session.filter = ""),
    });
  }
</script>

<svelte:window {onkeydown} />

<div class="page">
  <section class="content">
    <PageHeader
      icon={AppWindow}
      title="Installed apps"
      meta="{fmtCount(visible.length)} apps  ·  {fmtSize(total)}{session.appsLoading ? '  ·  measuring sizes...' : ''}"
      desc="Pick an app to see its folders. Uninstall runs the app's own uninstaller; afterwards Freespace finds the files it left behind so you can delete them too."
    />

    <div class="toolbar">
      <Field
        bind:this={search}
        bind:value={session.filter}
        icon={Search}
        placeholder="Search apps  (Ctrl+F)"
        width={360}
        clearable
        title="Ctrl+F jumps here, Esc clears it"
      />
      <span class="tabs"><Segmented items={tabs} value={session.appShow} onchange={(s) => (session.appShow = s)} /></span>
      <SortMenu value={session.sort} groupLabel="Publisher" onchange={(s) => (session.sort = s)} />
    </div>

    <div class="table">
      <Card>
        {#if session.apps.length === 0 && session.appsLoading}
          <p class="loading"><span class="spin"></span> Reading the installed programs list...</p>
        {:else}
          <Table
            bind:this={table}
            rows={visible}
            {columns}
            key={(a) => a.id}
            sort={session.sort}
            onsort={(s) => (session.sort = s)}
            focused={session.appFocus}
            onfocus={(a) => session.focusApp(a.id)}
            onopen={(a) => session.focusApp(a.id)}
          >
            {#snippet cell(a, key)}
              {#if key === "name"}
                {@const Icon = a.store ? Store : AppWindow}
                <span class="name" class:gone={a.removed}>
                  <span class="icon" class:store={a.store}><Icon size={15} /></span>
                  <b>{a.name}</b>
                  {#if a.removed}<span class="pill">Uninstalled</span>{/if}
                </span>
              {:else if key === "publisher"}
                <span class="muted">{a.publisher}</span>
              {:else if key === "version"}
                <span class="muted small">{a.version}</span>
              {:else if key === "installed"}
                <span class="muted small">{a.installed}</span>
              {:else}
                <b class="size">{a.size > 0 ? fmtSize(a.size) : "-"}</b>
              {/if}
            {/snippet}
            {#snippet empty()}
              <div class="empty">
                <b>No apps match your search</b>
                <p>Check the spelling or switch the filter to All.</p>
              </div>
            {/snippet}
          </Table>
        {/if}
      </Card>
    </div>
  </section>

  <aside>
    {#if focused}
      <AppDetail app={focused} />
    {:else}
      <div class="empty">
        <span class="tile"><AppWindow size={26} /></span>
        <b>Select an app</b>
        <p>See where it is installed, uninstall it, and remove the folders it leaves behind.</p>
      </div>
    {/if}
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
  @container (max-width: 820px) {
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
  .name b {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 600;
  }
  .gone b {
    color: var(--mut);
  }
  .icon {
    width: 26px;
    height: 26px;
    flex: none;
    display: grid;
    place-items: center;
    border-radius: 7px;
    background: var(--pane);
    color: var(--fg2);
  }
  .icon.store {
    background: var(--acc-bg);
    color: var(--acc);
  }
  .pill {
    flex: none;
    padding: 1px 8px;
    border-radius: var(--r-sm);
    background: var(--safe-bg);
    color: var(--safe);
    font-size: 12px;
    font-weight: 600;
  }
  .muted {
    color: var(--mut);
  }
  .small {
    font-size: 13px;
  }
  .size {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .loading {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 24px;
    color: var(--mut);
  }
  .spin {
    width: 16px;
    height: 16px;
    border: 2px solid var(--line);
    border-top-color: var(--acc);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
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
