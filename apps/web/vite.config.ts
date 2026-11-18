import UnoCSS from "unocss/vite";
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import wasm from "vite-plugin-wasm";

export default defineConfig({
  plugins: [solid(), UnoCSS(), wasm()],
  server: { host: "127.0.0.1", port: 5173 },
  css: { transformer: "lightningcss" },
  build: { target: "esnext" },
});
