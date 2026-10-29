# Evolution plan

Current implementation/evidence lives in [STATUS](docs/STATUS.md); product invariants in [CONTEXT](docs/CONTEXT.md). Architecture and UX remain the design references, not a completed-feature list. This plan is the single prioritized backlog.

## General direction

1. Ship a trustworthy macOS ingest vertical slice: plan → verify required copies → clean SD, with archive-only exception and deferred work.
2. Make catalog, recovery, Stalker and editor handoffs dependable without moving old terabytes.
3. Grow the dashboard and launcher on the same service boundary.
4. Expand automation/API/MCP capabilities and platform adapters only after the data-loss boundary is validated.

## Actualización de contrato · 2026-10-28

Exclusión de liberación con Clipper implementada/probada e instalada en build
9a2c9764c7eb; ver [STATUS](docs/STATUS.md). Antes de habilitar retiro en operación
real, reiniciar CLI/TUI y validar el ciclo pequeño con cola/relink y ambas builds
actualizadas. Conserva los gates operativos/SD de abajo. Cierre productor posterior en este working tree: evidencia de tiempo sin
inventar zona, miembros/media_bytes, hash/revisiones de manifest, mapping de
Clipper y retitle coordinado implementados. Evidencia y límites en
[STATUS](docs/STATUS.md); [handoff para las siguientes sesiones](docs/contracts/stalker-retitle-v1.md).

1. Stalker ya implementó capability/UI/CLI y nüün ya despacha recuperación
   `status:pending` bajo locks, revalidando origen y destino. Validar con builds
   actualizadas en el ciclo pequeño autorizado; fixtures no sustituyen aceptación física.
2. Clipper debe adoptar los campos aditivos de sources con fixtures antiguas;
   nueva revisión exige decisión explícita. Portable handoff aún sin consumidor.
3. Sólo después: aceptación física pequeña autorizada, builds reiniciadas y
   gates operativos de abajo. No se instaló/abrió una app en este cierre.

## Now · next sessions (2026-10-27)

