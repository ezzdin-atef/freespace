<script lang="ts">
  import { Folder, HardDrive, Play } from "../lib/icons";
  import { session } from "../lib/session.svelte";
  import { driveLabel, fmtSize } from "../lib/format";
  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import Field from "../components/Field.svelte";
  import Kbd from "../components/Kbd.svelte";
  import Pill from "../components/Pill.svelte";
  import UsageBar from "../components/UsageBar.svelte";

  let folder = $state("");

  // The system drive gets the primary button, so the page has one obvious action.
  const system = $derived(session.systemDrive());
  const scan = (root: string) => root.trim() && session.startScan(root.trim());

  const shortcuts = [
    ["Up / Down", "Move through a list"],
    ["Space", "Tick the highlighted row"],
    ["Del", "Clean what is ticked"],
    ["Enter / Backspace", "Open a folder / go back up"],
    ["Ctrl F", "Search"],
    ["Esc", "Clear search, close a dialog"],
  ];
</script>

<div class="col">
  <h1>Free up space on your PC</h1>
  <p class="lead">
    Pick a drive to scan, review what Freespace finds, then clean it. Nothing is deleted until you tick it and confirm.
  </p>

  <div class="drives">
    <Card>
      {#each session.drives as d (d.root)}
        {@const used = Math.max(d.total - d.free, 0)}
        {@const frac = used / Math.max(d.total, 1)}
        <div class="drive">
          <div class="tile"><HardDrive size={18} /></div>
          <div class="name">
            <b>{d.kind} disk ({driveLabel(d.root)})</b>
            <small>{fmtSize(d.free)} free of {fmtSize(d.total)}</small>
          </div>
          <div class="usage">
            <div class="usage-text">
              <small>{Math.round(frac * 100)}% used</small>
              <small>{fmtSize(used)}</small>
            </div>
            <UsageBar {frac} />
          </div>
          <Button
            label="Scan"
            icon={Play}
            primary={d.root === system?.root}
            disabled={session.busy}
            width={92}
            onclick={() => scan(d.root)}
          />
        </div>
      {/each}
      <div class="folder">
        <b>Scan one folder</b>
        <p>Faster when you only want to clean a projects or downloads folder.</p>
        <div class="row">
          <Field
            bind:value={folder}
            icon={Folder}
            placeholder="Paste a folder path, e.g. D:\projects"
            onenter={() => scan(folder)}
          />
          <Button label="Scan" primary disabled={session.busy} width={84} onclick={() => scan(folder)} />
        </div>
      </div>
    </Card>
  </div>

  <h2>Good to know</h2>
  <div class="grid">
    <Card pad={20}>
      <h3>Safety labels</h3>
      <div class="kv">
        <Pill tone="safe" label="Safe" />
        <span>Rebuilt or downloaded again automatically. Fine to delete.</span>
        <Pill tone="review" label="Review" />
        <span>Probably fine, but check it is not something you need.</span>
        <Pill tone="danger" label="Do not delete" />
        <span>Windows, installed programs, your profile folders, .git. Never deleted.</span>
      </div>
    </Card>
    <Card pad={20}>
      <h3>Keyboard shortcuts</h3>
      <div class="kv keys">
        {#each shortcuts as [keys, what] (keys)}
          <Kbd {keys} />
          <span>{what}</span>
        {/each}
      </div>
    </Card>
  </div>
</div>

<style>
  .col {
    max-width: 1040px;
    margin: 0 auto;
  }
  h1 {
    font-size: 26px;
    font-weight: 700;
    letter-spacing: -0.01em;
    line-height: 1.25;
  }
  .lead {
    margin-top: 6px;
    font-size: 15px;
    color: var(--mut);
  }
  .drives {
    margin-top: 24px;
  }
  .drive {
    height: 72px;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 0 20px 0 22px;
  }
  .drive + .drive {
    border-top: 1px solid var(--line);
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
  .name {
    width: 180px;
    flex: none;
  }
  .name b {
    display: block;
    font-size: 15px;
    font-weight: 600;
  }
  small {
    font-size: 13px;
    color: var(--mut);
  }
  .usage {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-right: 24px;
  }
  .usage-text {
    display: flex;
    justify-content: space-between;
  }
  .folder {
    padding: 16px 20px 18px 22px;
    background: var(--row-hover);
    border-top: 1px solid var(--line);
  }
  .folder b {
    font-size: 15px;
    font-weight: 600;
  }
  .folder p {
    margin-top: 2px;
    font-size: 13px;
    color: var(--mut);
  }
  .row {
    display: flex;
    gap: 8px;
    margin-top: 12px;
  }
  h2 {
    margin: 32px 0 12px;
    font-size: 16px;
    font-weight: 600;
  }
  h3 {
    margin-bottom: 10px;
    font-size: 15px;
    font-weight: 600;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: 16px;
    padding-bottom: 12px;
  }
  .kv {
    display: grid;
    grid-template-columns: 110px 1fr;
    gap: 10px;
    color: var(--fg2);
  }
  .kv.keys {
    grid-template-columns: 140px 1fr;
    gap: 6px 10px;
    align-items: center;
  }
</style>
