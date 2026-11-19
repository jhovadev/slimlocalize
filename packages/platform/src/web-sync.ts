/**
 * Browser-only development collaboration. Uses the Rust wire protocol and document schema.
 * IMPORTANT: the current relay has no production authentication. Enforce loopback only.
 */
import { LoroDoc } from "loro-crdt";
import { LoroAdaptor } from "loro-adaptors/loro";
import type { CreateEnvironmentInput, SetConfigVariableInput } from "./types";
import { WebPresence, type WebPeer } from "./web-presence";
import {
  environmentKey,
  parseEnvironmentKey,
  safeEnvironmentKind,
} from "./web-env-collab";
import { LoroWebsocketClient } from "loro-websocket";
import type {
  SaveTranslationInput,
  TranslationEntry,
  WorkspaceService,
} from "./types";
import {
  createWebWorkspaceService,
  exportWebProjectCode,
  importWebProjectCode,
} from "./web";

export type { WebPeer } from "./web-presence";
export type WebConnectionStatus =
  "off" | "connecting" | "connected" | "disconnected" | "error";
export interface WebSyncInfo {
  status: WebConnectionStatus;
  projectId: string | null;
  message?: string;
}
export interface WebSyncWorkspaceService extends WorkspaceService {
  webSyncStart(projectId: string): Promise<void>;
  restoreWebSync(): Promise<void>;
  webSyncStop(): Promise<void>;
  webSyncInfo(): WebSyncInfo;
  webSyncShareCode(projectId: string): Promise<string>;
  webSyncJoinCode(code: string): Promise<void>;
  subscribeChanges(callback: (projectId: string) => void): () => void;
  subscribeSync(callback: (info: WebSyncInfo) => void): () => void;
  webSyncPeers(): WebPeer[];
  subscribePeers(callback: (peers: WebPeer[]) => void): () => void;
  webSyncSetFocus(focus: string | null): void;
  webSyncSetName(name: string): void;
  webSyncConfigure(url: string, teamKey: string): void;
  webSyncServer(): string;
}

const DEFAULT_RELAY =
  typeof window !== "undefined" && window.location.protocol === "https:"
    ? `wss://${window.location.host}/sync/`
    : "ws://127.0.0.1:8790";
const LAST_PROJECT = "slimlocalize.websync.activeProject.v1";
const DATABASE_NAME = "slimlocalize.websync.v1";
const SCHEMA = 1;
const SEPARATOR = "\x1f";

type DiskDocument = {
  id: string;
  schema: 1;
  peerId: number;
  snapshot: Uint8Array;
  pending?: Uint8Array;
};
/** Enforce TLS for all remote relays and forbid credentials in URLs. */
export function validateCollaborationRelay(value: string): string {
  const url = new URL(value);
  if (url.username || url.password || url.hash || url.search) {
    throw new Error("Do not put collaboration credentials in a URL");
  }
  const local = ["127.0.0.1", "localhost", "[::1]"].includes(url.hostname);
  if (url.protocol !== "wss:" && !(url.protocol === "ws:" && local)) {
    throw new Error("Remote collaboration requires wss:// (TLS)");
  }
  return url.href;
}
function randomPeerId(): number {
  const data = new Uint32Array(1);
  crypto.getRandomValues(data);
  return data[0] || 1;
}
function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    if (typeof indexedDB === "undefined")
      return reject(new Error("IndexedDB is unavailable"));
    const req = indexedDB.open(DATABASE_NAME, SCHEMA);
    req.onupgradeneeded = () => {
      if (!req.result.objectStoreNames.contains("documents"))
        req.result.createObjectStore("documents", { keyPath: "id" });
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () =>
      reject(req.error ?? new Error("Could not open IndexedDB"));
  });
}
async function loadDisk(id: string): Promise<DiskDocument | undefined> {
  const db = await openDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction("documents", "readonly");
      const req = tx.objectStore("documents").get(id);
      req.onsuccess = () => resolve(req.result as DiskDocument | undefined);
      req.onerror = () => reject(req.error);
    });
  } finally {
    db.close();
  }
}
async function saveDisk(record: DiskDocument): Promise<void> {
  const db = await openDb();
  try {
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction("documents", "readwrite");
      tx.objectStore("documents").put(record);
      tx.oncomplete = () => resolve();
      tx.onabort = () =>
        reject(tx.error ?? new Error("IndexedDB transaction aborted"));
      tx.onerror = () =>
        reject(tx.error ?? new Error("IndexedDB transaction failed"));
    });
  } finally {
    db.close();
  }
}
function composite(
  input: Pick<SaveTranslationInput, "namespace" | "key" | "localeCode">,
): string {
  if (
    [input.namespace, input.key, input.localeCode].some(
      (part) => !part || part.includes(SEPARATOR),
    )
  )
    throw new Error("Invalid collaboration key");
  return [input.namespace, input.key, input.localeCode].join(SEPARATOR);
}
function decodeTranslation(
  id: string,
  value: unknown,
): {
  namespace: string;
  key: string;
  localeCode: string;
  value: string;
} | null {
  const parts = id.split(SEPARATOR);
  if (parts.length !== 3 || parts.some((p) => !p) || typeof value !== "string")
    return null;
  return { namespace: parts[0]!, key: parts[1]!, localeCode: parts[2]!, value };
}

