import { splitProps } from "solid-js";
import { cx } from "./components";

/** Bundled, stroke-based Lucide icons. SVG works in Tauri and web without CSS-generated icon classes. */
const lucide = {
  check: () => <path d="M20 6 9 17l-5-5" />,
  database: () => (
    <>
      <ellipse cx="12" cy="5" rx="9" ry="3" />
      <path d="M3 5v14a9 3 0 0 0 18 0V5M3 12a9 3 0 0 0 18 0" />
    </>
  ),
  download: () => (
    <>
      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3" />
    </>
  ),
  folder: () => (
    <path d="M20 20H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2Z" />
  ),
  "folder-plus": () => (
    <>
      <path d="M12 10v6m-3-3h6M20 20H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2Z" />
    </>
  ),
  globe: () => (
    <>
      <circle cx="12" cy="12" r="10" />
      <path d="M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10Z" />
    </>
  ),
  languages: () => (
    <>
      <path d="m5 8 6 6m-7 0 6-6 2-3H2m5-2v3m6 15 5-11 5 11m-9-4h8" />
    </>
  ),
  menu: () => <path d="M4 6h16M4 12h16M4 18h16" />,
  plus: () => <path d="M12 5v14M5 12h14" />,
  search: () => (
    <>
      <circle cx="11" cy="11" r="8" />
      <path d="m21 21-4.35-4.35" />
    </>
  ),
  settings: () => (
    <>
      <path d="M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8Z" />
      <path d="M20 12a8 8 0 0 0-.1-1.2l1.7-1.3-2-3.4-2 .8a8 8 0 0 0-2-1.2L15.2 3h-4l-.4 2.7a8 8 0 0 0-2 1.2l-2-.8-2 3.4 1.7 1.3a8 8 0 0 0 0 2.4l-1.7 1.3 2 3.4 2-.8a8 8 0 0 0 2 1.2l.4 2.7h4l.4-2.7a8 8 0 0 0 2-1.2l2 .8 2-3.4-1.7-1.3A8 8 0 0 0 20 12Z" />
    </>
  ),
  "sliders-horizontal": () => (
    <>
      <path d="M4 7h9m4 0h3M4 17h3m4 0h9" />
      <circle cx="15" cy="7" r="2" />
      <circle cx="9" cy="17" r="2" />
    </>
  ),
  upload: () => (
    <>
      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M17 8l-5-5-5 5M12 3v12" />
    </>
  ),
  wifiOff: () => (
    <>
      <path d="M2 2l20 20M12 20h.01M8.53 16.11a6 6 0 0 1 6.94 0M5 12.55a11 11 0 0 1 11.17-2.08M2 8.82a16 16 0 0 1 19.4 0" />
    </>
  ),
  "chevron-right": () => <path d="m9 18 6-6-6-6" />,
  "chevron-down": () => <path d="m6 9 6 6 6-6" />,
  x: () => <path d="M18 6 6 18M6 6l12 12" />,
  pencil: () => (
    <>
      <path d="M12 20h9M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z" />
    </>
  ),
  "more-horizontal": () => (
    <>
      <circle cx="5" cy="12" r="1" />
      <circle cx="12" cy="12" r="1" />
      <circle cx="19" cy="12" r="1" />
    </>
  ),
  "refresh-cw": () => (
    <>
      <path d="M20 11a8 8 0 0 0-14.9-3M4 4v4h4M4 13a8 8 0 0 0 14.9 3M20 20v-4h-4" />
    </>
  ),
} as const;

export type AppIconName = keyof typeof lucide;

export function AppIcon(props: {
  name: AppIconName;
  size?: number;
  class?: string;
  label?: string;
}) {
  const [local, rest] = splitProps(props, ["name", "size", "class", "label"]);
  return (
    <svg
      {...rest}
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      width={local.size ?? 16}
      height={local.size ?? 16}
      fill="none"
      stroke="currentColor"
      stroke-width="1.8"
      stroke-linecap="round"
      stroke-linejoin="round"
      class={cx("app-icon", local.class)}
      role={local.label ? "img" : undefined}
      aria-label={local.label}
      aria-hidden={local.label ? undefined : "true"}
    >
      {lucide[local.name]()}
    </svg>
  );
}
