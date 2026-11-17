import { createMemo, createSignal, For, onMount, Show } from "solid-js";
import type {
  CheckReport,
  ConfigVariable,
  Environment,
  Locale,
  Namespace,
  PlatformInfo,
  Project,
  TranslationEntry,
  Workspace,
  WorkspaceService,
} from "@slimlocalize/platform";
import { errorMessage } from "@slimlocalize/platform";
import {
  Badge,
  Button,
  EmptyState,
  Notice,
  SelectField,
  StatusChip,
  cx,
} from "./components";
import {
  ConfigForm,
  EnvironmentForm,
  ImportForm,
  LocaleForm,
  NamespaceForm,
  ProjectForm,
  TranslationEditor,
  WorkspaceForm,
} from "./forms";
import {
  IconCheck,
  IconDatabase,
  IconDownload,
  IconFolder,
  IconGlobe,
  IconPlus,
  IconSearch,
  IconUpload,
} from "./icons";

type View = "translations" | "environments" | "validation";
type StatusKind = "idle" | "loading" | "saving" | "saved" | "error";
interface PageStatus {
  kind: StatusKind;
  message?: string;
}

export function AppShell(props: { service: WorkspaceService }) {
  const [workspaces, setWorkspaces] = createSignal<Workspace[]>([]);
  const [workspaceId, setWorkspaceId] = createSignal<string | null>(null);
  const [projects, setProjects] = createSignal<Project[]>([]);
  const [projectId, setProjectId] = createSignal<string | null>(null);
  const [locales, setLocales] = createSignal<Locale[]>([]);
  const [localeCode, setLocaleCode] = createSignal("");
  const [namespaces, setNamespaces] = createSignal<Namespace[]>([]);
  const [namespaceName, setNamespaceName] = createSignal("common");
  const [entries, setEntries] = createSignal<TranslationEntry[]>([]);
  const [environments, setEnvironments] = createSignal<Environment[]>([]);
  const [environmentId, setEnvironmentId] = createSignal("");
  const [variables, setVariables] = createSignal<ConfigVariable[]>([]);
  const [report, setReport] = createSignal<CheckReport | null>(null);
  const [view, setView] = createSignal<View>("translations");
  const [status, setStatus] = createSignal<PageStatus>({
    kind: "loading",
    message: "Opening local workspace…",
  });
  const [notice, setNotice] = createSignal<string | null>(null);
  const [search, setSearch] = createSignal("");
  const [selected, setSelected] = createSignal<TranslationEntry | null>(null);
  const [showWorkspaceForm, setShowWorkspaceForm] = createSignal(false);
  const [showProjectForm, setShowProjectForm] = createSignal(false);
  const [showLocaleForm, setShowLocaleForm] = createSignal(false);
  const [showNamespaceForm, setShowNamespaceForm] = createSignal(false);
  const [showEnvForm, setShowEnvForm] = createSignal(false);
  const [showImport, setShowImport] = createSignal(false);
  let savedTimer: ReturnType<typeof setTimeout> | undefined;

  const workspace = createMemo(
    () => workspaces().find((item) => item.id === workspaceId()) ?? null,
  );
  const project = createMemo(
    () => projects().find((item) => item.id === projectId()) ?? null,
  );
  const environment = createMemo(
    () => environments().find((item) => item.id === environmentId()) ?? null,
  );
  const platform = props.service.platform;
  const filtered = createMemo(() => {
    const query = search().trim().toLowerCase();
    if (!query) return entries();
    return entries().filter(
      (entry) =>
        entry.key.toLowerCase().includes(query) ||
        (entry.value ?? "").toLowerCase().includes(query),
    );
  });

  function finish(message?: string) {
    setStatus({ kind: "saved", message: message ?? "Saved locally" });
    if (savedTimer) clearTimeout(savedTimer);
    savedTimer = setTimeout(() => setStatus({ kind: "idle" }), 2200);
  }
  function fail(cause: unknown) {
    setStatus({ kind: "error", message: errorMessage(cause) });
  }
  async function act<T>(message: string, action: () => Promise<T>): Promise<T> {
    setStatus({ kind: "saving", message });
    setNotice(null);
    try {
      const result = await action();
      finish();
      return result;
    } catch (cause) {
      fail(cause);
      throw cause;
    }
  }

  async function loadEntries(
    pid = projectId(),
    code = localeCode(),
    ns = namespaceName(),
  ) {
    if (!pid || !code) {
      setEntries([]);
      return;
    }
    setEntries(await props.service.listTranslations(pid, code, ns || null));
  }
  async function loadVariables(id = environmentId()) {
    if (!id) {
      setVariables([]);
      return;
    }
    setVariables(await props.service.listConfigVariables(id));
  }
  async function openProject(id: string) {
    setStatus({ kind: "loading", message: "Loading project…" });
    setProjectId(id);
    setView("translations");
    setReport(null);
    setSelected(null);
    setEntries([]);
    setSearch("");
    try {
      const [loc, ns, envs] = await Promise.all([
        props.service.listLocales(id),
        props.service.listNamespaces(id),
        props.service.listEnvironments(id),
      ]);
      setLocales(loc);
      setNamespaces(ns);
      setEnvironments(envs);
      const locale = loc.find((item) => item.isDefault) ?? loc[0];
      const namespace = ns.find((item) => item.name === "common") ?? ns[0];
      const env = envs.find((item) => item.name === "development") ?? envs[0];
      setLocaleCode(locale?.code ?? "");
      setNamespaceName(namespace?.name ?? "");
      setEnvironmentId(env?.id ?? "");
      if (locale && namespace)
        await loadEntries(id, locale.code, namespace.name);
      if (env) await loadVariables(env.id);
      setStatus({ kind: "idle" });
    } catch (cause) {
      fail(cause);
    }
  }

  async function openWorkspace(id: string) {
    setWorkspaceId(id);
    setProjectId(null);
    setProjects([]);
    setLocales([]);
    setEntries([]);
    setEnvironments([]);
    setVariables([]);
    setStatus({ kind: "loading", message: "Loading workspace…" });
    try {
      const list = await props.service.listProjects(id);
      setProjects(list);
      if (list[0]) await openProject(list[0].id);
      else {
        setShowProjectForm(true);
        setStatus({ kind: "idle" });
      }
    } catch (cause) {
      fail(cause);
    }
  }

  onMount(async () => {
    try {
      const list = await props.service.listWorkspaces();
      setWorkspaces(list);
      if (list[0]) await openWorkspace(list[0].id);
      else {
        setShowWorkspaceForm(true);
        setStatus({ kind: "idle" });
      }
    } catch (cause) {
      fail(cause);
    }
  });

  async function createWorkspace(
    input: Parameters<WorkspaceService["createWorkspace"]>[0],
  ) {
    const created = await act("Creating workspace", () =>
      props.service.createWorkspace(input),
    );
    setWorkspaces((current) =>
      [...current, created].sort((a, b) => a.name.localeCompare(b.name)),
    );
    setShowWorkspaceForm(false);
    await openWorkspace(created.id);
  }
  async function createProject(
    input: Parameters<WorkspaceService["createProject"]>[1],
  ) {
    const current = workspace();
    if (!current) throw new Error("Select a workspace first.");
    const created = await act("Creating project", () =>
      props.service.createProject(current.id, input),
    );
    setProjects((items) =>
      [...items, created].sort((a, b) => a.name.localeCompare(b.name)),
    );
    setShowProjectForm(false);
    await openProject(created.id);
  }
  async function addLocale(code: string, isDefault: boolean) {
    const current = project();
    if (!current) throw new Error("Select a project first.");
    const locale = await act("Adding locale", () =>
      props.service.addLocale(current.id, { code, isDefault }),
    );
    setLocales(await props.service.listLocales(current.id));
    setLocaleCode(locale.code);
    setShowLocaleForm(false);
    await loadEntries(current.id, locale.code, namespaceName());
  }

  async function createNamespace(name: string) {
    const current = project();
    if (!current) throw new Error("Select a project first.");
    const namespace = await act("Creating namespace", () =>
      props.service.createNamespace(current.id, name),
    );
    setNamespaces(await props.service.listNamespaces(current.id));
    setNamespaceName(namespace.name);
    setShowNamespaceForm(false);
    await loadEntries(current.id, localeCode(), namespace.name);
  }
  async function saveTranslation(
    input: Omit<
      Parameters<WorkspaceService["saveTranslation"]>[0],
      "projectId"
    >,
  ) {
    const current = project();
    if (!current) throw new Error("Select a project first.");
    const saved = await act("Saving translation", () =>
      props.service.saveTranslation({ ...input, projectId: current.id }),
    );
    setSelected(saved);
    await loadEntries(current.id, saved.localeCode, input.namespace);
    if (saved.localeCode !== localeCode()) setLocaleCode(saved.localeCode);
  }

  async function createEnvironment(
    input: Parameters<WorkspaceService["createEnvironment"]>[1],
  ) {
    const current = project();
    if (!current) throw new Error("Select a project first.");
    const env = await act("Creating environment", () =>
      props.service.createEnvironment(current.id, input),
    );
    setEnvironments(await props.service.listEnvironments(current.id));
    setEnvironmentId(env.id);
    setShowEnvForm(false);
    await loadVariables(env.id);
  }
  async function saveVariable(
    input: Omit<
      Parameters<WorkspaceService["setConfigVariable"]>[0],
      "environmentId"
    >,
  ) {
    const env = environment();
    if (!env) throw new Error("Select an environment first.");
    await act("Saving variable", () =>
      props.service.setConfigVariable({ ...input, environmentId: env.id }),
    );
    await loadVariables(env.id);
  }
  async function runCheck() {
    const current = project();
    if (!current) return;
    const result = await act("Validating project", () =>
      props.service.checkProject(current.id),
    );
    setReport(result);
    setView("validation");
  }
  async function exportProject() {
    const current = project();
    if (!current) return;
    const result = await act("Exporting JSON", () =>
      props.service.exportProject(current.id, { nested: true }),
    );
    setNotice(
      `Exported ${result.files.length} locale files (${result.totalKeys} values) to ${result.outputDir}.`,
    );
  }

  async function importFromPath(path: string) {
    const current = project();
    if (!current || !props.service.importFromPath)
      throw new Error("Path import is unavailable.");
    const result = await act("Importing translations", () =>
      props.service.importFromPath!(current.id, path, namespaceName()),
    );
    setShowImport(false);
    setNotice(
      `Imported ${result.filesProcessed} files (${result.valuesWritten} values).`,
    );
    await openProject(current.id);
  }
  async function importFromFiles(
    files: Array<{ fileName: string; content: string }>,
  ) {
    const current = project();
    if (!current || !props.service.importFromFiles)
      throw new Error("File import is unavailable.");
    const result = await act("Importing translations", () =>
      props.service.importFromFiles!(current.id, files, namespaceName()),
    );
    setShowImport(false);
    setNotice(
      `Imported ${result.filesProcessed} files (${result.valuesWritten} values).`,
    );
    await openProject(current.id);
  }
  function selectLocale(code: string) {
    setLocaleCode(code);
    setSelected(null);
    void loadEntries(projectId(), code, namespaceName()).catch(fail);
  }
  function selectNamespace(name: string) {
    setNamespaceName(name);
    setSelected(null);
    void loadEntries(projectId(), localeCode(), name).catch(fail);
  }
  function selectEnvironment(id: string) {
    setEnvironmentId(id);
    void loadVariables(id).catch(fail);
  }

  return (
    <div class="app">
      <header class="topbar">
        <div class="brand">
          <span class="brand__mark" aria-hidden="true">
            <IconGlobe size={18} />
          </span>
          <span>SlimLocalize</span>
          <span class="brand__tag">local-first</span>
        </div>
        <div class="topbar__spacer" />
        <div class="topbar__controls">
          <label class="workspace-control">
            <span class="sr-only">Workspace</span>
            <select
              class="select"
              value={workspaceId() ?? ""}
              disabled={workspaces().length === 0}
              onChange={(event) =>
                void openWorkspace(event.currentTarget.value)
              }
            >
              <option value="" disabled>
                No workspace
              </option>
              <For each={workspaces()}>
                {(item) => <option value={item.id}>{item.name}</option>}
              </For>
            </select>
          </label>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => setShowWorkspaceForm(true)}
          >
            <IconPlus /> Workspace
          </Button>
          <span class="storage-indicator" title={platform.storage}>
            <span class="storage-indicator__dot" />
            <span class="storage-indicator__label">{platform.storage}</span>
          </span>
          <StatusChip kind={status().kind} message={status().message} />
        </div>
      </header>

      <div class="app-body">
        <aside class="sidebar" aria-label="Projects">
          <Show when={showWorkspaceForm() && workspace()}>
            <div class="sidebar-form">
              <WorkspaceForm
                onSubmit={createWorkspace}
                onCancel={() => setShowWorkspaceForm(false)}
              />
            </div>
          </Show>
          <div class="sidebar__eyebrow">Projects</div>
          <div class="sidebar__section">
            <div class="sidebar__projects">
              <For each={projects()}>
                {(item) => (
                  <button
                    class={cx(
                      "project-button",
                      item.id === projectId() && "project-button--active",
                    )}
                    onClick={() => void openProject(item.id)}
                  >
                    <IconFolder />
                    <span>
                      <span>{item.name}</span>
                      <span class="project-button__meta">
                        {item.defaultLocale ?? "No default locale"}
                      </span>
                    </span>
                  </button>
                )}
              </For>
            </div>
            <Show when={workspace()}>
              <Button
                size="sm"
                variant="ghost"
                block
                onClick={() => setShowProjectForm((open) => !open)}
              >
                <IconPlus /> New project
              </Button>
            </Show>
            <Show when={showProjectForm() && workspace()}>
              <div class="sidebar-form">
                <ProjectForm
                  onSubmit={createProject}
                  onCancel={() => setShowProjectForm(false)}
                />
              </div>
            </Show>
          </div>
          <div class="sidebar__footer">
            <div class="row">
              <IconDatabase /> Local data
            </div>
            <p>
              {platform.kind === "desktop"
                ? "SQLite database on this device. Works offline."
                : "Browser localStorage adapter. No server or account required."}
            </p>
            <Show when={platform.capabilities.importFromPath}>
              <p>Path imports are resolved inside the Rust core.</p>
            </Show>
          </div>
        </aside>

        <main class="content">
          <Show when={notice()}>
            <div class="stack" style="margin-bottom:16px">
              <Notice tone="success">{notice()}</Notice>
            </div>
          </Show>
          <Show when={status().kind === "error"}>
            <div class="stack" style="margin-bottom:16px">
              <Notice tone="error">{status().message}</Notice>
            </div>
          </Show>
          <Show when={platform.kind === "web"}>
            <div class="stack" style="margin-bottom:16px">
              <Notice tone="neutral">
                Web preview stores data in this browser only. The desktop app
                uses SQLite; path imports and native backups require the desktop
                build or CLI.
              </Notice>
            </div>
          </Show>

          <Show when={!workspace()}>
            <section class="hero">
              <div>
                <div class="hero__eyebrow">
                  Local first · Cloud optional · MIT
                </div>
                <h1 class="hero__title">Your translations, on your machine.</h1>
                <p class="hero__text">
                  Create a workspace to manage locales, namespaces, translation
                  keys and environment configuration. The desktop app persists
                  to SQLite; the web build uses browser storage.
                </p>
              </div>
              <div class="hero__card">
                <h2 class="panel__title">Create your first workspace</h2>
                <p class="panel__hint" style="margin-bottom:12px">
                  No account, network or external service required.
                </p>
                <WorkspaceForm onSubmit={createWorkspace} />
              </div>
            </section>
          </Show>

          <Show when={workspace() && !project()}>
            <div class="panel">
              <EmptyState
                icon={<IconFolder size={22} />}
                title="No projects in this workspace"
                description="Projects hold locales, translation keys and environments. Create one to start."
                action={
                  <div style="min-width:320px;text-align:left">
                    <ProjectForm onSubmit={createProject} />
                  </div>
                }
              />
            </div>
          </Show>

          <Show when={project()}>
            {(current) => (
              <>
                <div class="page-header">
                  <div>
                    <h1 class="page-header__title">{current().name}</h1>
                    <p class="page-header__subtitle">
                      <span class="mono">{current().slug}</span>
                      <span class="muted">
                        {" "}
                        · {locales().length} locales · {entries().length} keys
                        in view
                      </span>
                    </p>
                  </div>
                  <div class="page-header__actions">
                    <Show when={current().defaultLocale}>
                      <Badge tone="accent">
                        default {current().defaultLocale}
                      </Badge>
                    </Show>
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => setShowImport((open) => !open)}
                    >
                      <IconUpload /> Import
                    </Button>
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => void exportProject()}
                    >
                      <IconDownload /> Export
                    </Button>
                    <Button size="sm" onClick={() => void runCheck()}>
                      <IconCheck /> Check
                    </Button>
                  </div>
                </div>

                <Show when={showImport()}>
                  <div class="panel" style="margin-bottom:16px">
                    <div class="panel__header">
                      <h2 class="panel__title">Import translations</h2>
                      <p class="panel__hint">
                        Namespace: {namespaceName() || "common"}
                      </p>
                    </div>
                    <div class="panel__body">
                      <ImportForm
                        mode={
                          platform.capabilities.importFromPath
                            ? "path"
                            : "files"
                        }
                        onSubmitPath={importFromPath}
                        onSubmitFiles={importFromFiles}
                        onCancel={() => setShowImport(false)}
                      />
                    </div>
                  </div>
                </Show>

                <Show
                  when={locales().length === 0}
                  fallback={
                    <div class="stack">
                      <div class="row">
                        <div
                          class="tabs"
                          role="tablist"
                          aria-label="Project views"
                        >
                          <button
                            class={cx(
                              "tab",
                              view() === "translations" && "tab--active",
                            )}
                            onClick={() => setView("translations")}
                          >
                            Translations
                          </button>
                          <button
                            class={cx(
                              "tab",
                              view() === "environments" && "tab--active",
                            )}
                            onClick={() => setView("environments")}
                          >
                            Environments
                          </button>
                          <button
                            class={cx(
                              "tab",
                              view() === "validation" && "tab--active",
                            )}
                            onClick={() => setView("validation")}
                          >
                            Validation
                          </button>
                        </div>
                        <span class="grow" />
                        <span class="status-chip">
                          {platform.label} · {platform.storage}
                        </span>
                      </div>

                      <Show when={view() === "translations"}>
                        <div class="panel">
                          <div class="toolbar">
                            <SelectField
                              ariaLabel="Locale"
                              value={localeCode()}
                              onChange={selectLocale}
                              options={locales().map((item) => ({
                                value: item.code,
                                label: `${item.code}${item.isDefault ? " · default" : ""}`,
                              }))}
                            />
                            <SelectField
                              ariaLabel="Namespace"
                              value={namespaceName()}
                              onChange={selectNamespace}
                              options={namespaces().map((item) => ({
                                value: item.name,
                                label: item.name,
                              }))}
                            />
                            <input
                              class="input search-input"
                              type="search"
                              placeholder="Search keys or values…"
                              value={search()}
                              onInput={(event) =>
                                setSearch(event.currentTarget.value)
                              }
                              aria-label="Search translations"
                            />
                            <span class="toolbar__spacer" />
                            <Button
                              size="sm"
                              variant="ghost"
                              onClick={() => setShowLocaleForm((open) => !open)}
                            >
                              <IconPlus /> Locale
                            </Button>
                            <Button
                              size="sm"
                              variant="ghost"
                              onClick={() =>
                                setShowNamespaceForm((open) => !open)
                              }
                            >
                              <IconPlus /> Namespace
                            </Button>
                            <Button
                              size="sm"
                              variant="secondary"
                              onClick={() => setSelected(null)}
                            >
                              <IconPlus /> New key
                            </Button>
                          </div>

                          <Show when={showLocaleForm() || showNamespaceForm()}>
                            <div
                              class="panel__body"
                              style="border-bottom:1px solid var(--color-border)"
                            >
                              <Show when={showLocaleForm()}>
                                <LocaleForm
                                  onSubmit={addLocale}
                                  onCancel={() => setShowLocaleForm(false)}
                                />
                              </Show>
                              <Show when={showNamespaceForm()}>
                                <NamespaceForm
                                  onSubmit={createNamespace}
                                  onCancel={() => setShowNamespaceForm(false)}
                                />
                              </Show>
                            </div>
                          </Show>

                          <div class="translation-layout">
                            <div class="table-wrap">
                              <Show
                                when={filtered().length > 0}
                                fallback={
                                  <EmptyState
                                    icon={<IconSearch size={22} />}
                                    title="No translation keys"
                                    description="Create your first key in the editor, or import JSON locale files."
                                    action={
                                      <Button
                                        size="sm"
                                        variant="secondary"
                                        onClick={() => setShowImport(true)}
                                      >
                                        <IconUpload /> Import JSON
                                      </Button>
                                    }
                                  />
                                }
                              >
                                <table class="data-table">
                                  <thead>
                                    <tr>
                                      <th>Key</th>
                                      <th>Value</th>
                                      <th>Status</th>
                                    </tr>
                                  </thead>
                                  <tbody>
                                    <For each={filtered()}>
                                      {(entry) => (
                                        <tr
                                          classList={{
                                            "is-selected":
                                              selected()?.keyId === entry.keyId,
                                          }}
                                          onClick={() => setSelected(entry)}
                                        >
                                          <td>
                                            <div class="mono">
                                              {entry.namespace}/{entry.key}
                                            </div>
                                            <Show when={entry.description}>
                                              <div class="muted">
                                                {entry.description}
                                              </div>
                                            </Show>
                                          </td>
                                          <td class="value-cell">
                                            <Show
                                              when={entry.value !== null}
                                              fallback={
                                                <Badge tone="warning">
                                                  Missing
                                                </Badge>
                                              }
                                            >
                                              <span title={entry.value ?? ""}>
                                                {entry.value}
                                              </span>
                                            </Show>
                                          </td>
                                          <td>{statusBadge(entry)}</td>
                                        </tr>
                                      )}
                                    </For>
                                  </tbody>
                                </table>
                              </Show>
                            </div>

                            <div class="editor">
                              <div class="editor__header row">
                                <div>
                                  <div class="panel__title">
                                    {selected() ? "Selected key" : "New key"}
                                  </div>
                                  <div class="muted mono">
                                    {localeCode() || "no locale"}
                                  </div>
                                </div>
                                <span class="grow" />
                                <Show when={selected()}>
                                  <Badge tone="neutral">
                                    {selected()!.namespace}
                                  </Badge>
                                </Show>
                              </div>
                              <div class="editor__body">
                                <TranslationEditor
                                  entry={selected()}
                                  localeCode={localeCode()}
                                  namespaces={namespaces()}
                                  defaultNamespace={namespaceName() || "common"}
                                  onSave={saveTranslation}
                                  onNew={() => setSelected(null)}
                                />
                              </div>
                            </div>
                          </div>
                        </div>
                      </Show>

                      <Show when={view() === "environments"}>
                        <div class="panel">
                          <div class="toolbar">
                            <SelectField
                              ariaLabel="Environment"
                              value={environmentId()}
                              onChange={selectEnvironment}
                              options={environments().map((item) => ({
                                value: item.id,
                                label: `${item.name} · ${item.kind}`,
                              }))}
                            />
                            <span class="toolbar__spacer" />
                            <Button
                              size="sm"
                              variant="ghost"
                              onClick={() => setShowEnvForm((open) => !open)}
                            >
                              <IconPlus /> Environment
                            </Button>
                          </div>
                          <Show when={showEnvForm()}>
                            <div
                              class="panel__body"
                              style="border-bottom:1px solid var(--color-border)"
                            >
                              <EnvironmentForm
                                onSubmit={createEnvironment}
                                onCancel={() => setShowEnvForm(false)}
                              />
                            </div>
                          </Show>
                          <Show when={environment()}>
                            <div class="panel__body stack">
                              <div class="row">
                                <h2 class="panel__title">
                                  {environment()!.name}
                                </h2>
                                <Badge tone="neutral">
                                  {environment()!.kind}
                                </Badge>
                                <span class="grow" />
                                <span class="muted">
                                  Public and local variables are stored; secrets
                                  are not supported in F1.
                                </span>
                              </div>
                              <Show
                                when={variables().length > 0}
                                fallback={
                                  <EmptyState
                                    icon={<IconDatabase size={22} />}
                                    title="No variables"
                                    description="Add a public or local configuration value for this environment."
                                  />
                                }
                              >
                                <div class="table-wrap">
                                  <table class="data-table">
                                    <thead>
                                      <tr>
                                        <th>Key</th>
                                        <th>Value</th>
                                        <th>Visibility</th>
                                      </tr>
                                    </thead>
                                    <tbody>
                                      <For each={variables()}>
                                        {(variable) => (
                                          <tr>
                                            <td class="mono">{variable.key}</td>
                                            <td class="value-cell">
                                              {variable.value}
                                            </td>
                                            <td>
                                              <Badge
                                                tone={
                                                  variable.visibility ===
                                                  "public"
                                                    ? "accent"
                                                    : "neutral"
                                                }
                                              >
                                                {variable.visibility}
                                              </Badge>
                                            </td>
                                          </tr>
                                        )}
                                      </For>
                                    </tbody>
                                  </table>
                                </div>
                              </Show>
                              <div class="divider" />
                              <h3 class="panel__title">
                                Add or update variable
                              </h3>
                              <ConfigForm onSubmit={saveVariable} />
                            </div>
                          </Show>
                        </div>
                      </Show>

                      <Show when={view() === "validation"}>
                        <div class="panel">
                          <div class="panel__body stack">
                            <div class="row">
                              <div>
                                <h2 class="panel__title">Project validation</h2>
                                <p class="panel__hint">
                                  Missing keys and placeholder differences
                                  against the default locale.
                                </p>
                              </div>
                              <span class="grow" />
                              <Button onClick={() => void runCheck()}>
                                <IconCheck /> Run check
                              </Button>
                            </div>
                            <Show
                              when={report()}
                              fallback={
                                <EmptyState
                                  icon={<IconCheck size={22} />}
                                  title="Not checked yet"
                                  description="Run validation to detect missing translations and placeholder mismatches."
                                />
                              }
                            >
                              <div class="stat-grid">
                                <div class="stat">
                                  <div class="stat__value">
                                    {report()!.locales.length}
                                  </div>
                                  <div class="stat__label">Locales</div>
                                </div>
                                <div class="stat">
                                  <div class="stat__value">
                                    {report()!.totalKeys}
                                  </div>
                                  <div class="stat__label">Keys</div>
                                </div>
                                <div class="stat">
                                  <div class="stat__value">
                                    {report()!.missing.length}
                                  </div>
                                  <div class="stat__label">Missing</div>
                                </div>
                                <div class="stat">
                                  <div class="stat__value">
                                    {report()!.placeholderMismatches.length}
                                  </div>
                                  <div class="stat__label">
                                    Placeholder issues
                                  </div>
                                </div>
                              </div>
                              <Show
                                when={report()!.defaultLocale}
                                fallback={
                                  <Notice tone="neutral">
                                    No default locale configured; missing-key
                                    checks are skipped.
                                  </Notice>
                                }
                              >
                                <p class="muted">
                                  Source locale:{" "}
                                  <strong>{report()!.defaultLocale}</strong>
                                </p>
                              </Show>
                              <Show when={report()!.missing.length > 0}>
                                <h3 class="panel__title">
                                  Missing translations
                                </h3>
                                <ul class="issue-list">
                                  <For each={report()!.missing}>
                                    {(issue) => (
                                      <li class="issue-item">
                                        <Badge tone="warning">
                                          {issue.localeCode}
                                        </Badge>
                                        <span class="mono">
                                          {issue.namespace}/{issue.key}
                                        </span>
                                      </li>
                                    )}
                                  </For>
                                </ul>
                              </Show>
                              <Show
                                when={
                                  report()!.placeholderMismatches.length > 0
                                }
                              >
                                <h3 class="panel__title">
                                  Placeholder mismatches
                                </h3>
                                <ul class="issue-list">
                                  <For each={report()!.placeholderMismatches}>
                                    {(issue) => (
                                      <li class="issue-item">
                                        <Badge tone="danger">
                                          {issue.localeCode}
                                        </Badge>
                                        <span class="mono">
                                          {issue.namespace}/{issue.key}
                                        </span>
                                        <span class="muted">
                                          expected{" "}
                                          {JSON.stringify(issue.expected)}, got{" "}
                                          {JSON.stringify(issue.actual)}
                                        </span>
                                      </li>
                                    )}
                                  </For>
                                </ul>
                              </Show>
                              <Show
                                when={
                                  !report()!.missing.length &&
                                  !report()!.placeholderMismatches.length
                                }
                              >
                                <Notice tone="success">
                                  No issues found. All locales are complete.
                                </Notice>
                              </Show>
                            </Show>
                          </div>
                        </div>
                      </Show>
                    </div>
                  }
                >
                  <div class="panel">
                    <EmptyState
                      icon={<IconGlobe size={22} />}
                      title="Add your first locale"
                      description="Translations are stored per locale. Start with a default locale such as en-US."
                      action={
                        <div style="min-width:300px;text-align:left">
                          <LocaleForm onSubmit={addLocale} />
                        </div>
                      }
                    />
                  </div>
                </Show>
              </>
            )}
          </Show>
        </main>
      </div>
    </div>
  );
}

function statusBadge(entry: TranslationEntry) {
  if (!entry.status) return <Badge tone="danger">missing</Badge>;
  const tone: "accent" | "success" | "warning" =
    entry.status === "reviewed"
      ? "accent"
      : entry.status === "translated"
        ? "success"
        : "warning";
  return <Badge tone={tone}>{entry.status}</Badge>;
}
