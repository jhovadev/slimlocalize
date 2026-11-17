import type {
  AddLocaleInput,
  CheckReport,
  CreateProjectInput,
  CreateWorkspaceInput,
  ExportOptions,
  ExportResult,
  ImportResult,
  Locale,
  Project,
  SaveTranslationInput,
  TranslationEntry,
  Workspace,
  WorkspaceService,
} from "@slimlocalize/platform";
import { WorkspaceServiceError } from "@slimlocalize/platform";

/**
 * Small typed facade for scripts, agents and integrations. It never bypasses
 * the selected platform adapter, so desktop still writes through Rust/SQLite and
 * web writes through its explicit browser adapter.
 */
export class SlimLocalizeClient {
  constructor(readonly service: WorkspaceService) {}

  listWorkspaces(): Promise<Workspace[]> {
    return this.service.listWorkspaces();
  }

  createWorkspace(input: CreateWorkspaceInput): Promise<Workspace> {
    return this.service.createWorkspace(input);
  }

  async resolveWorkspace(reference: string): Promise<Workspace> {
    const workspaces = await this.service.listWorkspaces();
    const match = workspaces.find(
      (workspace) => workspace.id === reference || workspace.slug === reference,
    );
    if (!match) {
      throw new WorkspaceServiceError(
        "not_found",
        `Workspace not found: ${reference}`,
      );
    }
    return match;
  }

  createProject(
    workspaceId: string,
    input: CreateProjectInput,
  ): Promise<Project> {
    return this.service.createProject(workspaceId, input);
  }

  async resolveProject(
    workspaceReference: string,
    projectReference: string,
  ): Promise<Project> {
    const workspace = await this.resolveWorkspace(workspaceReference);
    const projects = await this.service.listProjects(workspace.id);
    const match = projects.find(
      (project) =>
        project.id === projectReference || project.slug === projectReference,
    );
    if (!match) {
      throw new WorkspaceServiceError(
        "not_found",
        `Project not found in ${workspace.slug}: ${projectReference}`,
      );
    }
    return match;
  }

  listLocales(projectId: string): Promise<Locale[]> {
    return this.service.listLocales(projectId);
  }

  addLocale(projectId: string, input: AddLocaleInput): Promise<Locale> {
    return this.service.addLocale(projectId, input);
  }

  listTranslations(
    projectId: string,
    locale: string,
    namespace?: string,
  ): Promise<TranslationEntry[]> {
    return this.service.listTranslations(projectId, locale, namespace);
  }

  saveTranslation(input: SaveTranslationInput): Promise<TranslationEntry> {
    return this.service.saveTranslation(input);
  }

  checkProject(projectId: string): Promise<CheckReport> {
    return this.service.checkProject(projectId);
  }

  exportProject(
    projectId: string,
    options?: ExportOptions,
  ): Promise<ExportResult> {
    return this.service.exportProject(projectId, options);
  }

  importFromPath(
    projectId: string,
    path: string,
    namespace?: string,
  ): Promise<ImportResult> {
    if (!this.service.importFromPath) {
      throw new WorkspaceServiceError(
        "unsupported",
        "This platform cannot import by filesystem path. Use the web file input or CLI.",
      );
    }
    return this.service.importFromPath(projectId, path, namespace);
  }

  importFromFiles(
    projectId: string,
    files: Array<{ fileName: string; content: string }>,
    namespace?: string,
  ): Promise<ImportResult> {
    if (!this.service.importFromFiles) {
      throw new WorkspaceServiceError(
        "unsupported",
        "This platform cannot import browser files. Use the desktop path import or CLI.",
      );
    }
    return this.service.importFromFiles(projectId, files, namespace);
  }
}

export function createClient(service: WorkspaceService): SlimLocalizeClient {
  return new SlimLocalizeClient(service);
}
