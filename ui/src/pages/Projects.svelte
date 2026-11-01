<script lang="ts">
  import { Box, CheckSquare, Code, Search } from "../lib/icons";
  import type { ProjectRow } from "../lib/api";
  import { session, type ProjectShow, type Sort } from "../lib/session.svelte";
  import { fmtAgo, fmtCount, fmtSize } from "../lib/format";
  import { handleListKeys } from "../lib/listKeys";
  import BulkBar from "../components/BulkBar.svelte";
  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import Field from "../components/Field.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import Segmented from "../components/Segmented.svelte";
  import SortMenu from "../components/SortMenu.svelte";
  import Table, { type Column } from "../components/Table.svelte";
  import ProjectDetail from "./ProjectDetail.svelte";

  let table: Table<ProjectRow, Sort> | undefined = $state();
  let search: Field | undefined = $state();

  const visible = $derived.by(() => {
    // Every word must match somewhere: name, type, branch or folder ("rust old", "feature-x").
    const words = session.filter.trim().toLowerCase().split(/\s+/).filter(Boolean);
    const haystack = (p: ProjectRow) => `${p.name} ${p.kinds.join(" ")} ${p.branch ?? ""} ${p.path}`.toLowerCase();
    const rows = session.projects.filter(
      (p) =>
        (session.projectShow === "all" || (session.projectShow === "dormant") === p.dormant) &&
        (words.length === 0 || words.every((w) => haystack(p).includes(w))),
    );
    const byName = (a: ProjectRow, b: ProjectRow) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
    switch (session.sort) {
      case "size":
        // What can be freed is what this page is for; total size breaks ties between projects with nothing to clean.
        return rows.sort((a, b) => b.cleanable - a.cleanable || b.size - a.size);
      case "name":
        return rows.sort(byName);
      case "group":
        return rows.sort((a, b) => a.kinds.join().localeCompare(b.kinds.join()) || byName(a, b));
      case "date":
        return rows.sort((a, b) => (b.last_active ?? 0) - (a.last_active ?? 0) || byName(a, b));
    }
  });

  const total = $derived(visible.reduce((n, p) => n + p.size, 0));
  const cleanable = $derived(visible.reduce((n, p) => n + p.cleanable, 0));
  const dormantSafe = $derived(session.dormantSafeBytes);
  const dormantCount = $derived(session.projects.filter((p) => p.dormant).length);
  const caches = $derived(session.groupByCategory.get("DevCache"));
  const focused = $derived(session.projects.find((p) => p.id === session.projectFocus) ?? null);

  // The detail panel is never empty while there is a row to show.
  $effect(() => {
    if (focused === null && visible.length > 0) session.projectFocus = visible[0].id;
  });

  const columns: Column<Sort>[] = [
    { key: "name", title: "Project", width: "minmax(180px, 1fr)", sort: "name" },
    { key: "kinds", title: "Type", width: "minmax(90px, 150px)", sort: "group" },
    { key: "active", title: "Last active", width: "130px", sort: "date" },
    { key: "size", title: "Total", width: "96px", right: true },
    { key: "cleanable", title: "Cleanable", width: "110px", right: true, sort: "size" },
  ];
  const tabs = $derived<{ key: ProjectShow; label: string; count: string }[]>([
    { key: "all", label: "All", count: String(session.projects.length) },
    { key: "active", label: "Active", count: String(session.projects.length - dormantCount) },
    { key: "dormant", label: "Dormant", count: String(dormantCount) },
  ]);

  function onkeydown(e: KeyboardEvent) {
    handleListKeys(e, {
      keys: visible.map((p) => p.id),
      focused: session.projectFocus,
      setFocus: (id, i) => {
        session.projectFocus = id;
        table?.scrollToIndex(i);
      },
      focusSearch: () => search?.focus(),
      clearFilter: () => (session.filter = ""),
      toggle: (id) => {
        const p = session.projects.find((x) => x.id === id);
        if (p) session.toggleProject(p);
      },
      open: (id) => {
        const p = session.projects.find((x) => x.id === id);
        if (p) session.revealPath(p.path);
      },
      clean: () => session.requestClean(),
    });
  }
</script>

