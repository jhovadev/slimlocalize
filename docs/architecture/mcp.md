# MCP

The local MCP module exposes read-only tools over the shared domain service:

- `list_workspaces`
- `list_projects`
- `list_locales`
- `get_translation`
- `search_translations`
- `find_missing_translations`
- `validate_project`
- `list_environments`

`propose_translation` is documented as a future contract but is not callable.
Configuration values are intentionally not exposed through MCP.

Remote MCP requires authentication, authorization and transport security.
