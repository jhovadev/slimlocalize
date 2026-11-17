import { invoke } from "@tauri-apps/api/core";

import type {
  AddLocaleInput,
  AppInfo,
  CheckReport,
  CollabStatus,
  ConfigVariable,
  CreateEnvironmentInput,
  CreateProjectInput,
  CreateWorkspaceInput,
  Environment,
  ExportOptions,
  ExportResult,
  ImportResult,
  Locale,
  Namespace,
  PlatformInfo,
  Project,
  SaveTranslationInput,
  SetConfigVariableInput,
  TranslationEntry,
  Workspace,
  WorkspaceService,
} from "./types";
import { WorkspaceServiceError } from "./types";

export const TAURI_PLATFORM: PlatformInfo = {
  kind: "desktop",
  label: "Desktop",
  storage: "SQLite (local, WAL)",
  localFirst: true,
  capabilities: {
    importFromPath: true,
    importFromFiles: false,
    downloadExports: false,
    nativeDatabase: true,
    sync: false,
  },
};

async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    if (
      error &&
      typeof error === "object" &&
      "message" in error &&
      typeof (error as { message: unknown }).message === "string"
    ) {
      const payload = error as { code?: unknown; message: string };
      throw new WorkspaceServiceError(
        typeof payload.code === "string" ? payload.code : "desktop_error",
        payload.message,
      );
    }
    if (typeof error === "string") {
      throw new WorkspaceServiceError("desktop_error", error);
    }
    throw new WorkspaceServiceError("desktop_error", "Desktop command failed.");
  }
}

export function createTauriWorkspaceService(): WorkspaceService {
  return {
    platform: TAURI_PLATFORM,

    getInfo: () => call<AppInfo>("get_app_info"),
    listWorkspaces: () => call<Workspace[]>("list_workspaces"),
    createWorkspace: (input: CreateWorkspaceInput) =>
      call<Workspace>("create_workspace", { input }),

    listProjects: (workspaceId: string) =>
      call<Project[]>("list_projects", { workspaceId }),
    createProject: (workspaceId: string, input: CreateProjectInput) =>
      call<Project>("create_project", { workspaceId, input }),

    listLocales: (projectId: string) =>
      call<Locale[]>("list_locales", { projectId }),
    addLocale: (projectId: string, input: AddLocaleInput) =>
      call<Locale>("add_locale", { projectId, input }),
    setDefaultLocale: (projectId: string, code: string) =>
      call<Locale>("set_default_locale", { projectId, code }),

    listNamespaces: (projectId: string) =>
      call<Namespace[]>("list_namespaces", { projectId }),
    createNamespace: (projectId: string, name: string) =>
      call<Namespace>("create_namespace", { projectId, name }),

    listTranslations: (
      projectId: string,
      localeCode: string,
      namespace?: string | null,
    ) =>
      call<TranslationEntry[]>("list_translations", {
        projectId,
        localeCode,
        namespace: namespace ?? null,
      }),
    saveTranslation: (input: SaveTranslationInput) =>
      call<TranslationEntry>("save_translation", { input }),

    listEnvironments: (projectId: string) =>
      call<Environment[]>("list_environments", { projectId }),
    createEnvironment: (projectId: string, input: CreateEnvironmentInput) =>
      call<Environment>("create_environment", { projectId, input }),

    listConfigVariables: (environmentId: string) =>
      call<ConfigVariable[]>("list_config_variables", { environmentId }),
    setConfigVariable: (input: SetConfigVariableInput) =>
      call<ConfigVariable>("set_config_variable", { input }),

    checkProject: (projectId: string) =>
      call<CheckReport>("check_project", { projectId }),

    collabStatus: (projectId: string) =>
      call<CollabStatus>("collab_status", { projectId }),
    collabBootstrap: (projectId: string) =>
      call<CollabStatus>("collab_bootstrap", { projectId }),
    collabExport: (projectId: string) =>
      call<string>("collab_export", { projectId }),
    collabImport: (projectId: string, payload: string) =>
      call<CollabStatus>("collab_import", { projectId, payload }),
    collabRebuildProjection: (projectId: string) =>
      call<void>("collab_rebuild_projection", { projectId }),
    collabValidate: (projectId: string) =>
      call<CheckReport>("collab_validate", { projectId }),

    exportProject: (projectId: string, options: ExportOptions = {}) =>
      call<ExportResult>("export_project", {
        projectId,
        nested: options.nested ?? false,
        namespace: options.namespace ?? null,
      }),

    importFromPath: (
      projectId: string,
      path: string,
      namespace?: string | null,
    ) =>
      call<ImportResult>("import_project", {
        projectId,
        path,
        namespace: namespace ?? "common",
      }),
  };
}
