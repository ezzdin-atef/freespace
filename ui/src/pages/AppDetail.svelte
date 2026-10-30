<script lang="ts">
  import { FolderOpen, ListRemove, Rescan, Trash } from "../lib/icons";
  import type { AppRow } from "../lib/api";
  import { session } from "../lib/session.svelte";
  import { fmtCount, fmtSize } from "../lib/format";
  import { SAFETY_TONE } from "../lib/safety";
  import AppLogo from "../components/AppLogo.svelte";
  import Button from "../components/Button.svelte";
  import Callout from "../components/Callout.svelte";
  import Checkbox from "../components/Checkbox.svelte";
  import Pill from "../components/Pill.svelte";

  // Right-hand panel of Installed apps: details, uninstall, and the folders the app leaves behind.
  let { app }: { app: AppRow } = $props();

  const left = $derived(session.leftovers[app.id]);
  const ticks = $derived(session.leftoverTicks[app.id] ?? []);
  const ticked = $derived((left?.items ?? []).filter((l) => ticks.includes(l.id)));
  const tickedBytes = $derived(ticked.reduce((n, l) => n + l.size, 0));
</script>

<div class="detail">
  <div class="head">
    <AppLogo {app} size={44} />
    <div>
      <h2>{app.name}</h2>
      <p class="muted">{app.publisher || "Unknown publisher"}</p>
    </div>
  </div>
  <div class="pills">
    <span class="kind" class:store={app.store}>{app.store ? "Store app" : "Desktop app"}</span>
    {#if app.removed}<Pill tone="safe" label="Uninstalled" />{/if}
  </div>

  <dl>
    <dt>Version</dt>
    <dd>{app.version || "-"}</dd>
    <dt>Size</dt>
    <dd>{app.size > 0 ? fmtSize(app.size) : "Unknown"}</dd>
    {#if app.installed}
      <dt>Installed</dt>
      <dd>{app.installed}</dd>
    {/if}
  </dl>
  {#if app.location}<p class="path">{app.location}</p>{/if}

  <div class="actions">
    {#if app.has_uninstaller}
      <Button
        label="Uninstall"
        icon={Trash}
        variant="danger"
        wide
        disabled={app.removed || session.busy}
        onclick={() => session.askUninstall(app.id)}
      />
    {:else if !app.store}
      <Button
        label="Remove from installed list"
        icon={ListRemove}
        wide
        disabled={app.removed}
        title="Its uninstaller is missing. Removes the app from Windows' installed list."
        onclick={() => session.askRemoveEntry(app.id)}
      />
    {/if}
    {#if app.location}
      <Button label="Open install folder" icon={FolderOpen} wide onclick={() => session.revealPath(app.location!)} />
    {/if}
  </div>

  <div class="section">
    <h3>Files and folders</h3>
    <Button label="Look again" icon={Rescan} variant="ghost" onclick={() => session.scanLeftovers(app.id)} />
  </div>

  {#if !left || left.loading}
    <p class="loading"><span class="spin"></span> Looking for the app's folders...</p>
  {:else if left.items.length === 0}
    <p class="muted">{app.removed ? "Nothing left behind." : "No folders found with this app's name."}</p>
  {:else}
    {#if !app.removed && app.has_uninstaller}
      <Callout tone="review" title="Uninstall first"><p>Deleting these while the app is still installed will break it.</p></Callout>
    {/if}
    <ul class="leftovers">
      {#each left.items as l (l.id)}
        <li title="{l.path}&#10;{fmtCount(l.files)} files&#10;{l.note}">
          <Checkbox
            checked={ticks.includes(l.id)}
            disabled={l.safety === "protected"}
            label="Tick {l.what}"
            onchange={() => session.toggleLeftover(app.id, l.id)}
          />
          <div class="body">
            <div class="top"><b>{l.what}</b><b>{fmtSize(l.size)}</b></div>
            <p class="lpath">{l.path}</p>
            {#if l.safety !== "safe"}<Pill tone={SAFETY_TONE[l.safety]} label={l.note} />{/if}
          </div>
        </li>
      {/each}
    </ul>
    <Button
      label="Delete {ticked.length} selected ({fmtSize(tickedBytes)})"
      icon={Trash}
      variant="danger"
      wide
      disabled={ticked.length === 0 || session.busy}
      onclick={() => session.requestDeleteLeftovers(app.id)}
    />
  {/if}
</div>

<style>
  .detail {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .kind.store {
    background: var(--acc-bg);
    color: var(--acc);
  }
  h2 {
    font-size: 17px;
    font-weight: 700;
    line-height: 1.3;
    overflow-wrap: anywhere;
  }
  .muted {
    color: var(--mut);
  }
  .pills {
    display: flex;
    gap: 6px;
  }
  .kind {
    padding: 1px 9px;
    border-radius: var(--r-sm);
    background: var(--pane);
    color: var(--fg2);
    font-size: 13px;
    font-weight: 600;
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
  .path,
  .lpath {
    font: 13px var(--mono);
    color: var(--mut);
    overflow-wrap: anywhere;
    user-select: text;
  }
  .actions {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .section {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 6px;
    padding-top: 14px;
    border-top: 1px solid var(--line);
  }
  h3 {
    font-size: 15px;
    font-weight: 600;
  }
  .leftovers {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .leftovers li {
    display: flex;
    gap: 10px;
    padding: 10px;
    border: 1px solid var(--line);
    border-radius: var(--r-ctl);
  }
  .leftovers li:hover {
    background: var(--row-hover);
  }
  .body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    align-items: flex-start;
  }
  .top {
    width: 100%;
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }
  .top b {
    font-weight: 600;
  }
  .lpath {
    font-size: 12px;
  }
  .loading {
    display: flex;
    align-items: center;
    gap: 10px;
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
</style>
