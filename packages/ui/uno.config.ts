import { defineConfig, presetIcons, presetWind4 } from "unocss";

export default defineConfig({
  presets: [
    presetWind4(),
    presetIcons({
      scale: 1.2,
      extraProperties: {
        "display": "inline-block",
        "vertical-align": "middle",
      },
    }),
  ],
  content: {
    pipeline: {
      include: [
        /\.([jt]sx?|html|mdx?)$/,
        /packages\/ui\/src\/.*\.(tsx|ts)$/,
        /apps\/(client|web)\/src\/.*\.(tsx|ts|html)$/,
      ],
    },
  },
  shortcuts: {
    "app-shell": "grid min-h-screen grid-rows-[auto_1fr]",
    "panel": "rounded-xl border border-line bg-surface shadow-sm",
    "field": "flex flex-col gap-1 min-w-0",
    "btn": "inline-flex items-center justify-center gap-2 min-h-8.5 px-3.25 rounded-md text-sm font-semibold transition-colors",
  },
  theme: {
    colors: {
      canvas: "var(--color-canvas)",
      surface: "var(--color-surface)",
      "surface-muted": "var(--color-surface-muted)",
      line: "var(--color-border)",
      text: "var(--color-text)",
      secondary: "var(--color-text-secondary)",
      muted: "var(--color-text-muted)",
      accent: "var(--color-accent)",
      "accent-soft": "var(--color-accent-soft)",
      success: "var(--color-success)",
      warning: "var(--color-warning)",
      danger: "var(--color-danger)",
    },
  },
});
