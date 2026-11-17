import type { JSX } from "solid-js";

type IconProps = { size?: number; class?: string };

function makeIcon(path: JSX.Element): (props: IconProps) => JSX.Element {
  return (props) => (
    <svg
      width={props.size ?? 16}
      height={props.size ?? 16}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.8"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
      class={props.class}
    >
      {path}
    </svg>
  );
}

export const IconFolder = makeIcon(
  <path d="M3 7a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" />,
);
export const IconGlobe = makeIcon(
  <>
    <circle cx="12" cy="12" r="9" />
    <path d="M3 12h18M12 3a15 15 0 0 1 0 18a15 15 0 0 1 0-18Z" />
  </>,
);
export const IconCheck = makeIcon(<path d="m4 12 5 5L20 6" />);
export const IconPlus = makeIcon(<path d="M12 5v14M5 12h14" />);
export const IconSearch = makeIcon(
  <>
    <circle cx="11" cy="11" r="7" />
    <path d="m20 20-3.5-3.5" />
  </>,
);
export const IconDownload = makeIcon(
  <path d="M12 3v12m0 0 4-4m-4 4-4-4M4 19h16" />,
);
export const IconUpload = makeIcon(
  <path d="M12 16V4m0 0 4 4m-4-4-4 4M4 20h16" />,
);
export const IconDatabase = makeIcon(
  <>
    <ellipse cx="12" cy="5" rx="8" ry="3" />
    <path d="M4 5v14c0 1.7 3.6 3 8 3s8-1.3 8-3V5M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3" />
  </>,
);
