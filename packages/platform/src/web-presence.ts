/** Transient collaborator presence. Never persisted or treated as authentication. */
import { EphemeralStore } from "loro-crdt";
import { LoroEphemeralAdaptor } from "loro-adaptors/loro";
import type { LoroWebsocketClient } from "loro-websocket";

export interface WebPeer {
  id: string;
  name: string;
  color: string;
  focus: string | null;
}

const COLORS = ["#d8a279", "#a9bc91", "#8fafc7", "#c7a3c7", "#ddb98c"] as const;
const MAX_NAME = 32;
const randomId = () => crypto.randomUUID();
const normalizeName = (name: string): string =>
  name
    .trim()
    .replace(/[\x00-\x1f]/g, "")
    .slice(0, MAX_NAME) || "Guest";

export class WebPresence {
  private readonly id = randomId();
  private readonly key = `peer:${this.id}`;
  private readonly store = new EphemeralStore(35_000);
  private readonly adaptor = new LoroEphemeralAdaptor(this.store);
  private readonly listeners = new Set<(peers: WebPeer[]) => void>();
  private readonly color =
    COLORS[crypto.getRandomValues(new Uint8Array(1))[0] % COLORS.length]!;
  private name: string;
  private focus: string | null = null;
  private unsub: (() => void) | undefined;
  private heartbeat: ReturnType<typeof setInterval> | undefined;
  private room: { destroy(): Promise<void> } | undefined;
  private connected = false;

  constructor(name: string) {
    this.name = normalizeName(name);
  }

  async join(
    client: LoroWebsocketClient,
    roomId: string,
    auth = new Uint8Array(),
  ): Promise<void> {
    this.room = await client.join({ roomId, crdtAdaptor: this.adaptor, auth });
    this.connected = true;
    this.unsub = this.store.subscribe(() => this.notify());
    this.announce();
    this.heartbeat = setInterval(() => this.announce(), 12_000);
    this.notify();
  }

  announce(): void {
    if (!this.connected) return;
    this.store.set(this.key, {
      id: this.id,
      name: this.name,
      color: this.color,
      focus: this.focus,
    });
  }

  setName(name: string): void {
    this.name = normalizeName(name);
    this.announce();
  }
  setFocus(focus: string | null): void {
    this.focus = typeof focus === "string" ? focus.slice(0, 180) : null;
    this.announce();
  }

  peers(): WebPeer[] {
    if (!this.connected) return [];
    const states = this.store.getAllStates() as Record<string, unknown>;
    return Object.entries(states).flatMap(([key, state]) => {
      if (
        !key.startsWith("peer:") ||
        key === this.key ||
        !state ||
        typeof state !== "object"
      )
        return [];
      const p = state as Record<string, unknown>;
      if (
        typeof p.id !== "string" ||
        typeof p.name !== "string" ||
        typeof p.color !== "string"
      )
        return [];
      return [
        {
          id: p.id.slice(0, 80),
          name: normalizeName(p.name),
          color: COLORS.includes(p.color as (typeof COLORS)[number])
            ? p.color
            : COLORS[0],
          focus: typeof p.focus === "string" ? p.focus.slice(0, 180) : null,
        },
      ];
    });
  }

  subscribe(listener: (peers: WebPeer[]) => void): () => void {
    this.listeners.add(listener);
    listener(this.peers());
    return () => {
      this.listeners.delete(listener);
    };
  }
  private notify(): void {
    for (const listener of this.listeners) listener(this.peers());
  }

  disconnected(): void {
    this.connected = false;
    this.notify();
  }
  reconnected(): void {
    this.connected = true;
    this.announce();
    this.notify();
  }
  async destroy(): Promise<void> {
    if (this.heartbeat) clearInterval(this.heartbeat);
    this.heartbeat = undefined;
    if (this.connected) this.store.delete(this.key);
    this.connected = false;
    this.unsub?.();
    this.unsub = undefined;
    try {
      await this.room?.destroy();
    } finally {
      this.room = undefined;
      this.store.destroy();
      this.notify();
    }
  }
}
