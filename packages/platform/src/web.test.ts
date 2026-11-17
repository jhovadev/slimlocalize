import { beforeEach, describe, expect, it } from "vitest";

import {
  createWebWorkspaceService,
  extractPlaceholders,
  normalizeLocaleCode,
  slugify,
} from "./web";
import { WorkspaceServiceError } from "./types";

describe("web platform adapter", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("persists a full workspace flow across adapter instances", async () => {
    const service = createWebWorkspaceService();
    const workspace = await service.createWorkspace({ name: "Demo Team" });
    expect(workspace.slug).toBe("demo-team");

    const project = await service.createProject(workspace.id, {
      name: "Demo App",
      defaultLocale: "en-US",
    });
    expect(project.defaultLocale).toBe("en-US");

    await service.addLocale(project.id, { code: "es-PE" });
    await service.saveTranslation({
      projectId: project.id,
      namespace: "common",
      key: "welcome",
      localeCode: "en-US",
      value: "Welcome, {name}!",
      status: "translated",
    });
    await service.saveTranslation({
      projectId: project.id,
      namespace: "common",
      key: "welcome",
      localeCode: "es-PE",
      value: "¡Bienvenido!",
      status: "draft",
    });

    const entries = await service.listTranslations(
      project.id,
      "es-PE",
      "common",
    );
    expect(entries).toHaveLength(1);
    expect(entries[0]?.value).toBe("¡Bienvenido!");

    const report = await service.checkProject(project.id);
    expect(report.defaultLocale).toBe("en-US");
    expect(report.totalKeys).toBe(1);
    expect(report.placeholderMismatches).toHaveLength(1);

    // A new adapter reads the same localStorage namespace: reload persistence.
    const reloaded = createWebWorkspaceService();
    const workspaces = await reloaded.listWorkspaces();
    expect(workspaces).toHaveLength(1);
    const projects = await reloaded.listProjects(workspaces[0]!.id);
    expect(projects).toHaveLength(1);
    const reloadedEntries = await reloaded.listTranslations(
      projects[0]!.id,
      "es-PE",
      "common",
    );
    expect(reloadedEntries[0]?.value).toBe("¡Bienvenido!");
  });

  it("creates default environments and stores public configuration", async () => {
    const service = createWebWorkspaceService();
    const workspace = await service.createWorkspace({ name: "Config" });
    const project = await service.createProject(workspace.id, { name: "App" });
    const environments = await service.listEnvironments(project.id);
    expect(environments.map((environment) => environment.name)).toEqual([
      "development",
      "production",
      "staging",
    ]);
    const development = environments.find(
      (environment) => environment.name === "development",
    )!;
    await service.setConfigVariable({
      environmentId: development.id,
      key: "PUBLIC_API_URL",
      value: "http://localhost:3000",
      visibility: "public",
    });
    const variables = await service.listConfigVariables(development.id);
    expect(variables[0]?.value).toBe("http://localhost:3000");
  });

  it("imports nested JSON and exports deterministic content", async () => {
    const service = createWebWorkspaceService();
    const workspace = await service.createWorkspace({ name: "Import" });
    const project = await service.createProject(workspace.id, {
      name: "Fixture",
    });
    const report = await service.importFromFiles!(
      project.id,
      [
        {
          fileName: "en-US.json",
          content: JSON.stringify({
            welcome: "Welcome, {name}!",
            auth: { signIn: "Sign in" },
          }),
        },
        {
          fileName: "es-PE.json",
          content: JSON.stringify({
            welcome: "Hola",
            auth: { signIn: "Iniciar sesión" },
          }),
        },
      ],
      "common",
    );
    expect(report.filesProcessed).toBe(2);
    expect(report.keysCreated).toBe(2);
    expect(report.localesCreated).toBe(2);

    const locales = await service.listLocales(project.id);
    expect(locales.find((locale) => locale.isDefault)?.code).toBe("en-US");

    const first = await service.exportProject(project.id, {
      nested: true,
      download: false,
    });
    const second = await service.exportProject(project.id, {
      nested: true,
      download: false,
    });
    expect(first.contents).toEqual(second.contents);
    const exported = JSON.parse(first.contents?.["en-US.json"] ?? "{}") as {
      auth: { signIn: string };
      welcome: string;
    };
    expect(exported.auth.signIn).toBe("Sign in");
    expect(exported.welcome).toBe("Welcome, {name}!");
    expect(first.contents?.["en-US.json"]?.endsWith("\n")).toBe(true);
  });

  it("validates locales, keys and duplicate resources", async () => {
    const service = createWebWorkspaceService();
    const workspace = await service.createWorkspace({ name: "Validation" });
    expect((await service.listWorkspaces())[0]?.slug).toBe("validation");
    await expect(
      service.createWorkspace({ name: "Validation" }),
    ).rejects.toMatchObject({ code: "conflict" });

    const project = await service.createProject(workspace.id, { name: "App" });
    await expect(
      service.addLocale(project.id, { code: "../../etc" }),
    ).rejects.toMatchObject({ code: "validation_error" });
    const locale = await service.addLocale(project.id, { code: "en-us" });
    expect(locale.code).toBe("en-US");
    expect(locale.isDefault).toBe(true);
    await expect(
      service.addLocale(project.id, { code: "en-US" }),
    ).rejects.toMatchObject({ code: "conflict" });
    await expect(
      service.saveTranslation({
        projectId: project.id,
        namespace: "common",
        key: "a..b",
        localeCode: "en-US",
        value: "x",
        status: "draft",
      }),
    ).rejects.toBeInstanceOf(WorkspaceServiceError);
  });

  it("detects missing values and switches default locale", async () => {
    const service = createWebWorkspaceService();
    const workspace = await service.createWorkspace({ name: "Checks" });
    const project = await service.createProject(workspace.id, { name: "App" });
    await service.addLocale(project.id, { code: "en-US" });
    await service.addLocale(project.id, { code: "es-PE" });
    await service.saveTranslation({
      projectId: project.id,
      namespace: "common",
      key: "save",
      localeCode: "en-US",
      value: "Save",
      status: "translated",
    });
    const report = await service.checkProject(project.id);
    expect(report.missing).toEqual([
      { localeCode: "es-PE", namespace: "common", key: "save" },
    ]);
    const changed = await service.setDefaultLocale(project.id, "es-PE");
    expect(changed.isDefault).toBe(true);
    const locales = await service.listLocales(project.id);
    expect(locales.filter((locale) => locale.isDefault)).toHaveLength(1);
  });
});

describe("web validation helpers", () => {
  it("slugifies accents deterministically", () => {
    expect(slugify("Diseño Público")).toBe("diseno-publico");
    expect(slugify("---")).toBe("item");
  });

  it("canonicalizes locale codes", () => {
    expect(normalizeLocaleCode("es_pe")).toBe("es-PE");
    expect(normalizeLocaleCode("zh-hant-tw")).toBe("zh-Hant-TW");
    expect(() => normalizeLocaleCode("en-")).toThrow();
  });

  it("extracts common placeholder forms", () => {
    expect(
      extractPlaceholders("Hi {name}, {{count}}, ${currency}, %d, %1$s"),
    ).toEqual(["%1$s", "%d", "count", "currency", "name"]);
    expect(extractPlaceholders("{not a name} {ok}")).toEqual(["ok"]);
  });
});
