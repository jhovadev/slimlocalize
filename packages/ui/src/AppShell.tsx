import {
  createEffect,
  createMemo,
  createSignal,
  For,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import type {
  CheckReport,
  CollabStatus,
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
  TextareaField,
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
import { AppIcon } from "./AppIcon";
import "./styles/ux03.css";
import "./styles/ux031.css";
import "./styles/ux04.css";
import "./styles/ux05.css";
import { NamespaceTree } from "./NamespaceTree";
import { WebSyncPanel } from "./WebSyncPanel";
import { WebPresenceStrip } from "./WebPresenceStrip";
import { QuickAddKey } from "./QuickAddKey";
import { InlineValue } from "./InlineValue";
import { CompactSelect } from "./CompactSelect";

type View = "translations" | "environments" | "validation" | "settings";
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
  const [treeEntries, setTreeEntries] = createSignal<TranslationEntry[]>([]);
  const [environments, setEnvironments] = createSignal<Environment[]>([]);
  const [environmentId, setEnvironmentId] = createSignal("");
  const [variables, setVariables] = createSignal<ConfigVariable[]>([]);
  const [report, setReport] = createSignal<CheckReport | null>(null);
  const [collabStatus, setCollabStatus] = createSignal<CollabStatus | null>(
    null,
  );
  const [collabPayload, setCollabPayload] = createSignal("");
  const [collabBusy, setCollabBusy] = createSignal(false);
  const [view, setView] = createSignal<View>("translations");
  const [status, setStatus] = createSignal<PageStatus>({
    kind: "loading",
    message: "Opening local workspace…",
  });
  const [notice, setNotice] = createSignal<string | null>(null);
  const [search, setSearch] = createSignal("");
  const [selected, setSelected] = createSignal<TranslationEntry | null>(null);
  const [showQuickAdd, setShowQuickAdd] = createSignal(false);
  const [showCollabPanel, setShowCollabPanel] = createSignal(false);
  const [mobileNavOpen, setMobileNavOpen] = createSignal(false);
  let searchInput: HTMLInputElement | undefined;
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

  createEffect(() => {
    const nextFocus =
      view() === "environments"
        ? `environment:${environment()?.name ?? ""}`
        : view() === "translations" && selected()
          ? `${selected()!.namespace}/${selected()!.key}`
          : null;
    props.service.webSyncSetFocus?.(nextFocus);
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
    const [scoped, all] = await Promise.all([
      props.service.listTranslations(pid, code, ns || null),
      props.service.listTranslations(pid, code, null),
    ]);
    if (projectId() === pid && localeCode() === code) {
      setEntries(scoped);
      setTreeEntries(all);
    }
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
    setShowQuickAdd(false);
    setShowCollabPanel(false);
    setMobileNavOpen(false);
    setEntries([]);
    setTreeEntries([]);
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
      await loadCollabStatus();
      setStatus({ kind: "idle" });
    } catch (cause) {
      fail(cause);
    }
  }

  async function openWorkspace(id: string) {
    setWorkspaceId(id);
    setProjectId(null);
    setMobileNavOpen(false);
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
        setShowProjectForm(false);
        setStatus({ kind: "idle" });
      }
    } catch (cause) {
      fail(cause);
    }
  }

  // Keyboard shortcuts complement visible buttons; they are never required.
  onMount(() => {
    const handleShortcut = (event: KeyboardEvent) => {
      if (
        event.defaultPrevented ||
        showQuickAdd() ||
        !project() ||
        !locales().length
      )
        return;
      if (
        !(event.ctrlKey || event.metaKey) ||
        event.altKey ||
        event.key.toLowerCase() !== "k"
      )
        return;
      event.preventDefault();
      setView("translations");
      if (event.shiftKey) setShowQuickAdd(true);
      else queueMicrotask(() => searchInput?.focus());
    };
    window.addEventListener("keydown", handleShortcut);
    onCleanup(() => window.removeEventListener("keydown", handleShortcut));
  });

  onMount(async () => {
    try {
      const list = await props.service.listWorkspaces();
      setWorkspaces(list);
      const pairedProjectId =
        platform.kind === "web"
          ? localStorage.getItem("slimlocalize.websync.activeProject.v1")
          : null;
      if (pairedProjectId) {
        for (const item of list) {
          const found = (await props.service.listProjects(item.id)).find(
            (candidate) => candidate.id === pairedProjectId,
          );
          if (found) {
            await openWorkspace(item.id);
            await openProject(found.id);
            return;
          }
        }
      }
      if (list[0]) await openWorkspace(list[0].id);
      else {
        setShowWorkspaceForm(true);
        setStatus({ kind: "idle" });
      }
    } catch (cause) {
      fail(cause);
    }
  });

  onMount(() => {
    const unsubscribe = props.service.subscribeChanges?.((changedProjectId) => {
      if (projectId() !== changedProjectId) return;
      void Promise.all([
        props.service.listLocales(changedProjectId),
        props.service.listNamespaces(changedProjectId),
        props.service.listEnvironments(changedProjectId),
      ])
        .then(([nextLocales, nextNamespaces, nextEnvironments]) => {
          setLocales(nextLocales);
          setNamespaces(nextNamespaces);
          setEnvironments(nextEnvironments);
          return Promise.all([
            loadEntries(changedProjectId, localeCode(), namespaceName()),
            loadVariables(),
          ]);
        })
        .catch(fail);
    });
    onCleanup(() => unsubscribe?.());
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
  async function loadCollabStatus() {
    const current = project();
    if (!current || !props.service.collabStatus) return;
    try {
      setCollabStatus(await props.service.collabStatus(current.id));
    } catch (cause) {
      fail(cause);
    }
  }
  async function bootstrapCollab() {
    const current = project();
    if (!current || !props.service.collabBootstrap) return;
    setCollabBusy(true);
    try {
      setCollabStatus(await props.service.collabBootstrap(current.id));
      finish("Collaboration document ready");
      await loadCollabStatus();
    } catch (cause) {
      fail(cause);
    } finally {
      setCollabBusy(false);
    }
  }
  async function exportCollab() {
    const current = project();
    if (!current || !props.service.collabExport) return;
    setCollabBusy(true);
    try {
      setCollabPayload(await props.service.collabExport(current.id));
      finish("Sync envelope exported");
    } catch (cause) {
      fail(cause);
    } finally {
      setCollabBusy(false);
    }
  }
  async function importCollab() {
    const current = project();
    if (!current || !props.service.collabImport || !collabPayload().trim())
      return;
    setCollabBusy(true);
    try {
      setCollabStatus(
        await props.service.collabImport(current.id, collabPayload()),
      );
      await openProject(current.id);
      finish("Sync envelope imported");
    } catch (cause) {
      fail(cause);
    } finally {
      setCollabBusy(false);
    }
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
      <Show when={workspace()}>
        <header class="topbar ux03-topbar">
          <Button
            size="sm"
            variant="ghost"
            class="ux-mobile-menu"
            aria-label="Open navigation"
            aria-expanded={mobileNavOpen()}
            onClick={() => setMobileNavOpen((value) => !value)}
          >
            <AppIcon name="menu" />
          </Button>
          <div class="ux03-breadcrumbs">
            <CompactSelect
              ariaLabel="Choose workspace"
              value={workspaceId() ?? ""}
              options={workspaces().map((item) => ({
                value: item.id,
                label: item.name,
              }))}
              onChange={(id) => void openWorkspace(id)}
            />
            <Button
              size="sm"
              variant="ghost"
              class="ux03-workspace-add"
              aria-label="New workspace"
              title="New workspace"
              onClick={() => setShowWorkspaceForm(true)}
            >
              <AppIcon name="plus" />
            </Button>
            <Show when={project()}>
              <AppIcon name="chevron-right" size={15} class="ux03-separator" />
              <span class="ux03-project-name" title={project()?.name}>
                {project()?.name}
              </span>
            </Show>
          </div>
          <span class="topbar__spacer" />
          <div class="ux03-topbar-actions">
            <Button
              size="sm"
              variant="secondary"
              class="ux03-new-project"
              onClick={() => setShowProjectForm(true)}
            >
              <AppIcon name="folder-plus" /> <span>New project</span>
            </Button>
            <Show
              when={
                project() && locales().length > 0 && view() === "translations"
              }
            >
              <Button size="sm" onClick={() => setShowQuickAdd(true)}>
                <AppIcon name="plus" /> New key
              </Button>
            </Show>
            <Show when={project()}>
              <Button
                size="sm"
                variant="ghost"
                class="ux03-settings-button"
                aria-label="Settings"
                title="Settings"
                onClick={() => setView("settings")}
              >
                <AppIcon name="settings" />
              </Button>
            </Show>
          </div>
        </header>
      </Show>

      <Show when={showQuickAdd() && project() && locales().length > 0}>
        <QuickAddKey
          localeCode={localeCode()}
          namespaces={namespaces()}
          defaultNamespace={namespaceName() || "common"}
          onClose={() => setShowQuickAdd(false)}
          onSave={async (input) => {
            await saveTranslation(input);
            setSelected(null);
          }}
        />
      </Show>

      <Show when={showWorkspaceForm() && workspace()}>
        <div
          class="ux03-dialog-backdrop"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget)
              setShowWorkspaceForm(false);
          }}
        >
          <section
            class="ux03-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="ux03-workspace-title"
            ref={(element) =>
              queueMicrotask(() => element.querySelector("input")?.focus())
            }
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.stopPropagation();
                setShowWorkspaceForm(false);
              }
            }}
          >
            <div class="ux03-dialog-heading">
              <div>
                <h2 id="ux03-workspace-title">New workspace</h2>
                <p>Organize your projects in one place.</p>
              </div>
              <Button
                size="sm"
                variant="ghost"
                aria-label="Close new workspace"
                onClick={() => setShowWorkspaceForm(false)}
              >
                <AppIcon name="x" />
              </Button>
            </div>
            <WorkspaceForm
              onSubmit={createWorkspace}
              onCancel={() => setShowWorkspaceForm(false)}
            />
          </section>
        </div>
      </Show>

      <Show when={showProjectForm() && workspace()}>
        <div
          class="ux03-dialog-backdrop"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) setShowProjectForm(false);
          }}
        >
          <section
            class="ux03-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="ux03-project-title"
            ref={(element) =>
              queueMicrotask(() => element.querySelector("input")?.focus())
            }
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.stopPropagation();
                setShowProjectForm(false);
              }
              if (event.key === "Tab") {
                const focusable = Array.from(
                  event.currentTarget.querySelectorAll<HTMLElement>(
                    "button:not([disabled]), input:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex='-1'])",
                  ),
                );
                const first = focusable[0],
                  last = focusable[focusable.length - 1];
                if (!first || !last) return;
                if (event.shiftKey && document.activeElement === first) {
                  event.preventDefault();
                  last.focus();
                }
                if (!event.shiftKey && document.activeElement === last) {
                  event.preventDefault();
                  first.focus();
                }
              }
            }}
          >
            <div class="ux03-dialog-heading">
              <div>
                <h2 id="ux03-project-title">New project</h2>
                <p>Give your project a name.</p>
              </div>
              <Button
                size="sm"
                variant="ghost"
                aria-label="Close new project"
                onClick={() => setShowProjectForm(false)}
              >
                <AppIcon name="x" />
              </Button>
            </div>
            <ProjectForm
              onSubmit={createProject}
              onCancel={() => setShowProjectForm(false)}
            />
          </section>
        </div>
      </Show>

      <div class="app-body">
        <Show when={workspace()}>
          <Show when={mobileNavOpen()}>
            <button
              class="ux-mobile-scrim"
              aria-label="Close navigation"
              onClick={() => setMobileNavOpen(false)}
            />
          </Show>
          <aside
            class={cx("sidebar", mobileNavOpen() && "sidebar--open")}
            aria-label="Projects and navigation"
          >
            <div class="ux-sidebar-heading">
              <span>Projects</span>
            </div>
            <div class="sidebar__projects">
              <For each={projects()}>
                {(item) => (
                  <button
                    class={cx(
                      "project-button",
                      item.id === projectId() && "project-button--active",
                    )}
                    onClick={() => {
                      setMobileNavOpen(false);
                      void openProject(item.id);
                    }}
                  >
                    <AppIcon name="folder" />
                    <span class="ux-project-name">{item.name}</span>
                  </button>
                )}
              </For>
            </div>
            <Show when={project()}>
              <nav class="ux-sidebar-navigation" aria-label="Project sections">
                <button
                  class={cx(
                    "ux-nav-item",
                    view() === "translations" && "ux-nav-item--active",
                  )}
                  aria-current={view() === "translations" ? "page" : undefined}
                  onClick={() => {
                    setView("translations");
                    setMobileNavOpen(false);
                  }}
                >
                  <AppIcon name="languages" /> Translations
                </button>
                <button
                  class={cx(
                    "ux-nav-item",
                    view() === "environments" && "ux-nav-item--active",
                  )}
                  aria-current={view() === "environments" ? "page" : undefined}
                  onClick={() => {
                    setView("environments");
                    setMobileNavOpen(false);
                  }}
                >
                  <AppIcon name="settings" /> Environments
                </button>
                <button
                  class={cx(
                    "ux-nav-item",
                    view() === "validation" && "ux-nav-item--active",
                  )}
                  aria-current={view() === "validation" ? "page" : undefined}
                  onClick={() => {
                    setView("validation");
                    setMobileNavOpen(false);
                  }}
                >
                  <AppIcon name="check" /> Validation
                </button>
                <button
                  class={cx(
                    "ux-nav-item",
                    view() === "settings" && "ux-nav-item--active",
                  )}
                  aria-current={view() === "settings" ? "page" : undefined}
                  onClick={() => {
                    setView("settings");
                    setMobileNavOpen(false);
                  }}
                >
                  <AppIcon name="sliders-horizontal" /> Settings
                </button>
              </nav>
              <Show
                when={
                  view() === "translations" && project() && locales().length > 0
                }
              >
                <NamespaceTree
                  namespaces={namespaces()}
                  entries={treeEntries()}
                  namespace={namespaceName()}
                  selectedKey={selected()?.key}
                  onSelect={(ns, key) => {
                    selectNamespace(ns);
                    if (key)
                      setSelected(
                        treeEntries().find(
                          (entry) =>
                            entry.namespace === ns && entry.key === key,
                        ) ?? null,
                      );
                    setMobileNavOpen(false);
                  }}
                />
                <Show
                  when={
                    platform.kind === "web" &&
                    project() &&
                    (view() === "translations" || view() === "environments")
                  }
                >
                  <WebPresenceStrip
                    service={props.service}
                    projectId={project()!.id}
                  />
                </Show>
              </Show>
            </Show>
          </aside>
        </Show>

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

          <Show when={!workspace()}>
            <section class="ux-onboarding">
              <h1>Create your workspace</h1>
              <p>Keep your projects and translations organized.</p>
              <div class="ux-onboarding__form">
                <WorkspaceForm onSubmit={createWorkspace} />
              </div>
            </section>
          </Show>

          <Show when={workspace() && !project()}>
            <div class="panel">
              <EmptyState
                icon={<AppIcon name="folder" size={22} />}
                title="Start with a project"
                description="Give your first project a name to get started."
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
                      {locales().length} languages · {entries().length} keys
                    </p>
                  </div>
                  <div class="page-header__actions">
                    <Show when={view() === "translations"}>
                      <Button
                        size="sm"
                        variant="secondary"
                        onClick={() => setShowImport((open) => !open)}
                      >
                        <AppIcon name="upload" /> Import
                      </Button>
                      <Button
                        size="sm"
                        variant="secondary"
                        onClick={() => void exportProject()}
                      >
                        <AppIcon name="download" /> Export
                      </Button>
                    </Show>
                  </div>
                </div>

                <Show when={showImport() && view() === "translations"}>
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

                <Show when={view() === "settings"}>
                  <section class="panel ux-settings">
                    <div class="panel__body stack">
                      <div>
                        <h2 class="panel__title">Settings</h2>
                        <p class="panel__hint">
                          Project preferences and advanced tools.
                        </p>
                      </div>
                      <div class="ux-settings__detail">
                        <h3>Advanced</h3>
                        <p class="muted">{platform.storage}</p>
                        <Show when={props.service.collabStatus}>
                          <Button
                            size="sm"
                            variant="secondary"
                            aria-expanded={showCollabPanel()}
                            onClick={() => setShowCollabPanel((open) => !open)}
                          >
                            {showCollabPanel()
                              ? "Hide collaboration tools"
                              : "Collaboration tools"}
                          </Button>
                        </Show>
                      </div>
                    </div>
                  </section>
                  <Show when={project() && platform.kind === "web"}>
                    <WebSyncPanel
                      service={props.service}
                      project={project()!}
                    />
                  </Show>
                  <Show when={props.service.collabStatus && showCollabPanel()}>
                    <div class="panel" style="margin-bottom:16px">
                      <div class="panel__header">
                        <h2 class="panel__title">Local collaboration</h2>
                        <Show when={collabStatus()}>
                          <Badge tone="accent">
                            peer {collabStatus()!.peerId}
                          </Badge>
                        </Show>
                      </div>
                      <div class="panel__body stack">
                        <Show
                          when={collabStatus()}
                          fallback={
                            <p class="muted">
                              No collaboration document has been initialized for
                              this project.
                            </p>
                          }
                        >
                          <div class="row">
                            <span class="muted">
                              Document:{" "}
                              <strong>{collabStatus()!.documentKey}</strong>
                            </span>
                            <span class="muted">
                              Updates: {collabStatus()!.updateCount}
                            </span>
                            <span class="muted">
                              Pending:{" "}
                              {collabStatus()!.hasPending ? "yes" : "no"}
                            </span>
                            <span class="muted">
                              Version: {collabStatus()!.versionVector.length}{" "}
                              bytes
                            </span>
                          </div>
                        </Show>
                        <div class="row">
                          <Button
                            size="sm"
                            variant="secondary"
                            loading={collabBusy()}
                            onClick={() => void bootstrapCollab()}
                          >
                            Bootstrap document
                          </Button>
                          <Button
                            size="sm"
                            variant="secondary"
                            loading={collabBusy()}
                            onClick={() => void exportCollab()}
                          >
                            Export update
                          </Button>
                          <Button
                            size="sm"
                            variant="ghost"
                            loading={collabBusy()}
                            onClick={() =>
                              void (
                                project() &&
                                props.service.collabRebuildProjection?.(
                                  project()!.id,
                                )
                              )
                            }
                          >
                            Rebuild projection
                          </Button>
                          <Button
                            size="sm"
                            variant="ghost"
                            loading={collabBusy()}
                            onClick={() =>
                              void (
                                project() &&
                                props.service
                                  .collabValidate?.(project()!.id)
                                  .then(setReport)
                              )
                            }
                          >
                            Validate
                          </Button>
                        </div>
                        <TextareaField
                          label="Sync envelope (JSON)"
                          value={collabPayload()}
                          onInput={setCollabPayload}
                          rows={6}
                          hint="Export an envelope, share the file, then paste or load it here and import."
                        />
                        <div class="form-actions">
                          <Button
                            loading={collabBusy()}
                            onClick={() => void importCollab()}
                          >
                            Import update
                          </Button>
                        </div>
                      </div>
                    </div>
                  </Show>
                </Show>
                <Show when={view() !== "settings"}>
                  <Show
                    when={locales().length === 0}
                    fallback={
                      <div class="stack">
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
                                ref={searchInput}
                                class="input search-input"
                                type="search"
                                placeholder="Search keys or values… · Ctrl/⌘ K"
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
                                onClick={() =>
                                  setShowLocaleForm((open) => !open)
                                }
                              >
                                <AppIcon name="plus" /> Locale
                              </Button>
                              <Button
                                size="sm"
                                variant="ghost"
                                onClick={() =>
                                  setShowNamespaceForm((open) => !open)
                                }
                              >
                                <AppIcon name="plus" /> Namespace
                              </Button>
                            </div>

                            <Show
                              when={showLocaleForm() || showNamespaceForm()}
                            >
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

                            <div
                              class={cx(
                                "translation-layout",
                                selected() && "translation-layout--details",
                              )}
                            >
                              <div class="table-wrap">
                                <Show
                                  when={filtered().length > 0}
                                  fallback={
                                    <EmptyState
                                      icon={<AppIcon name="search" size={22} />}
                                      title={
                                        search().trim()
                                          ? "No matching keys"
                                          : "No translation keys yet"
                                      }
                                      description={
                                        search().trim()
                                          ? "Try another search or clear the filter."
                                          : "Create a key in seconds, or import an existing JSON locale file."
                                      }
                                      action={
                                        <div class="row ux-empty-actions">
                                          <Button
                                            size="sm"
                                            onClick={() =>
                                              search().trim()
                                                ? setSearch("")
                                                : setShowQuickAdd(true)
                                            }
                                          >
                                            <AppIcon name="plus" />{" "}
                                            {search().trim()
                                              ? "Clear search"
                                              : "Create key"}
                                          </Button>
                                          <Show when={!search().trim()}>
                                            <Button
                                              size="sm"
                                              variant="secondary"
                                              onClick={() =>
                                                setShowImport(true)
                                              }
                                            >
                                              <AppIcon name="upload" /> Import
                                              JSON
                                            </Button>
                                          </Show>
                                        </div>
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
                                                selected()?.keyId ===
                                                entry.keyId,
                                            }}
                                            onClick={() => setSelected(entry)}
                                            tabIndex={0}
                                            aria-selected={
                                              selected()?.keyId === entry.keyId
                                            }
                                            onKeyDown={(event) => {
                                              if (
                                                event.key === "Enter" ||
                                                event.key === " "
                                              ) {
                                                event.preventDefault();
                                                setSelected(entry);
                                              }
                                            }}
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
                                              <InlineValue
                                                entry={entry}
                                                onSave={async (input) => {
                                                  await saveTranslation(input);
                                                  setSelected(null);
                                                }}
                                              />
                                            </td>
                                            <td>{statusBadge(entry)}</td>
                                          </tr>
                                        )}
                                      </For>
                                    </tbody>
                                  </table>
                                </Show>
                              </div>

                              <Show when={selected()}>
                                <div class="editor">
                                  <div class="editor__header row">
                                    <div>
                                      <div class="panel__title">
                                        {selected()
                                          ? "Selected key"
                                          : "New key"}
                                      </div>
                                      <div class="muted mono">
                                        {localeCode() || "no locale"}
                                      </div>
                                    </div>
                                    <span class="grow" />
                                    <Button
                                      size="sm"
                                      variant="ghost"
                                      aria-label="Close translation details"
                                      onClick={() => setSelected(null)}
                                    >
                                      Close
                                    </Button>
                                    <Show when={selected()}>
                                      <span class="ux-inline-meta">
                                        {selected()!.namespace}
                                      </span>
                                    </Show>
                                  </div>
                                  <div class="editor__body">
                                    <TranslationEditor
                                      entry={selected()}
                                      localeCode={localeCode()}
                                      namespaces={namespaces()}
                                      defaultNamespace={
                                        namespaceName() || "common"
                                      }
                                      onSave={saveTranslation}
                                      onNew={() => setSelected(null)}
                                    />
                                  </div>
                                </div>
                              </Show>
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
                                <AppIcon name="plus" /> Environment
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
                                  <span class="ux-inline-meta">
                                    {environment()!.kind}
                                  </span>
                                  <span class="grow" />
                                </div>
                                <Show
                                  when={variables().length > 0}
                                  fallback={
                                    <EmptyState
                                      icon={
                                        <AppIcon name="database" size={22} />
                                      }
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
                                              <td class="mono">
                                                {variable.key}
                                              </td>
                                              <td class="value-cell">
                                                {variable.value}
                                              </td>
                                              <td>
                                                <span class="ux-inline-meta">
                                                  {variable.visibility}
                                                </span>
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
                                  <h2 class="panel__title">
                                    Project validation
                                  </h2>
                                  <p class="panel__hint">
                                    Missing keys and placeholder differences
                                    against the default locale.
                                  </p>
                                </div>
                                <span class="grow" />
                                <Button onClick={() => void runCheck()}>
                                  <AppIcon name="check" /> Run check
                                </Button>
                              </div>
                              <Show
                                when={report()}
                                fallback={
                                  <EmptyState
                                    icon={<AppIcon name="check" size={22} />}
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
                                          <span class="ux-inline-meta">
                                            {issue.localeCode}
                                          </span>
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
                                          <span class="ux-inline-meta">
                                            {issue.localeCode}
                                          </span>
                                          <span class="mono">
                                            {issue.namespace}/{issue.key}
                                          </span>
                                          <span class="muted">
                                            expected{" "}
                                            {JSON.stringify(issue.expected)},
                                            got {JSON.stringify(issue.actual)}
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
                        icon={<AppIcon name="globe" size={22} />}
                        title="Choose your first language"
                        description="Pick the language you usually write in."
                        action={
                          <div style="min-width:300px;text-align:left">
                            <LocaleForm onSubmit={addLocale} />
                          </div>
                        }
                      />
                    </div>
                  </Show>
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
  const name = entry.status ?? "missing";
  return (
    <span class={cx("ux-translation-status", `ux-translation-status--${name}`)}>
      <span class="ux-translation-status__dot" aria-hidden="true" />
      {name}
    </span>
  );
}
