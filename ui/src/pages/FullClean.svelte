<script lang="ts">
  import { Broom, Sparkle, Undo } from "../lib/icons";
  import { session } from "../lib/session.svelte";
  import { categoryIcon } from "../lib/icons";
  import { fmtCount, fmtSize } from "../lib/format";
  import Button from "../components/Button.svelte";
  import Callout from "../components/Callout.svelte";
  import Card from "../components/Card.svelte";
  import Checkbox from "../components/Checkbox.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import Stats from "../components/Stats.svelte";

  // What Full clean picks is decided in Rust (`full_clean_ids`); this page sums it up per category.
  const picks = $derived.by(() => {
    const per = new Map<string, { count: number; bytes: number }>();
    for (const id of session.fullIds) {
      const f = session.findingById.get(id);
      if (!f) continue;
      const e = per.get(f.category) ?? { count: 0, bytes: 0 };
      e.count++;
      e.bytes += f.size;
      per.set(f.category, e);
    }
    return [...per.entries()].map(([category, e]) => ({ category, ...e })).sort((a, b) => b.bytes - a.bytes);
  });
  const total = $derived(picks.reduce((n, p) => n + p.bytes, 0));
  const items = $derived(picks.reduce((n, p) => n + p.count, 0));
  const hiberfil = $derived(session.findings.find((f) => f.name.toLowerCase() === "hiberfil.sys")?.size ?? null);
  const can = $derived(!session.busy && (total > 0 || session.dism || session.hibernateOff));
</script>

<div class="page">
  <section class="content">
    <PageHeader
      icon={Sparkle}
      title="Full clean"
      desc="Everything that is safe to remove, plus Windows leftovers only needed for rollback or troubleshooting. Your documents, photos, downloads, installed programs, large files, Python venvs and JS dist folders are not touched."
    />
    <Stats
      items={[
        { label: "Will be freed", value: fmtSize(total), tone: "safe" },
        { label: "Categories", value: fmtCount(picks.length) },
        { label: "Items", value: fmtCount(items) },
      ]}
    />
    <Card>
      <div class="head row"><span>Category</span><span class="r">Items</span><span class="r">Size</span></div>
      {#each picks as p (p.category)}
        {@const Icon = categoryIcon(p.category)}
        <div class="row">
          <span class="cat"><Icon size={16} />{session.groupByCategory.get(p.category)?.label ?? p.category}</span>
          <span class="r muted">{fmtCount(p.count)}</span>
          <b class="r">{fmtSize(p.bytes)}</b>
        </div>
      {:else}
        <p class="none">Nothing to clean in this scan.</p>
      {/each}
    </Card>
  </section>

  <aside>
    <h3>Extra Windows maintenance</h3>
    {#if !session.isAdmin}
      <Callout tone="review" title="Needs administrator">
        <button class="link" onclick={() => session.relaunchAdmin()}>Restart Freespace as administrator</button>
      </Callout>
    {/if}
    <div class="option" class:off={!session.isAdmin}>
      <Checkbox checked={session.dism} disabled={!session.isAdmin} label="Clean up the component store" onchange={() => (session.dism = !session.dism)} />
      <div>
        <b>Clean up the component store</b>
        <p>DISM /StartComponentCleanup removes superseded update files from WinSxS. Takes 5-15 minutes.</p>
      </div>
    </div>
    <div class="option" class:off={!session.isAdmin}>
      <Checkbox
        checked={session.hibernateOff}
        disabled={!session.isAdmin}
        label="Turn off hibernation"
        onchange={() => (session.hibernateOff = !session.hibernateOff)}
      />
      <div>
        <b>Turn off hibernation{hiberfil !== null ? ` (frees ${fmtSize(hiberfil)})` : ""}</b>
        <p>Deletes hiberfil.sys and disables Fast Startup. Undo with powercfg /hibernate on.</p>
      </div>
    </div>
    <div class="actions">
      <Button label="Run full clean ({fmtSize(total)})" icon={Broom} variant="danger" wide disabled={!can} onclick={() => session.fullClean(true)} />
      <Button label="Just select these items" wide onclick={() => session.fullClean(false)} />
    </div>
    <Callout tone="info" title="Want a truly fresh Windows?">
      <p>
        Cleaning can't remove installed programs or registry clutter. Windows' own Reset this PC reinstalls Windows and
        removes all apps (you can keep your files). Back up first.
      </p>
      <div class="reset"><Button label="Open Reset this PC" icon={Undo} onclick={() => session.resetPc()} /></div>
    </Callout>
  </aside>
</div>

<style>
  .page {
    height: 100%;
    display: grid;
    grid-template-columns: minmax(0, 1fr) clamp(300px, 25vw, 380px);
  }
  .content {
    min-width: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 24px 28px 20px;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 110px 120px;
    align-items: center;
    height: 40px;
    padding: 0 14px;
    border-bottom: 1px solid var(--pane);
  }
  .row:last-child {
    border-bottom: 0;
  }
  .head {
    height: 38px;
    background: var(--row-hover);
    border-bottom: 1px solid var(--line);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--mut);
  }
  .r {
    text-align: right;
  }
  .cat {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .cat :global(svg) {
    color: var(--mut);
  }
  .muted {
    color: var(--mut);
  }
  b {
    font-weight: 600;
  }
  .none {
    padding: 20px;
    color: var(--mut);
  }
  aside {
    overflow-y: auto;
    overflow-x: hidden;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 20px;
    background: var(--surface);
    border-left: 1px solid var(--line);
  }
  h3 {
    font-size: 16px;
    font-weight: 700;
  }
  .option {
    display: flex;
    gap: 12px;
  }
  .option p {
    font-size: 13px;
    color: var(--mut);
  }
  .option.off b {
    color: var(--mut);
  }
  .actions {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 4px 0 8px;
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--acc);
    text-decoration: underline;
    text-align: left;
  }
  .reset {
    margin-top: 6px;
  }
</style>
