import type {
  AddLocaleInput,
  AppInfo,
  CheckReport,
  ConfigVariable,
  CreateEnvironmentInput,
  CreateProjectInput,
  CreateWorkspaceInput,
  Environment,
  EnvironmentKind,
  ExportOptions,
  ExportFileResult,
  ExportResult,
  ImportFileInput,
  ImportResult,
  Locale,
  MissingTranslation,
  Namespace,
  PlaceholderMismatch,
  PlatformInfo,
  Project,
  SaveTranslationInput,
  SetConfigVariableInput,
  TranslationEntry,
  TranslationStatus,
  Workspace,
  WorkspaceService,
} from "./types";
import { WorkspaceServiceError } from "./types";

const STORAGE_KEY = "slimlocalize.web.v1";
const DEFAULT_NAMESPACE = "common";
const MAX_NAME = 120;
const MAX_KEY = 256;
const MAX_VALUE = 100_000;

export const WEB_PLATFORM: PlatformInfo = {
  kind: "web",
  label: "Web",
  storage: "Browser localStorage",
  localFirst: true,
  capabilities: {
    importFromPath: false,
    importFromFiles: true,
    downloadExports: true,
    nativeDatabase: false,
    sync: false,
  },
};

interface StoredKey {
  id: string;
  projectId: string;
  namespaceId: string;
  key: string;
  description: string | null;
  createdAt: string;
  updatedAt: string;
}

interface StoredValue {
  id: string;
  keyId: string;
  localeId: string;
  value: string;
  status: TranslationStatus;
  updatedAt: string;
}

interface WebState {
  schemaVersion: 1;
  workspaces: Workspace[];
  projects: Project[];
  locales: Locale[];
  namespaces: Namespace[];
  keys: StoredKey[];
  values: StoredValue[];
  environments: Environment[];
  configVariables: ConfigVariable[];
}

const emptyState = (): WebState => ({
  schemaVersion: 1,
  workspaces: [],
  projects: [],
  locales: [],
  namespaces: [],
  keys: [],
  values: [],
  environments: [],
  configVariables: [],
});

function webStorage(): Storage {
  if (typeof globalThis.localStorage === "undefined") {
    throw new WorkspaceServiceError(
      "storage_unavailable",
      "Browser localStorage is unavailable. Use the desktop app for native SQLite persistence.",
    );
  }
  return globalThis.localStorage;
}

