# SlimLocalize UX-04 + SYNC-01

## Implementación

**UX-04:** explorador de claves por namespace y segmentos separados por `.`. Usa la lista de traducciones existente y conserva los IDs; no modifica los esquemas de SQLite ni Loro. Es accesible mediante teclado y el mismo árbol se muestra en el drawer de escritorio/móvil.

**SYNC-01 (desarrollo local):** el cliente web usa `loro-crdt` + `loro-websocket` + `loro-adaptors` y persiste snapshots y actualizaciones pendientes en IndexedDB. El documento usa el `LoroMap("translations")` y la clave `namespace\x1fkey\x1flocale` usados por el núcleo Rust. Al recibir operaciones las proyecta sobre el adaptador web para que aparezcan en la tabla. Los cambios se guardan localmente incluso sin conexión.

La primera versión sincroniza **valores de traducción**, no configuración, roles, nombres de proyecto o secretos. El código de emparejamiento transporta solo metadatos de proyecto e identificadores; **no es una invitación con autenticación**. El relay está en modo de desarrollo y permite escrituras anónimas; debe permanecer en loopback.

## Prueba local

```bash
bun install
just sync-up
just sync-ps
just dev-web
```

En navegador A: crea un workspace, proyecto, idioma y al menos una clave. Abre **Settings → Share this project** y pulsa **Connect** y **Copy pairing code**.

En navegador B: usa un perfil diferente o ventana privada, abre la misma web, crea un workspace temporal si la app pide onboarding y abre **Settings** de un proyecto. Pega el código y pulsa **Join project**. El navegador se recargará con el proyecto compartido. Para facilitar esta operación, también puede usarse cualquier proyecto local provisional como entrada al panel de Settings.

Cambia un valor en A. Comprueba que aparece en B. Desconecta B (DevTools Offline), cambia valores en A y B, reconecta B y comprueba convergencia. Cierra y reabre B para validar la recuperación de IndexedDB. Haz las mismas comprobaciones después de reiniciar el relay.

**Limitaciones explícitas:** no se ha auditado la persistencia exacta de ACK del relay, no se ha implementado OAuth ni ACL y no debe exponerse el puerto 8790 a Internet. La colaboración Tauri ↔ web requiere que ambos dispositivos compartan también metadatos de proyecto y una conexión Rust en vivo, todavía fuera de este bloque. En modo de desarrollo el WebSocket es `ws://127.0.0.1:8790` y solo funciona en la misma máquina (o con un túnel local protegido).

## Aceptación

- `bun run typecheck`
- `bun run --cwd packages/ui test`
- `bun run build:web`
- `just ci`
- `just sync-relay-e2e`
- Dos perfiles de navegador con almacenamiento aislado, actualización A↔B, edición offline, reinicio del relay y recuperación de IndexedDB.

No publicar como sincronización de producción sin autenticación, pruebas reales de dos navegadores y auditoría de durabilidad del servidor.
