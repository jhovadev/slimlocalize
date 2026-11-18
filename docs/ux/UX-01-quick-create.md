# UX-01 — Compact editor and Quick Add

Scope: UI only. Existing `WorkspaceService` keeps responsibility for writes/validation and local persistence. No schema or backend changes.

## Changes

- Translation details no longer occupy a permanent 360px side panel. Select a row to open it; press Close to reclaim the table.
- New Key opens a keyboard-trapped quick-create dialog with key, value and namespace. `Create & add another` supports repeated entry.
- Cells can be edited directly with click; Enter commits, Escape cancels, blur commits. Errors remain visible and edits can be retried.
- `Ctrl/Cmd+K` focuses search and `Ctrl/Cmd+Shift+K` opens Quick Add when a project has locales.
- Empty states provide a context-aware primary action; no-match results offer Clear search.
- The diagnostic Loro collaboration block is hidden behind Sync details, not presented as the main editing task.
- Layout uses semantic CSS tokens and adapts to narrower viewports, including a bottom-sheet-like quick-create dialog.

## Verification requested on the actual repository

1. Confirm branch/commit and `git apply --check` before applying; do not force.
2. `bun run typecheck`, `bun run test`, `bun run build:web`.
3. `just dev-web` and `just dev-desktop`: create key, add another, retry invalid input, inline edit, keyboard search, close details.
4. Test 390, 768, 1024, 1440 pixel viewport widths, light and dark, especially mobile WebView scrolling.
5. Confirm SQLite persistence by relaunching desktop. No MCP, collaboration, server or storage schema changes expected.

## Limitations

- This is a focused UX vertical slice, not an entire shadcn/Solid UI migration.
- Table still shows one selected locale and namespace, not the planned multi-language grid.
- No live peer-presence, authenticated sharing, or remote MCP is added.
