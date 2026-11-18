/** Shared environment projection helpers. Only non-secret, public values are eligible. */
import type { EnvironmentKind } from "./types";

const SEPARATOR = "\x1f";
const KINDS = new Set<EnvironmentKind>([
  "development",
  "staging",
  "production",
  "custom",
]);

export function environmentKey(name: string, key: string): string {
  if (
    !name.trim() ||
    !key.trim() ||
    name.includes(SEPARATOR) ||
    key.includes(SEPARATOR)
  ) {
    throw new Error("Invalid shared configuration identifier");
  }
  return `${name}${SEPARATOR}${key}`;
}

export function parseEnvironmentKey(
  value: string,
): { name: string; key: string } | null {
  const pieces = value.split(SEPARATOR);
  if (pieces.length !== 2 || pieces.some((part) => !part || part.length > 120))
    return null;
  return { name: pieces[0]!, key: pieces[1]! };
}

export function safeEnvironmentKind(kind: unknown): EnvironmentKind | null {
  return typeof kind === "string" && KINDS.has(kind as EnvironmentKind)
    ? (kind as EnvironmentKind)
    : null;
}
