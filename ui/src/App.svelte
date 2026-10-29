<script lang="ts">
  import {
    AppWindow,
    Broom,
    ClipboardList,
    File,
    House,
    LayoutGrid,
    Play,
    Shield,
    ShieldCheck,
    Sparkles,
    Square,
  } from "@lucide/svelte";
  import { session, type View } from "./lib/session.svelte";
  import { categoryIcon } from "./lib/icons";
  import { fmtCompact, fmtCount, fmtSeconds, fmtSize } from "./lib/format";
  import Button from "./components/Button.svelte";
  import DrivePicker from "./components/DrivePicker.svelte";
  import NavItem from "./components/NavItem.svelte";
  import NavSection from "./components/NavSection.svelte";
  import Toast from "./components/Toast.svelte";
  import Home from "./pages/Home.svelte";
  import Pending from "./pages/Pending.svelte";
  import Findings from "./pages/Findings.svelte";
  import Scanning from "./pages/Scanning.svelte";

  session.start();

  const v = $derived(session.view);
  const is = (page: View["page"]) => v.page === page;
  const isCat = (c: string | null) => v.page === "cleanable" && v.category === c;
  // List pages lay out their own scrolling table and detail pane.
  const fill = $derived(!session.scanning && (v.page === "cleanable" || v.page === "large"));

  const scanStats = $derived.by(() => {
    const s = session.scan;
    if (!s) return null;
    let tip = `${fmtCount(s.files)} files in ${fmtCount(s.dirs)} folders`;
    if (s.errors > 0) tip += `\n${fmtCount(s.errors)} folders could not be read (run as administrator to see them)`;
    return { text: `${fmtCompact(s.files)} files scanned in ${fmtSeconds(s.elapsed_ms)}`, tip };
  });
</script>

<header>
  <div class="logo"><Broom size={16} strokeWidth={2.25} /></div>
  <div class="brand">LogCleaner</div>
  <div class="sep"></div>
  <DrivePicker />
  {#if session.scanning}
    <Button label="Stop scan" icon={Square} onclick={() => session.stopScan()} />
  {:else}
    <Button label="Scan" icon={Play} primary title="Scan the selected drive or folder" onclick={() => session.startScan()} />
  {/if}
  <div class="grow"></div>
  {#if scanStats}
    <span class="stats" title={scanStats.tip}>{scanStats.text}</span>
  {/if}
  {#if session.isAdmin}
    <span class="admin"><ShieldCheck size={16} /> Administrator</span>
  {:else}
    <Button
      label="Run as admin"
      icon={Shield}
      title="Needed to clean Windows temp, update cache, Windows.old and other users' files, and to uninstall most apps."
      onclick={() => session.relaunchAdmin()}
    />
  {/if}
</header>

<div class="body">
  <nav>
    <NavItem icon={House} label="Home" active={is("home")} onclick={() => session.go({ page: "home" })} />
    <!-- Scan-dependent pages stay hidden until there is a scan, rather than showing as greyed-out rows. -->
    {#if session.scan}
      <NavSection label="Clean up" />
      <NavItem
        icon={Broom}
        label="All cleanable"
        count={fmtSize(session.cleanTotal)}
        active={isCat(null)}
        onclick={() => session.go({ page: "cleanable", category: null })}
      />
      {#each session.cleanGroups as g (g.category)}
        <NavItem
          icon={categoryIcon(g.category)}
          label={g.label}
          count={fmtSize(g.total)}
          active={isCat(g.category)}
          onclick={() => session.go({ page: "cleanable", category: g.category })}
        />
      {/each}
      <NavSection label="Explore" />
      <NavItem icon={LayoutGrid} label="Space usage" active={is("usage")} onclick={() => session.go({ page: "usage" })} />
      <NavItem
        icon={File}
        label="Large files"
        count={String(session.largeCount)}
        active={is("large")}
        onclick={() => session.go({ page: "large" })}
      />
      <NavItem icon={Sparkles} label="Full clean" active={is("full")} onclick={() => session.go({ page: "full" })} />
    {/if}
    {#if session.hasReport}
      {#if !session.scan}<NavSection label="Explore" />{/if}
      <NavItem icon={ClipboardList} label="Last report" active={is("report")} onclick={() => session.go({ page: "report" })} />
    {/if}
    <NavSection label="Programs" />
    <NavItem
      icon={AppWindow}
      label="Installed apps"
      count={session.appCount === null ? "..." : String(session.appCount)}
      active={is("apps")}
      onclick={() => session.go({ page: "apps" })}
    />
  </nav>

  <main class:fill>
    {#if session.scanning && !is("apps")}
      <Scanning p={session.scanning} />
    {:else if v.page === "home"}
      <Home />
    {:else if v.page === "cleanable"}
      <Findings category={v.category} />
    {:else if v.page === "large"}
      <Findings category={null} large />
    {:else if v.page === "full"}
      <Pending title="Full clean" phase={5} />
    {:else if v.page === "report"}
      <Pending title="Last report" phase={5} />
    {:else if v.page === "usage"}
      <Pending title="Space usage" phase={6} />
    {:else}
      <Pending title="Installed apps" phase={6} />
    {/if}
  </main>
</div>

<Toast />

<style>
  :global(#app) {
    display: grid;
    grid-template-rows: 56px 1fr;
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 20px;
    background: var(--surface);
    border-bottom: 1px solid var(--line);
  }
  .logo {
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    border-radius: 7px;
    background: var(--acc);
    color: #fff;
  }
  .brand {
    font-size: 17px;
    font-weight: 700;
  }
  .sep {
    width: 1px;
    height: 22px;
    margin: 0 4px;
    background: var(--line);
  }
  .grow {
    flex: 1;
  }
  .stats {
    font-size: 13px;
    color: var(--mut);
  }
  .admin {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
    color: var(--safe);
  }
  .body {
    display: grid;
    grid-template-columns: 248px 1fr;
    min-height: 0;
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px;
    overflow-y: auto;
    background: var(--surface);
    border-right: 1px solid var(--line);
  }
  main {
    min-width: 0;
    overflow: auto;
    padding: 32px 40px;
  }
  main.fill {
    overflow: hidden;
    padding: 0;
  }
</style>
