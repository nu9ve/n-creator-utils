# Implementation status

Updated: 2026-10-27, America/Hermosillo. Checkpoint HEAD: `3ec01d3`; this session extends existing working changes with the unified window/profile/discovery work and the UI follow-up below. See the current Git diff before resuming. Older entries record the evidence and working state at their own dates. It is **not a production release**. [Next priorities](../PLAN.md), [release gates](release.md).

## Instalación solicitada · 2026-10-28

Build release local `b0c413211b4d`, compilada desde el working tree actual,
firmada ad-hoc y verificada por builder/instalador. Instalada a petición del
usuario en `~/Applications/nuun.app`, sin abrirla; backend Python instalado
comparado byte a byte con el repo. Artefacto `/private/tmp/nuun-latest-20261028.app`;
anterior conservada en `~/Library/Application Support/n-creator-utils/app-backups/55eaeaec.app`.
Runtime externo `/usr/bin/python3`, sin notarización ni nueva aceptación física.
Los límites de adopción Stalker/Clipper del cierre siguiente permanecen vigentes.

## Cierre productor Stalker/Clipper · 2026-10-28 · fixtures, sin instalación

Implementación en working tree sobre HEAD `3360d1d`. Se preservaron `.gitlab-ci.yml`
y `.vscode/` preexistentes sin editar; no commits/restage/reset. No se escribieron
repos/proyectos Clipper ni manifests Stalker externos, no se usaron SD, MEDIA,
ESSD o capturas personales. Nombres MEDIA/ESSD en tests son identidades fixture.

- `capture_time` aditivo en sources/handoff congela evidencia original y origen,
  instante absoluto sólo con offset, zona IANA/offset cuando existen. Prioridad
  Stalker epochWallClock → createdAt, Sony/ffprobe con procedencia. mtime/nombre
  legacy no inventan UTC; catálogos sin evidencia siguen legibles con null.
- `editable_members`, `media_bytes`, total `bytes`, hash crudo de manifest,
  revision/content_revision explícitos y mapping opaco `clipper_profile` (sin
  heurística, guardado por RPC de perfil, omisión de cliente antiguo lo conserva).
  Listar usa recibos/stat; no abre manifests ni hashea media para la lista.
- Handoff/schema/ejemplo real sintético: identidad, selección/orden y miembros
  por volumen con sus paths/hashes/recibos. No exige nombres iguales entre
  réplicas; mantiene documentos v1 anteriores. No es prueba de presencia actual.
- `retitle_capture`, `retitle_operations`, `cancel_plan` implementados en servicio
  y CLI. Archivo verificado de cualquier revisión, jobs/planes revisados activos,
  planes de release sin completar, referencias/colas/offline/desconocidos bloquean.
  Exclusión existente Clipper + writer de catálogo durante checks/mutación;
  ambos FDs heredados mantienen locks en el productor si muere nüün. Intent
  durable pending, recuperación mismo operation_id por status, validación CAS/ACK,
  publicación atómica de resultado + invalidación de planes anteriores. Nüün no
  escribe manifests ni adopta la revisión nueva en Clipper/catálogo de assets.

Evidencia: **174/174 Python** (`python3 -m unittest discover -s tests -q`,
27.048 s, pase final). Incluye **16 contratos** y **44 de liberación**, también
ejecutados de forma dirigida durante el cierre. Hay subprocess productor sintético
real, muerte de coordinador mientras el hijo retiene ambos locks, recuperación
sin repetir apply, muerte de propietario del writer, referencias/cola/offline,
planes cancelados/antiguos, CAS/ACK inconsistente, idempotencia/conflicto,
Stalker 1/2/3 y rechazo major desconocido, schema y handoff v1 antiguo. JSON Schema
con herramienta ya instalada, sin dependencia runtime nueva. El ejecutable fixture
sólo escribe su raíz temporal y no sustituye el journal/fsync de Stalker.
Los mensajes “Installed” de la suite son tests de instalador **simulados en tmp**;
no se construyó, instaló ni abrió una app real. Sin cambios Swift; no se ejecutó
captura, export, relink o limpieza personal. Compileall y `git diff --check` correctos; 119 enlaces locales revisados sin destinos faltantes.