function loadState(): WebState {
  const raw = webStorage().getItem(STORAGE_KEY);
  if (!raw) {
    return emptyState();
  }
  try {
    const parsed = JSON.parse(raw) as Partial<WebState>;
    if (parsed.schemaVersion !== 1) {
      throw new Error("unsupported web storage schema");
    }
    return {
      schemaVersion: 1,
      workspaces: parsed.workspaces ?? [],
      projects: parsed.projects ?? [],
      locales: parsed.locales ?? [],
      namespaces: parsed.namespaces ?? [],
      keys: parsed.keys ?? [],
      values: parsed.values ?? [],
      environments: parsed.environments ?? [],
      configVariables: parsed.configVariables ?? [],
    };
  } catch (error) {
    throw new WorkspaceServiceError(
      "storage_corrupt",
      `The local web workspace could not be read: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
}

function saveState(state: WebState): void {
  webStorage().setItem(STORAGE_KEY, JSON.stringify(state));
}

function newId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (char) => {
    const random = (Math.random() * 16) | 0;
    const value = char === "x" ? random : (random & 0x3) | 0x8;
    return value.toString(16);
  });
}

function now(): string {
  return new Date().toISOString();
}

function fail(code: string, message: string): never {
  throw new WorkspaceServiceError(code, message);
}

export function slugify(input: string): string {
  const mapped = input
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();
  const slug = mapped
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .replace(/-{2,}/g, "-");
  return slug || "item";
}

export function normalizeLocaleCode(input: string): string {
  const trimmed = input.trim();
  if (!trimmed || trimmed.length > 35) {
    fail(
      "validation_error",
      "Locale code must contain between 1 and 35 characters.",
    );
  }
  if (!/^[A-Za-z0-9_-]+$/.test(trimmed)) {
    fail(
      "validation_error",
      "Locale code may only contain letters, digits, dashes and underscores.",
    );
  }
  const parts = trimmed.replace(/_/g, "-").split("-");
  if (parts.length === 0 || parts[0].length < 2 || parts[0].length > 3) {
    fail(
      "validation_error",
      "The language subtag must have 2 or 3 characters (for example en or es-PE).",
    );
  }
  if (!/^[A-Za-z]+$/.test(parts[0])) {
    fail("validation_error", "The language subtag must be alphabetic.");
  }
  parts.forEach((part, index) => {
    if (!part || !/^[A-Za-z0-9]+$/.test(part)) {
      fail("validation_error", "Locale subtags must be alphanumeric.");
    }
    const validLength =
      index === 0
        ? part.length === 2 || part.length === 3
        : part.length === 4
          ? /^[A-Za-z]+$/.test(part)
          : part.length === 2
            ? /^[A-Za-z]+$/.test(part)
            : part.length === 3
              ? /^\d+$/.test(part)
              : part.length >= 5 && part.length <= 8;
    if (!validLength) {
      fail("validation_error", `Invalid locale subtag "${part}".`);
    }
  });
  return parts
    .map((part, index) => {
      if (index === 0) return part.toLowerCase();
      if (part.length === 4) {
        return part.charAt(0).toUpperCase() + part.slice(1).toLowerCase();
      }
      if (part.length === 2) return part.toUpperCase();
      return part.toLowerCase();
    })
    .join("-");
}

function validateName(field: string, value: string): string {
  const trimmed = value.trim();
  if (!trimmed) fail("validation_error", `${field} must not be empty.`);
  if (trimmed.length > MAX_NAME)
    fail("validation_error", `${field} is too long.`);
  if (/[\u0000-\u001f\u007f]/.test(trimmed)) {
    fail("validation_error", `${field} must not contain control characters.`);
  }
  return trimmed;
}

function validateSlug(value: string): void {
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(value) || value.length > 64) {
    fail(
      "validation_error",
      "Slug must use lowercase letters, digits and single dashes (max 64).",
    );
  }
}

function validateNamespace(value: string): void {
  if (
    !value ||
    value.length > 64 ||
    !/^[A-Za-z0-9_.-]+$/.test(value) ||
    value.startsWith(".") ||
    value.endsWith(".") ||
    value.includes("..")
  ) {
    fail(
      "validation_error",
      "Namespace must be 1-64 characters without path separators.",
    );
  }
}

function validateKey(value: string): string {
  const trimmed = value.trim();
  if (!trimmed || trimmed.length > MAX_KEY) {
    fail(
      "validation_error",
      "Translation key must contain between 1 and 256 characters.",
    );
  }
  if (/[\u0000-\u001f\u007f]/.test(trimmed)) {
    fail(
      "validation_error",
      "Translation key must not contain control characters.",
    );
  }
  if (
    trimmed.startsWith(".") ||
    trimmed.endsWith(".") ||
    trimmed.includes("..")
  ) {
    fail(
      "validation_error",
      "Translation key must not start/end with a dot or contain consecutive dots.",
    );
  }
  return trimmed;
}

function validateValue(value: string): void {
  if (value.length > MAX_VALUE) fail("validation_error", "Value is too long.");
  if (value.includes("\u0000"))
    fail("validation_error", "Value must not contain NUL.");
}

function validateConfigKey(value: string): string {
  const trimmed = value.trim();
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(trimmed) || trimmed.length > 128) {
    fail(
      "validation_error",
      "Config key must start with a letter/underscore and be 1-128 characters.",
    );
  }
  return trimmed;
}

function workspaceById(state: WebState, id: string): Workspace {
  const workspace = state.workspaces.find((item) => item.id === id);
  if (!workspace) fail("not_found", `Workspace not found: ${id}`);
  return workspace;
}

function projectById(state: WebState, id: string): Project {
  const project = state.projects.find((item) => item.id === id);
  if (!project) fail("not_found", `Project not found: ${id}`);
  return project;
}

function localeByCode(
  state: WebState,
  projectId: string,
  code: string,
): Locale {
  const locale = state.locales.find(
    (item) =>
      item.projectId === projectId && item.code === normalizeLocaleCode(code),
  );
  if (!locale) fail("not_found", `Locale not found in project: ${code}`);
  return locale;
}

function environmentById(state: WebState, id: string): Environment {
  const environment = state.environments.find((item) => item.id === id);
  if (!environment) fail("not_found", `Environment not found: ${id}`);
  return environment;
}

function ensureNamespace(
  state: WebState,
  projectId: string,
  name: string,
): Namespace {
  validateNamespace(name);
  const existing = state.namespaces.find(
    (item) => item.projectId === projectId && item.name === name,
  );
  if (existing) return existing;
  const namespace: Namespace = {
    id: newId(),
    projectId,
    name,
    createdAt: now(),
  };
  state.namespaces.push(namespace);
  return namespace;
}

function ensureKey(
  state: WebState,
  projectId: string,
  namespaceId: string,
  key: string,
  description: string | null,
): { storedKey: StoredKey; created: boolean } {
  const normalized = validateKey(key);
  const existing = state.keys.find(
    (item) => item.namespaceId === namespaceId && item.key === normalized,
  );
  if (existing) {
    if (description !== null) {
      existing.description = description;
      existing.updatedAt = now();
    }
    return { storedKey: existing, created: false };
  }
  const timestamp = now();
  const storedKey: StoredKey = {
    id: newId(),
    projectId,
    namespaceId,
    key: normalized,
    description,
    createdAt: timestamp,
    updatedAt: timestamp,
  };
  state.keys.push(storedKey);
  return { storedKey, created: true };
}

function translationEntry(
  state: WebState,
  storedKey: StoredKey,
  locale: Locale,
): TranslationEntry {
  const namespace = state.namespaces.find(
    (item) => item.id === storedKey.namespaceId,
  );
  const value = state.values.find(
    (item) => item.keyId === storedKey.id && item.localeId === locale.id,
  );
  return {
    keyId: storedKey.id,
    key: storedKey.key,
    namespace: namespace?.name ?? DEFAULT_NAMESPACE,
    description: storedKey.description,
    localeCode: locale.code,
    value: value?.value ?? null,
    status: value?.status ?? null,
    updatedAt: value?.updatedAt ?? null,
  };
}

export function createWebWorkspaceService(): WorkspaceService {
  return {
    platform: WEB_PLATFORM,

    async getInfo(): Promise<AppInfo> {
      return {
        name: "SlimLocalize",
        version: "0.1.0-web",
        platform: "web",
        storage: "localStorage",
        databasePath: `localStorage://${STORAGE_KEY}`,
        localFirst: true,
      };
    },

    async listWorkspaces(): Promise<Workspace[]> {
      return loadState()
        .workspaces.slice()
        .sort((a, b) => a.name.localeCompare(b.name));
    },

    async createWorkspace(input: CreateWorkspaceInput): Promise<Workspace> {
      const state = loadState();
      const name = validateName("Workspace name", input.name);
      const slug = (input.slug?.trim() || slugify(name)).toLowerCase();
      validateSlug(slug);
      if (state.workspaces.some((item) => item.slug === slug)) {
        fail("conflict", `Workspace slug already exists: ${slug}`);
      }
      const timestamp = now();
      const workspace: Workspace = {
        id: newId(),
        slug,
        name,
        description: input.description?.trim() || null,
        createdAt: timestamp,
        updatedAt: timestamp,
      };
      state.workspaces.push(workspace);
      saveState(state);
      return workspace;
    },

    async listProjects(workspaceId: string): Promise<Project[]> {
      const state = loadState();
      workspaceById(state, workspaceId);
      return state.projects
        .filter((item) => item.workspaceId === workspaceId)
        .sort((a, b) => a.name.localeCompare(b.name));
    },

    async createProject(
      workspaceId: string,
      input: CreateProjectInput,
    ): Promise<Project> {
      const state = loadState();
      workspaceById(state, workspaceId);
      const name = validateName("Project name", input.name);
      const slug = (input.slug?.trim() || slugify(name)).toLowerCase();
      validateSlug(slug);
      if (
        state.projects.some(
          (item) => item.workspaceId === workspaceId && item.slug === slug,
        )
      ) {
        fail("conflict", `Project slug already exists in workspace: ${slug}`);
      }
      const timestamp = now();
      const defaultLocale = input.defaultLocale
        ? normalizeLocaleCode(input.defaultLocale)
        : null;
      const project: Project = {
        id: newId(),
        workspaceId,
        slug,
        name,
        description: input.description?.trim() || null,
        defaultLocale,
        createdAt: timestamp,
        updatedAt: timestamp,
      };
      state.projects.push(project);
      state.namespaces.push({
        id: newId(),
        projectId: project.id,
        name: DEFAULT_NAMESPACE,
        createdAt: timestamp,
      });
      if (defaultLocale) {
        state.locales.push({
          id: newId(),
          projectId: project.id,
          code: defaultLocale,
          name: defaultLocale,
          isDefault: true,
          createdAt: timestamp,
        });
      }
      const kinds: EnvironmentKind[] = ["development", "staging", "production"];
      for (const name of kinds) {
        state.environments.push({
          id: newId(),
          projectId: project.id,
          name,
          kind: name,
          createdAt: timestamp,
        });
      }
      saveState(state);
      return project;
    },

    async listLocales(projectId: string): Promise<Locale[]> {
      const state = loadState();
      projectById(state, projectId);
      return state.locales
        .filter((item) => item.projectId === projectId)
        .sort((a, b) => a.code.localeCompare(b.code));
    },

    async addLocale(projectId: string, input: AddLocaleInput): Promise<Locale> {
      const state = loadState();
      const project = projectById(state, projectId);
      const code = normalizeLocaleCode(input.code);
      if (
        state.locales.some(
          (item) => item.projectId === projectId && item.code === code,
        )
      ) {
        fail("conflict", `Locale already exists in project: ${code}`);
      }
      const existing = state.locales.filter(
        (item) => item.projectId === projectId,
      );
      const isDefault = Boolean(input.isDefault) || existing.length === 0;
      const name = input.name?.trim() || code;
      validateName("Locale name", name);
      if (isDefault) {
        for (const locale of state.locales) {
          if (locale.projectId === projectId) locale.isDefault = false;
        }
      }
      const locale: Locale = {
        id: newId(),
        projectId,
        code,
        name,
        isDefault,
        createdAt: now(),
      };
      state.locales.push(locale);
      project.defaultLocale = isDefault
        ? code
        : (project.defaultLocale ??
          state.locales.find(
            (item) => item.projectId === projectId && item.isDefault,
          )?.code ??
          null);
      project.updatedAt = now();
      saveState(state);
      return locale;
    },

    async setDefaultLocale(projectId: string, code: string): Promise<Locale> {
      const state = loadState();
      const project = projectById(state, projectId);
      const locale = localeByCode(state, projectId, code);
      for (const item of state.locales) {
        if (item.projectId === projectId)
          item.isDefault = item.id === locale.id;
      }
      locale.isDefault = true;
      project.defaultLocale = locale.code;
      project.updatedAt = now();
      saveState(state);
      return locale;
    },

    async listNamespaces(projectId: string): Promise<Namespace[]> {
      const state = loadState();
      projectById(state, projectId);
      return state.namespaces
        .filter((item) => item.projectId === projectId)
        .sort((a, b) => a.name.localeCompare(b.name));
    },

    async createNamespace(projectId: string, name: string): Promise<Namespace> {
      const state = loadState();
      projectById(state, projectId);
      const normalized = name.trim();
      validateNamespace(normalized);
      if (
        state.namespaces.some(
          (item) => item.projectId === projectId && item.name === normalized,
        )
      ) {
        fail("conflict", `Namespace already exists: ${normalized}`);
      }
      const namespace: Namespace = {
        id: newId(),
        projectId,
        name: normalized,
        createdAt: now(),
      };
      state.namespaces.push(namespace);
      saveState(state);
      return namespace;
    },

    async listTranslations(
      projectId: string,
      localeCode: string,
      namespace?: string | null,
    ): Promise<TranslationEntry[]> {
      const state = loadState();
      projectById(state, projectId);
      const locale = localeByCode(state, projectId, localeCode);
      if (namespace) validateNamespace(namespace);
      const namespaceIds = new Set(
        state.namespaces
          .filter(
            (item) =>
              item.projectId === projectId &&
              (!namespace || item.name === namespace),
          )
          .map((item) => item.id),
      );
      return state.keys
        .filter(
          (item) =>
            item.projectId === projectId && namespaceIds.has(item.namespaceId),
        )
        .sort((a, b) => {
          const left =
            state.namespaces.find((item) => item.id === a.namespaceId)?.name ??
            "";
          const right =
            state.namespaces.find((item) => item.id === b.namespaceId)?.name ??
            "";
          return left.localeCompare(right) || a.key.localeCompare(b.key);
        })
        .map((item) => translationEntry(state, item, locale));
    },

    async saveTranslation(
      input: SaveTranslationInput,
    ): Promise<TranslationEntry> {
      const state = loadState();
      projectById(state, input.projectId);
      const locale = localeByCode(state, input.projectId, input.localeCode);
      validateNamespace(input.namespace);
      validateValue(input.value);
      const namespace = ensureNamespace(
        state,
        input.projectId,
        input.namespace,
      );
      const description = input.description?.trim() || null;
      const { storedKey } = ensureKey(
        state,
        input.projectId,
        namespace.id,
        input.key,
        description,
      );
      const timestamp = now();
      let value = state.values.find(
        (item) => item.keyId === storedKey.id && item.localeId === locale.id,
      );
      if (value) {
        value.value = input.value;
        value.status = input.status;
        value.updatedAt = timestamp;
      } else {
        value = {
          id: newId(),
          keyId: storedKey.id,
          localeId: locale.id,
          value: input.value,
          status: input.status,
          updatedAt: timestamp,
        };
        state.values.push(value);
      }
      storedKey.updatedAt = timestamp;
      saveState(state);
      return translationEntry(state, storedKey, locale);
    },

    async listEnvironments(projectId: string): Promise<Environment[]> {
      const state = loadState();
      projectById(state, projectId);
      return state.environments
        .filter((item) => item.projectId === projectId)
        .sort((a, b) => a.name.localeCompare(b.name));
    },

    async createEnvironment(
      projectId: string,
      input: CreateEnvironmentInput,
    ): Promise<Environment> {
      const state = loadState();
      projectById(state, projectId);
      const name = input.name.trim();
      validateNamespace(name);
      if (
        state.environments.some(
          (item) => item.projectId === projectId && item.name === name,
        )
      ) {
        fail("conflict", `Environment already exists: ${name}`);
      }
      const environment: Environment = {
        id: newId(),
        projectId,
        name,
        kind: input.kind,
        createdAt: now(),
      };
      state.environments.push(environment);
      saveState(state);
      return environment;
    },

    async listConfigVariables(
      environmentId: string,
    ): Promise<ConfigVariable[]> {
      const state = loadState();
      environmentById(state, environmentId);
      return state.configVariables
        .filter((item) => item.environmentId === environmentId)
        .sort((a, b) => a.key.localeCompare(b.key));
    },

    async setConfigVariable(
      input: SetConfigVariableInput,
    ): Promise<ConfigVariable> {
      const state = loadState();
      environmentById(state, input.environmentId);
      const key = validateConfigKey(input.key);
      validateValue(input.value);
      const timestamp = now();
      const existing = state.configVariables.find(
        (item) =>
          item.environmentId === input.environmentId && item.key === key,
      );
      if (existing) {
        existing.value = input.value;
        existing.visibility = input.visibility;
        existing.updatedAt = timestamp;
        saveState(state);
        return existing;
      }
      const variable: ConfigVariable = {
        id: newId(),
        environmentId: input.environmentId,
        key,
        value: input.value,
        visibility: input.visibility,
        createdAt: timestamp,
        updatedAt: timestamp,
      };
      state.configVariables.push(variable);
      saveState(state);
      return variable;
    },

    async checkProject(projectId: string): Promise<CheckReport> {
      const state = loadState();
      const project = projectById(state, projectId);
      const locales = state.locales
        .filter((item) => item.projectId === projectId)
        .sort((a, b) => a.code.localeCompare(b.code));
      const keys = state.keys
        .filter((item) => item.projectId === projectId)
        .sort((a, b) => {
          const left =
            state.namespaces.find((item) => item.id === a.namespaceId)?.name ??
            "";
          const right =
            state.namespaces.find((item) => item.id === b.namespaceId)?.name ??
            "";
          return left.localeCompare(right) || a.key.localeCompare(b.key);
        });
      const defaultLocale =
        project.defaultLocale ??
        locales.find((item) => item.isDefault)?.code ??
        null;
      const report: CheckReport = {
        projectId,
        defaultLocale,
        locales: locales.map((item) => item.code),
        totalKeys: keys.length,
        checkedValues: 0,
        missing: [],
        placeholderMismatches: [],
      };
      if (!defaultLocale) return report;

      const valueFor = (
        key: StoredKey,
        locale: Locale,
      ): StoredValue | undefined =>
        state.values.find(
          (item) => item.keyId === key.id && item.localeId === locale.id,
        );

      const defaultLocaleRecord = locales.find(
        (item) => item.code === defaultLocale,
      );
      for (const locale of locales) {
        for (const key of keys) {
          const namespaceName =
            state.namespaces.find((item) => item.id === key.namespaceId)
              ?.name ?? DEFAULT_NAMESPACE;
          const value = valueFor(key, locale);
          if (value && value.value.trim()) {
            report.checkedValues += 1;
            if (locale.code === defaultLocale) continue;
            const source = defaultLocaleRecord
              ? valueFor(key, defaultLocaleRecord)
              : undefined;
            if (source && source.value.trim()) {
              const expected = extractPlaceholders(source.value);
              const actual = extractPlaceholders(value.value);
              if (
                expected.length !== actual.length ||
                expected.some((token, index) => token !== actual[index])
              ) {
                const mismatch: PlaceholderMismatch = {
                  localeCode: locale.code,
                  sourceLocale: defaultLocale,
                  namespace: namespaceName,
                  key: key.key,
                  expected,
                  actual,
                };
                report.placeholderMismatches.push(mismatch);
              }
            }
          } else if (locale.code !== defaultLocale) {
            const missing: MissingTranslation = {
              localeCode: locale.code,
              namespace: namespaceName,
              key: key.key,
            };
            report.missing.push(missing);
          }
        }
      }
      return report;
    },

    async exportProject(
      projectId: string,
      options: ExportOptions = {},
    ): Promise<ExportResult> {
      const state = loadState();
      const project = projectById(state, projectId);
      if (options.namespace) validateNamespace(options.namespace);
      const locales = state.locales
        .filter((item) => item.projectId === projectId)
        .sort((a, b) => a.code.localeCompare(b.code));
      const keys = state.keys.filter((item) => item.projectId === projectId);
      const contents: Record<string, string> = {};
      const files: ExportFileResult[] = [];
      let totalKeys = 0;
      for (const locale of locales) {
        const valuesByKey = new Map<string, StoredValue>();
        for (const value of state.values) {
          if (value.localeId === locale.id) valuesByKey.set(value.keyId, value);
        }
        const selected: Array<{
          key: string;
          namespace: string;
          value: string;
        }> = [];
        for (const key of keys) {
          const namespaceName =
            state.namespaces.find((item) => item.id === key.namespaceId)
              ?.name ?? DEFAULT_NAMESPACE;
          if (options.namespace && namespaceName !== options.namespace)
            continue;
          const value = valuesByKey.get(key.id);
          if (value) {
            selected.push({
              key: key.key,
              namespace: namespaceName,
              value: value.value,
            });
          }
        }
        selected.sort((a, b) => a.key.localeCompare(b.key));
        const origins = new Map<string, string>();
        for (const item of selected) {
          const origin = origins.get(item.key);
          if (origin !== undefined && origin !== item.namespace) {
            fail(
              "conflict",
              `Key "${item.key}" exists in namespaces "${origin}" and "${item.namespace}"; use a namespace filter.`,
            );
          }
          origins.set(item.key, item.namespace);
        }
        const flat: Record<string, string> = {};
        for (const item of selected) flat[item.key] = item.value;
        const json = options.nested ? nestedObject(flat) : flat;
        const content = `${JSON.stringify(sortObject(json), null, 2)}\n`;
        const fileName = `${locale.code}.json`;
        contents[fileName] = content;
        files.push({
          localeCode: locale.code,
          relativePath: fileName,
          keyCount: selected.length,
        });
        totalKeys += selected.length;
      }
      if (options.download !== false) {
        for (const [fileName, content] of Object.entries(contents)) {
          downloadFile(fileName, content);
        }
      }
      return {
        projectId,
        outputDir: `browser://${project.slug}`,
        files,
        totalKeys,
        mode: "download",
        contents,
      };
    },

    async importFromFiles(
      projectId: string,
      files: ImportFileInput[],
      namespaceName: string | null = DEFAULT_NAMESPACE,
    ): Promise<ImportResult> {
      const state = loadState();
      const project = projectById(state, projectId);
      const namespaceValue = namespaceName ?? DEFAULT_NAMESPACE;
      validateNamespace(namespaceValue);
      if (files.length === 0)
        fail("validation_error", "No JSON files were selected.");
      if (files.length > 500)
        fail("limit_exceeded", "At most 500 files can be imported at once.");

      const parsed: Array<{ localeCode: string; key: string; value: string }> =
        [];
      const seen = new Map<string, Set<string>>();
      for (const file of files) {
        const stem = file.fileName.replace(/\.json$/i, "");
        if (stem === file.fileName) {
          fail(
            "validation_error",
            `Import file must use the .json extension: ${file.fileName}`,
          );
        }
        const localeCode = normalizeLocaleCode(stem);
        let json: unknown;
        try {
          json = JSON.parse(file.content);
        } catch (error) {
          fail(
            "invalid_json",
            `Invalid JSON in ${file.fileName}: ${error instanceof Error ? error.message : String(error)}`,
          );
        }
        if (!json || typeof json !== "object" || Array.isArray(json)) {
          fail(
            "validation_error",
            `${file.fileName} must contain a JSON object.`,
          );
        }
        const flat = new Map<string, string>();
        flattenObject(json as Record<string, unknown>, "", flat);
        const localeSeen = seen.get(localeCode) ?? new Set<string>();
        for (const [key, value] of flat) {
          const normalized = validateKey(key);
          if (localeSeen.has(normalized)) {
            fail(
              "conflict",
              `Duplicate key "${normalized}" for locale ${localeCode}.`,
            );
          }
          localeSeen.add(normalized);
          parsed.push({ localeCode, key: normalized, value });
        }
        seen.set(localeCode, localeSeen);
      }

      parsed.sort(
        (a, b) =>
          a.localeCode.localeCompare(b.localeCode) ||
          a.key.localeCompare(b.key),
      );
      const namespace = ensureNamespace(state, projectId, namespaceValue);
      let localesCreated = 0;
      let keysCreated = 0;
      let valuesWritten = 0;
      const localeIds = new Map<string, Locale>();
      for (const entry of parsed) {
        if (!localeIds.has(entry.localeCode)) {
          let locale = state.locales.find(
            (item) =>
              item.projectId === projectId && item.code === entry.localeCode,
          );
          if (!locale) {
            const existingCount = state.locales.filter(
              (item) => item.projectId === projectId,
            ).length;
            locale = {
              id: newId(),
              projectId,
              code: entry.localeCode,
              name: entry.localeCode,
              isDefault: existingCount === 0,
              createdAt: now(),
            };
            state.locales.push(locale);
            if (locale.isDefault) project.defaultLocale = locale.code;
            localesCreated += 1;
          }
          localeIds.set(entry.localeCode, locale);
        }
      }
      for (const entry of parsed) {
        const locale = localeIds.get(entry.localeCode);
        if (!locale) fail("internal_error", "Import locale was not resolved.");
        const { storedKey, created } = ensureKey(
          state,
          projectId,
          namespace.id,
          entry.key,
          null,
        );
        if (created) keysCreated += 1;
        const existingValue = state.values.find(
          (item) => item.keyId === storedKey.id && item.localeId === locale.id,
        );
        const timestamp = now();
        if (existingValue) {
          existingValue.value = entry.value;
          existingValue.status = "translated";
          existingValue.updatedAt = timestamp;
        } else {
          state.values.push({
            id: newId(),
            keyId: storedKey.id,
            localeId: locale.id,
            value: entry.value,
            status: "translated",
            updatedAt: timestamp,
          });
        }
        valuesWritten += 1;
      }
      saveState(state);
      return {
        path: "browser://selected-files",
        namespace: namespaceValue,
        filesProcessed: files.length,
        localesCreated,
        keysCreated,
        valuesWritten,
        skippedFiles: [],
      };
    },
  };
}

