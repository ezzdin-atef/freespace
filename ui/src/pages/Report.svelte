<script lang="ts">
  import { CheckCircle, Clipboard, Rescan } from "../lib/icons";
  import { session } from "../lib/session.svelte";
  import { fmtCount, fmtSeconds, fmtSize } from "../lib/format";
  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import Stats from "../components/Stats.svelte";

  const r = $derived(session.report);
</script>

{#if !r}
  <div class="empty">
    <span class="tile"><Clipboard size={26} /></span>
    <b>Nothing cleaned yet</b>
    <p>After you clean, the results appear here.</p>
  </div>
{:else}
  <PageHeader
    icon={Clipboard}
    title="Cleaning report"
    meta="{fmtSeconds(r.elapsed_ms)}{r.cancelled ? '  ·  stopped early' : ''}"
    desc={r.used_trash ? "Items were moved to the Recycle Bin - the space is freed when you empty it." : "Rescan to refresh the sizes."}
  >
    {#snippet actions()}
      <Button label="Rescan" icon={Rescan} primary disabled={session.busy} onclick={() => session.startScan()} />
    {/snippet}
  </PageHeader>

  <div class="body">
    <Stats
      items={[
        { label: r.used_trash ? "Moved to bin" : "Freed", value: fmtSize(r.freed), tone: "safe" },
        { label: "Files removed", value: fmtCount(r.files) },
        { label: "Couldn't delete", value: fmtCount(r.failed), tone: r.failed > 0 ? "review" : undefined },
        { label: "Protected, skipped", value: fmtCount(r.skipped.length), tone: r.skipped.length > 0 ? "danger" : undefined },
      ]}
    />

    <Card pad={18}>
      {#each r.maintenance as m, i (i)}<p class="maint">{m}</p>{/each}
      {#if r.skipped.length === 0 && r.failed === 0 && r.maintenance.length === 0}
        <p class="ok"><CheckCircle size={18} /> Everything selected was removed.</p>
      {/if}
      {#if r.skipped.length > 0}
        <h3 class="danger">Protected items that were skipped</h3>
        <ul>
          {#each r.skipped.slice(0, 200) as s, i (i)}<li>{s.path} - {s.reason}</li>{/each}
        </ul>
      {/if}
      {#if r.failed > 0}
        <h3 class="review">Files that could not be deleted</h3>
        <p class="muted">Usually open in a running app or needing administrator. Normal for Temp folders.</p>
        <ul>
          {#each r.failures as f, i (i)}<li>{f.path} - {f.reason}</li>{/each}
        </ul>
      {/if}
    </Card>
  </div>
{/if}

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: 16px;
    margin-top: 20px;
  }
  .maint {
    color: var(--fg2);
  }
  .ok {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 15px;
    font-weight: 600;
    color: var(--safe);
  }
  h3 {
    margin: 4px 0 6px;
    font-size: 15px;
    font-weight: 600;
  }
  h3 + ul,
  .muted + ul {
    margin-bottom: 12px;
  }
  .danger {
    color: var(--danger);
  }
  .review {
    color: var(--review);
  }
  .muted {
    margin-bottom: 6px;
    color: var(--mut);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    font: 13px var(--mono);
    color: var(--mut);
    user-select: text;
  }
  li {
    padding: 1px 0;
    overflow-wrap: anywhere;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 80px 20px;
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
