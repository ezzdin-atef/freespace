<script lang="ts">
  import { Folder, HardDrive } from "../lib/icons";
  import { session } from "../lib/session.svelte";
  import { driveLabel, fmtSize } from "../lib/format";
  import Button from "./Button.svelte";
  import Field from "./Field.svelte";
  import Select from "./Select.svelte";
  import UsageBar from "./UsageBar.svelte";

  // Top-bar target picker: pick a drive to scan it right away, or type a folder.
  let open = $state(false);
  let folder = $state("");
  let root: HTMLDivElement;

  const target = $derived.by(() => {
    const d = session.driveFor(session.root);
    return d
      ? { icon: HardDrive, label: `${driveLabel(d.root)}  ${d.kind} disk` }
      : { icon: Folder, label: session.root };
  });

  function toggle() {
    open = !open;
    if (open) folder = session.driveFor(session.root) ? "" : session.root;
  }

  function scan(path: string) {
    if (!path.trim()) return;
    open = false;
    session.startScan(path.trim());
  }

  function onWindowPointer(e: PointerEvent) {
    if (open && !root.contains(e.target as Node)) open = false;
  }
</script>

<svelte:window onpointerdown={onWindowPointer} onkeydown={(e) => open && e.key === "Escape" && (open = false)} />

<div class="picker" bind:this={root}>
  <Select
    icon={target.icon}
    value={target.label}
    disabled={session.busy}
    title="Choose the drive or folder to scan"
    onclick={toggle}
  />
  {#if open}
    <div class="popup" role="dialog" aria-label="Choose what to scan">
      <div class="heading">Drives</div>
      {#each session.drives as d (d.root)}
        {@const used = Math.max(d.total - d.free, 0)}
        {@const on = d.root.toLowerCase() === session.root.trim().toLowerCase()}
        <button class="drive" class:on onclick={() => scan(d.root)}>
          <HardDrive size={16} />
          <span class="name">{driveLabel(d.root)}&nbsp; {d.kind} disk</span>
          <span class="free">{fmtSize(d.free)} free of {fmtSize(d.total)}</span>
          <span class="bar"><UsageBar frac={used / Math.max(d.total, 1)} /></span>
        </button>
      {/each}
      <div class="heading">Or a folder</div>
      <div class="row">
        <Field bind:value={folder} icon={Folder} placeholder="Paste a folder path, e.g. D:\projects" onenter={() => scan(folder)} />
        <Button label="Scan" primary width={84} onclick={() => scan(folder)} />
      </div>
    </div>
  {/if}
</div>

<style>
  .picker {
    position: relative;
  }
  .popup {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 20;
    width: 380px;
    padding: 6px;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--r-card);
    box-shadow: 0 8px 24px rgb(15 23 42 / 0.12);
  }
  .heading {
    padding: 8px 10px 4px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--mut2);
  }
  .drive {
    width: 100%;
    display: grid;
    grid-template-columns: 16px 1fr auto;
    grid-template-rows: auto auto;
    column-gap: 10px;
    row-gap: 8px;
    align-items: center;
    padding: 10px;
    border: 0;
    border-radius: var(--r-ctl);
    background: transparent;
    text-align: left;
  }
  .drive:hover {
    background: var(--pane);
  }
  .drive.on {
    background: var(--acc-bg);
  }
  .drive :global(svg) {
    color: var(--mut);
  }
  .drive.on :global(svg),
  .drive.on .name {
    color: var(--acc);
  }
  .name {
    font-weight: 600;
  }
  .free {
    font-size: 13px;
    color: var(--mut);
  }
  .bar {
    grid-column: 2 / 4;
  }
  .row {
    display: flex;
    gap: 8px;
    padding: 4px 4px 6px;
  }
</style>
