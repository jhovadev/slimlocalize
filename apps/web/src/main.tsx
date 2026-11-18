import { render } from "solid-js/web";
import "@unocss/reset/tailwind.css";
import "@slimlocalize/ui/tokens.css";
import "virtual:uno.css";
import "@slimlocalize/ui/styles.css";
import { AppShell } from "@slimlocalize/ui";
import { createSynchronizedWebWorkspaceService } from "@slimlocalize/platform/web-sync";

const root = document.getElementById("root");
if (!root) {
  throw new Error("Missing #root element");
}

const service = createSynchronizedWebWorkspaceService();
// Restore a project only if the user explicitly enabled local collaboration before.
void service
  .restoreWebSync()
  .catch((error) => console.warn("Local collaboration unavailable:", error));
render(() => <AppShell service={service} />, root);
