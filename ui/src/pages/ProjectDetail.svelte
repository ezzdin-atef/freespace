<script lang="ts">
  import { FolderOpen } from "../lib/icons";
  import type { ProjectRow } from "../lib/api";
  import { session } from "../lib/session.svelte";
  import { fmtAgo, fmtCount, fmtSize } from "../lib/format";
  import { SAFETY_TONE } from "../lib/safety";
  import Button from "../components/Button.svelte";
  import Callout from "../components/Callout.svelte";
  import Checkbox from "../components/Checkbox.svelte";
  import Pill from "../components/Pill.svelte";
  import UsageBar from "../components/UsageBar.svelte";

  // Right-hand panel of Projects: where the project's space goes, and each thing it can rebuild.
  let { project }: { project: ProjectRow } = $props();

  const safeBytes = $derived(project.artifacts.filter((a) => a.safety === "safe").reduce((n, a) => n + a.size, 0));
  const source = $derived(Math.max(project.size - project.cleanable, 0));
</script>

<div class="detail">
  <div>
    <h2>{project.name}</h2>
    <p class="path">{project.path}</p>
  </div>
  <div class="pills">
    {#each project.kinds as k (k)}<span class="kind">{k}</span>{/each}
    {#if project.dormant}<Pill tone="review" label="Dormant" />{:else}<Pill tone="safe" label="Active" />{/if}
  </div>

  <div class="split">
    <UsageBar frac={project.cleanable / Math.max(project.size, 1)} alarm={false} />
    <p class="muted">{fmtSize(project.cleanable)} cleanable of {fmtSize(project.size)}</p>
  </div>

  <dl>
    <dt>Your files</dt>
    <dd>{fmtSize(source)}</dd>
    <dt>Files in total</dt>
    <dd>{fmtCount(project.files)}</dd>
    <dt>Last active</dt>
    <dd>{fmtAgo(project.last_active)}</dd>
    <dt>Git</dt>
    <dd>{project.branch ?? "Not a repository"}</dd>
  </dl>

  <Button label="Open project folder" icon={FolderOpen} wide onclick={() => session.revealPath(project.path)} />

  {#if project.artifacts.length === 0}
    <p class="muted">Nothing to clean: this project has no dependencies or build output on disk.</p>
  {:else}
    {#if project.dormant && safeBytes > 0}
      <Callout tone="safe" title="Good candidate">
        <p>Not touched {fmtAgo(project.last_active)}. {fmtSize(safeBytes)} can be rebuilt whenever you come back to it.</p>
      </Callout>
    {:else if !project.dormant}
      <Callout tone="info" title="In active use">
        <p>Your next install or build here will take longer if you delete these.</p>
      </Callout>
    {/if}

    <h3>What can go</h3>
    <ul>
      {#each project.artifacts as a (a.id)}
        <li title="{a.path}&#10;{fmtCount(a.files)} files&#10;{a.note}">
          <Checkbox
            checked={session.selected.has(a.id)}
            disabled={a.safety === "protected"}
            label="Tick {a.rel}"
            onchange={() => session.toggle(a.id)}
          />
          <div class="body">
            <div class="top"><b class="rel">{a.rel}</b><b>{fmtSize(a.size)}</b></div>
            <p class="what">{a.label}</p>
            {#if a.safety !== "safe"}<Pill tone={SAFETY_TONE[a.safety]} label={a.note} wrap />{/if}
            <code>{a.restore}</code>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .detail {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  h2 {
    font-size: 17px;
    font-weight: 700;
    line-height: 1.3;
    overflow-wrap: anywhere;
  }
  h3 {
    margin-top: 6px;
    padding-top: 14px;
    border-top: 1px solid var(--line);
    font-size: 15px;
    font-weight: 600;
  }
  .muted {
    color: var(--mut);
  }
  .path {
    font: 13px var(--mono);
    color: var(--mut);
    overflow-wrap: anywhere;
    user-select: text;
  }
  .pills {
    display: flex;
    flex-wrap: wrap;
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
  .split {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
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
    min-width: 0;
    margin: 0;
    font-weight: 600;
    text-align: right;
    overflow-wrap: anywhere;
  }
  ul {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    gap: 10px;
    padding: 10px;
    border: 1px solid var(--line);
    border-radius: var(--r-ctl);
  }
  li:hover {
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
    flex: none;
    font-weight: 600;
  }
  .top .rel {
    flex: 0 1 auto;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .what {
    font-size: 13px;
    color: var(--mut);
  }
  code {
    font: 12px var(--mono);
    color: var(--fg2);
    overflow-wrap: anywhere;
    user-select: text;
  }
</style>
