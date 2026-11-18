import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import { QuickAddKey } from "./QuickAddKey";
import { InlineValue } from "./InlineValue";

const namespace = {
  id: "ns1",
  projectId: "p1",
  name: "common",
  createdAt: "now",
};
const entry = {
  keyId: "k1",
  key: "auth.signIn",
  namespace: "common",
  localeCode: "en-US",
  value: "Sign in",
  status: "translated" as const,
  description: null,
  updatedAt: null,
};

describe("UX-01", () => {
  it("creates a key with one action and preserves locale context", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();
    render(() => (
      <QuickAddKey
        localeCode="en-US"
        defaultNamespace="common"
        namespaces={[namespace]}
        onSave={onSave}
        onClose={onClose}
      />
    ));
    fireEvent.input(screen.getByPlaceholderText("auth.signIn"), {
      target: { value: "home.title" },
    });
    fireEvent.input(screen.getByPlaceholderText("Sign in"), {
      target: { value: "Welcome" },
    });
    fireEvent.submit(
      screen.getByRole("button", { name: "Create key" }).closest("form")!,
    );
    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith(
        expect.objectContaining({
          key: "home.title",
          value: "Welcome",
          localeCode: "en-US",
          namespace: "common",
        }),
      ),
    );
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
  });
  it("edits a value directly without losing metadata", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(() => <InlineValue entry={entry} onSave={onSave} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit auth.signIn" }));
    fireEvent.input(
      screen.getByRole("textbox", { name: "Value for auth.signIn" }),
      {
        target: { value: "Log in" },
      },
    );
    fireEvent.keyDown(
      screen.getByRole("textbox", { name: "Value for auth.signIn" }),
      { key: "Enter" },
    );
    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith(
        expect.objectContaining({
          key: "auth.signIn",
          value: "Log in",
          namespace: "common",
          localeCode: "en-US",
        }),
      ),
    );
  });
});