<svelte:window {onkeydown} />

<div class="page">
  <section class="content">
    <PageHeader
      icon={Code}
      title="Projects"
      meta="{fmtCount(visible.length)} projects  ·  {fmtSize(total)} total  ·  {fmtSize(cleanable)} cleanable"
      desc="Every code project in this scan, and what it can rebuild: dependencies, build output and caches. Your source code is never offered for deletion."
    >
      {#snippet actions()}
        {#if caches}
          <Button
            label="Package caches {fmtSize(caches.total)}"
            icon={Box}
            title={caches.description}
            onclick={() => session.go({ page: "cleanable", category: "DevCache" })}
          />
        {/if}
        <Button
          label="Tick safe in dormant"
          icon={CheckSquare}
          disabled={dormantSafe === 0}
          title="Ticks what can safely be rebuilt in projects not touched for 90 days ({fmtSize(dormantSafe)}). Anything that needs a look stays unticked."
          onclick={() => session.tickDormantSafe()}
        />
      {/snippet}
    </PageHeader>

    <div class="toolbar">
      <Field
        bind:this={search}
        bind:value={session.filter}
        icon={Search}
        placeholder="Search name, type, branch, path  (Ctrl+F)"
        width={360}
        clearable
        title="Ctrl+F jumps here, Esc clears it"
      />
      <span class="tabs"><Segmented items={tabs} value={session.projectShow} onchange={(s) => (session.projectShow = s)} /></span>
      <SortMenu value={session.sort} groupLabel="Type" dateLabel="Last active" onchange={(s) => (session.sort = s)} />
    </div>

    <div class="table">
      <Card>
        <Table
          bind:this={table}
          rows={visible}
          {columns}
          key={(p) => p.id}
          sort={session.sort}
          onsort={(s) => (session.sort = s)}
          checked={(p) => session.projectTicks(p) === "all"}
          mixed={(p) => session.projectTicks(p) === "some"}
          checkable={(p) => p.artifacts.length > 0}
          ontoggle={(p) => {
            session.projectFocus = p.id;
            session.toggleProject(p);
          }}
          onsetall={(rows, on) => session.setProjects(rows, on)}
          focused={session.projectFocus}
          onfocus={(p) => (session.projectFocus = p.id)}
          onopen={(p) => session.revealPath(p.path)}
        >
          {#snippet cell(p, key)}
            {#if key === "name"}
              <span class="name" title={p.path}><b>{p.name}</b>{#if p.branch}<span class="branch">{p.branch}</span>{/if}</span>
            {:else if key === "kinds"}
              <span class="muted small" title={p.kinds.join(", ")}>{p.kinds.join(", ") || "Git"}</span>
            {:else if key === "active"}
              <span class="small" class:dormant={p.dormant} class:muted={!p.dormant}>{fmtAgo(p.last_active)}</span>
            {:else if key === "size"}
              <span class="muted num">{fmtSize(p.size)}</span>
            {:else}
              <b class="num">{p.cleanable > 0 ? fmtSize(p.cleanable) : "-"}</b>
            {/if}
          {/snippet}
          {#snippet empty()}
            <div class="empty">
              <span class="tile"><Code size={26} /></span>
              <b>No projects match</b>
              <p>Check the spelling or switch the filter to All.</p>
            </div>
          {/snippet}
        </Table>
      </Card>
    </div>

    <BulkBar />
  </section>

  <aside>
    {#if focused}
      <ProjectDetail project={focused} />
    {:else}
      <div class="empty">
        <span class="tile"><Code size={26} /></span>
        <b>Select a project</b>
        <p>See what it can rebuild, how to get it back, and how long ago you last worked on it.</p>
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
    overflow-x: hidden;
    padding: 20px;
    background: var(--surface);
    border-left: 1px solid var(--line);
  }
  .name {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }
  .name b {
    flex: none;
    max-width: 70%;
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 600;
  }
  .branch {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font: 12px var(--mono);
    color: var(--mut);
  }
  .muted {
    color: var(--mut);
  }
  .small {
    font-size: 13px;
  }
  .dormant {
    color: var(--review);
    font-weight: 600;
  }
  .num {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .muted.num {
    font-weight: 400;
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
