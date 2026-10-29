<script lang="ts">
  import { Search, Stop } from "../lib/icons";
  import type { ScanProgress } from "../lib/api";
  import { session } from "../lib/session.svelte";
  import { fmtCompact, fmtCount, fmtSeconds, fmtSize } from "../lib/format";
  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import ProgressBar from "../components/ProgressBar.svelte";
  import Stats from "../components/Stats.svelte";

  let { p }: { p: ScanProgress } = $props();

  let stopping = $state(false);
  // Bytes read vs the drive's used space; capped below 100% because the estimate is rough.
  const frac = $derived(p.expected ? Math.min(p.bytes / Math.max(p.expected, 1), 0.999) : null);
  const perSec = $derived(p.files / Math.max(p.elapsed_ms / 1000, 0.001));

  function stop() {
    stopping = true;
    session.stopScan();
  }
</script>

<PageHeader
  icon={Search}
  title="Scanning {p.root}"
  desc="Reading every folder in parallel. You can keep using your PC; results appear when the scan finishes."
>
  {#snippet actions()}
    <Button label={stopping ? "Stopping..." : "Stop scan"} icon={Stop} disabled={stopping} onclick={stop} />
  {/snippet}
</PageHeader>

<div class="body">
  <Card pad={22}>
    <div class="headline">
      <b>{frac === null ? "Working..." : `${Math.round(frac * 100)}%`}</b>
      <span>{fmtSize(p.bytes)} read</span>
    </div>
    <ProgressBar {frac} />
    <p class="current" title={p.current}>{p.current || " "}</p>
  </Card>

  <Stats
    items={[
      { label: "Files", value: fmtCount(p.files) },
      { label: "Folders", value: fmtCount(p.dirs) },
      { label: "Speed", value: `${fmtCompact(Math.round(perSec))} files/s` },
      { label: "Elapsed", value: fmtSeconds(p.elapsed_ms) },
    ]}
  />
  {#if p.errors > 0}
    <p class="note">
      {fmtCount(p.errors)} folders could not be read{session.isAdmin ? "." : " - run as administrator to include them."}
    </p>
  {/if}
</div>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: 16px;
    margin-top: 20px;
  }
  .headline {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-bottom: 12px;
  }
  .headline b {
    font-size: 26px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .headline span {
    font-size: 15px;
    color: var(--mut);
  }
  .current {
    margin-top: 10px;
    font: 13px var(--mono);
    color: var(--mut);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .note {
    font-size: 13px;
    color: var(--mut);
  }
</style>
