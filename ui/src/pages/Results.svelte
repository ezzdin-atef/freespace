<script lang="ts">
  import { Broom } from "@lucide/svelte";
  import { session } from "../lib/session.svelte";
  import { categoryIcon } from "../lib/icons";
  import { fmtCompact, fmtCount, fmtSeconds, fmtSize } from "../lib/format";
  import Card from "../components/Card.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import Stats from "../components/Stats.svelte";

  // Scan overview: what was found, per category. Phase 4 replaces this with the findings table.
  let { category }: { category: string | null } = $props();

  const shown = $derived(category ? session.cleanGroups.filter((g) => g.category === category) : session.cleanGroups);
  const title = $derived(category ? (shown[0]?.label ?? "Nothing left here") : "All cleanable");
  const meta = $derived(
    session.scan
      ? `${fmtCompact(session.scan.files)} files in ${session.scan.root} scanned in ${fmtSeconds(session.scan.elapsed_ms)}`
      : "",
  );
</script>

<PageHeader icon={category ? categoryIcon(category) : Broom} {title} {meta} desc={category ? shown[0]?.description : ""} />

<div class="body">
  {#if !category}
    <Stats
      items={[
        { label: "Cleanable", value: fmtSize(session.cleanTotal), tone: "safe" },
        { label: "Groups", value: fmtCount(session.cleanGroups.length) },
        { label: "Large files", value: fmtCount(session.largeCount) },
      ]}
    />
  {/if}

  <Card>
    {#each shown as g (g.category)}
      {@const Icon = categoryIcon(g.category)}
      <button class="group" onclick={() => session.go({ page: "cleanable", category: g.category })}>
        <span class="tile"><Icon size={18} /></span>
        <span class="text">
          <b>{g.label}</b>
          <small>{g.description}</small>
        </span>
        <span class="size">
          <b>{fmtSize(g.total)}</b>
          <small>{fmtCount(g.count)} {g.count === 1 ? "item" : "items"}</small>
        </span>
      </button>
    {:else}
      <p class="empty">Nothing cleanable was found in this scan.</p>
    {/each}
  </Card>
  <p class="note">Item lists with selection and cleaning arrive in the next migration phase.</p>
</div>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: 16px;
    margin-top: 20px;
  }
  .group {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 20px;
    border: 0;
    background: transparent;
    text-align: left;
  }
  .group + .group {
    border-top: 1px solid var(--line);
  }
  .group:hover {
    background: var(--row-hover);
  }
  .tile {
    width: 36px;
    height: 36px;
    flex: none;
    display: grid;
    place-items: center;
    border-radius: var(--r-ctl);
    background: var(--acc-bg);
    color: var(--acc);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .size {
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
  }
  b {
    font-weight: 600;
  }
  small {
    font-size: 13px;
    color: var(--mut);
  }
  .empty,
  .note {
    color: var(--mut);
  }
  .empty {
    padding: 20px;
  }
  .note {
    font-size: 13px;
  }
</style>
