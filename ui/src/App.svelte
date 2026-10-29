<script lang="ts">
  import { AppWindow, Broom, Folder, HardDrive, House, Play, Shield, ShieldCheck } from "@lucide/svelte";
  import { init, type Drive } from "./lib/api";
  import Button from "./components/Button.svelte";
  import NavItem from "./components/NavItem.svelte";
  import NavSection from "./components/NavSection.svelte";
  import Select from "./components/Select.svelte";
  import Home from "./pages/Home.svelte";

  type View = "home" | "apps";

  let drives = $state<Drive[]>([]);
  let isAdmin = $state(false);
  let root = $state("C:\\");
  let view = $state<View>("home");
  let error = $state("");

  init()
    .then((r) => {
      drives = r.drives;
      isAdmin = r.is_admin;
      root = (r.drives.find((d) => d.root.toUpperCase() === "C:\\") ?? r.drives[0])?.root ?? "C:\\";
    })
    .catch((e) => (error = String(e)));

  const target = $derived.by(() => {
    const d = drives.find((d) => d.root.toLowerCase() === root.trim().toLowerCase());
    return d ? { icon: HardDrive, label: `${d.root.replace(/\\$/, "")}  ${d.kind} disk` } : { icon: Folder, label: root };
  });

  // Scanning is wired in phase 3; until then the buttons only pick the target.
  function scan(path: string) {
    root = path;
  }
</script>

<header>
  <div class="logo"><Broom size={16} strokeWidth={2.25} /></div>
  <div class="brand">LogCleaner</div>
  <div class="sep"></div>
  <Select icon={target.icon} value={target.label} title="Choose the drive or folder to scan" />
  <Button label="Scan" icon={Play} primary title="Scan the selected drive or folder" onclick={() => scan(root)} />
  <div class="grow"></div>
  {#if isAdmin}
    <span class="admin"><ShieldCheck size={16} /> Administrator</span>
  {:else}
    <Button
      label="Run as admin"
      icon={Shield}
      title="Needed to clean Windows temp, update cache, Windows.old and other users' files, and to uninstall most apps."
    />
  {/if}
</header>

<div class="body">
  <nav>
    <NavItem icon={House} label="Home" active={view === "home"} onclick={() => (view = "home")} />
    <NavSection label="Programs" />
    <NavItem icon={AppWindow} label="Installed apps" active={view === "apps"} onclick={() => (view = "apps")} />
  </nav>

  <main>
    {#if error}
      <p class="error">Could not load drives: {error}</p>
    {:else if view === "home"}
      <Home {drives} bind:folder={root} onscan={scan} />
    {:else}
      <p class="todo">Installed apps arrive in phase 6.</p>
    {/if}
  </main>
</div>

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
    background: var(--surface);
    border-right: 1px solid var(--line);
  }
  main {
    overflow: auto;
    padding: 40px;
  }
  .error {
    color: var(--danger);
  }
  .todo {
    color: var(--mut);
  }
</style>
