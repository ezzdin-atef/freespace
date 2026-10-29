<script lang="ts">
  import { Hammer } from "@lucide/svelte";
  import PageHeader from "../components/PageHeader.svelte";

  // Stand-in for pages the Tauri migration has not reached yet (see docs/tauri-migration.md).
  let { title, phase }: { title: string; phase: number } = $props();
</script>

<PageHeader
  icon={Hammer}
  {title}
  desc="This page is being moved to the new interface (migration phase {phase}). Until then, run the previous version with: cargo run --release --features egui"
/>
