import { createSignal, onCleanup, onMount, Show } from "solid-js";
import type { Project, WorkspaceService } from "@slimlocalize/platform";

interface DevelopmentWebSync {
  webSyncStart(projectId: string): Promise<void>;
  webSyncStop(): Promise<void>;
  webSyncInfo(): { status: string; projectId: string | null; message?: string };
  webSyncShareCode(projectId: string): Promise<string>;
  webSyncJoinCode(code: string): Promise<void>;
  webSyncSetName(name: string): void;
  subscribeSync(
    cb: (status: {
      status: string;
      projectId: string | null;
      message?: string;
    }) => void,
  ): () => void;
}
function available(
  service: WorkspaceService,
): service is WorkspaceService & DevelopmentWebSync {
  return (
    typeof (service as Partial<DevelopmentWebSync>).webSyncStart === "function"
  );
}

/** All collaboration controls are kept in Settings, not the main app header. */
export function WebSyncPanel(props: {
  service: WorkspaceService;
  project: Project;
}) {
  const [code, setCode] = createSignal("");
  const [displayName, setDisplayName] = createSignal(
    localStorage.getItem("slimlocalize.websync.displayName") ?? "",
  );
  const [status, setStatus] = createSignal("off");
  const [error, setError] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  onMount(() => {
    if (!available(props.service)) return;
    const off = props.service.subscribeSync((value) => {
      setStatus(value.status);
      if (value.message) setError(value.message);
    });
    onCleanup(off);
  });
  const run = async (action: () => Promise<void>): Promise<void> => {
    if (busy()) return;
    setBusy(true);
    setError("");
    try {
      await action();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Show when={available(props.service)}>
      <section class="ux-websync" aria-labelledby="ux-websync-title">
        <h3 id="ux-websync-title">Share this project</h3>
        <p>
          Share translations and public environment variables with another
          browser. Local-only values stay on this device. Development testing
          only.
        </p>
        <label class="ux-websync-name">
          Your display name
          <input
            aria-label="Your collaborator name"
            autocomplete="nickname"
            maxlength={32}
            value={displayName()}
            onInput={(event) => {
              setDisplayName(event.currentTarget.value);
              (
                props.service as WorkspaceService & DevelopmentWebSync
              ).webSyncSetName(event.currentTarget.value);
            }}
            placeholder="How others see you"
          />
        </label>
        <div class="ux-websync-actions">
          <button
            type="button"
            disabled={busy()}
            onClick={() =>
              void run(() =>
                (
                  props.service as WorkspaceService & DevelopmentWebSync
                ).webSyncStart(props.project.id),
              )
            }
          >
            Connect
          </button>
          <button
            type="button"
            disabled={busy()}
            onClick={() =>
              void run(async () => {
                const result = await (
                  props.service as WorkspaceService & DevelopmentWebSync
                ).webSyncShareCode(props.project.id);
                setCode(result);
                if (navigator.clipboard?.writeText)
                  await navigator.clipboard.writeText(result).catch(() => {});
              })
            }
          >
            Copy pairing code
          </button>
          <button
            type="button"
            disabled={busy()}
            onClick={() =>
              void run(() =>
                (
                  props.service as WorkspaceService & DevelopmentWebSync
                ).webSyncStop(),
              )
            }
          >
            Disconnect
          </button>
        </div>
        <div class="ux-websync-state" role="status">
          {status() === "connected"
            ? "Connected"
            : status() === "connecting"
              ? "Connecting…"
              : status() === "disconnected"
                ? "Offline — changes saved on this device"
                : status() === "error"
                  ? "Connection error"
                  : "Not connected"}
        </div>
        <label for="ux-websync-code">Pair another browser</label>
        <textarea
          id="ux-websync-code"
          rows={3}
          value={code()}
          onInput={(event) => setCode(event.currentTarget.value)}
          placeholder="Paste a pairing code, or copy yours"
          spellcheck={false}
        />
        <button
          type="button"
          class="ux-websync-join"
          disabled={busy() || !code().startsWith("slim-dev-1:")}
          onClick={() =>
            void run(async () => {
              await (
                props.service as WorkspaceService & DevelopmentWebSync
              ).webSyncJoinCode(code());
              // Existing AppShell bootstrap reads the newly imported workspace on startup.
              window.location.reload();
            })
          }
        >
          Join project
        </button>
        <Show when={error()}>
          <p role="alert" class="ux-websync-error">
            {error()}
          </p>
        </Show>
        <small>
          Do not expose this development relay to the internet. Shared text is
          not encrypted.
        </small>
      </section>
    </Show>
  );
}
