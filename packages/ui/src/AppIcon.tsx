import { splitProps, type JSX } from "solid-js";

import { cx } from "./components";

const iconClasses = {
  check: "i-lucide-check",
  database: "i-lucide-database",
  download: "i-lucide-download",
  folder: "i-lucide-folder",
  globe: "i-lucide-globe",
  languages: "i-lucide-languages",
  plus: "i-lucide-plus",
  search: "i-lucide-search",
  settings: "i-lucide-settings",
  upload: "i-lucide-upload",
  wifiOff: "i-lucide-wifi-off",
} as const;

export type AppIconName = keyof typeof iconClasses;

export function AppIcon(props: {
  name: AppIconName;
  size?: number;
  class?: string;
  label?: string;
}) {
  const [local, rest] = splitProps(props, ["name", "size", "class", "label"]);
  return (
    <span
      {...rest}
      class={cx(iconClasses[local.name], local.class)}
      style={{ "font-size": `${local.size ?? 16}px` }}
      role={local.label ? "img" : undefined}
      aria-label={local.label}
      aria-hidden={local.label ? undefined : "true"}
    />
  );
}
