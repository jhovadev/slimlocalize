import UnoCSS from "unocss/vite";
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig({
  plugins: [solid(), UnoCSS()],
  server: { host: "127.0.0.1", port: 5173 },
  build: { target: "es2022" },
});
