import { cleanup, fireEvent, render, screen } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { createWebWorkspaceService } from "@slimlocalize/platform/web";
import { AppIcon } from "./AppIcon";
import { AppShell } from "./AppShell";

beforeEach(() => localStorage.clear());
afterEach(cleanup);

describe("UX-03 navigation and icon reliability", () => {
  it("renders an actual bundled SVG icon, not an invisible generated CSS class", () => {
    const { container } = render(() => (
      <AppIcon name="folder-plus" label="New project icon" />
    ));
    const svg = container.querySelector("svg");
    expect(svg).not.toBeNull();
    expect(svg?.querySelector("path")).not.toBeNull();
    expect(svg?.getAttribute("aria-label")).toBe("New project icon");
  });

  it("provides an obvious project creation action with a dialog", async () => {
    const service = createWebWorkspaceService();
    await service.createWorkspace({ name: "UX 03 test workspace" });
    render(() => <AppShell service={service} />);
    fireEvent.click(
      await screen.findByRole("button", { name: /new project/i }),
    );
    expect(
      await screen.findByRole("dialog", { name: "New project" }),
    ).toBeInTheDocument();
  });
});
