import { render } from "solid-js/web";
import "@unocss/reset/tailwind.css";
import "virtual:uno.css";
import "@slimlocalize/ui/styles.css";
import { AppShell } from "@slimlocalize/ui";
import { createWebWorkspaceService } from "@slimlocalize/platform/web";

const root = document.getElementById("root");
if (!root) {
  throw new Error("Missing #root element");
}

render(() => <AppShell service={createWebWorkspaceService()} />, root);