| When | Task |
| --- | --- |
| Implementado 2026-10-27 · fixtures y QA visual aislada; aceptación física pendiente | Ingreso: SDs/fuentes conocidas, una fila en completo y dos en compacto/selección, tres proyectos directos + Más recientes/Otro…, fallback por unidad, inspección explícita y expulsión segura por el servicio compartido. Barra nativa en el mismo renglón y navegación persistente en fullscreen. [Flujo](docs/features/nuun-window.md), evidencia/build en [STATUS](docs/STATUS.md). |
| Corregido 2026-10-27 · regresión en fixtures y QA aislada | Transporte del plan como documento original, sin reserialización Swift; digest alterado sigue bloqueado. Ingreso compacto/selección desplaza revisión/errores sin cortar el pie. La suite Swift ahora prueba plan → apply real con números problemáticos y documentos alterados. Reintentar el caso reportado con una revisión nueva; no se ejecutó la tarjeta del usuario. Evidencia en [STATUS](docs/STATUS.md). |
| Prioridad operativa · Untitled inaccesible | Resolver el fallo de montaje ExFAT reportado por el usuario antes de retomar pruebas físicas. Sin job reciente/limpieza registrada; `fsck_exfat -n` ejecutado por el usuario confirma corrupción de boot/bitmap/directorio. Imagen completa en MEDIA502 propuesta, pendiente de autorización y ejecución. No reparar/formatear ni usar como fixture. Evidencia en [STATUS](docs/STATUS.md). |
| Siguiente nüün · aceptación y rendimiento | Un ciclo pequeño y explícito de SD → plan → copias/verificación → limpieza según cámara → expulsión manual; validar popup/remount y Stalker con proyectos por sesión. Después medir discovery frío/caliente, primera miniatura, hover, CPU/memoria en una biblioteca grande y throughput de copia frente al baseline. No ampliar paralelismo ni relajar hashes sin mediciones. H6/DJI/GoPro y matriz de pantallas/accesibilidad siguen pendientes. |
| Done 2026-10-26 · fixtures; aceptación real pendiente | Bandeja Stalker actual/anteriores + tags/sugerencias; Espacio con auditoría hash, liberación de raws/originales, Clipper deps/ACK y snapshots completos del catálogo. [Uso/límites](docs/operations.md#bandeja-stalker-proyectos-y-liberar-espacio); evidencia en STATUS. |
| Implementado e instalado 2026-10-27 · aceptación visual pendiente | Preview central/hover; resumen y fallback de proyecto por sesión en lotes Stalker; tarjetas compactas con metadata superpuesta; pestañas centradas en el topbar y controles de modo al extremo derecho; pies fijos; popup SD activado por defecto con opt-out. Raíces Stalker distintas aún requieren planes separados. Evidencia en STATUS. |
| Instalado 2026-10-27 · aceptación visual pendiente | Mostrar fases reales de hash, comparación del archivo, Clipper, revalidación, snapshot y retiro; conteos verificados por destino MEDIA/ESSD. Ignorar en discovery un `.stalker` que quedó con pura metadata Finder, sin borrar la carpeta. Build `8dd235483514` instalado en `~/Applications/nuun.app`; verificar que desaparezca el error `manifest.json` y que un lote muestre las etapas. [Evidencia](docs/STATUS.md). |
| Próxima sesión Stalker | Investigar nombre genérico `Stalker_...` pese a proyecto asignado desde nüün, y medir por qué la pista de micrófono parece acercarse al tamaño de video. Preguntas de captura agregadas al [contrato/prompt](docs/contracts/next-sessions.md). |
| Done 2026-10-26 | nüün: una ventana con tres modos animados, carga automática, previews sin cards, bandeja, ajustes por áreas y perfiles guardados/rutas congeladas. [Flujo/límites](docs/features/nuun-window.md), evidencia en STATUS. |
| Done 2026-10-26 | Shared surface palette and NSWindow chrome; light + #FFBB5C defaults, themed cards/fields/buttons, vertical compact importer. Native build/signing and visual validation; no ingest behavior changed. |
| Done 2026-10-26 | n creator utils identity/icon/native menus, theme + HSV, thumbnail grid, compact importer and explicit none/all/selected editing copies. 93 Python / 7 Swift tests; installed build `1cc34b5f090f`. Remaining: physical reconnect-popup trial, large-card UI/performance/accessibility and release gates. |
| Done 2026-10-26 | Native “Sólo respaldo” + plan with work omitted/cleanup enabled. Local installer rebuilds both surfaces to one registered app path, preserves prior bundle and rejects running instances. 7 Swift + 3 installer tests passed; installed build `c4820e569776` reviewed in UI at plan stage only. Portable release runtime/updater remain pending. |
| Done 2026-10-26 | CLI shorthand `save SOURCE COUNTRY PROJECT` and explicit `--no-work` for archive-only media with no deferred work. Six focused fixture tests; EOS_DIGITAL → MEDIA502 preview only, no physical copy/cleanup. [Usage](docs/operations.md#sólo-respaldo-sin-archivos-de-trabajo). |
| Done | Sony cycle closed 2026-10-25 (backup → camera format → `confirm-cleanup`). Camera format stays the Sony cleanup; see [sony-validation](docs/sony-validation.md). Next card types: H6, DJI, GoPro. |
| En código · aceptación real pendiente | Stalker → MEDIA/ESSD → reenlace/ACK → retiro local corre tras verificar dentro de **Ejecutar plan revisado**. Ajustes permite conservar el original y liberarlo desde Espacio después. Validado en fixtures; falta aceptar con una sesión pequeña. Las adopciones CLI/TUI de Stalker/Clipper ya constan en [contratos](docs/contracts/README.md); quedan leases, proyectos antiguos no adoptados y demás pedidos abiertos. |
| Installed · visual acceptance pending | Build `e9e84e130b5c` instalado en `~/Applications/nuun.app`. Revisar visualmente el nuevo titlebar/topbar, preview/hover y el lote de dos sesiones; luego aceptar popup al reconectar SD. No se abrió la app ni se ejecutó un plan de ingest. |
| Operational pending (manual) | Run `media coverage --work ESSD --archive MEDIA502` (read-only, hours). Review its report before any deletion. |
| After coverage review | Usar los planes implementados para archivar raws que sólo estén en ESSD y liberar por proyecto, revisando dependencias. Auditoría/liberación real aún no ejecutadas; backups de proyectos editoriales, rehash periódico y retención de snapshots siguen pendientes. Policy: [backup/work](docs/features/replication-policy.md). |
| First new card | Measure the parallel copy speed against the 829 s / 6.35 GB baseline. |

## P0 · close the current implementation

| Task | Acceptance / owner |
| --- | --- |
| Review plan validation, source cleanup races, receipt durability and replay | Independent review done 2026-10-25; durability, source re-read and metadata fixes applied. Remaining: authenticated/recorded plan digests, job owner/lease. `media/{planner,transfer,catalog}.py`. |
| Re-run the final snapshot and build a clean package | Full Python suite, Swift tests, current wheel contents, actual handoff schema/semantics and JSON CLI roundtrip. Record exact evidence in STATUS; no inherited green counts. |
| Harden recovery and the location graph | All relevant `asset_jobs`/locations can resolve; recovered work/ownership can reconcile without source SD; later human metadata has a durable backup/export strategy. `catalog/library/recovery`. |
| Physical acceptance | Sony procedure, H6 required SSD1/confirmed exception, DJI drone versus 360, ExFAT/HFS+ and ESSD reconnects using a small explicit trial. [Protocol](docs/sony-validation.md). |
| Freeze a release candidate | Align versions, bundle a verified relocatable runtime, package/sign/notarize and test Gatekeeper on a clean user. [Release gates](docs/release.md). |

Do not start by polishing more panels or testing full personal cards. Local fixture coverage precedes hardware; a hardware pass does not replace fault injection.

## P1 · complete daily workflows

| Feature | Done when |
| --- | --- |
| Stalker backup → local release | Implemented as one reviewed apply flow: verify required MEDIA/ESSD copies, query/relink Clipper with ACK, then journal the release. The default releases when all checks pass; Ajustes → Capturas can preserve the original. Unknown dependencies/queues and operation failures retain it. Validate this default on one real, small session before routine use. A five-session 1.59 GB manual release was verified on 2026-10-26; that was a separate historical operation. |
| Device/model registry and capture timestamps | Metadata evidence + user-confirmed model/category persist. DJI ambiguity resolved per file/unit, without global brand or residual-directory guesses. Timezone/precision/fallback explicit. |
| Native onboarding and first-run disk setup | User selects registered archive/work roles and sees identity, free/reserved space and capabilities. No absent-disk fallback. |
| Stalker operational closeout | Real recording/finalizing/recovery/multi-session events, event loss/remount and CPU/IO contention validated; producer finalization/revision agreement documented. |
| Work delivery and restoration UX | Pending/skipped/ready clear by project/selection, preview before large jobs, offline requests retained, order preserved. |
| Clipper handoff/relink adoption | Consumer accepts the library contract and ACKs complete session/voice/tracks/queue updates. No direct JSON edits here. Coordinated in Clipper's repo. |
| Work release and single-archive safeguards | Auditoría con hash, planes de liberación de raws y snapshots SQLite implementados (fixtures); aceptar en hardware y mantener revisión humana para consumidores no registrados. Falta backup de proyectos editoriales, retención y rehash periódico. [Policy](docs/features/replication-policy.md). |
| Stalker/Clipper naming adoption | Stalker names sessions per [naming v1](docs/contracts/media-naming-v1.md) at finalization; Clipper library picker keeps capture order and resolves bundles by manifest/IDs. Coordinated in their repos. |
| Deliveries and published archive | Bundle chosen variants/thumbnails/copy; explicit per-channel publication; `subidos` convention only a configurable suggestion. Idempotent verified archive into final collections. |

## P2 · dashboard, launcher and maintainability

Detailed, prioritized ideas: [dashboard/launcher backlog](docs/features/dashboard-launcher.md).

- Action center for blocked jobs, missing destinations, work pending and final deliveries pending archive.
- Storage/coverage view for logical versus allocated bytes, online/offline snapshots, replication and space reservation.
- Compact launcher/menubar commands with last-project choice, launch at login, real notifications and runtime health.
- Diagnostics bundle with redaction, version inventory, job events and copyable retry command; log/cache quotas.
- Imports/search for historical archives without bulk rename. Profiles/aliases and saved collections reusable by other surfaces.
- Downloads/stems as isolated derived-asset jobs; do not grow the media worker into another editor.

## P3 · broader automation and platforms

Read-only MCP stdio already exists. Mutations need reviewed-plan IDs/digests, clear client approval/cancel semantics and common error/progress contracts; do not expose arbitrary shell/filesystem tools.

Windows/Linux require their own volume identity/events/safe IO/locking, path and filesystem compatibility, packaging and hardware acceptance. The current native UI is macOS-only. Future HTTP/multi-user catalogs are not prerequisites for a local dashboard.

## Parallel sessions / task ownership

Use [development handoff](docs/development.md#cerrar-y-retomar-una-sesión). Choose one vertical feature/module, state files in scope, contract changes and validation. Separate UI work from transfer integrity review; shared protocol/catalog changes need coordination. Respect current staged/unstaged work. Update STATUS and this plan when a priority actually changes.


## Coordinación editorial · 2026-10-29

Handoff y zoom sostenido ya adoptados por Clipper CLI/TUI: no reimplementarlos.
Delivery de un archivo y consumidor de adopción project-only disponibles; ver
[contrato común](docs/contracts/media-library-v1.md#consumidor-clipper--extensión-del-29-octubre-2026).
Siguiente paso productor: diseñar el registro/publicación de revisión metadata-only
bajo un mismo asset con recibos y ambas copias conservadas, y su recorrido UI.
No relajar retitle con referencias ni la protección de colas para adelantarlo.
La aceptación física/builds efectivas sigue pendiente, independiente de fixtures.
