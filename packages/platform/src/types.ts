export type TranslationStatus = "draft" | "translated" | "reviewed";
export type EnvironmentKind =
  "development" | "staging" | "production" | "custom";
export type ConfigVisibility = "public" | "local";

export interface PlatformCapabilities {
  /** Can import by server-side path (desktop only). */
  importFromPath: boolean;
  /** Can import browser File objects by reading their contents. */
  importFromFiles: boolean;
  /** Exports are written to a directory or downloaded by the browser. */
  downloadExports: boolean;
  nativeDatabase: boolean;
  sync: boolean;
}

export interface PlatformInfo {
  kind: "desktop" | "web";
  label: string;
  storage: string;
  localFirst: true;
  capabilities: PlatformCapabilities;
}

export interface AppInfo {
  name: string;
  version: string;
  platform: string;
  storage: string;
  databasePath: string;
  localFirst: boolean;
}

export interface Workspace {
  id: string;
  slug: string;
  name: string;
  description: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CreateWorkspaceInput {
  name: string;
  slug?: string | null;
  description?: string | null;
}

export interface Project {
  id: string;
  workspaceId: string;
  slug: string;
  name: string;
  description: string | null;
  defaultLocale: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CreateProjectInput {
  name: string;
  slug?: string | null;
  description?: string | null;
  defaultLocale?: string | null;
}

export interface Locale {
  id: string;
  projectId: string;
  code: string;
  name: string;
  isDefault: boolean;
  createdAt: string;
}

export interface AddLocaleInput {
  code: string;
  name?: string | null;
  isDefault?: boolean;
}

export interface Namespace {
  id: string;
  projectId: string;
  name: string;
  createdAt: string;
}

export interface TranslationEntry {
  keyId: string;
  key: string;
  namespace: string;
  description: string | null;
  localeCode: string;
  value: string | null;
  status: TranslationStatus | null;
  updatedAt: string | null;
}

export interface SaveTranslationInput {
  projectId: string;
  namespace: string;
  key: string;
  localeCode: string;
  value: string;
  status: TranslationStatus;
  description?: string | null;
}

export interface Environment {
  id: string;
  projectId: string;
  name: string;
  kind: EnvironmentKind;
  createdAt: string;
}

export interface CreateEnvironmentInput {
  name: string;
  kind: EnvironmentKind;
}

export interface ConfigVariable {
  id: string;
  environmentId: string;
  key: string;
  value: string;
  visibility: ConfigVisibility;
  createdAt: string;
  updatedAt: string;
}

export interface SetConfigVariableInput {
  environmentId: string;
  key: string;
  value: string;
  visibility: ConfigVisibility;
}

export interface MissingTranslation {
  localeCode: string;
  namespace: string;
  key: string;
}

export interface PlaceholderMismatch {
  localeCode: string;
  sourceLocale: string;
  namespace: string;
  key: string;
  expected: string[];
  actual: string[];
}

export interface CheckReport {
  projectId: string;
  defaultLocale: string | null;
  locales: string[];
  totalKeys: number;
  checkedValues: number;
  missing: MissingTranslation[];
  placeholderMismatches: PlaceholderMismatch[];
}

export interface ImportFileInput {
  fileName: string;
  content: string;
}

export interface ImportResult {
  path: string;
  namespace: string;
  filesProcessed: number;
  localesCreated: number;
  keysCreated: number;
  valuesWritten: number;
  skippedFiles: string[];
}

export interface ExportFileResult {
  localeCode: string;
  relativePath: string;
  keyCount: number;
}

export interface ExportOptions {
  nested?: boolean;
  namespace?: string | null;
  /** Web only: trigger browser downloads. Defaults to true in the UI. */
  download?: boolean;
}

export interface ExportResult {
  projectId: string;
  outputDir: string;
  files: ExportFileResult[];
  totalKeys: number;
  mode: "directory" | "download";
  /** Web adapter only: deterministic file contents, useful for integrations/tests. */
  contents?: Record<string, string>;
}

/**
 * Shared contract used by desktop, web, the CLI-facing SDK and tests.
 * Domain validation and persistence are implemented by each adapter.
 */
export interface CollabStatus {
  projectId: string;
  documentId: string;
  peerId: number;
  documentKey: string;
  versionVector: number[];
  updateCount: number;
  hasPending: boolean;
  schemaVersion: number;
}

export interface WorkspaceService {
  readonly platform: PlatformInfo;
  /** Optional ephemeral selection hint used only by collaborative clients. */
  webSyncSetFocus?(focus: string | null): void;
  /** Emitted after remote web updates are projected into the local domain. */
  subscribeChanges?(callback: (projectId: string) => void): () => void;
  getInfo(): Promise<AppInfo>;

  listWorkspaces(): Promise<Workspace[]>;
  createWorkspace(input: CreateWorkspaceInput): Promise<Workspace>;

  listProjects(workspaceId: string): Promise<Project[]>;
  createProject(
    workspaceId: string,
    input: CreateProjectInput,
  ): Promise<Project>;

  listLocales(projectId: string): Promise<Locale[]>;
  addLocale(projectId: string, input: AddLocaleInput): Promise<Locale>;
  setDefaultLocale(projectId: string, code: string): Promise<Locale>;

  listNamespaces(projectId: string): Promise<Namespace[]>;
  createNamespace(projectId: string, name: string): Promise<Namespace>;

  listTranslations(
    projectId: string,
    localeCode: string,
    namespace?: string | null,
  ): Promise<TranslationEntry[]>;
  saveTranslation(input: SaveTranslationInput): Promise<TranslationEntry>;

  listEnvironments(projectId: string): Promise<Environment[]>;
  createEnvironment(
    projectId: string,
    input: CreateEnvironmentInput,
  ): Promise<Environment>;

  listConfigVariables(environmentId: string): Promise<ConfigVariable[]>;
  setConfigVariable(input: SetConfigVariableInput): Promise<ConfigVariable>;

  checkProject(projectId: string): Promise<CheckReport>;
  exportProject(
    projectId: string,
    options?: ExportOptions,
  ): Promise<ExportResult>;
  collabStatus?(projectId: string): Promise<CollabStatus>;
  collabBootstrap?(projectId: string): Promise<CollabStatus>;
  collabExport?(projectId: string): Promise<string>;
  collabImport?(projectId: string, payload: string): Promise<CollabStatus>;
  collabRebuildProjection?(projectId: string): Promise<void>;
  collabValidate?(projectId: string): Promise<CheckReport>;

  /** Desktop adapter: import a directory or JSON file path. */
  importFromPath?(
    projectId: string,
    path: string,
    namespace?: string | null,
  ): Promise<ImportResult>;

  /** Web adapter: import browser-selected JSON files. */
  importFromFiles?(
    projectId: string,
    files: ImportFileInput[],
    namespace?: string | null,
  ): Promise<ImportResult>;
}

export class WorkspaceServiceError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "WorkspaceServiceError";
    this.code = code;
  }
}

export function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "string") {
    return error;
  }
  return "Unexpected error";
}