export function createSynchronizedWebWorkspaceService(): WebSyncWorkspaceService {
  const base = createWebWorkspaceService();
  const changes = new Set<(projectId: string) => void>();
  const statusListeners = new Set<(info: WebSyncInfo) => void>();
  const peerListeners = new Set<(peers: WebPeer[]) => void>();
  let mirrorQueue: Promise<void> = Promise.resolve();
  let presence: WebPresence | undefined;
  let unlistenPeers: (() => void) | undefined;
  let localFocus: string | null = null;
  let localName =
    localStorage.getItem("slimlocalize.websync.displayName") ||
    `Guest ${crypto.randomUUID().slice(0, 4)}`;
  let info: WebSyncInfo = { status: "off", projectId: null };
  let doc: LoroDoc | undefined;
  let client: LoroWebsocketClient | undefined;
  let room: { destroy(): Promise<void> } | undefined;
  let unsubscribeDoc: (() => void) | undefined;
  let unsubscribeStatus: (() => void) | undefined;
  let persistence: Promise<void> = Promise.resolve();
  let connecting: Promise<void> | undefined;
  let currentProject: string | null = null;
  let currentPeer = 1;
  let relayAddress = DEFAULT_RELAY;
  // Team keys are never saved in localStorage, IndexedDB or pairing codes.
  let relayKey = "";
  // One-time rejoin handoff for the reload after importing a project.
  try {
    const cached = sessionStorage.getItem("slimlocalize.websync.rejoin.v1");
    sessionStorage.removeItem("slimlocalize.websync.rejoin.v1");
    if (cached) {
      const data = JSON.parse(cached) as {
        url: string;
        key: string;
        until: number;
      };
      if (data.until > Date.now() && data.until < Date.now() + 120_001) {
        relayAddress = validateCollaborationRelay(data.url);
        relayKey = data.key;
      }
    }
  } catch {
    /* Bad or unavailable session storage: require key again. */
  }
  let generation = 0;
  let applyingRemote = false;

  const updateStatus = (status: WebConnectionStatus, message?: string) => {
    info = { status, projectId: currentProject, message };
    for (const cb of statusListeners) cb({ ...info });
  };
  const dispatchChange = (projectId: string) => {
    for (const cb of changes) cb(projectId);
  };
  const notifyPeers = (peers: WebPeer[]) => {
    for (const cb of peerListeners) cb(peers);
  };

  const persist = (): Promise<void> => {
    if (!doc || !currentProject) return persistence;
    const document = doc;
    const id = `project:${currentProject}`;
    const peerId = currentPeer;
    // Serialize in event order; snapshots exclude pending imports, which are saved separately.
    persistence = persistence
      .catch(() => {})
      .then(async () => {
        const snapshot = document.export({ mode: "snapshot" });
        const pending = document.exportPendingUpdates();
        await saveDisk({
          id,
          schema: 1,
          peerId,
          snapshot,
          pending: pending ?? undefined,
        });
      });
    return persistence;
  };

  const mirror = async (projectId: string, source: LoroDoc) => {
    if (applyingRemote) return;
    applyingRemote = true;
    try {
      const payload =
        (source.toJSON() as { translations?: Record<string, unknown> })
          .translations ?? {};
      const translations = Object.entries(payload)
        .map(([key, value]) => decodeTranslation(key, value))
        .filter((value): value is NonNullable<typeof value> => value !== null);
      const locales = await base.listLocales(projectId);
      const existingLocales = new Set(locales.map((locale) => locale.code));
      for (const translation of translations) {
        if (!existingLocales.has(translation.localeCode)) {
          await base.addLocale(projectId, { code: translation.localeCode });
          existingLocales.add(translation.localeCode);
        }
        const current = await base.listTranslations(
          projectId,
          translation.localeCode,
          translation.namespace,
        );
        const prior = current.find((entry) => entry.key === translation.key);
        if (prior?.value === translation.value) continue;
        await base.saveTranslation({
          projectId,
          ...translation,
          status: prior?.status ?? "translated",
          description: prior?.description,
        });
      }
      // Rebuild the non-secret environment projection from the same document.
      // Never overwrite local-only overrides; these are private to this device.
      const remoteEnvironments =
        (source.toJSON() as { environments?: Record<string, unknown> })
          .environments ?? {};
      const remoteVariables =
        (source.toJSON() as { env_variables?: Record<string, unknown> })
          .env_variables ?? {};
      const allEnvironments = await base.listEnvironments(projectId);
      const known = new Map(
        allEnvironments.map((environment) => [environment.name, environment]),
      );
      for (const [name, kind] of Object.entries(remoteEnvironments)) {
        const validKind = safeEnvironmentKind(kind);
        if (!validKind || !name || name.length > 120 || known.has(name))
          continue;
        try {
          known.set(
            name,
            await base.createEnvironment(projectId, { name, kind: validKind }),
          );
        } catch (error) {
          console.warn("Could not import shared environment:", error);
        }
      }
      for (const [encoded, value] of Object.entries(remoteVariables)) {
        if (typeof value !== "string" || value.length > 100_000) continue;
        const id = parseEnvironmentKey(encoded);
        if (!id) continue;
        const env = known.get(id.name);
        if (!env) continue;
        const old = (await base.listConfigVariables(env.id)).find(
          (variable) => variable.key === id.key,
        );
        if (
          old?.visibility === "local" ||
          (old?.visibility === "public" && old.value === value)
        )
          continue;
        await base.setConfigVariable({
          environmentId: env.id,
          key: id.key,
          value,
          visibility: "public",
        });
      }
      dispatchChange(projectId);
    } finally {
      applyingRemote = false;
    }
  };

  const stop = async () => {
    generation++;
    unsubscribeDoc?.();
    unsubscribeDoc = undefined;
    unsubscribeStatus?.();
    unsubscribeStatus = undefined;
    await persistence;
    unlistenPeers?.();
    unlistenPeers = undefined;
    await presence?.destroy();
    presence = undefined;
    notifyPeers([]);
    try {
      await room?.destroy();
    } finally {
      room = undefined;
      client?.destroy();
      client = undefined;
      doc = undefined;
      currentProject = null;
      connecting = undefined;
      updateStatus("off");
    }
  };

  const start = async (projectId: string): Promise<void> => {
    if (
      currentProject === projectId &&
      (info.status === "connecting" ||
        info.status === "connected" ||
        info.status === "disconnected")
    )
      return connecting;
    if (currentProject) await stop();
    const myGeneration = ++generation;
    currentProject = projectId;
    updateStatus("connecting");
    connecting = (async () => {
      const id = `project:${projectId}`;
      const stored = await loadDisk(id);
      if (generation !== myGeneration) return;
      currentPeer = randomPeerId();
      const next = new LoroDoc();
      next.setPeerId(currentPeer);
      if (stored?.snapshot) next.import(stored.snapshot);
      if (stored?.pending) next.import(stored.pending);
      doc = next;
      const existing =
        (next.toJSON() as { translations?: Record<string, unknown> })
          .translations ?? {};
      // Bootstrap existing browser keys *once*, before joining the room. Never overwrite
      // a value already represented in this CRDT snapshot.
      const locales = await base.listLocales(projectId);
      for (const locale of locales) {
        for (const entry of await base.listTranslations(
          projectId,
          locale.code,
          null,
        )) {
          if (entry.value === null) continue;
          const key = composite({
            namespace: entry.namespace,
            key: entry.key,
            localeCode: locale.code,
          });
          if (!(key in existing))
            next.getMap("translations").set(key, entry.value);
        }
      }
      const localEnvironments = await base.listEnvironments(projectId);
      const mapEnvironments = next.getMap("environments");
      const mapVariables = next.getMap("env_variables");
      for (const env of localEnvironments) {
        if (mapEnvironments.get(env.name) === undefined)
          mapEnvironments.set(env.name, env.kind);
        for (const variable of await base.listConfigVariables(env.id)) {
          if (variable.visibility !== "public") continue;
          const key = environmentKey(env.name, variable.key);
          if (mapVariables.get(key) === undefined)
            mapVariables.set(key, variable.value);
        }
      }
      next.commit();
      await persist();
      if (generation !== myGeneration) return;
      unsubscribeDoc = next.subscribe((event) => {
        void persist().catch((error) => updateStatus("error", String(error)));
        if (event.by === "import") {
          mirrorQueue = mirrorQueue
            .catch(() => {})
            .then(() => mirror(projectId, next));
          void mirrorQueue.catch((error) =>
            updateStatus("error", String(error)),
          );
        }
      });
      const ws = new LoroWebsocketClient({
        url: validateCollaborationRelay(relayAddress),
      });
      client = ws;
      unsubscribeStatus = ws.onStatusChange((state) => {
        if (generation !== myGeneration) return;
        if (state === "connected") presence?.reconnected();
        else if (state === "disconnected") {
          presence?.disconnected();
          notifyPeers([]);
        }
        updateStatus(
          state === "connected"
            ? "connected"
            : state === "connecting"
              ? "connecting"
              : "disconnected",
        );
      });
      await ws.waitConnected();
      if (generation !== myGeneration) return;
      const auth = new TextEncoder().encode(relayKey);
      room = await ws.join({
        roomId: id,
        crdtAdaptor: new LoroAdaptor(next),
        auth,
      });
      presence = new WebPresence(localName);
      presence.setFocus(localFocus);
      unlistenPeers = presence.subscribe(notifyPeers);
      await presence.join(ws, id, auth);
      await mirrorQueue;
      await mirror(projectId, next);
      localStorage.setItem(LAST_PROJECT, projectId);
      updateStatus("connected");
    })().catch((error) => {
      updateStatus(
        "error",
        String(error instanceof Error ? error.message : error),
      );
      throw error;
    });
    return connecting;
  };

  return {
    ...base,
    // The service shares one domain contract: only values are CRDT-synced in this first version.
    platform: {
      ...base.platform,
      capabilities: { ...base.platform.capabilities, sync: true },
    },
    async saveTranslation(
      input: SaveTranslationInput,
    ): Promise<TranslationEntry> {
      const saved = await base.saveTranslation(input);
      if (doc && currentProject === input.projectId && !applyingRemote) {
        doc.getMap("translations").set(composite(input), input.value);
        doc.commit();
        await persist();
      }
      dispatchChange(input.projectId);
      return saved;
    },
    async createEnvironment(projectId: string, input: CreateEnvironmentInput) {
      const created = await base.createEnvironment(projectId, input);
      if (doc && currentProject === projectId) {
        doc.getMap("environments").set(created.name, created.kind);
        doc.commit();
        await persist();
      }
      dispatchChange(projectId);
      return created;
    },
    async setConfigVariable(input: SetConfigVariableInput) {
      const envs = currentProject
        ? await base.listEnvironments(currentProject)
        : [];
      const env = envs.find(
        (candidate) => candidate.id === input.environmentId,
      );
      const existing = (
        await base.listConfigVariables(input.environmentId)
      ).find((variable) => variable.key === input.key);
      if (existing?.visibility === "public" && input.visibility === "local") {
        throw new Error(
          "A previously shared variable cannot be made private retroactively. Create a different local variable and rotate exposed credentials if needed.",
        );
      }
      const saved = await base.setConfigVariable(input);
      if (
        env &&
        doc &&
        currentProject === env.projectId &&
        saved.visibility === "public"
      ) {
        doc.getMap("environments").set(env.name, env.kind);
        doc
          .getMap("env_variables")
          .set(environmentKey(env.name, saved.key), saved.value);
        doc.commit();
        await persist();
      }
      if (env) dispatchChange(env.projectId);
      return saved;
    },
    webSyncConfigure(url: string, teamKey: string) {
      if (currentProject)
        throw new Error("Disconnect before changing the collaboration server");
      const parsed = validateCollaborationRelay(url.trim());
      const key = teamKey.trim();
      if (parsed.startsWith("wss:") && key.length < 32) {
        throw new Error(
          "Enter the 32+ character team key provided by the server owner",
        );
      }
      relayAddress = parsed;
      relayKey = key;
    },
    webSyncServer() {
      return relayAddress;
    },
    webSyncPeers() {
      return presence?.peers() ?? [];
    },
    subscribePeers(callback: (peers: WebPeer[]) => void) {
      peerListeners.add(callback);
      callback(presence?.peers() ?? []);
      return () => {
        peerListeners.delete(callback);
      };
    },
    webSyncSetFocus(focus: string | null) {
      localFocus = focus;
      presence?.setFocus(focus);
    },
    webSyncSetName(name: string) {
      localName = name.trim().slice(0, 32) || "Guest";
      localStorage.setItem("slimlocalize.websync.displayName", localName);
      presence?.setName(localName);
    },
    async webSyncStart(projectId: string) {
      await start(projectId);
    },
    async webSyncStop() {
      localStorage.removeItem(LAST_PROJECT);
      await stop();
      relayKey = "";
      try {
        sessionStorage.removeItem("slimlocalize.websync.rejoin.v1");
      } catch {
        /* ignored */
      }
    },
    webSyncInfo() {
      return { ...info };
    },
    async webSyncShareCode(projectId: string) {
      return exportWebProjectCode(projectId);
    },
    async webSyncJoinCode(code: string) {
      const imported = importWebProjectCode(code);
      localStorage.setItem(LAST_PROJECT, imported.id);
      await start(imported.id);
      // The UI reloads after importing metadata: hand off credentials once for 2 minutes.
      if (relayKey && relayAddress.startsWith("wss:")) {
        try {
          sessionStorage.setItem(
            "slimlocalize.websync.rejoin.v1",
            JSON.stringify({
              url: relayAddress,
              key: relayKey,
              until: Date.now() + 120_000,
            }),
          );
        } catch {
          /* Browser disabled sessionStorage. Re-enter key after reload. */
        }
      }
    },
    subscribeChanges(callback: (projectId: string) => void) {
      changes.add(callback);
      return () => {
        changes.delete(callback);
      };
    },
    subscribeSync(callback: (info: WebSyncInfo) => void) {
      statusListeners.add(callback);
      callback({ ...info });
      return () => {
        statusListeners.delete(callback);
      };
    },
    // Called by the browser entrypoint only when the user previously enabled sync.
    async restoreWebSync() {
      const id = localStorage.getItem(LAST_PROJECT);
      if (id && (!relayAddress.startsWith("wss:") || relayKey)) await start(id);
    },
  } as WebSyncWorkspaceService;
}