function flattenObject(
  object: Record<string, unknown>,
  prefix: string,
  out: Map<string, string>,
): void {
  for (const [key, value] of Object.entries(object)) {
    if (!key) fail("validation_error", "Empty keys are not allowed.");
    const path = prefix ? `${prefix}.${key}` : key;
    if (value && typeof value === "object" && !Array.isArray(value)) {
      flattenObject(value as Record<string, unknown>, path, out);
    } else if (typeof value === "string") {
      out.set(path, value);
    } else if (Array.isArray(value)) {
      fail("validation_error", `Arrays are not supported at key "${path}".`);
    } else {
      fail("validation_error", `Value at key "${path}" must be a string.`);
    }
  }
}

function nestedObject(flat: Record<string, string>): Record<string, unknown> {
  const root: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(flat)) {
    const parts = key.split(".");
    let cursor = root;
    for (let index = 0; index < parts.length; index += 1) {
      const part = parts[index];
      if (!part)
        fail("validation_error", `Invalid empty segment in key "${key}".`);
      if (index === parts.length - 1) {
        if (part in cursor)
          fail("conflict", `Nested key collision at "${key}".`);
        cursor[part] = value;
      } else {
        const next = cursor[part];
        if (next === undefined) {
          const created: Record<string, unknown> = {};
          cursor[part] = created;
          cursor = created;
        } else if (
          typeof next === "object" &&
          next !== null &&
          !Array.isArray(next)
        ) {
          cursor = next as Record<string, unknown>;
        } else {
          fail("conflict", `Nested key collision at "${key}".`);
        }
      }
    }
  }
  return root;
}

