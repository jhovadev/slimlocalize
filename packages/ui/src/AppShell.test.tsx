import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { beforeEach, describe, expect, it } from "vitest";
import { createWebWorkspaceService } from "@slimlocalize/platform/web";

import { AppShell } from "./AppShell";

describe("AppShell", () => {
  beforeEach(() => localStorage.clear());

  it("bootstraps from real web persistence and creates a workspace", async () => {
    render(() => <AppShell service={createWebWorkspaceService()} />);
    expect(
      await screen.findByText("Your translations, on your machine."),
    ).toBeInTheDocument();
    fireEvent.input(screen.getByLabelText("Workspace name"), {
      target: { value: "Demo Team" },
    });
    fireEvent.submit(
      screen.getByRole("button", { name: "Create workspace" }).closest("form")!,
    );
    await waitFor(() =>
      expect(
        screen.getByText("No projects in this workspace"),
      ).toBeInTheDocument(),
    );
    const service = createWebWorkspaceService();
    const workspaces = await service.listWorkspaces();
    expect(workspaces[0]?.slug).toBe("demo-team");
  });
});
