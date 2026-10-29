<script lang="ts">
  import { X } from "../lib/icons";
  import type { IconComponent } from "../lib/types";

  let {
    value = $bindable(""),
    placeholder = "",
    icon: Icon,
    width,
    clearable = false,
    title,
    onenter,
  }: {
    value?: string;
    placeholder?: string;
    icon?: IconComponent;
    width?: number;
    /** Shows an x button while there is text; Esc also clears. */
    clearable?: boolean;
    title?: string;
    onenter?: () => void;
  } = $props();

  let input: HTMLInputElement;

  export function focus() {
    input.focus();
    input.select();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Enter") onenter?.();
    if (e.key === "Escape" && clearable) {
      value = "";
      input.blur();
    }
  }
</script>

<label class="field" style:flex={width ? `0 1 ${width}px` : undefined} {title}>
  {#if Icon}<Icon size={16} />{/if}
  <input bind:this={input} bind:value {placeholder} spellcheck="false" autocomplete="off" {onkeydown} />
  {#if clearable && value}
    <button class="clear" aria-label="Clear" onclick={() => (value = "")}><X size={14} /></button>
  {/if}
</label>

<style>
  .field {
    flex: 1;
    min-width: 0;
    height: var(--ctl-h);
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--r-ctl);
    cursor: text;
    transition:
      border-color 0.12s,
      box-shadow 0.12s;
  }
  .field:hover {
    border-color: var(--line-strong);
  }
  .field:focus-within {
    border-color: var(--acc);
    box-shadow: var(--ring);
  }
  .field > :global(svg) {
    color: var(--mut2);
    flex: none;
  }
  input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: 0;
    background: transparent;
    user-select: text;
  }
  input::placeholder {
    color: var(--mut2);
  }
  input:focus-visible {
    box-shadow: none;
  }
  .clear {
    width: 22px;
    height: 22px;
    flex: none;
    display: grid;
    place-items: center;
    padding: 0;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--mut);
  }
  .clear:hover {
    background: var(--pane);
    color: var(--fg);
  }
</style>
