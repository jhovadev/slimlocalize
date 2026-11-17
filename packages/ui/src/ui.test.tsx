import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";

import { EmptyState } from "./components";
import { LocaleForm, WorkspaceForm } from "./forms";

describe("shared UI", () => {
  it("renders an accessible empty state", () => {
    render(() => (
      <EmptyState
        title="Nothing here"
        description="Create something."
        action={<button>Go</button>}
      />
    ));
    expect(screen.getByText("Nothing here")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Go" })).toBeInTheDocument();
  });

  it("submits the workspace form with typed values", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(() => <WorkspaceForm onSubmit={onSubmit} />);
    fireEvent.input(screen.getByLabelText("Workspace name"), {
      target: { value: "Demo" },
    });
    fireEvent.input(screen.getByLabelText("Slug (optional)"), {
      target: { value: "demo" },
    });
    fireEvent.submit(
      screen.getByRole("button", { name: "Create workspace" }).closest("form")!,
    );
    await waitFor(() =>
      expect(onSubmit).toHaveBeenCalledWith({ name: "Demo", slug: "demo" }),
    );
  });

  it("shows form errors returned by the caller", async () => {
    const onSubmit = vi.fn().mockRejectedValue(new Error("Invalid locale"));
    render(() => <LocaleForm onSubmit={onSubmit} />);
    fireEvent.input(screen.getByLabelText("Locale code"), {
      target: { value: "??" },
    });
    fireEvent.submit(
      screen.getByRole("button", { name: "Add locale" }).closest("form")!,
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Invalid locale",
    );
  });
});