**Límites / siguiente dueño:** Stalker todavía debe implementar/anunciar
`stalker-coordinated-retitle/1` (capabilities/status/apply, CAS propio, journal
manifest/rename/library y FDs heredados). Sin capability el coordinador bloquea
con guía; `archived:false` no autoriza. Clipper tiene que adoptar campos nuevos;
handoff portable aún sin consumidor. Exclusión de usuario/host/catálogo y apps
participantes; no cubre procesos viejos, otro catálogo/host ni Final Cut/Atlas.
No hay validación física ni nueva build instalada ni declaración de release.
[Contrato productor](contracts/media-library-v1.md#productor-aditivo-de-biblioteca--28-octubre-2026),
[handoff Stalker/API/schema/fixtures/aceptación](contracts/stalker-retitle-v1.md),
[siguientes dos sesiones](contracts/next-sessions.md#prioridad-vigente--2026-10-28).

## Contrato con Clipper · 2026-10-28 · backend instalado; aceptación física pendiente

`media/clipper.py` y `release.py` coordinan relink/retiro con Clipper mediante
`media-release-lock/1`: nüün posee el flock exclusivo durante comprobaciones y
mutación; token de operación autoriza sólo sus subprocesses. CLI anterior sin
capability y trabajo activo bloquean conservando origen. Cierre/crash libera
el lock del kernel. Requiere actualizar/reiniciar consumidores; otros hosts,
Final Cut y retitle de Stalker no participan. No cambia planner, hashes,
política de limpieza ni los gates de corrupción/SD documentados abajo.

Evidencia: 44/44 pruebas de `test_media_release.py` (20.294 s), con guard
inyectado en fixtures del motor; pruebas de protocolo real en Clipper dentro
de 146/146 dirigidas (41.69 s), incluyendo deps → relink por CLI → ACK → deps,
versión anterior, exclusión y recuperación después de kill. Todo con carpetas
sintéticas/catálogo aislado; sin copias ni limpieza de media personal.

Build release **9a2c9764c7eb**, `/private/tmp/nuun-contract-20261028.app`, firmado
ad-hoc y verificado, instalado en `~/Applications/nuun.app` sin abrirlo.
Anterior: `~/Library/Application Support/n-creator-utils/app-backups/fc442411.app`.
Backend de `media/clipper.py`/`media/release.py` coincide byte a byte entre repo,
bundle de revisión e instalación. Runtime externo `/usr/bin/python3`, sin
notarización. Sin cambios Swift, sin aceptación GUI/física ni declaración de
production-ready. [Contrato](contracts/media-library-v1.md#exclusión-local-de-liberación--28-octubre-2026),
[entrega Clipper](../../../ai/clipper/docs/2026-10-28-contract-hardening.md).

## Incidente operativo Untitled · 2026-10-27 · corrupción confirmada; preservación pendiente

Solicitud explícita del usuario: tarjeta antes legible ahora inaccesible. macOS detecta SD USB removible de 127,865,454,592 B, partición ExFAT `disk4s1`, UUID `C8BF5026-0077-3F03-AA08-019E0A1DC444`, sin MountPoint. `/Volumes/Untitled` no existe. El tamaño/espacio de volumen reportado como cero mientras está desmontada no demuestra que esté vacía. No se pudo listar su contenido ni confirmar borrado/formateo.

Logs locales: Finder solicitó expulsión a las 03:39:34 (America/Hermosillo); tras reconectar a las 03:40:16, FSKit ExFAT devolvió `Code=201`, seguido de fallo de montaje `0x00001001`. Las reconexiones/intentos de montaje registran además fases automáticas `repaired disk`; ese texto no prueba qué bytes se modificaron. Se intentó una vez `diskutil mount readOnly disk4s1` y falló. No se lanzó reparación ni formateo explícitos. `fsck_exfat -n /dev/rdisk4s1` fue rechazado por permisos y `sudo -n` exige contraseña; el diagnóstico no pudo ejecutarse. No hay fsck activo en segundo plano. Se pidió al usuario ejecutar el diagnóstico `sudo ... -n` directamente en Terminal, sin compartir su contraseña y reidentificando primero si reconecta.

Catálogo consultado con SQLite `mode=ro`: no hay job reciente de Untitled/terra-6-1. El único ingreso registrado de esa raíz es `0de68317-6a1d-573b-944a-0720db1dc1d5`, del 25, proyecto `debloateando-mi-tele`, 30 archivos, `cleaned_files=0`, acción/formato en cámara confirmado históricamente. Sus 30 copias de MEDIA502 siguen presentes con tamaño de recibo (sólo stat, sin rehash); de siete copias de trabajo, seis coinciden y una falta/cambió. Esto no confirma respaldo del material reciente de la tarjeta. El rechazo `Plan digest mismatch` ocurre antes de registrar job/copiar/limpiar.

El usuario ejecutó `sudo /sbin/fsck_exfat -n /dev/rdisk4s1` y compartió la salida: main boot region inválida (se pudo consultar la alternativa), sin upper-case translation table ni main allocation bitmap, entradas de directorio raíz inesperadas/fuera de fin, y volumen declarado corrupto. Todas las propuestas Extend/Delete/Replace/Create/Update se respondieron NO. Esto confirma corrupción de estructuras, no demuestra borrado de los archivos ni establece la causa. Se revalidó el mismo UUID y estado desmontado. MEDIA502 dispone de 2,691,439,788,032 B libres; ESSD sólo 23,113,080,832 B y no cabe una imagen completa de 127,865,454,592 B. Se ofreció preservar imagen en una carpeta nueva de MEDIA502 y revisar desde copia, sin reparar la SD. Esa copia aún no está autorizada/creada. Siguiente paso: confirmar el resguardo y ejecutarlo con acceso de lectura al dispositivo antes de considerar reparaciones. No usar esta tarjeta como fixture ni ejecutar plan/cleanup para probar la app. Estado: corrupción ExFAT confirmada, causa exacta y contenido recuperable todavía sin confirmar.

## Corrección de digest y overflow en Ingreso · 2026-10-27

La captura reportada combina `Plan digest mismatch` al ejecutar y un pie cortado cuando el plan/error hacen crecer compacto. El bridge anterior reconstruía el objeto JSON con Codable antes de devolverlo a Python; se reprodujo en fixtures un mismatch por representación numérica (`1.0` convertido a `1`). El JSON original del plan real fallido no se conservó para comparar sus campos, por lo que no se atribuye a un campo concreto de esa tarjeta. `plan_document` devuelve el documento canónico como string; la app conserva ese string y decodifica una copia sólo para presentación. Apply lo recibe intacto y mantiene todas las validaciones de digest/plan/preflight. El JSON general conserva además UInt64 para identidades. Un documento/digest alterado sigue bloqueado antes de crear job o copiar. Ante `plan_changed`, la UI descarta la revisión y pide otra.

Compacto y selección usan scroll para encabezado/opciones/revisión/error y mantienen el pie fuera. El texto del error crece y se puede seleccionar; al aparecer una revisión/error se desplaza hacia ese contenido. QA visual aislada reprodujo copia de todo el lote + trabajo diferido + plan + un error sintético de más de mil caracteres: texto y rutas accesibles, scroll funcional y pie/botón completos dentro de 440×700. El error se inyectó antes de transferir, sólo en el backend QA; no se ejecutó el plan real de la tarjeta.

Evidencia: **158/158 Python** (44.380 s), incluidos cuatro tests de documento/apply, y **9/9 Swift**. La regresión Swift usa proceso Python real con catálogo/volúmenes fixture: transporte reconstruido falla, no crea job, documento original verifica dos copias y conserva origen, modificación posterior vuelve a fallar. Se cubren UInt64 máximo y representación float integral. `git diff --check` y compileall. Build **`4e3c1fd0214b`**, [nuun-plan-document-20261027.app](../dist/nuun-plan-document-20261027.app), instalado en `~/Applications/nuun.app`. Bundle anterior en `~/Library/Application Support/n-creator-utils/app-backups/597f6193.app`. Firma ad-hoc, runtime externo `/usr/bin/python3`, sin notarizar. Se verificó la firma y que el backend/binario instalado coinciden con el artefacto; no se abrió la instalación principal ni se ejecutó el plan del usuario. Reintentar desde **Revisar plan** en el build actualizado; no reutilizar la revisión del proceso viejo. Aceptación del caso real pendiente.

## Ingreso, expulsión y barra nativa · 2026-10-27

Implementado el selector de SDs removibles/fuentes Stalker conocidas, con carpeta manual detrás de «Otra…». Completo agrupa fuente/expulsión a la izquierda, perfil/país/ciudad al centro y archivo/SSD a la derecha en un renglón; compacto/selección usan dos. Tres proyectos recientes son badges directos, con «Más recientes» y texto detrás de «Otro…». Cambiar el lote conserva proyectos individuales; Stalker requiere inspección antes de planear. Tipo de fuente, copia y limpieza siguen debajo y «Revisar plan» sigue como primaria. Elegir fuente/cambiar modo no escanea: inspección, elegir archivos y preparar lote son solicitudes explícitas.

`source_cards`/`eject_card` en el servicio compartido agregan expulsión normal y explícita, nunca automática. Revalidan identidad/montaje, rechazan archivo/SSD e imágenes y comparan stores actuales tras remount. Comparten el bloqueo interproceso de copia/verificación/limpieza; registro de destino también lo respeta. La UI bloquea expulsión durante operación/inspección. No se llama diskutil desde Swift ni se añade expulsión a transfer. El bloqueo cubre el mismo catálogo, no otras apps ni catálogos independientes. [Semántica local](contracts/media-library-v1.md#fuentes-y-expulsión-local-2026-10-27).

Pestañas, nombre/icono y modo pasan al renglón nativo con los controles del sistema. Se recompone el accesorio al terminar el resize animado para corregir clipping al reducir; en fullscreen se fija la misma barra sobre el contenido mientras el accesorio nativo está oculto. El pie conserva el estado final visible. Previews 16:9 reparten el ancho disponible, overlays de perfil tienen contraste explícito y la caché distingue centro/hover y reutiliza resultados de peticiones en espera. No hay benchmark que cuantifique mejora de velocidad de copia o de tarjetas grandes.

Validación: **154/154 Python** (`python3 -m unittest discover -s tests -v`, 42.256 s, incluidos **11 casos de expulsión**), **7/7 Swift** tras el último ajuste de titlebar, `compileall` y `git diff --check`. QA visual con app de identidad separada y backend limitado a `/private/tmp/nuun-ingest-qa`: completo/compacto/selección, tres badges, entrada manual, cuatro screenshots generados, cambio de proyecto de lote que conserva una unidad propia, plan con ambos proyectos, copia/verificación sin limpieza y expulsión simulada posterior. Elegir SD no produjo discovery antes de pedir inspección. Fullscreen conserva navegación y el regreso a compacto conserva barra/modelo. No se usaron contenidos de SDs personales, MEDIA o ESSD como fixtures; la expulsión física no fue ejecutada.

Build **`d272b7e2cb92`**, artefacto [nuun-ingreso-20261027-validated.app](../dist/nuun-ingreso-20261027-validated.app), instalado en `~/Applications/nuun.app`. Versión previa preservada en `~/Library/Application Support/n-creator-utils/app-backups/da85f69e.app`. Firma ad-hoc verificada, runtime externo `/usr/bin/python3`, sin notarizar. La instalación principal no se abrió tras reemplazarla; la revisión visual se hizo en QA aislada. **111 enlaces locales** revisados sin destinos faltantes. La aceptación física de remount/popup/expulsión y el ciclo pequeño Stalker siguen pendientes, junto con benchmarks grandes, pantallas/Spaces/accesibilidad y gates de release. Este build es de desarrollo; no demuestra preparación para producción.

## Stage progress and Stalker residue · code in working tree · 2026-10-27

Transfers now report verified-file counts per destination (MEDIA/ESSD), verified bytes across all destinations, and copy bytes counted once per source file. Stalker release reports its current stage, session, file, and bytes read from that file: source hashing, archive matching, bundle validation, Clipper/dependencies, work-copy verification, revalidation, catalog snapshots, and journaled release. Stage changes persist immediately; updates within a stage are limited to about one per second. The UI uses actual counts/bytes rather than an estimated percentage.

The job shown in the screenshot (`22ee38a6-5622-502c-b52e-0bcdcecf02e5`) finished `completed`: 14/14 verified copies on each of MEDIA502 and ESSD, two sessions released, and two local release plans completed with 7/7 files each. The `manifest.json` error came from the post-operation refresh: the source retained one `.stalker` folder with only `.DS_Store` (6148 B), which release intentionally ignores, so the directory itself stayed. Discovery now ignores a manifest-less Stalker folder only when every entry is Finder metadata or an empty directory; missing-manifest folders containing any other file still fail. No metadata was manually removed.

Validation: **65/65** `test_media_engine.py`, **44/44** `test_media_release.py`, **7/7** Swift tests, `compileall`, `git diff --check`, and bundle signature verification. Build **`8dd235483514`** at [Nu9veMedia-local-20261027-progress.app](../dist/Nu9veMedia-local-20261027-progress.app) uses external `/usr/bin/python3`, is ad-hoc signed, and is not notarized. Installed to `~/Applications/nuun.app`; prior app preserved at `~/Library/Application Support/n-creator-utils/app-backups/aea5e3e3.app`. No media contents were opened and the app was not launched or visually checked. Verify the real UI refresh/progress on next use. Investigation of generic Stalker folder naming and large microphone tracks is recorded for the next Stalker session.

## Stalker batch + compact window · 2026-10-27 · installed; visual acceptance pending

Stalker planning now allows an empty batch project when each session already has a project; the batch value fills only sessions without one, and an incomplete batch gets assignment guidance. The pre-plan and reviewed-plan summaries list the projects per session. Stalker preview cards overlay date/duration/tracks and profile on the midpoint video thumbnail; an assigned project is a compact label with a pencil, while quick project choices appear only for unassigned/editing sessions. Editing a project/profile/country/camera/copy selection invalidates any reviewed plan. The full window moves centered navigation into a 50 pt native titlebar accessory, removes the repeated logo/header row, and places the mode button at the far right. Compact/selection uses the same top row for “Ingreso” and expand.

Evidence: **43/43 release/Stalker fixtures**, **64/64 media-engine fixtures**, **7/7 Swift tests**, release bundle build/signature verification, and `git diff --check` passed. Build **`e9e84e130b5c`** (`dist/Nu9veMedia-local-20261027-0207-stalker-batch.app`) is installed at `~/Applications/nuun.app`; the installer preserved the previous bundle at `~/Library/Application Support/n-creator-utils/app-backups/9b8aefe7.app`. The app was not opened or visually inspected, and no real media or volumes were accessed. Check visual spacing/titlebar accessory behavior and the reported two-session workflow before treating the UX as accepted. This is not a production/release-ready build.

## UI follow-up · 2026-10-27 · code only

Added an AVFoundation video thumbnail path that samples the midpoint, then samples one of six coarse positions when hovering; image thumbnails remain Quick Look and thumbnail concurrency/cache limits remain bounded. Stalker discovery now exposes the manifest's safe screen-video path, duration and local timestamp to the Inbox and ingest grid. Inbox rows show video preview/date/duration/track count, three recent project buttons are directly selectable, the library-level action prepares every pending session under that root in one reviewed plan, and the app has fixed contextual footers for Ingreso, Biblioteca, Actividad, Bandeja and Espacio. The compact/selection importer also pins its plan action in a footer. The mount-popup preference now defaults to enabled for new users; a saved user opt-out still wins.

Checks at that point: **3/3 Stalker engine fixtures**, **9/9 capture workflow fixtures**, and **7/7 Swift tests/build** passed; `git diff --check` passed. The app was not launched or visually inspected, and no personal volumes/media were read, copied or removed. Visual acceptance of video thumbnails/hover and footers, and a real remount-popup check, remain pending. A batch is one library root per plan; separate Stalker roots still require separate plans.

## Stalker backup + local release · 2026-10-27 · installed; visual acceptance pending

`Ejecutar plan revisado` now owns the full Stalker flow: write/verify required MEDIA and optional ESSD copies, inspect Clipper, relink through its public CLI and validate ACK when needed, then run the catalog-recorded release journal for each local `.stalker` bundle. `Ajustes → Capturas → Conservar el original Stalker después del respaldo` defaults off; plans freeze this choice, and turning it on preserves originals for manual release later. Unknown dependencies, queued exports, unavailable Clipper or failed revalidation retain the source and report the reason. An interrupted per-session release resumes from the same recorded plan. This is a journaled workflow, not an ACID transaction across volumes, the catalog and Clipper. Current follow-up checks and installation evidence are recorded in the newer entry above.

Evidence: **40/40 release/Clipper fixtures**, **64/64 media-engine fixtures**, **7/7 Swift tests**, `compileall` and `git diff --check` passed. The local development bundle `dist/Nu9veMedia-local-20261027-013555-3f10d1.app`, build ID **`ed53ad7dc3da`**, is installed at `~/Applications/nuun.app`. The installer preserved the previous build at `~/Library/Application Support/n-creator-utils/app-backups/3b71b831.app`. The app was not opened or visually inspected; no real volumes or media were accessed. This is not a production/release-ready build.

## Stalker / espacio · implementación 2026-10-26

Bandeja descubre `stalker-library/1` actual + anteriores, muestra sesiones sin proyecto y guarda etiquetas por UUID sin reescribir manifests. Sugerencia de proyecto/país versión 1 confirmable y asignación por unidad. Espacio agrega auditoría con rehash actual, planes registrados de liberación por carpeta de raws/original Stalker, revisión de Clipper vía CLI/ACK, preferencia de ESSD verificado para reenlace local y bloqueo por colas/proyectos offline/antiguos no adoptados. Backup completo SQLite/perfiles/tags/decisiones a MEDIA, verificado, explícito y antes/después del retiro; también tras ingreso por `apply`.

[Uso y límites](operations.md#bandeja-stalker-proyectos-y-liberar-espacio). **Operación solicitada 2026-10-26:** cinco bundles Stalker completos de `~/Movies/Stalker`, asignados a `nuun-creator-utils` (1), `terra-6-1` (2) y `soundshop-tests` (2), liberados mediante cinco planes registrados después de hash actual en origen/MEDIA/ESSD. Total: **31 archivos, 1,593,007,238 bytes**. Los cinco terminaron `completed`; 7/7 o 3/3 copias por bundle permanecen en MEDIA502 y ESSD, los cinco originales ya no existen y los snapshots previos/finales del catálogo se guardaron sin errores. Clipper no reportó dependencias, colas ni relinks para esas fuentes; no había procesos abiertos de Clipper/nüün. La operación se lanzó por el mismo servicio local, sin abrir la UI.

La eliminación posterior al backup sigue siendo una prioridad de producto, no un comportamiento automático: hoy la oferta/revisión y confirmación de liberar es un segundo paso. No se abrió la app ni se hicieron pruebas visuales en este cierre. No significa aceptación física ni release de producción. La fila de Clipper del contrato documenta CLI/TUI ya implementados; aún no hay lease compartido ni garantía para proyectos/consumidores fuera de registros conocidos. Los proyectos antiguos no adoptados bloquean el retiro y requieren resolver su enlace en Clipper. No se archivan/retiran proyectos editoriales ni renders desde el nuevo plan de raws.

Evidencia final de este cambio: **134/134 Python** (`python3 -m unittest discover -s tests -v`, 37.848 s; 36 pruebas nuevas de este flujo), **7/7 Swift** y build release/firma ad-hoc verificados. `compileall`, `git diff --check` y 115 enlaces locales revisados. Artefacto `dist/nuun-stalker-space-20261026.app`, build **`affd140113f9`**, instalado en `~/Applications/nuun.app` después de que el usuario cerró la instancia anterior; no se abrió después. [Cierre y aceptación pendiente](releases/2026-10-26-stalker-space.md).

## Implemented

| Capability | Owner | Current limit |
| --- | --- | --- |
| Safe `save`, explicit legacy command, project/profile/country/city | [save](../media/save.py), [entry](../nu9ve.py) | Flags plus `save SOURCE COUNTRY PROJECT` shorthand; `--no-work` explicitly archives with work skipped. Historical engine remains `legacy-save`. Named layout restores historical names/folders (Sony XML duration/date verified on a real ILCE-6400 card; H6/DJI/GoPro naming only fixture-level). |
| Stable plan/digest, paired-file assignment, actual destination preview, required disks and space preflight | [planner](../media/planner.py) | Simple stem/sidecar pairing; DJI category explicit; date routing uses file mtime; no full EXIF/device registry. |
| Volume identity + physical-store overlap checks | [volumes](../media/volumes.py) | macOS diskutil/df; fixtures simulate devices. Exact cloned filesystem identity remains a limitation; other OS backends are incomplete. |
| Copy in bounded blocks, destination read-back SHA-256, per-file reattempts, mutation lock and journal | [transfer](../media/transfer.py), [IO](../media/common.py) | `F_FULLFSYNC` for files/dirs/receipts and SQLite `fullfsync`; `F_NOCACHE` on copy/verify reads is best effort (already-cached pages can still answer). No multi-GB hardware benchmark yet; F_FULLFSYNC support on ExFAT not measured. No rollback promise. |
| Cleanup after all required receipts; interruption recovery | [transfer](../media/transfer.py) | Each source file is re-read and must match the verified hash right before unlink. Finder metadata (`.DS_Store`, `._*`, `.TemporaryItems`) is ignored, not deleted. Material appearing during cleanup ends `source_has_new_material` (terminal). Stalker retains source; Sony media-only cleanup leaves camera indexes intact and remains fixture-tested (physical acceptance pending). |
| Explicit archive-only + pending/skipped work | [planner](../media/planner.py), [library](../media/library.py) | Registered expected work target required for pending. Later delivery is explicitly selected, not a silent bulk copy. |
| Creative profiles/workspace folders | [workspaces](../media/workspaces.py), [layout](../media/layout.py), [settings](../native/MediaMenu/Sources/MediaMenu/SettingsView.swift) | One work disk per job; profile disks are suggestions. Registry itself still needs metadata backup; plan routes are frozen. |
| SQLite catalog, stable project IDs/rename aliases, asset/project associations | [catalog](../media/catalog.py) | Shared-asset resolution still primarily uses the initial job. Broader relink/location policy needs review. |
| Restore, work delivery, handoff export and archive receipt recovery | [library](../media/library.py), [recovery](../media/recovery.py) | Recovery restores receipt-time metadata, not every later local decision. Recovered work-location/ownership reconciliation needs more coverage. |
| Per-variant/channel publication metadata | [publication](../media/publication.py) | Explicit registration only; does not upload or automatically archive `subidos`. |
| Completed Stalker discovery and incremental index | [captures](../media/captures.py), [planner](../media/planner.py) | Completed manifest revisions tracked; no producer lock/lease or cross-app relink ACK. |
| nüün menubar, unified compact/selection/full window, automatic discovery, inbox, assignments, plan review, library and sectioned settings | [app](../native/MediaMenu/Sources/MediaMenu/Application.swift) | Main flows implemented; final physical/UX/accessibility/performance acceptance pending. |
| Native mount/wake events and configured Stalker FSEvents | [app](../native/MediaMenu/Sources/MediaMenu/Application.swift), [watcher](../native/MediaMenu/Sources/MediaMenuCore/CaptureWatcher.swift) | Initial implementation; sustained real capture/idle tests and event-loss scenarios remain. No periodic idle scan. |
| Shared one-request JSON service; read-only MCP stdio | [service](../media/service.py), [MCP](../media/mcp.py) | MCP has no destructive tools or HTTP server. No installed external client integration yet. |
| App packaging helper | [builder](../scripts/build_media_app.py) | Development external runtime or supplied relocatable runtime; no provisioned release runtime/Developer ID/notarization. |

## Validation evidence

- 2026-10-26, nüün unified-window closeout: final artifact `dist/nuun-final-20261026.app`, installed at `~/Applications/nuun.app`, build **`2386647728a2`**, closed after installation (not reopened/tested visually after the user requested stopping visual tests). One AppKit window now renders compact/selection/full through shared WindowRoot/Model, with animated size transitions, Reduce Motion support, retained mode on reopen, manually controlled host sizing and native fullscreen handling. Auto discovery has an independent cancellable reader, generation IDs to discard obsolete responses and reuse by source/type/archive; lazy thumbnails remain bounded. Flat previews expose per-unit project/recent project/profile and camera classification; source counts/camera evidence/root routing summary precede the authoritative plan. Source kind and main navigation use icon buttons. Inbox and settings sections (profiles/disks/appearance/captures/diagnostics) added. Profiles are local config with relative raw folders and normalized volume IDs; routes are frozen into plans/receipts. Stalker-only discovery/planning/source validation now skip unrelated folders in a shared library, exclude active/already archived sessions consistently and expose manifest metadata. **98/98 Python tests passed** after final backend edits (including profile path/role validation, frozen routes, Stalker metadata/archived filtering/unrelated editor folder and installer migration). **7/7 Swift tests passed** after final read-generation changes; final release build, ad-hoc signature verification and installer passed. Before the stop request, UI build `1b1ddd17ea61` verified auto load of 104 JPG + Canon support file, same-window compact→selection→full, marking one item, review of a 105-file mixed plan without applying, preserved plan/selection on expansion, and profile/disk settings. Marks/plan were discarded; final pre-close state remained only archive. No live media copied/deleted. Search uses ASCII filename plus registered keywords; Spotlight metadata returned null and automated Spotlight UI was unavailable, so actual nuun/nun matching/ranking is pending. Final Stalker-only filter and fast-cancellation fixes were technically checked, not visually re-tested, as requested. [User acceptance and artifact record](releases/2026-10-26-local.md).
- 2026-10-26, coherent surface theme (UI-only): installed build **`bb3684deb158`**, release build + ad-hoc codesign verification passed. New [Appearance.swift](../native/MediaMenu/Sources/MediaMenu/Appearance.swift) owns shared seed-derived canvas/cards/fields/borders/buttons, native window background/appearance and transparent titlebar. Fresh defaults are Light + **#FFBB5C**; existing custom settings are preserved until explicit reset. The reset preset was applied for this user as requested. Selected-action text chooses black/white by luminance; native interactive tint is darker in light mode for legibility. Compact importer is now 408 pt wide, with vertical choices, wrapped explanations and explicit accessible selection values. Visual review on the final installed build covered light/dark surfaces and native chrome; returned to light and left the vertical importer open for EOS_DIGITAL / `kasakarla-electroritual`, none/only archive, cleanup enabled. No plan applied or media copied/deleted. No Python/transfer changes or backend-suite rerun; verification was compile/sign/install plus native UI review. System-theme switching and full accessibility/contrast matrix remain outside this check.
- 2026-10-26, native identity/grid/quick importer: installed **n creator utils.app**, build **`1cc34b5f090f`** at `~/Applications/`, with code-generated ICNS and LaunchServices registration; old Nu9ve Media install moved to app-backups. Normal app menus/Dock while windows are visible; accessory after all close. `Application.swift` now owns model/menu/events, `ImportViews.swift` owns grid, Quick Look thumbnails, appearance and compact importer. Three batch choices: none/all/selected editing copies, per-unit project entry, advanced profile/camera, cached thumbnails (4 active, 24 MB/160 entries), persisted system/light/dark + HSV accent, opt-in mount popup and manual demo/⌘I. **93/93 Python tests passed** on the final backend (including selected-only SSD copies, full archive+cleanup, later explicit delivery/handoff, partial deferred work, all-deselected confirmation and renamed installer migration). **7/7 Swift tests passed** before final copy-only label changes; final release build and ad-hoc signature checks passed after those labels. UI review on preceding build `828fb2613b65`: actual EOS_DIGITAL thumbnail grid, native menus, settings, and compact importer for the same project; no copies/cleanup. Final installed build opened and retains appearance preferences; its EOS_DIGITAL grid was loaded again for `kasakarla-electroritual`, left at none/only archive with cleanup enabled and no plan applied. Physical reconnect popup, selection performance on huge cards and broader accessibility/theme contrast acceptance remain untested; manual popup is not evidence of a mount event. No release-ready claim, no bundled Python runtime.
- 2026-10-26, native archive-only + local install (uncommitted): explicit “Sólo respaldo” works even with ESSD online and omits both work target and pending delivery. Plan invalidates when that choice changes; summary shows work status, bytes and cleanup. **7/7 Swift tests passed**, including two destination-policy tests and isolated Python RPC. **3/3 installer fixture tests passed** (preserved previous bundle, running-app refusal, rollback on swap failure). Release-mode Swift build, ad-hoc codesign verification and installation/LaunchServices registration succeeded at `~/Applications/Nu9ve Media.app`, build **`c4820e569776`**. Python is external `/usr/bin/python3`; backend source is bundled and resolved relative to installed Resources, overriding old runtime preferences. Launcher opens/reopens the window; `--background` keeps menubar only. UI review on the installed build, normal catalog: EOS_DIGITAL → MEDIA502, project `kasakarla-electroritual`, 105 files / 211.2 MB, work skipped, cleanup enabled, paths expanded. Left at **Ejecutar plan revisado**, never clicked; no media copied/deleted. Canon cleanup remains untested physically. Future code changes require rerunning [installer](../scripts/install_media_app.py); no automatic updater or portable runtime is claimed.
- 2026-10-26, CLI archive-only usability (uncommitted): restored `save SOURCE COUNTRY PROJECT` shorthand on the verified engine, with explicit `--no-work` (`skipped`, no SSD dependency), per-operation archive confirmation and per-file destination preview. **6/6 focused fixture tests passed** (`python3 -m unittest discover -s tests -p test_media_engine.py -k MediaSaveTests -v`): argument conflicts, separate city, preview without copying, `--yes` cannot authorize archive-only, declined confirmation, verified archive/cleanup with work offline and default SSD preserved. Full suite not re-run for this change. Read-only operational preview using a temporary copy of the catalog: EOS_DIGITAL has 104 JPGs + Canon CTG, 211,248,779 bytes, no recognized videos; MEDIA502 routing passed (images → `images/2026/09`, support → `metadata/generic/support`). No media copied/deleted. Model remains `generic` without metadata/device identification; this preview is not Canon cleanup hardware acceptance.
- 2026-10-25 on `5346663` (before the fixes below): Python 3.9.16 full suite **63/63 OK**; Swift **5/5 OK**.
- 2026-10-25 after review fixes (uncommitted): Python 3.9.16 and 3.12 full suite **68/68 OK** (new: transient bad SD read, Finder metadata, crash before owner marker, disk-image replica, new file during cleanup); Swift build without warnings and **5/5 OK**. Suite time rose from ~2 s to ~15 s because of real `F_FULLFSYNC`.
- Named layout (uncommitted): 71/71 Python tests on 3.9 and 3.12, including a Sony fixture card (dated names from XML, work gets only clips, support tree, handoff) and same-second collisions.
- **First live operational run, 2026-10-25** (user-requested): Sony ILCE-6400 SD (`Untitled`, ExFAT, 30 files, 6.35 GB) plus 3 completed Stalker sessions from `~/Downloads` (1.87 GB) → MEDIA502 (ExFAT) + ESSD (HFS+), project `debloateando-mi-tele`. Results: Sony `camera_action_required` in 829 s (~7.7 MB/s effective over copy+verify+second copy+verify), Stalker `source_retained` in 28/65/101 s. Receipts on both disks, no `.ncpartial` left, independent `shasum` matched SD/ESSD/MEDIA for a sampled clip and `mic.wav`. Nothing deleted: SD still holds its clips (format in camera), sessions remain in Downloads. An earlier attempt with the schema-1 nested layout was stopped by the user's choice; its partial work copy and catalog job were removed.
- 2026-10-25, second pass (uncommitted): Stalker bundles named `YYMONDD-HHMMSS_STALKER_MM-SS_…[].stalker` from the manifest so they interleave with clips; `media relayout JOB_ID` renamed the 3 real sessions in place on MEDIA502 and ESSD (no bytes copied); a fresh `recover-catalog` from MEDIA502 into a temporary catalog re-hashed all 4 jobs (8.2 GB, 70 s) and verified. That check found and fixed a real ExFAT bug: macOS `._*.json` AppleDouble companions broke receipt recovery.
- Copy engine now reads each source once and writes all destinations in parallel threads, verifies them in parallel, and records live progress (`phase`, copied/verified bytes, MB/s) shown by the app. Fixture-tested (75 tests); **speedup not yet measured on hardware**.
- 2026-10-25, third pass (uncommitted): `media coverage` read-only audit (fixtures only, **not yet run on ESSD/MEDIA**); `media captures` CLI. Fixed a real bug: Stalker's uppercase `sessionID` made archived sessions look pending in `captures` and in the app's badge; the 3 real sessions now report `archived: true`. Added `media confirm-cleanup` (plus an app button) to close a Sony job after the camera format; the pre-format index snapshot of the real card is kept for comparison. Python 78/78.
- **First complete card cycle, 2026-10-25:** Sony SD backed up → formatted in camera → `confirm-cleanup` → job `completed`. Pre/post index diff in [sony-validation](sony-validation.md). Found and fixed: a stalled `diskutil`/`df` on a freshly mounted card crashed the CLI instead of returning a JSON error.
- Editor interface (uncommitted): read-only `media projects` and `media sources --project` (ordered sources, local paths, IDs, hashes, `captured_at`/`duration_seconds`) for Clipper quick import; 1.7 s on the real project. Contract index now has an adoption table, the stable local interface and edit rules for other sessions. Python 80/80 (also under TZ=UTC).
- Uncommitted: `sources` checks presence/size and reports hand-deleted work copies (`removed_from_work`) and lost archive copies (`warnings`) instead of failing. Sony profile `sony_clips_then_format` (fixture-tested only; next real card pending). Camera/screen N:M grouping case documented for Clipper's autogroup. Python 80/80.
- Clipper check (read-only, in its repo): `expand_inputs` on the real project folder lists 7 clips + 3 renamed bundles in capture order.
- [CI](../.gitlab-ci.yml) added: Python 3.9/3.12 in Linux containers, Swift as a manual macOS job. **Not yet run on GitLab or Linux** (no local Docker daemon).
- Independent data-loss review (2026-10-25): no logic path deletes a source without same-run verified copies on every required destination. Fixed: drive-cache durability, single-SD-read replicas, Finder metadata blocking jobs, crash before owner marker, disk images counted as independent replicas. Open: see blockers 1.
- Failure fixtures include same-size corruption, missing/replaced/overlapping disks, insufficient space, symlinks, concurrent writer, changed source, partial cleanup and cancel/resume. Three real child-process hard exits exercise recovery during copy, after receipts and immediately after unlink.
- Native: latest recorded build completed; **5 Swift tests passed**, including a real Python RPC call with isolated state and exact 64-bit nanosecond roundtrip. Swift tools 6.3.3, target macOS 14+.
- Native UX fixes (uncommitted, compiled, **not visually re-tested**): a mounted card notifies once and appears as a suggestion instead of opening the window or replacing an in-progress source; the last project is offered ("Usar último") instead of prefilled; labels for Stalker-on-card and `source_has_new_material`.
- UI smoke test (earlier snapshot): a development .app, isolated catalog and two tiny local fixture files. Inspected paired files, showed SSD absent, requested archive-only confirmation, applied verified copy with source retained and displayed work pending. No real SD cleanup. Latest FSEvents/routing refinements were compiled but not re-tested visually.
- Earlier wheel/schema/link checks belonged to the earlier inventory-only snapshot. They are **not** evidence that the latest full package/handoff is release-validated. Rebuild/validate current outputs before release.

## Artifacts and local test context

| Item | Meaning |
| --- | --- |
| `native/MediaMenu/.build/.../debug/MediaMenu` | Most recently compiled native source. Ignored build output. |
| `dist/Nu9veMedia-dev.app`, `dist/Nu9veMedia-review.app` | Ad-hoc development snapshots. Existing outputs are deliberately not overwritten by the builder. Neither is notarized/current-release quality. |
| `/private/tmp/ncreator-media-ui-20261025` | Isolated UI catalog. Never confuse with the normal user catalog. |
| `/private/tmp/ncreator-media-ui-fixture-20261025` | Tiny test source/archive/work folders; no personal media. |

The review app was opened using the isolated `NU9VE_MEDIA_HOME`; its development runtime was pointed at the checkout's `nu9ve.py`. A running app may therefore use newer Python than its bundled source snapshot. Rebuild a fresh artifact for reproducible review; do not distribute this session's test bundle.

## Release blockers / known gaps

1. Remaining review items: plan digest is unauthenticated, so `apply` trusts plan-time checks (DJI review, archive-only confirmation) a caller could skip — record reviewed digests in the catalog or re-run checks; jobs have no owner/lease, so a killed run looks active; `progress.verified_copies` also counts delivery/recovery rows. Fresh package pass and physical SD/ExFAT disconnect trials remain necessary.
2. Sony cleanup acceptance and samples/rules for the actual DJI models; full metadata dates and device persistence are not implemented.
3. Full SQLite snapshots now back up human decisions/profiles; automatic retention and in-app full-state restoration remain open. Secondary-job location resolution, recovery/work ownership reconciliation and active-consumer leases still need hardening. Receipt recovery alone is not a complete backup of human metadata.
4. Clipper CLI/TUI has library sources/relink/deps (see its adoption row); nüün now requests relinks and validates ACKs. Handoff v1 consumption, consumer leases and real cross-app acceptance remain open. Old unadopted projects block release. Atlas/FCP integrations are design only.
5. No automatic archive of published deliveries or complete dashboard/launcher/onboarding. No runtime provisioning, notarization, Gatekeeper signoff or reproducible installer. Python package/native app versioning must be aligned before release.
6. Windows/Linux product support is not validated. Current secure IO/locking and native UI are macOS/Unix-oriented.

Do not use this status to authorize live media operations. Registration and backup runs must use the user's chosen disks and reviewed plans; automated tests stay in fixtures. No changes were made to the other repositories or sent to other chats.
