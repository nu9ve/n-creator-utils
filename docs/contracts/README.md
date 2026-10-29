# Contratos del ecosistema de media

Actualización 2026-10-27: hay productor de handoff, recibos, servicio JSON y MCP read-only en este repo. El contrato entre apps sigue **en revisión**; Stalker y Clipper (CLI/TUI) ya lo adoptaron con los límites de su fila. Estado completo: [STATUS](../STATUS.md). Propuesta inicial: 2026-10-24. Este directorio es el índice central de almacenamiento e intercambio entre n-creator-utils, Stalker y Clipper; Atlas participa como consumidor adicional. No declara adopción de cambios en los otros repos.

## Cierre productor · 2026-10-28

Nüün implementa metadata aditiva de reloj/procedencia, editable_members/media_bytes,
manifest_sha256, revisiones explícitas y mapping opaco clipper_profile. Handoff,
schema y ejemplo generado describen miembros por volumen/recibo. Detalle vigente:
[productor](media-library-v1.md#productor-aditivo-de-biblioteca--28-octubre-2026).
El coordinador de retitle y el despacho de recuperación pending están implementados;
Stalker ya adoptó la capability en fixtures (ver su fila y STATUS). `archived:false` solo ya no autoriza cambiar completed.
[APIs, schema, fixtures, errores y aceptación para Stalker](stalker-retitle-v1.md).
Clipper mantiene su adopción anterior; campos nuevos/portable handoff aún no
adoptados aquí. Evidencia de esta implementación en [STATUS](../STATUS.md).
Las filas históricas de pendientes abajo conservan origen/fecha; esta sección
sustituye el estado pendiente de reloj, miembros, manifest y mapping.

## Señal sostenida de Stalker · 2026-10-28

Revisión aditiva 1.4 del contrato propietario de captura: `mods` ahora puede
incluir flancos agregados y posición del cursor habilitado; encabezado opcional
`modifierTracking: "physical-fn/1"`. [Semántica y límites](../../../../ai/stalker/docs/input-events.md#ctrlfn-mantenidos-para-zoom-y-seguimiento).
Nüün conserva el bundle completo sin interpretar ni reescribir eventos.
No cambia naming, sessionID, digest, retitle ni la política de archivo/retiro.
El lector de Clipper acepta la adición; implementar el efecto Ctrl+Fn sostenido
es un pendiente de Clipper, no una adopción declarada por este índice.

## Autoridad y compatibilidad

| Contrato | Autoridad | Estado |
| --- | --- | --- |
| Captura, archivos, reloj y audio `stalker-clipper/1`, revisión 1.4 | [Stalker](../../../../ai/stalker/docs/clipper-contract.md), considerar notas locales actuales | Implementado por productor/consumidor con límites documentados en sus repos. No duplicar ese schema aquí. |
| Proyecto/timeline/perfiles y App Services | [Clipper](https://github.com/nu9ve/clipper/blob/e6981b1/apps/APP_CONTRACT.md) y [servicios](https://github.com/nu9ve/clipper/blob/e6981b1/apps/APP_SERVICES_CONTRACT.md) | Contratos existentes; no escribir proyectos desde el archivador. |
| Descubrimiento `nu9ve-inspection/1` | [inventory.py](../../manager/inventory.py) | Implementado aquí; observaciones sin garantía de importabilidad/integridad ni destinos. |
| Handoff de biblioteca `creator-media-handoff/1` | [Especificación](media-library-v1.md) + [schema](media-handoff.schema.json) | Productor `media/library.py` implementado; sin consumidor implementado (Clipper usa `sources`, ver su fila). [Ejemplo](examples/handoff.json) usa datos ficticios. |
| Naming y carpetas de media v1 | [Especificación](media-naming-v1.md), productor [layout.py](../../media/layout.py) | Implementado aquí (planes esquema 2, `relayout`) y usado en el primer respaldo real. Stalker lo adoptó en 0.4 y Clipper CLI/TUI el 2026-10-25 (ver filas de adopción). |
| Transferencia, resolución, relink, dependencias y publicación | [Especificación común](media-library-v1.md) | Deps/ACK/relink y exclusión host-local de liberación implementados; lease distribuido y automatización de publicación pendientes. |

El índice central enlaza formatos propietarios; sólo se centralizan decisiones que cruzan repos. Si un contrato propio cambia, actualizar este índice y las pruebas de compatibilidad. Cambios incompatibles requieren nueva versión mayor; adiciones opcionales se preservan/ignoran sin reinterpretar datos desconocidos. No aceptar un major desconocido ni ejecutar metadata.

## Puertas de adopción

1. Revisión de Stalker: estabilidad de `completed`, revisiones del manifest, metadata de proyecto opcional y señal de finalización.
2. Revisión de Clipper: importación de handoff, selección de biblioteca, relink integral `capture_sessions`/items/voz/eventos/colas, orden y perfil como sugerencia.
3. n-creator-utils: planner y transferencias verificadas primero; después publicar handoff a partir de recibos reales. Un JSON que pasa schema no demuestra que existan bytes correctos.
4. Fixture compartida copiada del contrato con versiones fijadas: importa por ruta, importa por biblioteca, relocaliza, abre cola y exporta sin duplicar audio. ACK con revisión y hashes.
5. Sólo entonces habilitar retiro de ubicaciones viejas, previa revisión de consumidores no integrados como Final Cut.

## Estado de adopción por app

Cada sesión de Stalker o Clipper actualiza **su fila** al terminar, con fecha y commit de su repo.

| App | Contrato / sección | Estado | Fecha · commit | Notas y límites |
| --- | --- | --- | --- | --- |
| n-creator-utils | Productor naming/handoff/sources aditivos y retitle coordinado; Clipper deps/ACK/media-release-lock/1 | Implementado en fixtures; capability Stalker y adopción de campos nuevos pendientes | 2026-10-28 · working tree sobre `3360d1d` | Registro Stalker actual/anteriores; tags locales/sugerencias confirmables; planes de retiro registrados + Clipper CLI/ACK. Exclusión host-local; proyectos antiguos no adoptados/colas/desconectados bloquean. [STATUS](../STATUS.md), [operación](../operations.md#bandeja-stalker-proyectos-y-liberar-espacio). |
| Stalker | naming v1, `stalker-clipper/1` rev. 1.4 y `stalker-coordinated-retitle/1` | Productor/UI/CLI instalados; gesto sostenido pendiente de consumo Clipper | 2026-10-28 · working tree sobre `e15918b` | Stalker 0.5.0/build 7 en `~/Applications/Stalker.app`, CLI firmada estable. Modificadores Ctrl/Fn con flancos agregados, Fn físico y ancla opt-in de cursor; reset en gaps. 87 pruebas Swift, lector real Clipper acepta fixture y 22 casos del puente contra CLI/backend nüün instalados. Naming, identidad, audio, reloj, locks/CAS y archivo sin cambio. Zoom sostenido todavía sin implementar en Clipper; Fn en teclado real y captura/export largo pendientes. [Revisión y evidencia](../../../../ai/stalker/docs/2026-10-28-input-contract-review.md). |
| Clipper | naming v1, sources aditivo, creator-media-handoff/1 editable, Stalker 1.3, deps/ACK/media-release-lock/1 | Adoptado en CLI/TUI; fixtures y CLIs reales; físico/distribución pendientes | 2026-10-28 · working tree sobre `e4f73d1` | Mapping explícito sin abreviaturas; tiempo/procedencia/zona, miembros/media_bytes y revisiones preservados. Handoff CLI/F6 valida catálogo, volúmenes, recibos/hash y reintento durable. UUID de sesión no adopta otra revisión. Retitle permitido/bloqueado con deps reales y ciclo handoff → prepare → cola → relink/ACK → liberar fixture. `delivery` y migración editorial de otra revisión pendientes; sin Studio. [Matriz/evidencia](../../../../ai/clipper/docs/2026-10-28-cli-tui-contract-adoption.md). |
| Atlas | library v1 como consumidor | Sin iniciar | — | — |

## Interfaz local estable para las apps (sólo lectura)

```text
python3 /Users/nu9ve/code/nu9ve/n-creator-utils/nu9ve.py media <comando>   # o `nu9ve media` si está instalado
```

- **Formato de respuesta:** stdout JSON `{"ok": true, "command": …, "data": …}` o `{"ok": false, "error": {"code", "message"}}`; código de salida 0/1.
- **Catálogo:** por defecto el del usuario. `--state-dir DIR` o `NU9VE_MEDIA_HOME` dan un catálogo aislado; **las pruebas de otras apps usan siempre uno aislado con fixtures**.

| Comando | Para qué | Datos |
| --- | --- | --- |
| `projects` | Selector de proyectos | `[{project, project_id, profile, assets, updated_at}]`, más reciente primero. |
| `sources --project P [--purpose edit\|archive]` | Quick import de un proyecto | `sources[]` en orden de captura: `order, asset_id, revision, kind (file\|camera_bundle\|stalker_session), session_id, path` (absoluto: archivo o carpeta `.stalker`), `name, volume, role, online, bytes, sha256` (de archivos), `captured_at` (hora local de inicio, sin zona) y `duration_seconds` (leídos del nombre fechado; `null` si no lo es), `alternatives[]` y `missing[]`. Aparte vienen `offline[]`, `removed_from_work[]` (borradas a mano de trabajo; apuntan al archivo) y `warnings[]` (`archive_copy_missing`). Prefiere la copia de trabajo en línea y comprueba presencia y tamaño, sin hashear. |
| `captures ROOT` | Estado de archivo de una biblioteca Stalker | `sessions[{path, session_id, status, archived}]`, `pending[]`. |
| `retitle-capture …`, `retitle-operations`, `cancel-plan ID` | Coordinación explícita; no son lectura/import automático | [API y capability requerida](stalker-retitle-v1.md). |
| `handoff ASSET…` | Intercambio portable `creator-media-handoff/1` | Ver [media-library-v1](media-library-v1.md). |
| `resolve ASSET` | Ubicaciones de un asset | Lento (identidad de cada volumen); preferir `sources`. |

Los comandos que mutan (`plan`, `apply`, `relayout`, `confirm-cleanup`, `deliver`, `restore`) sólo los inicia el usuario con un plan revisado; otra app no los llama sin esa revisión. Un campo nuevo opcional no rompe a nadie. Quitar o cambiar el significado de un campo exige nueva versión y nota aquí.

## Cómo actualizan este contrato otras sesiones

Una sesión de Stalker o Clipper **puede editar `docs/contracts/*.md` de este repo**, y nada más: ni código, ni pruebas, ni STATUS/PLAN.
- Actualiza su fila de adopción, con fecha y commit de su repo.
- Si toma o descubre una decisión que afecta a las tres apps (nombres, identidad, rutas, estados, señales), la registra en el contrato correspondiente con fecha, repo que la origina y compatibilidad. No reinterpreta en silencio.
- Si necesita un cambio en el productor (un campo, un comando, un bug), lo agrega a «Pendientes para n-creator-utils» en vez de editar código de este repo.
- Los textos de los prompts vigentes están en [next-sessions.md](next-sessions.md).

### Pendientes para n-creator-utils

| Fecha | Origen | Pedido | Estado |
| --- | --- | --- | --- |
| 2026-10-28 | Stalker · working tree sobre `fa68510` | **Despachar recuperación del productor retitle:** Stalker ya implementa capability/status/apply y journal durable. `media/retitle.py` sólo permite `status:not_found|applied`; tras SIGKILL parcial `pending` queda bloqueado sin camino operativo. Añadir rama explícita bajo los mismos locks editor/catálogo: validar request/resultado pendiente, revalidar archivo/planes y deps tanto para ruta original como destino journalizado, y repetir el `apply` existente con ambos FDs y el mismo ID. Después verificar/publicar ACK por el camino normal. No borrar journals, no adoptar desde status ni inventar otro lease/API. [Evidencia y reproducción](../../../../ai/stalker/docs/2026-10-28-coordinated-retitle.md#recuperación-explícita-y-pendiente-real-de-nüün). | Implementado en nüün: pending validado, guardas para origen/destino y apply con mismo request/FDs; evidencia en STATUS. Adopción física pendiente. |
| 2026-10-25 | Stalker `d88eaf9` | **Sugerir proyecto en la importación de SD:** leer `library.json.recentSession` (`sessionID`, `path`, `project`, `country`, `createdAt`, `endedAt`; o el `manifest.project` de las sesiones recientes de la biblioteca) y, si las capturas de la SD caen el mismo día o en horas cercanas a esa sesión, proponer ese proyecto/país como valor inicial del panel de ingreso. Sólo sugerencia: el usuario confirma. | Pendiente |
| 2026-10-25 | Stalker `d88eaf9` | Tolerar el renombre de una sesión `completed` **aún no archivada** (el usuario le pone proyecto después): el plan vuelve a leer el manifest al aplicar y, si cambió la ruta o el digest, replanifica en vez de copiar a medias. Nunca ocurre después de `archived: true`. | Pendiente |
| 2026-10-25 | Stalker `ecb18f1` | Usar `~/Library/Application Support/Stalker/library.json` (`stalker-library/1`: `path`, `previous[]`, `updatedAt`, `recentSession`) como valor por defecto de «Carpeta Stalker a observar», respetando una carpeta elegida a mano. Seguir revisando `previous[]` mientras tengan sesiones `completed` sin archivar (hoy `~/Downloads`). Sólo lectura: Stalker es el único escritor. | Implementado 2026-10-26 (fixtures): actual/anteriores + carpeta manual adicional, sin modificar registro. |
| 2026-10-25 | Stalker `ecb18f1` | Proponer `manifest.project.name`/`country` (si existe, `version` 1) como sugerencia de proyecto/país al asignar una sesión Stalker en el plan, sin aplicarlo en silencio. Ignorar versiones mayores con campos desconocidos. | Implementado 2026-10-26 (fixtures): sugerencia confirmable; tags por UUID en catálogo, sin mutar manifest. |
| 2026-10-25 | Stalker `ecb18f1` | Prueba de compatibilidad: un bundle que Stalker ya nombró (`26OCT24-195850_STALKER_03-48_MX-DEBLOATEANDO-MI-TELE[].stalker`) con el mismo proyecto/país en el plan debe archivarse con el mismo nombre y no requerir `relayout`. | Pendiente |
| 2026-10-25 | Stalker `ecb18f1` | Decidir si `slug()` debe convertir `_` en `-` dentro de PAIS/PROYECTO: hoy `mi_proyecto` produce `…_MX-MI_PROYECTO[]` y `_` es el separador de la gramática. Stalker copia `layout.py` tal cual y cambiará si cambia aquí. | Abierta |
| 2026-10-27 | Stalker · seguimiento solicitado tras un lote de dos sesiones | Investigar por qué las carpetas observadas/conservadas aún aparecen como `Stalker_yyyy-MM-dd_HH-mm-ss.stalker` aunque el lote tiene proyecto: revisar si proyecto/país llegan al manifest antes de `completed`, la ruta de finalización/renombre, colisiones y el estado de archivo. Confirmar que el nombre visible final usa la gramática acordada, o documentar cuándo el nombre genérico es esperado. La identidad/backup debe seguir basándose en `sessionID` + digest del manifest, nunca en el nombre de carpeta. | Pendiente para próxima sesión Stalker |
| 2026-10-27 | Stalker · seguimiento solicitado tras un lote de dos sesiones | Auditar el tamaño de las pistas de micrófono, que parecen cercanas al video de pantalla: registrar por archivo bytes, duración y bitrate, contenedor/codec, sample rate, canales, bit depth y ajustes efectivos de captura; comparar archivos de la misma sesión y estimar si el tamaño corresponde a audio PCM u otra configuración. No cambiar codec/calidad hasta explicar el dato y proponer una comparación controlada. | Pendiente para próxima sesión Stalker |
| 2026-10-25 | Clipper (working tree sobre `e6981b1`) | **`bytes` de `camera_bundle` no es el tamaño del clip:** `sources` suma todos los paths del asset (MP4 + XML Sony, ~1.4 KB más). Agregar `media_bytes` (o `members[{path,bytes,sha256}]` de lo editable) para validar una copia sin hashear. Clipper hoy valida cámaras sólo por SHA-256. | Implementado productor nüün y adoptado CLI/TUI Clipper 2026-10-28; evidencia y límites en la matriz enlazada de Clipper. Físico/distribución pendientes. |
| 2026-10-25 | Clipper | **Zona horaria en `sources`:** `captured_at` es hora local sin zona (del nombre); Stalker da `createdAt` UTC. Agregar `captured_at_utc` o el offset para agrupar sin suponer que el Mac está en la misma zona que la cámara. | Implementado productor nüün y adoptado CLI/TUI Clipper 2026-10-28; evidencia y límites en la matriz enlazada de Clipper. Físico/distribución pendientes. |
| 2026-10-25 | Clipper | **Revisión de sesión Stalker:** documentar qué es `revision` de un `stalker_session` y exponer `manifest_sha256` en `sources`; Clipper compara el digest del manifest (sha256 de `manifest.json`) al reimportar/relink. | Implementado productor nüün y adoptado CLI/TUI Clipper 2026-10-28; evidencia y límites en la matriz enlazada de Clipper. Físico/distribución pendientes. |
| 2026-10-25 | Clipper | **`.DS_Store` dentro de bundles en trabajo:** Finder escribió uno (6148 B) en `26OCT24-195850_STALKER…[].stalker` de ESSD. `present()` no lo ve, pero `relayout`, `restore` y la verificación deben ignorar `.DS_Store`/AppleDouble y no reportarlo como cambio de membresía. | Pendiente |
| 2026-10-25 | Clipper | **Carpetas de Clipper en `<prefix>/<perfil>/<proyecto>/`:** `clipper/` (proyecto) y `renders/` (finales) son de Clipper. «Liberar trabajo» y cualquier limpieza no los tratan como raw ni los borran; antes de retirar copias de trabajo, leer dependencias en `clipper/clipper-project.json` → `media_library.assets` (lista de proyectos enlazados en `~/Downloads/clipper-projects/.linked-projects.json`). Ya existe `clipper library deps --json` (ver fila siguiente). | Pendiente |
| 2026-10-25 | Clipper | **Consumir `clipper library deps --json` antes de «Liberar trabajo»:** no proponer retirar una copia de trabajo cuya ruta (o carpeta `.stalker`) aparezca con `in_use: true` o con `queued_exports`; si hay `offline_projects`, tratar sus dependencias como desconocidas y no liberar sin confirmación. Mismo comando para leer ACKs (`relinks[]`: `operation_id`, `asset_id`, `revision`, `from`, `to`, `still_pinned`) y registrarlos; para pedir un relink, `clipper library relink PROYECTO ASSET --to RUTA --operation-id UUID --json` (idempotente; otro payload con el mismo id = `operation_conflict`). | Implementado 2026-10-26 (fixtures): deps frescos, reenlace explícito vía CLI, ACK validado/persistido; offline/colas/proyectos antiguos desconocidos bloquean. Sin lease compartido ni aceptación real. |
| 2026-10-25 | Clipper | **Perfil de Clipper explícito:** `projects`/`sources` dan `profile: e64`; Clipper lo mapea a `erikiado64` por abreviatura (primera letra + código). Un campo opcional `clipper_profile` (o alias en el perfil del catálogo) evitaría la heurística. | Implementado productor nüün y adoptado CLI/TUI Clipper 2026-10-28; evidencia y límites en la matriz enlazada de Clipper. Físico/distribución pendientes. |
| 2026-10-25 | Clipper | **Desfase de reloj por dispositivo:** Clipper guarda `--clock-offset` por proyecto. Si se mide por cámara/Mac, guardarlo en el catálogo y exponerlo en `sources` para todas las apps. | Abierta |

## Otros consumidores

**Atlas:** «Revisa el modelo de asset/ubicación como consumidor. Conserva pares JPG+DNG, coverage exports y nombres de captura. Propón un adaptador que resuelva raíces por IDs/manifiesto sin renombrar entradas existentes, y diferencie estado humano persistente de derivados regenerables. El media manager no debe redefinir GPS, vuelo, quality o fotogrametría.»

### Retiro explícito implementado en n-creator-utils (2026-10-26)

`plan_release` persiste un plan en el catálogo; `relink_release` sólo invoca el CLI público de Clipper y verifica ACK; `apply_release` recibe ID/digest del plan almacenado, revalida copias/origen/consumidores y journaliza cada retiro. Un plan de ingreso Stalker libera el original después de verificar destinos por defecto, dentro del mismo flujo de aplicación; `retain-stalker-source` en ajustes mantiene el comportamiento manual para esa operación. La secuencia no es una transacción ACID entre discos/Clipper/catálogo: si falla una compuerta el original sigue disponible, y un retiro interrumpido se reanuda con su journal. El trabajo retirado conserva recibos históricos y se reporta por `sources` como retirado. Copias de editor/renders se preservan. Los tags de esta UI pertenecen al catálogo; no se escriben al manifiesto de Stalker. Los países opcionales por unidad se congelan en el layout sin cambiar planes anteriores.

Conservar la edición y las colas detenidas durante el retiro: aún falta un lease inter-app. El guard adicional lee proyectos/colas estándar/enlazados de Clipper para bloquear rutas antiguas ausentes en `deps`; no migra esos proyectos ni cubre consumidores desconocidos. Validación con fixtures; no declarar adopción probada en medios reales. [Procedimiento completo](../operations.md#bandeja-stalker-proyectos-y-liberar-espacio).


### Cierre CLI/TUI del 28 de octubre de 2026

Autoridad de captura vigente: Stalker `stalker-clipper/1` revisión 1.3, en su
working tree 0.5 sobre `d88eaf9`; su fila del 25 conserva evidencia histórica.
Stalker ya delega retiro a nüün: `archived --trash` no libera. Clipper consume
los campos opcionales nuevos; hora civil/PID no redefinen sincronización ni
clasifican ventanas/juegos por sí solos.

El límite histórico «nüün no lee ACK» de la adopción del 25 queda sustituido
por la implementación de `media/clipper.py`. La exclusión actual es
`media-release-lock/1` por usuario/host, no un lease distribuido ni adopción por
Final Cut. [Protocolo](media-library-v1.md#exclusión-local-de-liberación--28-octubre-2026),
[evidencia actual](../STATUS.md). Productor de zona/evidencia, miembros/media_bytes, hash/revisión de manifest,
mapping y coordinador de retitle implementados posteriormente en este working
tree; ver cierre productor al inicio. Quedan adopción del productor Stalker,
campos nuevos en Clipper, handoff consumidor y aceptación física.

### Adopción Clipper CLI/TUI · cierre posterior del 28 octubre

La fila anterior sustituye la limitación histórica «sin consumidor handoff» y
«campos nuevos pendientes en Clipper». El productor y la recuperación pending
de Stalker/nüün ya existen; no se adoptan revisiones editoriales automáticamente.
La evidencia actual y los límites viven en la [entrega Clipper](../../../../ai/clipper/docs/2026-10-28-cli-tui-contract-adoption.md).
No implica distribución ni aceptación física. `delivery` portable aún falla
como tipo no soportado en el editor. Retitle con referencias de Clipper sigue
bloqueado por el contrato vigente; no hay protocolo implementado de migración
metadata-only conservando identidad editorial entre revisiones.
