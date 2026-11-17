import { beforeEach, describe, expect, it } from "vitest";

import { createWebWorkspaceService } from "@slimlocalize/platform/web";
import { createClient } from "./index";

describe("client SDK", () => {
  beforeEach(() => localStorage.clear());

  it("resolves slugs and delegates domain operations", async () => {
    const service = createWebWorkspaceService();
    const client = createClient(service);
    const workspace = await client.createWorkspace({ name: "SDK Demo" });
    const project = await client.createProject(workspace.id, {
      name: "SDK App",
      defaultLocale: "en-US",
    });
    const resolved = await client.resolveProject("sdk-demo", "sdk-app");
    expect(resolved.id).toBe(project.id);
    await client.addLocale(project.id, { code: "es-PE" });
    await client.saveTranslation({
      projectId: project.id,
      namespace: "common",
      key: "save",
      localeCode: "en-US",
      value: "Save",
      status: "translated",
    });
    const report = await client.checkProject(project.id);
    expect(report.missing).toHaveLength(1);
    await expect(client.resolveWorkspace("missing")).rejects.toMatchObject({
      code: "not_found",
    });
  });
});
