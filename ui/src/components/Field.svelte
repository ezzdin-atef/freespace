<script lang="ts">
  import type { IconComponent } from "../lib/types";

  let {
    value = $bindable(""),
    placeholder = "",
    icon: Icon,
    onenter,
  }: {
    value?: string;
    placeholder?: string;
    icon?: IconComponent;
    onenter?: () => void;
  } = $props();
</script>

<label class="field">
  {#if Icon}<Icon size={16} />{/if}
  <input
    bind:value
    {placeholder}
    spellcheck="false"
    autocomplete="off"
    onkeydown={(e) => e.key === "Enter" && onenter?.()}
  />
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
  .field :global(svg) {
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
</style>
