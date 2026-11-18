import { createEffect, createSignal, For, onCleanup, Show } from "solid-js";
import type { WorkspaceService } from "@slimlocalize/platform";
import type { WebPeer } from "@slimlocalize/platform/web-sync";

type PresenceService = WorkspaceService & {
  webSyncPeers(): WebPeer[];
  subscribePeers(listener: (peers: WebPeer[]) => void): () => void;
};

function hasPresence(service: WorkspaceService): service is PresenceService {
  return (
    typeof (service as Partial<PresenceService>).subscribePeers === "function"
  );
}

/** Only visible if a real remote peer is detected; no decorative placeholder avatars. */
export function WebPresenceStrip(props: {
  service: WorkspaceService;
  projectId: string;
}) {
  const [peers, setPeers] = createSignal<WebPeer[]>([]);
  createEffect(() => {
    const currentProject = props.projectId;
    if (!hasPresence(props.service)) return;
    const unsubscribe = props.service.subscribePeers((next) => {
      // The service clears peers on room switch; prevent showing a stale project roster.
      setPeers(props.projectId === currentProject ? next : []);
    });
    onCleanup(unsubscribe);
  });
  return (
    <Show when={peers().length > 0}>
      <aside class="ux-peers" aria-label="Other people editing this project">
        <div
          class="ux-peers-avatars"
          aria-label={`${peers().length} other collaborators`}
        >
          <For each={peers()}>
            {(peer) => (
              <span
                class="ux-peers-avatar"
                title={peer.name}
                style={{ "--peer-color": peer.color }}
              >
                {peer.name.charAt(0).toUpperCase()}
              </span>
            )}
          </For>
        </div>
        <span class="ux-peers-caption">
          {peers().length === 1
            ? `${peers()[0]!.name} is here`
            : `${peers().length} others are here`}
        </span>
        <Show when={peers().find((peer) => peer.focus)}>
          {(peer) => (
            <span
              class="ux-peers-focus"
              title={`${peer().name} · ${peer().focus}`}
            >
              <span
                class="ux-peers-dot"
                style={{ "--peer-color": peer().color }}
              />
              {peer().name} · {peer().focus}
            </span>
          )}
        </Show>
      </aside>
    </Show>
  );
}
