import { AppIcon } from "./AppIcon";
import {
  createMemo,
  createSignal,
  createUniqueId,
  For,
  onCleanup,
  onMount,
  Show,
} from "solid-js";

export type CompactOption = { value: string; label: string };

/** Select-only combobox with keyboard navigation and no browser-native popup.
 * No dependency on external UI kits. This can be replaced by the Solid UI/Kobalte
 * primitive once the dependency is installed and audited in the main workspace. */
export function CompactSelect(props: {
  id?: string;
  ariaLabel: string;
  value: string;
  options: CompactOption[];
  onChange: (next: string) => void;
  disabled?: boolean;
  placeholder?: string;
}) {
  const [open, setOpen] = createSignal(false);
  const [active, setActive] = createSignal(0);
  const listId = createUniqueId();
  let root: HTMLDivElement | undefined;
  let trigger: HTMLButtonElement | undefined;

  const currentIndex = createMemo(() =>
    props.options.findIndex((option) => option.value === props.value),
  );
  const currentLabel = createMemo(
    () =>
      props.options[currentIndex()]?.label ?? props.placeholder ?? "Select…",
  );
  const isValid = (i: number) => i >= 0 && i < props.options.length;

  function begin() {
    if (props.disabled || !props.options.length) return;
    setActive(Math.max(0, currentIndex()));
    setOpen(true);
  }
  function choose(index: number) {
    if (!isValid(index)) return;
    props.onChange(props.options[index].value);
    setOpen(false);
    queueMicrotask(() => trigger?.focus());
  }
  function keydown(
    event: KeyboardEvent & { currentTarget: HTMLButtonElement },
  ) {
    const key = event.key;
    if (key === "Escape" && open()) {
      event.preventDefault();
      setOpen(false);
      return;
    }
    if (key === "Tab") {
      setOpen(false);
      return;
    }
    if (
      key === "ArrowDown" ||
      key === "ArrowUp" ||
      key === "Home" ||
      key === "End"
    ) {
      event.preventDefault();
      if (!open()) {
        begin();
        return;
      }
      if (key === "Home") setActive(0);
      else if (key === "End") setActive(props.options.length - 1);
      else
        setActive(
          (i) =>
            (i + (key === "ArrowDown" ? 1 : -1) + props.options.length) %
            props.options.length,
        );
      return;
    }
    if (key === "Enter" || key === " ") {
      event.preventDefault();
      if (open()) choose(active());
      else begin();
      return;
    }
    if (key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {
      const match = props.options.findIndex((option) =>
        option.label.toLowerCase().startsWith(key.toLowerCase()),
      );
      if (isValid(match)) {
        event.preventDefault();
        if (!open()) begin();
        setActive(match);
      }
    }
  }
  onMount(() => {
    const outside = (event: PointerEvent) => {
      if (root && event.target instanceof Node && !root.contains(event.target))
        setOpen(false);
    };
    document.addEventListener("pointerdown", outside);
    onCleanup(() => document.removeEventListener("pointerdown", outside));
  });
  return (
    <div class="ux-select" ref={root}>
      <button
        ref={trigger}
        type="button"
        id={props.id}
        class="ux-select__trigger"
        role="combobox"
        aria-label={props.ariaLabel}
        aria-haspopup="listbox"
        aria-expanded={open()}
        aria-controls={listId}
        aria-activedescendant={open() ? `${listId}-${active()}` : undefined}
        disabled={props.disabled}
        onClick={() => (open() ? setOpen(false) : begin())}
        onKeyDown={keydown}
      >
        <span class="ux-select__value">{currentLabel()}</span>
        <AppIcon name="chevron-down" class="ux-select__chevron" />
      </button>
      <Show when={open()}>
        <div
          class="ux-select__menu"
          role="listbox"
          id={listId}
          aria-label={props.ariaLabel}
        >
          <For each={props.options}>
            {(option, index) => (
              <button
                type="button"
                role="option"
                id={`${listId}-${index()}`}
                aria-selected={option.value === props.value}
                class="ux-select__option"
                classList={{
                  "ux-select__option--active": active() === index(),
                }}
                tabIndex={-1}
                onPointerMove={() => setActive(index())}
                onClick={() => choose(index())}
              >
                {option.label}
                <Show when={option.value === props.value}>
                  <AppIcon name="check" />
                </Show>
              </button>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
