import { cleanup, fireEvent, render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import type { Namespace, TranslationEntry } from "@slimlocalize/platform";
import { buildNamespaceTree, NamespaceTree } from "./NamespaceTree";

afterEach(() => cleanup());
const entries = [
  {
    namespace: "common",
    key: "auth.signIn",
    keyId: "k1",
    localeCode: "es",
    value: "Entrar",
    status: "translated",
    description: null,
    updatedAt: null,
  },
  {
    namespace: "common",
    key: "auth.signOut",
    keyId: "k2",
    localeCode: "es",
    value: "Salir",
    status: "translated",
    description: null,
    updatedAt: null,
  },
  {
    namespace: "pages",
    key: "home.title",
    keyId: "k3",
    localeCode: "es",
    value: "Hola",
    status: "translated",
    description: null,
    updatedAt: null,
  },
] as TranslationEntry[];
const namespaces = [{ name: "common" }, { name: "pages" }] as Namespace[];

describe("NamespaceTree", () => {
  it("groups dot-separated keys under their namespace", () => {
    const tree = buildNamespaceTree(namespaces, entries);
    expect(tree.map((node) => [node.name, node.count])).toEqual([
      ["common", 2],
      ["pages", 1],
    ]);
    expect(tree[0]?.children[0]?.name).toBe("auth");
    expect(tree[0]?.children[0]?.children.map((node) => node.key)).toEqual([
      "auth.signIn",
      "auth.signOut",
    ]);
  });
  it("selects an existing key without renaming or duplicating it", () => {
    const selected: string[] = [];
    render(() => (
      <NamespaceTree
        namespaces={namespaces}
        entries={entries}
        namespace="common"
        onSelect={(namespace, key) =>
          selected.push(`${namespace}/${key ?? ""}`)
        }
      />
    ));
    fireEvent.click(screen.getByRole("treeitem", { name: /auth/i }));
    // Clicking a folder row must expand it; the nested key is now visible.
    expect(screen.getByRole("treeitem", { name: /signIn/i })).toBeDefined();
    fireEvent.click(screen.getByRole("treeitem", { name: /signIn/i }));
    expect(selected.at(-1)).toBe("common/auth.signIn");
  });
});