function sortObject(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sortObject);
  if (value && typeof value === "object") {
    const source = value as Record<string, unknown>;
    const sorted: Record<string, unknown> = {};
    for (const key of Object.keys(source).sort()) {
      sorted[key] = sortObject(source[key]);
    }
    return sorted;
  }
  return value;
}

function downloadFile(fileName: string, content: string): void {
  try {
    if (
      typeof document === "undefined" ||
      typeof URL === "undefined" ||
      typeof URL.createObjectURL !== "function"
    ) {
      return;
    }
    const blob = new Blob([content], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = fileName;
    anchor.style.display = "none";
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
    URL.revokeObjectURL(url);
  } catch {
    // Download is a best-effort browser convenience; export still returns contents.
  }
}

/**
 * Placeholder scanner mirroring the Rust implementation (`{name}`, `{{name}}`,
 * `${name}`, `%s`, `%1$s`). ICU plural bodies are out of scope.
 */
export function extractPlaceholders(input: string): string[] {
  const chars = Array.from(input);
  const found = new Set<string>();
  let index = 0;
  while (index < chars.length) {
    if (chars[index] === "{") {
      if (chars[index + 1] === "{") {
        const parsed = parseBraced(chars, index + 2, "}}");
        if (parsed) {
          if (isPlaceholderName(parsed.name)) found.add(parsed.name);
          index = parsed.next;
          continue;
        }
      } else {
        const parsed = parseBraced(chars, index + 1, "}");
        if (parsed) {
          if (isPlaceholderName(parsed.name)) found.add(parsed.name);
          index = parsed.next;
          continue;
        }
      }
    }
    if (chars[index] === "$" && chars[index + 1] === "{") {
      const parsed = parseBraced(chars, index + 2, "}");
      if (parsed) {
        if (isPlaceholderName(parsed.name)) found.add(parsed.name);
        index = parsed.next;
        continue;
      }
    }
    if (chars[index] === "%") {
      if (chars[index + 1] === "%") {
        index += 2;
        continue;
      }
      const start = index;
      let cursor = index + 1;
      while (cursor < chars.length && /[0-9]/.test(chars[cursor] ?? ""))
        cursor += 1;
      if (chars[cursor] === "$") cursor += 1;
      if (cursor < chars.length && /[A-Za-z]/.test(chars[cursor] ?? "")) {
        found.add(chars.slice(start, cursor + 1).join(""));
        index = cursor + 1;
        continue;
      }
    }
    index += 1;
  }
  return Array.from(found).sort();
}

function parseBraced(
  chars: string[],
  start: number,
  terminator: string,
): { name: string; next: number } | null {
  let text = "";
  let index = start;
  while (index < chars.length) {
    if (
      terminator === "}}" &&
      chars[index] === "}" &&
      chars[index + 1] === "}"
    ) {
      return { name: normalizePlaceholderName(text), next: index + 2 };
    }
    if (terminator === "}" && chars[index] === "}") {
      return { name: normalizePlaceholderName(text), next: index + 1 };
    }
    text += chars[index];
    index += 1;
  }
  return null;
}

function normalizePlaceholderName(text: string): string {
  return (text.split(",")[0] ?? "").split(":")[0]?.trim() ?? "";
}

function isPlaceholderName(name: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(name);
}
