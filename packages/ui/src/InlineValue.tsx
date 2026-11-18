import { createEffect, createSignal, Show } from "solid-js";
import type {
  SaveTranslationInput,
  TranslationEntry,
} from "@slimlocalize/platform";
import { errorMessage } from "@slimlocalize/platform";

/** Edit only the value, keeping all key metadata and validation in Rust/domain. */
export function InlineValue(props: {
  entry: TranslationEntry;
  onSave: (input: Omit<SaveTranslationInput, "projectId">) => Promise<void>;
}) {
  const [editing, setEditing] = createSignal(false);
  const [draft, setDraft] = createSignal(props.entry.value ?? "");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  let input: HTMLInputElement | undefined;
  createEffect(() => {
    if (!editing()) setDraft(props.entry.value ?? "");
  });

  async function commit() {
    if (!editing() || busy()) return;
    if (draft() === (props.entry.value ?? "")) {
      setEditing(false);
      return;
    }
    setBusy(true);
    setError("");
    try {
      await props.onSave({
        namespace: props.entry.namespace,
        key: props.entry.key,
        localeCode: props.entry.localeCode,
        value: draft(),
        status: draft().trim() ? "translated" : "draft",
        description: props.entry.description ?? null,
      });
      setEditing(false);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }
  function cancel() {
    setDraft(props.entry.value ?? "");
    setError("");
    setEditing(false);
  }
  return (
    <div
      class="inline-value"
      onClick={(event) => event.stopPropagation()}
      onKeyDown={(event) => event.stopPropagation()}
    >
      <Show
        when={editing()}
        fallback={
          <button
            type="button"
            class="inline-value__preview"
            title="Edit translation value"
            aria-label={`Edit ${props.entry.key}`}
            onClick={() => {
              setEditing(true);
              queueMicrotask(() => input?.focus());
            }}
          >
            {props.entry.value ?? "＋ Add translation"}
          </button>
        }
      >
        <input
          ref={input}
          class="input inline-value__input"
          aria-label={`Value for ${props.entry.key}`}
          value={draft()}
          disabled={busy()}
          onInput={(event) => setDraft(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              event.stopPropagation();
              void commit();
            }
            if (event.key === "Escape") {
              event.preventDefault();
              event.stopPropagation();
              cancel();
            }
          }}
          onBlur={() => {
            if (!busy()) void commit();
          }}
        />
      </Show>
      <Show when={busy()}>
        <span class="inline-value__saving">Saving…</span>
      </Show>
      <Show when={error()}>
        <span class="inline-value__error" role="alert">
          {error()}
        </span>
      </Show>
    </div>
  );
}
