// Keyboard handling shared by the list pages (findings, space usage, apps).

export interface ListKeys<K> {
  /** Row keys in display order. */
  keys: K[];
  focused: K | null;
  setFocus: (k: K, index: number) => void;
  focusSearch?: () => void;
  clearFilter?: () => void;
  /** Space. */
  toggle?: (k: K) => void;
  /** Enter. */
  open?: (k: K) => void;
  /** Backspace. */
  back?: () => void;
  /** Del. */
  clean?: () => void;
}

/** Arrow keys move the focused row, Space ticks, Enter opens, Backspace goes up, Del cleans,
 *  Ctrl+F searches, Esc clears the filter. Ignored while typing or while a dialog is open. */
export function handleListKeys<K>(e: KeyboardEvent, o: ListKeys<K>) {
  if (e.ctrlKey && e.key.toLowerCase() === "f" && o.focusSearch) {
    e.preventDefault();
    o.focusSearch();
    return;
  }
  const el = document.activeElement as HTMLElement | null;
  const typing = el?.matches("input, textarea, select, [contenteditable]") ?? false;
  if (typing || document.querySelector("[role=dialog][aria-modal=true]")) return;
  if (e.key === "Escape") {
    o.clearFilter?.();
    return;
  }
  // A button focused from the keyboard keeps Space/Enter; one focused by a mouse click does not.
  if (el?.matches("button") && el.matches(":focus-visible")) return;

  const step = e.key === "ArrowDown" ? 1 : e.key === "ArrowUp" ? -1 : 0;
  if (step && o.keys.length > 0) {
    e.preventDefault();
    el?.blur();
    const pos = o.focused === null ? -1 : o.keys.indexOf(o.focused);
    const next = pos < 0 ? 0 : Math.min(Math.max(pos + step, 0), o.keys.length - 1);
    o.setFocus(o.keys[next], next);
  } else if (e.key === " " && o.focused !== null && o.toggle) {
    e.preventDefault();
    el?.blur();
    o.toggle(o.focused);
  } else if (e.key === "Enter" && o.focused !== null && o.open) {
    e.preventDefault();
    o.open(o.focused);
  } else if (e.key === "Backspace" && o.back) {
    e.preventDefault();
    o.back();
  } else if (e.key === "Delete" && o.clean) {
    e.preventDefault();
    o.clean();
  }
}
