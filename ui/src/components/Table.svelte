<script lang="ts" module>
  export interface Column<S extends string = string> {
    key: string;
    title: string;
    /** CSS grid track, e.g. "200px" or "minmax(200px, 1fr)". */
    width: string;
    right?: boolean;
    /** Clicking the header sorts by this. */
    sort?: S;
  }

  export const ROW_H = 40;
</script>

<script lang="ts" generics="T, S extends string">
  import { ArrowDown } from "@lucide/svelte";
  import type { Snippet } from "svelte";
  import Checkbox from "./Checkbox.svelte";

  // Virtualized table: only the rows in view (plus a margin) are in the DOM, so 50k findings scroll smoothly.
  let {
    rows,
    columns,
    key,
    sort,
    onsort,
    checked,
    checkable,
    ontoggle,
    focused = null,
    onfocus,
    onopen,
    cell,
    empty,
  }: {
    rows: T[];
    columns: Column<S>[];
    key: (row: T) => string | number;
    sort?: S;
    onsort?: (s: S) => void;
    /** When given, a checkbox column comes first. */
    checked?: (row: T) => boolean;
    checkable?: (row: T) => boolean;
    ontoggle?: (row: T) => void;
    focused?: string | number | null;
    onfocus?: (row: T) => void;
    onopen?: (row: T) => void;
    cell: Snippet<[T, string]>;
    empty?: Snippet;
  } = $props();

  const OVERSCAN = 8;
  let body: HTMLDivElement;
  let scrollTop = $state(0);
  let height = $state(400);

  const template = $derived((checked ? "48px " : "") + columns.map((c) => c.width).join(" "));
  const first = $derived(Math.max(Math.floor(scrollTop / ROW_H) - OVERSCAN, 0));
  const last = $derived(Math.min(Math.ceil((scrollTop + height) / ROW_H) + OVERSCAN, rows.length));
  const slice = $derived(rows.slice(first, last));

  /** Scrolls just enough to show row `i`. */
  export function scrollToIndex(i: number) {
    if (!body) return;
    const top = i * ROW_H;
    if (top < body.scrollTop) body.scrollTop = top;
    else if (top + ROW_H > body.scrollTop + body.clientHeight) body.scrollTop = top + ROW_H - body.clientHeight;
  }

  export function scrollToTop() {
    if (body) body.scrollTop = 0;
  }
</script>

<div class="table" style:--cols={template}>
  <div class="head row">
    {#if checked}<span></span>{/if}
    {#each columns as c (c.key)}
      {#if c.sort && onsort}
        <button class="th" class:right={c.right} class:on={sort === c.sort} onclick={() => onsort(c.sort!)}>
          {c.title}
          {#if sort === c.sort}<ArrowDown size={12} strokeWidth={2.5} />{/if}
        </button>
      {:else}
        <span class="th" class:right={c.right}>{c.title}</span>
      {/if}
    {/each}
  </div>

  <div class="body" bind:this={body} bind:clientHeight={height} onscroll={() => (scrollTop = body.scrollTop)}>
    {#if rows.length === 0}
      {@render empty?.()}
    {:else}
      <div class="spacer" style:height="{rows.length * ROW_H}px">
        {#each slice as row, i (key(row))}
          {@const k = key(row)}
          <div
            class="row item"
            class:focus={focused === k}
            class:checked={checked?.(row)}
            style:transform="translateY({(first + i) * ROW_H}px)"
            role="row"
            tabindex="-1"
            aria-selected={focused === k}
            onclick={() => onfocus?.(row)}
            ondblclick={() => onopen?.(row)}
            onkeydown={() => {}}
          >
            {#if checked}
              <span class="td check">
                <Checkbox
                  checked={checked(row)}
                  disabled={checkable ? !checkable(row) : false}
                  label="Tick for cleaning"
                  onchange={() => ontoggle?.(row)}
                />
              </span>
            {/if}
            {#each columns as c (c.key)}
              <span class="td" class:right={c.right}>{@render cell(row, c.key)}</span>
            {/each}
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .table {
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .row {
    display: grid;
    grid-template-columns: var(--cols);
    align-items: center;
  }
  .head {
    height: 38px;
    flex: none;
    background: var(--row-hover);
    border-bottom: 1px solid var(--line);
  }
  .th {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    padding: 0 14px;
    border: 0;
    background: transparent;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--mut);
    text-align: left;
  }
  button.th:hover {
    color: var(--fg2);
  }
  .th.on {
    color: var(--fg);
  }
  .th.right {
    justify-content: flex-end;
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .spacer {
    position: relative;
  }
  .item {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 40px;
    border-bottom: 1px solid var(--pane);
    outline: none;
  }
  .item:hover {
    background: var(--row-hover);
  }
  .item.checked {
    background: #f5f7ff;
  }
  .item.focus {
    background: var(--acc-bg);
    box-shadow: inset 3px 0 0 var(--acc);
  }
  .td {
    min-width: 0;
    padding: 0 14px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .td.right {
    text-align: right;
  }
  .td.check {
    display: grid;
    place-items: center;
    padding: 0;
  }
</style>
