import { createSignal, onCleanup, onMount, Show } from "solid-js";
import type { Namespace, SaveTranslationInput } from "@slimlocalize/platform";
import { errorMessage } from "@slimlocalize/platform";
import { Button } from "./components";
import { CompactSelect } from "./CompactSelect";

/** Minimal, keyboard-first creation surface. Persistence remains in WorkspaceService. */
export function QuickAddKey(props: {
  localeCode: string;
  defaultNamespace: string;
  namespaces: Namespace[];
  onSave: (input: Omit<SaveTranslationInput, "projectId">) => Promise<void>;
  onClose: () => void;
}) {
  const [key, setKey] = createSignal("");
  const [value, setValue] = createSignal("");
  const [namespace, setNamespace] = createSignal(props.defaultNamespace);
  const [again, setAgain] = createSignal(false);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  let panel: HTMLDivElement | undefined;
  let firstInput: HTMLInputElement | undefined;
  let previousFocus: HTMLElement | null = null;

  onMount(() => {
    previousFocus =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    firstInput?.focus();
    // The dialog is modal: keep keyboard focus inside it.
    function onKeydown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        if (!busy()) props.onClose();
        return;
      }
      if (event.key !== "Tab" || !panel) return;
      const nodes = Array.from(
        panel.querySelectorAll<HTMLElement>(
          "button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled])",
        ),
      );
      if (!nodes.length) return;
      const first = nodes[0];
      const last = nodes[nodes.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }
    document.addEventListener("keydown", onKeydown, true);
    onCleanup(() => {
      document.removeEventListener("keydown", onKeydown, true);
      previousFocus?.focus();
    });
  });

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy()) return;
    setError("");
    if (!key().trim()) {
      setError("Enter a key name.");
      firstInput?.focus();
      return;
    }
    setBusy(true);
    try {
      await props.onSave({
        namespace: namespace(),
        key: key().trim(),
        localeCode: props.localeCode,
        value: value(),
        status: value().trim() ? "translated" : "draft",
        description: null,
      });
      if (again()) {
        setKey("");
        setValue("");
        firstInput?.focus();
      } else props.onClose();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      class="ux-dialog-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !busy()) props.onClose();
      }}
    >
      <div
        ref={panel}
        class="ux-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="quick-add-title"
        aria-describedby="quick-add-description"
      >
        <div class="ux-dialog__heading">
          <div>
            <h2 id="quick-add-title" class="ux-dialog__title">
              New translation key
            </h2>
            <p id="quick-add-description" class="ux-dialog__hint">
              One key, one value. Add more details later.
            </p>
          </div>
          <Button
            size="sm"
            variant="ghost"
            class="ux-dialog__close"
            aria-label="Close quick add"
            onClick={props.onClose}
          >
            ✕
          </Button>
        </div>
        <form onSubmit={submit}>
          <div class="ux-dialog__fields">
            <label class="field">
              <span class="field__label">Key</span>
              <input
                ref={firstInput}
                class="input"
                required
                value={key()}
                onInput={(event) => setKey(event.currentTarget.value)}
                placeholder="auth.signIn"
                autocomplete="off"
              />
            </label>
            <label class="field">
              <span class="field__label">Value · {props.localeCode}</span>
              <textarea
                class="textarea"
                rows={3}
                value={value()}
                onInput={(event) => setValue(event.currentTarget.value)}
                placeholder="Sign in"
              />
            </label>
            <label class="field">
              <span class="field__label">Namespace</span>
              <CompactSelect
                ariaLabel="Namespace"
                value={namespace()}
                options={props.namespaces.map((item) => ({
                  value: item.name,
                  label: item.name,
                }))}
                onChange={setNamespace}
              />
            </label>
          </div>
          <Show when={error()}>
            <p class="field__error" role="alert">
              {error()}
            </p>
          </Show>
          <div class="ux-dialog__footer">
            <label class="row">
              <input
                type="checkbox"
                checked={again()}
                onChange={(event) => setAgain(event.currentTarget.checked)}
              />{" "}
              <span class="ux-dialog__shortcut">Create &amp; add another</span>
            </label>
            <div class="form-actions">
              <Button variant="secondary" onClick={props.onClose}>
                Cancel
              </Button>
              <Button type="submit" loading={busy()}>
                Create key
              </Button>
            </div>
          </div>
        </form>
      </div>
    </div>
  );
}
