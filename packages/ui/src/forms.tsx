import { createEffect, createSignal, Show } from "solid-js";
import type {
  CreateEnvironmentInput,
  CreateProjectInput,
  CreateWorkspaceInput,
  EnvironmentKind,
  Namespace,
  SaveTranslationInput,
  SetConfigVariableInput,
  TranslationEntry,
  TranslationStatus,
} from "@slimlocalize/platform";
import { errorMessage } from "@slimlocalize/platform";
import { Badge, Button, Field, SelectField, TextareaField } from "./components";

export function WorkspaceForm(props: {
  onSubmit: (input: CreateWorkspaceInput) => Promise<void>;
  onCancel?: () => void;
}) {
  const [name, setName] = createSignal("");
  const [slug, setSlug] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  async function submit(event: SubmitEvent) {
    event.preventDefault();
    setError("");
    setBusy(true);
    try {
      await props.onSubmit({ name: name(), slug: slug() || null });
      setName("");
      setSlug("");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }
  return (
    <form class="form-grid" onSubmit={submit}>
      <Field
        label="Workspace name"
        value={name()}
        onInput={setName}
        placeholder="Demo team"
        required
      />
      <Field
        label="Slug (optional)"
        value={slug()}
        onInput={setSlug}
        placeholder="demo-team"
        hint="Lowercase letters, digits and dashes."
      />
      <Show when={error()}>
        <span class="field__error" role="alert">
          {error()}
        </span>
      </Show>
      <div class="form-actions">
        <Show when={props.onCancel}>
          <Button variant="ghost" onClick={props.onCancel}>
            Cancel
          </Button>
        </Show>
        <Button type="submit" loading={busy()}>
          Create workspace
        </Button>
      </div>
    </form>
  );
}

export function ProjectForm(props: {
  onSubmit: (input: CreateProjectInput) => Promise<void>;
  onCancel?: () => void;
}) {
  const [name, setName] = createSignal("");
  const [locale, setLocale] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  async function submit(event: SubmitEvent) {
    event.preventDefault();
    setError("");
    setBusy(true);
    try {
      await props.onSubmit({ name: name(), defaultLocale: locale() || null });
      setName("");
      setLocale("");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }
  return (
    <form class="form-grid" onSubmit={submit}>
      <Field
        label="Project name"
        value={name()}
        onInput={setName}
        placeholder="Demo app"
        required
      />
      <Field
        label="First locale (optional)"
        value={locale()}
        onInput={setLocale}
        placeholder="en-US"
        hint="Becomes the default locale."
      />
      <Show when={error()}>
        <span class="field__error" role="alert">
          {error()}
        </span>
      </Show>
      <div class="form-actions">
        <Show when={props.onCancel}>
          <Button variant="ghost" onClick={props.onCancel}>
            Cancel
          </Button>
        </Show>
        <Button type="submit" loading={busy()}>
          Create project
        </Button>
      </div>
    </form>
  );
}

export function LocaleForm(props: {
  onSubmit: (code: string, isDefault: boolean) => Promise<void>;
  onCancel?: () => void;
}) {
  const [code, setCode] = createSignal("");
  const [isDefault, setDefault] = createSignal(false);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  async function submit(event: SubmitEvent) {
    event.preventDefault();
    setError("");
    setBusy(true);
    try {
      await props.onSubmit(code(), isDefault());
      setCode("");
      setDefault(false);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }
  return (
    <form class="form-grid" onSubmit={submit}>
      <Field
        label="Locale code"
        value={code()}
        onInput={setCode}
        placeholder="es-PE"
        required
        hint="BCP-47 style, e.g. en, es-PE, zh-Hant-TW."
      />
      <label class="row">
        <input
          type="checkbox"
          checked={isDefault()}
          onChange={(e) => setDefault(e.currentTarget.checked)}
        />{" "}
        Make default
      </label>
      <Show when={error()}>
        <span class="field__error" role="alert">
          {error()}
        </span>
      </Show>
      <div class="form-actions">
        <Show when={props.onCancel}>
          <Button variant="ghost" onClick={props.onCancel}>
            Cancel
          </Button>
        </Show>
        <Button type="submit" loading={busy()}>
          Add locale
        </Button>
      </div>
    </form>
  );
}

export function NamespaceForm(props: {
  onSubmit: (name: string) => Promise<void>;
  onCancel?: () => void;
}) {
  const [name, setName] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  async function submit(event: SubmitEvent) {
    event.preventDefault();
    setError("");
    setBusy(true);
    try {
      await props.onSubmit(name());
      setName("");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }
  return (
    <form class="form-grid" onSubmit={submit}>
      <Field
        label="Namespace"
        value={name()}
        onInput={setName}
        placeholder="auth"
        required
      />
      <Show when={error()}>
        <span class="field__error" role="alert">
          {error()}
        </span>
      </Show>
      <div class="form-actions">
        <Show when={props.onCancel}>
          <Button variant="ghost" onClick={props.onCancel}>
            Cancel
          </Button>
        </Show>
        <Button type="submit" loading={busy()}>
          Create namespace
        </Button>
      </div>
    </form>
  );
}

export function TranslationEditor(props: {
  entry: TranslationEntry | null;
  localeCode: string;
  namespaces: Namespace[];
  defaultNamespace: string;
  onSave: (input: Omit<SaveTranslationInput, "projectId">) => Promise<void>;
  onNew: () => void;
}) {
  const [key, setKey] = createSignal("");
  const [namespace, setNamespace] = createSignal(props.defaultNamespace);
  const [value, setValue] = createSignal("");
  const [status, setStatus] = createSignal<TranslationStatus>("translated");
  const [description, setDescription] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  createEffect(() => {
    const entry = props.entry;
    if (entry) {
      setKey(entry.key);
      setNamespace(entry.namespace);
      setValue(entry.value ?? "");
      setStatus(entry.status ?? "draft");
      setDescription(entry.description ?? "");
    } else {
      setKey("");
      setNamespace(props.defaultNamespace);
      setValue("");
      setStatus("translated");
      setDescription("");
    }
  });
  async function submit(event: SubmitEvent) {
    event.preventDefault();
    setError("");
    if (!key().trim()) {
      setError("Key is required.");
      return;
    }
    setBusy(true);
    try {
      await props.onSave({
        namespace: namespace(),
        key: key(),
        localeCode: props.localeCode,
        value: value(),
        status: status(),
        description: description() || null,
      });
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }
  return (
    <form onSubmit={submit} class="stack">
      <div class="row">
        <strong>{props.entry ? "Edit translation" : "New translation"}</strong>
        <span class="grow" />
        <Badge tone="accent">{props.localeCode}</Badge>
        <Button size="sm" variant="ghost" onClick={props.onNew}>
          New
        </Button>
      </div>
      <Show when={error()}>
        <div class="notice notice--error" role="alert">
          {error()}
        </div>
      </Show>
      <Field
        label="Key"
        value={key()}
        onInput={setKey}
        placeholder="auth.signIn"
        required
        disabled={Boolean(props.entry)}
      />
      <SelectField
        label="Namespace"
        value={namespace()}
        onChange={setNamespace}
        options={props.namespaces.map((item) => ({
          value: item.name,
          label: item.name,
        }))}
      />
      <TextareaField
        label="Value"
        value={value()}
        onInput={setValue}
        placeholder="Sign in"
        rows={5}
      />
      <SelectField
        label="Status"
        value={status()}
        onChange={(next) => setStatus(next as TranslationStatus)}
        options={[
          { value: "draft", label: "Draft" },
          { value: "translated", label: "Translated" },
          { value: "reviewed", label: "Reviewed" },
        ]}
      />
      <Field
        label="Description (optional)"
        value={description()}
        onInput={setDescription}
        placeholder="Context for translators"
      />
      <div class="form-actions">
        <Button type="submit" loading={busy()}>
          Save translation
        </Button>
      </div>
    </form>
  );
}

export function ConfigForm(props: {
  onSubmit: (
    input: Omit<SetConfigVariableInput, "environmentId">,
  ) => Promise<void>;
  onCancel?: () => void;
}) {
  const [key, setKey] = createSignal("");
  const [value, setValue] = createSignal("");
  const [visibility, setVisibility] = createSignal<"public" | "local">(
    "public",
  );
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  async function submit(event: SubmitEvent) {
    event.preventDefault();
    setError("");
    setBusy(true);
    try {
      await props.onSubmit({
        key: key(),
        value: value(),
        visibility: visibility(),
      });
      setKey("");
      setValue("");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }
  return (
    <form class="form-grid" onSubmit={submit}>
      <div class="form-row">
        <Field
          label="Key"
          value={key()}
          onInput={setKey}
          placeholder="PUBLIC_API_URL"
          required
        />
        <SelectField
          label="Visibility"
          value={visibility()}
          onChange={(next) => setVisibility(next as "public" | "local")}
          options={[
            { value: "public", label: "Public" },
            { value: "local", label: "Local (not shared)" },
          ]}
        />
      </div>
      <Field
        label="Value"
        value={value()}
        onInput={setValue}
        placeholder="http://localhost:3000"
        required
      />
      <Show when={error()}>
        <span class="field__error" role="alert">
          {error()}
        </span>
      </Show>
      <div class="form-actions">
        <Show when={props.onCancel}>
          <Button variant="ghost" onClick={props.onCancel}>
            Cancel
          </Button>
        </Show>
        <Button type="submit" loading={busy()}>
          Save variable
        </Button>
      </div>
    </form>
  );
}

export function EnvironmentForm(props: {
  onSubmit: (input: CreateEnvironmentInput) => Promise<void>;
  onCancel?: () => void;
}) {
  const [name, setName] = createSignal("");
  const [kind, setKind] = createSignal<EnvironmentKind>("custom");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  async function submit(event: SubmitEvent) {
    event.preventDefault();
    setError("");
    setBusy(true);
    try {
      await props.onSubmit({ name: name(), kind: kind() });
      setName("");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }
  return (
    <form class="form-grid" onSubmit={submit}>
      <Field
        label="Environment name"
        value={name()}
        onInput={setName}
        placeholder="qa"
        required
      />
      <SelectField
        label="Kind"
        value={kind()}
        onChange={(next) => setKind(next as EnvironmentKind)}
        options={[
          { value: "development", label: "Development" },
          { value: "staging", label: "Staging" },
          { value: "production", label: "Production" },
          { value: "custom", label: "Custom" },
        ]}
      />
      <Show when={error()}>
        <span class="field__error" role="alert">
          {error()}
        </span>
      </Show>
      <div class="form-actions">
        <Show when={props.onCancel}>
          <Button variant="ghost" onClick={props.onCancel}>
            Cancel
          </Button>
        </Show>
        <Button type="submit" loading={busy()}>
          Create environment
        </Button>
      </div>
    </form>
  );
}

export function ImportForm(props: {
  mode: "path" | "files";
  onSubmitPath?: (path: string) => Promise<void>;
  onSubmitFiles?: (
    files: Array<{ fileName: string; content: string }>,
  ) => Promise<void>;
  onCancel?: () => void;
}) {
  const [path, setPath] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  async function submitPath(event: SubmitEvent) {
    event.preventDefault();
    setError("");
    setBusy(true);
    try {
      await props.onSubmitPath?.(path());
      setPath("");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }
  async function chooseFiles(selected: FileList | null) {
    if (!selected?.length) return;
    setError("");
    setBusy(true);
    try {
      const files = await Promise.all(
        Array.from(selected).map(async (file) => ({
          fileName: file.name,
          content: await file.text(),
        })),
      );
      await props.onSubmitFiles?.(files);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }
  return (
    <form class="form-grid" onSubmit={submitPath}>
      <Show
        when={props.mode === "path"}
        fallback={
          <label class="field">
            <span class="field__label">JSON locale files</span>
            <input
              class="input"
              type="file"
              accept=".json,application/json"
              multiple
              disabled={busy()}
              onChange={(event) => void chooseFiles(event.currentTarget.files)}
            />
            <span class="field__hint">
              Each file must be named like en-US.json. Nested keys are
              flattened.
            </span>
          </label>
        }
      >
        <Field
          label="Directory or JSON file path"
          value={path()}
          onInput={setPath}
          placeholder="/data/locales"
          required
          hint="Read by the desktop app; paths are resolved in Rust."
        />
      </Show>
      <Show when={error()}>
        <span class="field__error" role="alert">
          {error()}
        </span>
      </Show>
      <div class="form-actions">
        <Show when={props.onCancel}>
          <Button variant="ghost" onClick={props.onCancel}>
            Cancel
          </Button>
        </Show>
        <Show when={props.mode === "path"}>
          <Button type="submit" loading={busy()}>
            Import files
          </Button>
        </Show>
      </div>
    </form>
  );
}
