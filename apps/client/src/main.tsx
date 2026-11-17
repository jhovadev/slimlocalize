import { render } from "solid-js/web";
import "@unocss/reset/tailwind.css";
import "@slimlocalize/ui/tokens.css";
import "virtual:uno.css";
import "@slimlocalize/ui/styles.css";
import { AppShell } from "@slimlocalize/ui";
import { createTauriWorkspaceService } from "@slimlocalize/platform/tauri";

const root = document.getElementById("root");
if (!root) {
  throw new Error("Missing #root element");
}

render(() => <AppShell service={createTauriWorkspaceService()} />, root);
