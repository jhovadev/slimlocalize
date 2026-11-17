import { defineConfig, presetUno } from "unocss";

export default defineConfig({
  presets: [presetUno()],
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
