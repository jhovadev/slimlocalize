import { createMemo, createSignal, For, Show } from "solid-js";
import type { Namespace, TranslationEntry } from "@slimlocalize/platform";

export interface TreeNode {
  id: string;
  name: string;
  namespace: string;
  key?: string;
  children: TreeNode[];
  count: number;
}

/** The tree is a view of existing keys, not a second persistence schema. */
export function buildNamespaceTree(
  namespaces: Pick<Namespace, "name">[],
  entries: Pick<TranslationEntry, "namespace" | "key">[],
): TreeNode[] {
  const result: TreeNode[] = [];
  const byNamespace = new Map<string, TreeNode>();
  const all = new Set([
    ...namespaces.map((n) => n.name),
    ...entries.map((e) => e.namespace),
  ]);
  for (const name of [...all].sort((a, b) => a.localeCompare(b))) {
    const root: TreeNode = {
      id: `ns:${name}`,
      name,
      namespace: name,
      children: [],
      count: 0,
    };
    result.push(root);
    byNamespace.set(name, root);
  }
  for (const entry of entries) {
    const root = byNamespace.get(entry.namespace);
    if (!root) continue;
    root.count++;
    let level = root.children;
    let parent = root;
    const segments = entry.key.split(".");
    segments.forEach((segment, index) => {
      const id = `${root.id}/${segments.slice(0, index + 1).join(".")}`;
      let node = level.find((item) => item.id === id);
      if (!node) {
        node = {
          id,
          name: segment,
          namespace: entry.namespace,
          children: [],
          count: 0,
        };
        level.push(node);
      }
      node.count++;
      if (index === segments.length - 1) node.key = entry.key;
      parent = node;
      level = parent.children;
    });
  }
  const sort = (nodes: TreeNode[]): void => {
    nodes.sort((a, b) => a.name.localeCompare(b.name));
    for (const node of nodes) sort(node.children);
  };
  sort(result);
  return result;
}

function Chevron(props: { open: boolean }) {
  return (
    <svg
      width="14"
      height="14"
      viewBox="0 0 24 24"
      aria-hidden="true"
      fill="none"
      stroke="currentColor"
      stroke-width="1.8"
      stroke-linecap="round"
      stroke-linejoin="round"
      class={
        props.open ? "ux-tree-chevron ux-tree-chevron--open" : "ux-tree-chevron"
      }
    >
      <path d="m9 18 6-6-6-6" />
    </svg>
  );
}

function Branch(props: {
  node: TreeNode;
  depth: number;
  namespace: string;
  selectedKey?: string;
  onSelect: (namespace: string, key?: string) => void;
  search: string;
}) {
  const [expanded, setExpanded] = createSignal(props.depth === 0);
  const hasChildren = () => props.node.children.length > 0;
  const visible = createMemo(() => {
    const query = props.search.toLowerCase().trim();
    if (!query) return true;
    const descendants = (node: TreeNode): boolean =>
      node.name.toLowerCase().includes(query) ||
      node.key?.toLowerCase().includes(query) === true ||
      node.children.some(descendants);
    return descendants(props.node);
  });
  const open = () => !!props.search.trim() || expanded();
  const active = () =>
    props.node.key
      ? props.namespace === props.node.namespace &&
        props.selectedKey === props.node.key
      : props.depth === 0 &&
        props.namespace === props.node.namespace &&
        !props.selectedKey;
  return (
    <Show when={visible()}>
      <div role="none" class="ux-tree-branch">
        <div
          role="none"
          class="ux-tree-row"
          style={{ "padding-left": `${8 + props.depth * 14}px` }}
        >
          <Show when={hasChildren()} fallback={<span class="ux-tree-indent" />}>
            <button
              type="button"
              class="ux-tree-expand"
              tabindex={-1}
              aria-label={`${open() ? "Collapse" : "Expand"} ${props.node.name}`}
              onClick={() => setExpanded((prev) => !prev)}
            >
              <Chevron open={open()} />
            </button>
          </Show>
          <button
            type="button"
            role="treeitem"
            aria-level={props.depth + 1}
            aria-selected={active()}
            aria-expanded={hasChildren() ? open() : undefined}
            class="ux-tree-item"
            classList={{ "ux-tree-item--selected": active() }}
            title={`${props.node.namespace}${props.node.key ? "/" + props.node.key : ""}`}
            onClick={() => {
              if (hasChildren()) setExpanded((previous) => !previous);
              props.onSelect(props.node.namespace, props.node.key);
            }}
            onKeyDown={(event) => {
              if (event.key === "ArrowRight" && hasChildren()) {
                event.preventDefault();
                setExpanded(true);
              } else if (event.key === "ArrowLeft" && hasChildren()) {
                event.preventDefault();
                setExpanded(false);
              } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
                const tree = event.currentTarget.closest('[role="tree"]');
                const nodes = Array.from(
                  tree?.querySelectorAll<HTMLButtonElement>(
                    '[role="treeitem"]',
                  ) ?? [],
                );
                const idx = nodes.indexOf(event.currentTarget);
                const next = nodes[idx + (event.key === "ArrowDown" ? 1 : -1)];
                if (next) {
                  event.preventDefault();
                  next.focus();
                }
              }
            }}
          >
            <span class="ux-tree-name">{props.node.name}</span>
            <Show when={!props.node.key && props.node.count > 0}>
              <span class="ux-tree-count">{props.node.count}</span>
            </Show>
          </button>
        </div>
        <Show when={hasChildren() && open()}>
          <div role="group">
            <For each={props.node.children}>
              {(child) => (
                <Branch
                  node={child}
                  depth={props.depth + 1}
                  namespace={props.namespace}
                  selectedKey={props.selectedKey}
                  onSelect={props.onSelect}
                  search={props.search}
                />
              )}
            </For>
          </div>
        </Show>
      </div>
    </Show>
  );
}

export function NamespaceTree(props: {
  namespaces: Namespace[];
  entries: TranslationEntry[];
  namespace: string;
  selectedKey?: string;
  onSelect: (namespace: string, key?: string) => void;
}) {
  const [query, setQuery] = createSignal("");
  const nodes = createMemo(() =>
    buildNamespaceTree(props.namespaces, props.entries),
  );
  return (
    <section class="ux-tree" aria-label="Namespaces and keys">
      <div class="ux-tree-heading">
        <span>Keys</span>
        <span class="ux-tree-count">{props.entries.length}</span>
      </div>
      <input
        class="ux-tree-search"
        type="search"
        aria-label="Filter keys in tree"
        placeholder="Filter keys…"
        value={query()}
        onInput={(event) => setQuery(event.currentTarget.value)}
      />
      <div role="tree" aria-label="Namespace tree" class="ux-tree-scroller">
        <For each={nodes()}>
          {(node) => (
            <Branch
              node={node}
              depth={0}
              namespace={props.namespace}
              selectedKey={props.selectedKey}
              onSelect={props.onSelect}
              search={query()}
            />
          )}
        </For>
        <Show when={nodes().length === 0}>
          <div class="ux-tree-empty">No keys yet</div>
        </Show>
      </div>
    </section>
  );
}
