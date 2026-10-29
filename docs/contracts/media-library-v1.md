# Biblioteca, handoff y relocalización v1

Estado: contrato borrador `creator-media-handoff/1`, revisión 0.1; productor y operaciones básicas ya existen en `media/`, consumidor Clipper pendiente. Los nombres conceptuales de secciones futuras no garantizan una operación instalada. Ver [STATUS](../STATUS.md) y [operations](../operations.md). Términos DEBE/NO DEBE fijan criterios de implementación futura; no afirman garantías del `save` legado.

## Identidad y archivo

- `asset_id` UUID identifica unidad lógica; `content_revision` identifica bytes/membresía. Mismo contenido en otra ruta conserva ambos; transformación real crea derivado o nueva revisión con procedencia.
- `project_id` es ID común de producción, independiente del slug y del path del proyecto Clipper. `profile_id` común se mapea a una referencia opaca de perfil Clipper; no sustituye su schema de perfiles.
- Stalker conserva `sessionID`; registrar mapping entre ese UUID y `asset_id`. Internos del bundle, `manifest.json` y nombres de pistas permanecen byte a byte. Proyecto/etiquetas del archivador van en catálogo/sidecar externo, no dentro del manifest ajeno.
- Una unidad con varios proyectos conserva un original y asociaciones N:M. Dedupe físico no equivale a quitar asociaciones ni a perder orden de edición.
- Miembros se expresan con rutas relativas POSIX y SHA-256 por archivo. Rechazar rutas absolutas, `..`, segmentos vacíos, backslash, drive letters, NUL, links y entradas especiales. Resolver dentro de la raíz y detectar colisiones Unicode/case en el destino. Schema sólo cubre forma; el receptor valida filesystem y contenido.
- UUID/serial de cámara sólo se usa si hay evidencia; volumen/SD se registra aparte. No exponer serie o eventos privados en un catálogo compartido por defecto.

## Handoff de biblioteca

[Schema JSON](media-handoff.schema.json), [ejemplo](examples/handoff.json). Se entrega una selección ordenada de assets y ubicaciones, todas de la misma producción/perfil de la selección. Un handoff mixto se divide en selecciones explícitas; el catálogo sí puede relacionar cada asset con múltiples proyectos.

Campos requeridos:

| Campo | Semántica |
| --- | --- |
| `schema`, `handoff_id`, `created_at` | Versión, idempotency key UUID y fecha RFC3339 UTC. Reusar ID con payload distinto es conflicto. |
| `project_id`, `profile_id` | Asociación confirmada de esta selección; no auto-crear ni sobreescribir proyecto/editor. |
| `assets` | Orden de entrada explícito mediante `order` único 0..N-1; cada asset/revisión aparece una sola vez. |
| `asset_id`, `content_revision`, `kind` | Identidad y tipo (`file`, `stalker_session`, `camera_bundle`, `delivery`). |
| `capture_session_id` | Obligatorio para `stalker_session`; igual al UUID del manifest validado. |
| `members` | Lista completa de miembros preservados, rutas relativas únicas, bytes y SHA-256. Bundle: conjunto completo; file: un solo miembro. No marcar capturas en progreso completas. |
| `locations` | Uno o más `volume_id`, `relative_path`, `role`, `receipt_id`. Raíz se resuelve localmente; no se serializan paths absolutos de macOS en el intercambio portable. |

`receipt_id` sólo referencia evidencia de copia: el consumidor verifica que corresponde al conjunto/revisión y al volumen. No es garantía basada en un string. Un importador puede leer una captura local aún sin archivo, pero debe registrar estado `pending_archive`; esa lectura directa es otro flujo, no un handoff fingido con recibos vacíos.

Para `kind=file`, `locations[].relative_path` apunta al archivo y su único miembro usa el basename de ese archivo; no concatenar ambos. Para bundles/entregas, la ubicación apunta a la carpeta raíz y los miembros son relativos a ella. Verificar basename y contenido en el caso de archivo; membresía y confinamiento en el caso de carpeta. El ejemplo usa un archivo vacío con hash real sólo para validar forma; no constituye media reproducible ni un recibo real.

El receptor DEBE validar major, límites de tamaño/número de assets/miembros, tipos, duplicados, rutas y hashes; campos desconocidos opcionales no alteran la interpretación. La v1 propone límites configurables de lectura (por ejemplo 16 MiB de handoff, 10000 assets), sin dejar que una metadata los desactive. No ejecutar comandos, abrir URLs remotas ni aplicar presets indicados por metadata sin una capacidad explícita.

Este contrato no redefine el tiempo Stalker ni convierte MOV de audio en video. El consumidor sigue el contrato productor para pistas/clock/legacy. País, ciudad, roles y capture timestamps pueden añadirse como metadata opcional versionada posteriormente.

## Resolución de ubicación

Entrada conceptual: asset ID, revisión esperada, propósito (`edit`, `archive`, `restore`) y preferencia de volumen. Resultado: candidatos online/offline con ruta local resuelta, validación vigente y capacidad. No elegir una revisión distinta porque el nombre coincida. Distinguir `not_found`, `volume_offline`, `revision_mismatch`, `permission_denied`, `unsupported_bundle` y `insufficient_space`.

Preferir copia de trabajo online verificada; después ubicación local durable o archivo online según propósito. La preferencia es política, no parte del ID. Resolver no debe mover archivos ni reescribir proyectos. Estar online no prueba integridad; comprobaciones rápidas invalidan caché ante cambios, checksums completos antes de confiar en una réplica para liberar otra.

## Recibo de transferencia

El motor produce `nu9ve-transfer-receipt/1` y `nu9ve-work-receipt/1`; la autoridad de su formato actual es [transfer.py](../../media/transfer.py), [library.py](../../media/library.py) y [recovery.py](../../media/recovery.py). Falta formalizar schema de recibos para adopción externa. Debe incluir `receipt_id`, `job_id`, digest/version del plan y política, asset/revisión, lista completa de miembros y hashes, volumen/ruta relativa por destino, identidad física conocida, fases y timestamps de lectura/verificación, resultado y errores. Distintos destinos tienen recibos independientes; falta de uno requerido mantiene la operación parcial.

Un plan aprobado queda inmutable. Cambiar proyecto, destinos, clasificación, política o conjunto de miembros produce otra revisión/digest del plan. El apply revalida precondiciones; no interpreta «dry-run exitoso» como permiso para usar un origen o montaje que cambió después.

### Excepción de destino de trabajo ausente

Con archivo MEDIA disponible, el usuario puede confirmar un plan `archive_only` cuando falte ESSD/SSD1. El plan DEBE conservar política habitual y efectiva, destino esperado, confirmación explícita de esa operación y motivo, junto con `work_delivery_status` por selección/asset: `pending`, `ready` o `skipped`. Ausencia de respuesta no confirma y la excepción no cambia el default del perfil.

`pending` registra intención de entregar al SSD después; `skipped` registra omisión voluntaria y no vuelve a encolarse automáticamente. `ready` requiere recibo real de entrega de trabajo. Después de archivo verificado, el plan efectivo confirmado puede limpiar SD aunque trabajo no se haya entregado. Estado de trabajo y cobertura de réplicas siguen visibles; no emitir recibos para destinos omitidos ni declarar redundancia inexistente.

La entrega posterior usa asset/revisión y ubicación de archivo, preflight de destino y copia verificada idempotente; nunca requiere SD ya limpiada. Conexión del SSD sólo sugiere pendientes agrupados: no aplica copias masivas sin selección. Cambios de pending/skipped se registran como decisiones; completar entrega registra recibo. Sólo SSD presente sin MEDIA no activa esta excepción, que omite trabajo, no archivo.

La UI automática de SD DEBE permanecer silenciosa si no hay ningún volumen destino registrado y montado de archivo/trabajo. Comprobar presencia antes de inventariar profundamente la SD. Esta regla no deshabilita inspect ni ingreso manual y no usa nombres de volumen como prueba de identidad.

Persistir recibo junto al archivo y en catálogo local mediante journal recuperable. Verificación por tamaño o hash calculado sólo durante la escritura no equivale a lectura de destino verificada. Reintentar mismo asset/revisión/destino debe ser idempotente, con comparación antes de reutilizar un resultado existente. No sobrescribir un archivo ajeno por coincidencia de nombre.

## Limpieza de SD como cierre del ingreso

El plan de ingreso DEBE declarar política de origen (`clean_after_verified` por defecto para SD; `keep` si se elige sólo copiar), destinos requeridos y perfil de limpieza de cámara. La fase de borrado es separada en el motor y journal pero forma parte del mismo flujo aprobado. No necesita un nuevo permiso rutinario cuando el plan ya incluye limpieza.

Sólo ejecutar después de verificar y registrar todas las copias requeridas. Revalidar identidad de tarjeta y revisión de cada miembro; no retirar archivos nuevos ni miembros excluidos/sin respaldo. Registrar resultado por miembro y reanudar limpieza parcial sin repetir importación. Hasta resolver archivos restantes no declarar la tarjeta lista. Reutilizar evidencia vigente de verificación, sin releer todo el archivo por cada retiro.

Resultado mínimo: estado de archivo (`pending`, `verified`, `failed`) y estado de origen (`retained`, `cleaning`, `cleanup_failed`, `camera_action_required`, `ready`), con miembros pendientes y razón. `ready` exige ausencia del material anterior incluido en la limpieza y perfil consistente para grabar. Un Sony sin procedimiento validado queda `camera_action_required` tras respaldo; no manipular índices por inferencia ni marcar éxito completo. Los directorios/índices necesarios de cámara pueden permanecer. El retiro de copias de trabajo y los leases editoriales siguen un flujo distinto.

## Relocalización y dependencias

Protocolo futuro, coordinado por app propietaria de referencias:

1. Preparar propuesta con `operation_id`, asset/revisión, raíz antigua/nueva por ubicaciones, manifest/member hashes y revisión esperada del proyecto consumidor. La copia nueva ya está completa y verificada.
2. Clipper valida conflicto concurrente y todas sus referencias: `items`, `capture_sessions`, `sync_reference_path`, tracks, voz, eventos, firmas, exports pendientes y snapshots de cola. Invalida/reconstruye caches que dependan de ruta sin cambiar offsets editoriales válidos por accidente.
3. Aplicar modificación atómica del proyecto según su servicio de mutación y responder ACK con operation ID, revisión anterior/nueva, ubicaciones adoptadas, pendientes y dependencias aún fijadas. Repetir operation ID devuelve mismo resultado o conflicto si cambió payload.
4. n-creator registra el ACK; la ubicación antigua se conserva hasta que todas las dependencias conocidas se resuelvan. Una cola inmutable puede retener la ruta antigua, necesitar regeneración o bloquear limpieza; no se promete reescribir todo snapshot a ciegas.
5. Si falla a mitad o no hay ACK, recuperar/reconciliar; no borrar fuente ni alegar que «relink terminado». Cambiar sólo `item.path` no satisface este protocolo.

Estado histórico 2026-10-25 (sustituido por las adopciones posteriores abajo): Clipper CLI/TUI hace relink **local y a pedido** de una fuente importada desde la biblioteca (misma revisión; `sessionID` + digest del manifest o SHA-256), reescribe items, `capture_sessions`, `sync_reference_path` y firmas de placement, e invalida previews. Cada relink publica un ACK (`clipper-relink-ack/1`: `operation_id` idempotente, asset/revisión, `from`/`to`, revisión del proyecto antes/después, `still_pinned` con los exports en cola que conservan la ruta vieja) en `clipper library deps --json`, junto con las dependencias por asset. n-creator-utils aún no lo consume y no hay leases. Ver [adopción Clipper](media-naming-v1.md#adopción-clipper-implementada-2026-10-25).

Un contrato de leases registra uso en proyecto/queue/Final Cut/Atlas, responsable y revisión. Un consumidor offline o no integrado no autoriza limpieza; ofrecer retención manual. Hash igual permite resolver nueva copia, pero el consumidor confirma su propia consistencia. Un asset cambiado invalida sync/transcripts/previews según el editor, no según el archivador.

## Finalización de Stalker y metadata común

Acuerdo de inmovilidad adoptado por Stalker 0.4.0 (2026-10-25, `ecb18f1`): `status=completed` es la última escritura, durable y atómica, después de cerrar writers y derivados; un bundle `completed` no se regenera ni renombra en sitio (la CLI de recuperación escribe fuera). La «revisión verificable» es el digest del manifest: si alguien edita el bundle, cambia y `captures` lo reporta `archived: false`. Detalle en [naming v1](media-naming-v1.md#adopción-stalker-implementada-2026-10-25). Una notificación del SO es sólo una pista; el archivador vuelve a leer y verifica manifest/membresía antes y después.

Stalker 0.4 guarda en el manifest `project: {version: 1, name, country}` (texto del usuario, sin IDs). `project_id`/`profile_id` comunes siguen siendo del catálogo; Stalker no los conoce todavía. Stalker mantiene ownership y compatibilidad. Nunca exigir nombrar un proyecto para poder grabar una prueba rápida. El organizador puede clasificar esa captura más tarde sin cambiar la identidad/reloj.

## Entregas y publicación

Entrega contiene ID, proyecto/perfil, variantes elegidas, hashes y roles (master, short, thumbnail, subtitle, copy). Estado por variante/canal: `candidate`, `approved`, `published`, con evidencia manual/URL opcional; cobertura de archivo se guarda aparte. Archivar sólo publicados puede ser política, pero entregas aprobadas aún no publicadas siguen siendo datos que pueden requerir respaldo.

No deducir publicación por existencia en Downloads o por terminar render. `subidos` es una convención configurable que propone un estado. Preservar nombres finales del usuario; cambios de nombre son ubicación/metadata, no contenido. Proyectos editables, bibliotecas FCP y assets externos necesitan cobertura aparte de las entregas.

## Transporte y fases

Fase 1: archivos JSON + helper local de una sola ejecución, stdout estructurado/stderr logs. Futuro NDJSON de progreso con versión, `event_id`, `job_id`, secuencia, fecha, fase y bytes por destino; el cliente recupera estado por job y tolera eventos repetidos/faltantes. No prometer exactly-once por notificaciones.

### Progreso local de ingest (n-creator-utils · 2026-10-27)

`job` devuelve progreso observable durante transferencia y liberación Stalker. En transferencia, `phase` distingue copia, verificación, limpieza y estado terminal; `copied_bytes`/`bytes_total` cuentan cada archivo una vez para la etapa de copia (aunque ya tenga recibos al reanudar), `verified_bytes`/`verify_total` cuentan lecturas de verificación de todos los destinos, y `destinations[]` muestra nombre/rol más archivos verificados de cada destino. La copia puede alimentar varios destinos desde una sola lectura; no sumar `copied_bytes` como si fuese el total escrito en cada volumen.

Durante liberación, `phase=releasing_stalker` se acompaña de `stage`, sesión actual y `sessions_done/sessions_total`; cuando una etapa lee archivos también informa ruta actual, `files_done/files_total` y bytes leídos del archivo actual. Las etapas incluyen hash/compare de origen y respaldo, revisión de bundle, dependencias/reenlace Clipper, revalidación de copias, snapshot de catálogo y liberación por archivo. Son lecturas/recibos reales, no porcentajes estimados. El servicio guarda cambios al cambiar de etapa y limita actualizaciones dentro de una etapa a aproximadamente una por segundo. Es progreso local de consulta, no un protocolo portable ni promesa de NDJSON.

Fase 2: API/MCP local `catalog.search`, `asset.resolve`, `ingest.plan`, `ingest.apply`, `job.status`, `restore.plan`, `delivery.register` y operaciones separadas de limpieza, todos nombres conceptuales aún inexistentes. IDs/digest, permisos y dedupe iguales a CLI; herramientas de lectura no inician jobs. Sin webhook remoto ni HTTP público para resolver el flujo local.

Fase 3: paths absolutos sólo en respuestas locales del adaptador; manifests usan IDs/rutas relativas. Windows necesita resolver volumen GUID/letra y restricciones de nombres; Linux su montaje. Un consumidor rechaza capacidades requeridas no soportadas. No convertir bookmarks macOS en contrato portable.

## Matriz mínima de conformidad

| Escenario | Resultado exigido |
| --- | --- |
| Bundle completed importado por ruta y por biblioteca | Mismas fuentes/identidades; no duplicar micrófono/audio de computadora. |
| Misma SD con Sony residual, DJI y otro fabricante | Clasificación por unidad, ambiguos a revisión, sin reemplazar todo el perfil por carpeta residual. |
| Job repetido / caída tras publicar antes de registrar catálogo | Reconcilia recibo y hashes sin duplicar; origen retenido hasta verificar destinos requeridos. |
| Limpieza tras éxito / caída a mitad / archivos nuevos | Retira sólo revisión respaldada, registra/reanuda pendientes y no borra material nuevo; SD lista sólo con limpieza y perfil de cámara consistentes. |
| ESSD desconectado o sustituido por disco del mismo nombre | Pausa, identidad verificada, no crea fallback en interno. |
| Stalker recording/finalizing o manifest cambiado | No transferencia final ni limpieza; recuperación en productor si hace falta. |
| Nueva ubicación de Stalker con cola activa | Relink integral/ACK o retención de antigua ruta; nunca actualización parcial silenciosa. |
| Perfil o project title renombrado | IDs y relaciones conservados, sin duplicar archivos por slug. |
| Short publicado pero master pendiente | Sólo variante confirmada publicada; archivo y limpieza independientes. |
| Handoff con traversal, symlink, hash falso o major desconocido | Rechazo antes de escritura/edición. |
| Falta SSD1/ESSD con MEDIA disponible | Preflight bloquea plan directo; sólo tras confirmar archive-only crea otro plan efectivo y registra trabajo pending/skipped. Puede liberar SD después de verificar archivo. |
| SSD de trabajo reconectado | Ofrece pendientes; entrega desde archivo con verificación/idempotencia o registra omisión. No requiere SD ni insiste sobre omitidos. |
| SD conectada sin MEDIA ni SSD de trabajo registrados montados | Sin popup/notificación/escaneo profundo ni jobs; acceso manual disponible. |
| Espacio insuficiente en destino requerido del plan efectivo | Bloquea antes del primer byte; no sustituye discos ni cambia política silenciosamente. |

## Selección de copias para editar en ingest (2026-10-26)

La entrada compartida `plan.assignments` admite por ruta relativa `{project, profile, camera, work}`; `work` es un booleano opcional (por defecto true para mantener compatibilidad). En lotes Stalker, el proyecto global es un valor de fallback: se usa sólo en sesiones sin proyecto individual y puede quedar vacío cuando todas las sesiones están asignadas. Si alguna sigue sin proyecto efectivo, el plan falla con guía de asignación antes de copiar. Pares/sidecars y bundles son indivisibles: elecciones contradictorias bloquean el plan. La unidad planeada registra `work_selected` si hubo elección explícita; false excluye sólo su copia al SSD. Cada archivo sigue requiriendo respaldo verificado en archivo antes de limpiar el origen. `preview.work_path` describe el destino efectivo de esa ejecución; `layout` puede conservar la receta de destino para una entrega explícita posterior.

En un lote mixto, seleccionados quedan ready o pending según la política; deseleccionados quedan skipped. Deseleccionar el lote completo requiere archive-only confirmado por operación. `deliver_work` explícito puede entregar posteriormente desde archivo también una unidad omitida; la omisión inicial no prohíbe editarla en el futuro. La selección pertenece al ingest, no es un queue ni estado editorial de Clipper. No cambia el JSON de Stalker/Clipper/Atlas ni demuestra adopción por sus consumidores.

## Perfiles y discovery local (2026-10-26)

`profiles` lista configuración local; `save_profile` guarda `{id,name,raw_dir,work,archive}` con roles/rutas validados y volúmenes canonicalizados a IDs. Los discos son sugerencias de UI; `raw_dir` controla el layout común CLI/Swift. Los perfiles usados se congelan como mapa opcional `profile_routes` en el plan schema 2 y su digest/recibos. Un cambio posterior de configuración no relocaliza assets ni cambia planes revisados. El backup del registro completo/etiquetas/defaults sigue pendiente; no atribuir esa configuración a un recibo.

`discover` acepta `source`, `source_kind` y `archive` opcionales para compartir con el planner el filtro de Stalker ya archivado. Cada unidad puede incluir `category`, `camera` (evidencia, no clasificación definitiva), `bytes` y para Stalker `session_id`, `created_at`, `duration_seconds`, `track_count` y `preview_path` opcional. Este último es una ruta relativa segura desde `source` al video de pantalla del bundle, sólo para miniaturas de UI; no cambia membresía, selección ni transferencia del plan. `capture_libraries.sessions[]` también puede incluir `duration_seconds`, `created_at_local` (hora local con offset) y `preview_path`, relativa a su `path` de bundle. Los contadores son de unidades/bundles, no archivos individuales. Sólo inspecciona bundles .stalker del primer nivel cuando source_kind=stalker (o el bundle directo), sin recorrer carpetas ajenas. No deriva metadata de grabaciones activas ni ejecuta transferencia. Ningún campo nuevo implica adopción por Stalker/Clipper/Atlas.


## Fuentes y expulsión local (2026-10-27)

Métodos aditivos de `media rpc`, compartidos por CLI y Swift; no se exponen como herramientas de mutación MCP:

- `source_cards {}` devuelve `[{path, name, kind: "card", identity}]`. Sólo metadatos de montajes externos removibles, sin recorrer archivos. Excluye imágenes de disco y discos físicos compartidos con destinos registrados; los destinos online se describen de nuevo porque su número diskN puede cambiar al remontar. Las bibliotecas configuradas siguen en `capture_libraries`.
- `eject_card {source, identity}` requiere el objeto de identidad íntegro de la lista actual. Devuelve `{ejected: true, source}` únicamente al completar una expulsión normal del sistema. Revalida raíz de montaje, UUID, inode/dispositivo y disco físico, rechaza archivo/trabajo y comparte el `writer.lock` de copy/verify/cleanup. Bloquea conservadoramente durante cualquier mutación, aunque use otra tarjeta. `register_volume` también respeta ese bloqueo.

Errores: `job_busy`, `not_a_card`, `destination_volume`, `volume_replaced`, `eject_failed`; `eject_uncertain` tras timeout no afirma que sea seguro retirar la tarjeta. Nunca hay force/fallback ni expulsión desde apply/resume. Fuera de macOS no se ofrece este backend. El bloqueo coordina procesos que usan el mismo catálogo; no es un lease global para aplicaciones ajenas o catálogos independientes. El sistema puede rechazar expulsión si otro proceso tiene archivos abiertos.


## Documento de plan para clientes nativos (2026-10-27)

`plan` conserva su respuesta objeto para CLI/clientes existentes. `plan_document` admite los mismos parámetros y devuelve el JSON canónico completo como string. Swift conserva ese documento intacto y decodifica una copia sólo para mostrar la revisión. `apply` acepta en `plan` tanto el objeto histórico como ese string; Python lo decodifica y pasa por la misma validación de digest y política, sin recalcular ni reemplazar el digest enviado por el usuario. Documento limitado a 12 MiB. No añade mutaciones MCP ni cambia el esquema de planes/recibos.

Esto evita que reserializar desde un árbol Codable altere números (`1.0` frente a `1`, precisión, exponentes) y cause un rechazo de un plan revisado. El árbol general de JSON también preserva UInt64 para identidades de volumen. Un documento modificado o un digest de revisión distinto siguen rechazándose antes de crear un job/copiar. Ante `plan_changed`, la UI descarta la revisión y requiere preparar otra; no aplica ni resume saltándose la comprobación.


## Exclusión local de liberación · 28 octubre 2026

Adición compatible a `clipper-library-deps/1`:
`capabilities: ["media-release-lock/1"]`. Implementada por Clipper CLI/TUI
sobre `3a156f5` y por este repo sobre `d634e71` (cambios locales).

`~/.cache/clipper/.media-access.lock` es una ruta fija por usuario/host.
Clipper adquiere flock compartido para trabajo, escrituras de proyecto y
encolar/reintentar/ejecutar variantes. `relink_release` y `apply_release` de
nüün conservan exclusivo desde las comprobaciones hasta terminar su operación.
Antes de continuar exigen la capability del CLI real. Un proceso viejo o
ocupado bloquea retiro con guía; no se reduce el contrato para continuar.

nüün escribe `{pid, token}` bajo el exclusivo; token aleatorio de 32 bytes
codificado en hex. Lo transmite sólo a sus subprocesses de Clipper mediante
`CLIPPER_MEDIA_RELEASE_TOKEN`. Clipper permite el relink autorizado bajo ese
exclusivo; otros escritores rechazan el trabajo. El propietario es el proceso
de liberación mismo, no un helper con vida independiente. EOF de un proceso
no es un ACK; el kernel libera el flock al cerrar/morir el propietario. La
metadata residual del archivo nunca bloquea por sí sola ni actúa como TTL.

El lock no reemplaza verificación de hashes/revisión, ACK, deps frescos, pins
de cola, offline ni referencias antiguas. Versiones previas de nüün no lo
usan: actualizar ambos lados y reiniciar procesos existentes es obligatorio.
No cubre otros hosts/usuarios ni consumidores que no participan, como Final
Cut o el retitle de Stalker. La mutación del proyecto/ACK es atómica en
Clipper; la operación completa entre catálogo, discos y editor sigue siendo
journalizada y recuperable, no una transacción ACID única.

## Productor aditivo de biblioteca · 28 octubre 2026

Esta sección sustituye las descripciones históricas de campos pendientes arriba.
Implementación: `media/evidence.py`, `layout.py`, `library.py`, `workspaces.py`.
Clipper sigue consumiendo `sources`; **no se declara adopción de estos campos
nuevos ni del handoff portable**. Se mantiene `creator-media-handoff/1`; todos
los campos nuevos son aditivos/opcionales en el schema para aceptar documentos
v1 anteriores. Un consumidor debe comprobar el major antes de interpretar.

### Reloj y procedencia

`sources[].capture_time` y `handoff.assets[].capture_time` contienen:

| Campo | Semántica |
| --- | --- |
| `original`, `provenance` | Texto original elegido y campo de origen (`manifest.epochWallClock`, `manifest.createdAt`, `sony.CreationDate`, `ffprobe.creation_time`), o null. |
| `originals` | Evidencia original adicional de Stalker: `epochWallClock`, `createdAt`, `captureTimeZone`, incluidos valores inválidos. |
| `captured_at_absolute` | Inicio RFC3339 normalizado a UTC **sólo si el valor original tiene offset/Z**; de lo contrario null. |
| `time_zone` | IANA válida declarada por productor, si además hay instante absoluto; null si falta/es inválida. |
| `offset_seconds` | Offset en ese instante de la IANA válida, o del timestamp original; null si desconocido. |

Stalker: `epochWallClock` válido con offset gana; después `createdAt` con offset.
No se aplica la zona del Mac a una cámara sin zona, ni se resuelve una hora
naive por una IANA potencialmente ambigua. La zona y el texto originales se
conservan. El reloj civil no hace sync acústico. Sony conserva CreationDate;
ffprobe conserva creation_time. mtime sólo sigue siendo fallback del naming,
**no evidencia de inicio absoluto**. El plan congela la evidencia y la incluye
en su digest; pasa a assets/recibos. Los catálogos/planes antiguos sin evidencia
emiten null: listar no abre manifests ni probea o hashea la biblioteca.

`captured_at` y `duration_seconds` actuales de `sources` conservan su semántica
legacy: civil naive y duración redondeada **del nombre**. No equivalen al nuevo
instante. `sources` conserva el orden lexicográfico de nombres actual (y `order`
consecutivo); no es una garantía de orden absoluto entre meses/zonas. El handoff
preserva exactamente el orden explícito de selección de IDs, desde cero.

### Miembros, revisiones y perfiles

- `bytes` de sources y handoff es el total de los miembros del asset en el
  plan, incluidos sidecars. `media_bytes` suma sólo `editable_members`:
  `{path,bytes,sha256}`; paths originales relativos al origen. Editables son
  archivos clasificados video/audio/imagen/raw_image por `layout.kind_of`;
  incluye las representaciones/raw de un Stalker, sin decidir mezcla editorial.
  No incluye manifest, eventos, XML ni soporte de cámara. Sources añade `volume_id`/`receipt_id` en ubicación elegida y alternativas, y
  `local_path` por miembro disponible en la ubicación elegida.
- `manifest_sha256` es SHA-256 de **bytes crudos** de manifest.json en recibos
  verificados, no el digest de un JSON reserializado. Se publica también en
  catálogos antiguos con recibos. En cámara/archivo: null.
- `revision` sigue siendo el SHA-256 legacy de JSON canónico de la lista
  ordenada según `asset.paths` de `{path,sha256}` de **todos** los miembros.
  Incluye los paths originales relativos al origen y el hash crudo del manifest.
  `content_revision = UUIDv5(NAMESPACE_URL, revision)` es la representación
  histórica portable de la misma revisión, ahora expuesta también en sources.
  No es una revisión editorial ni un contador de Stalker. Cambiar proyecto
  dentro del manifest cambia ambas revisiones; cambiar tags/título del catálogo,
  mapping de perfil o rutas de destino no las cambia. Renombrar el bundle de
  origen puede cambiar paths/asset ID al reingestar; `sessionID` sigue siendo la
  identidad de captura que permite relacionarlos. No prometer UUID de asset
  idéntico entre imports independientes: hoy deriva de identidad de origen y
  stamps/membresía del plan. `relayout` de copias conserva asset/revisión.
- `capture_manifest_digest` interno / `captures.manifest_digest` siguen siendo
  SHA-256 del JSON canónico parseado del manifest; pueden diferir de
  `manifest_sha256`. No intercambiarlos en CAS ni ACK de Clipper.
- `save_profile` RPC acepta `clipper_profile: string|null`: referencia opaca
  explícita; no importa schema ni configura un perfil de Clipper. Se publica en
  `projects`, sources (nivel proyecto y asset) y handoff. Ausente/null significa
  sin mapping; **no se infiere e64 → erikiado64**. Omitirlo al guardar desde un cliente antiguo conserva el mapping; null lo
  elimina. Es metadata vigente del catálogo; no modifica rutas congeladas ni content_revision. `profile_id` del
  handoff es el ID local (p.ej. e64), no el ID de Clipper.

### Ubicaciones y recibos portables

El [schema](media-handoff.schema.json) y el [ejemplo generado por código con
fixtures](examples/handoff.json) incluyen camera_bundle y Stalker. El ejemplo
no identifica volúmenes reales y no prueba existencia actual de sus rutas.
`locations[]` identifica volumen por UUID, rol y receipt_id real del recibo de
transferencia/trabajo. Representa verificación **histórica**, no rehash/presencia
actual. Las rutas son relativas a la raíz registrada del volumen.

Cada ubicación nueva lleva `path_kind` (`file` o `directory`) y `members` con
`original_path`, `relative_path`, `bytes`, `sha256`, `volume_relative_path`.
Este último resuelve inequívocamente cada archivo desde su volumen, incluso
pares JPG/DNG con múltiples archivos. En Stalker `relative_path` es interior
al bundle; en una ubicación file es el basename. `location.relative_path` es
el archivo primario o raíz del bundle. `asset.members` conserva la proyección
de la primera ubicación para lectores antiguos; para resolver otras copias,
usar **sus propios members**. Archivo/trabajo pueden tener basenames distintos.
El productor compara bytes/hash por original_path, nunca igualdad de nombres.
Handoff incluye todo Stalker; en cámara sólo media editable, no sidecars. Total
de camera_bundle y revisión siguen incluyendo los sidecars del original.

El schema valida forma. El receptor valida traversal, symlinks, colisiones,
UUID/major, identidad del volumen, bytes y hashes antes de adoptar o borrar.
Los campos nuevos son opcionales para documentos antiguos; no derivar una
ubicación múltiple a partir de un único path legacy sin verificarla.

## Retitle completado coordinado · implementación nüün, productor pendiente

Protocolo, errores, recuperación y aceptación exactos en
[handoff para Stalker](stalker-retitle-v1.md),
[schema de request/result/capabilities](stalker-retitle.schema.json).
`archived:false` es sólo observación. **No autoriza mutación**. El acuerdo
histórico que permitía renombrar con esa consulta suelta queda sustituido por
este protocolo. Nüün no escribe manifests ni proyectos Clipper; tampoco
reenlaza/adopta automáticamente una nueva revisión.


### Adopción Clipper CLI/TUI de los campos aditivos y handoff · 2026-10-28

Clipper sobre `e4f73d1` incorpora tiempo/procedencia/zona (unknown explícito),
media_bytes/miembros, hashes crudos/revisiones y clipper_profile sin inferencias.
CLI y F6 comparten consumidor handoff: volumen registrado/online, recibo real,
hash actual, confinamiento, límites, identidad y reintento durable. Sources y
pegado conservan compatibilidad. A esa fecha delivery y adopción de revisión
no estaban soportados; su extensión del 29 se describe abajo. Ubicaciones legacy
múltiples ambiguas y adopción implícita por UUID siguen rechazadas.
[Pruebas, matriz y pendientes](../../../../ai/clipper/docs/2026-10-28-cli-tui-contract-adoption.md).
Esta nota sustituye sólo el estado histórico «Clipper pendiente» de las
secciones de campos aditivos; no cambia semántica, schemas ni el productor.


### Consumidor Clipper · extensión del 29 octubre 2026

Handoff mantiene major 1: Clipper importa delivery de un único archivo de audio/video
con probe, miembros/recibos y hash verificados. Un delivery de varios miembros no
se convierte implícitamente en un clip. Nüün ya produce delivery mediante ingest
source_kind=delivery; no se cambia su formato ni se añade copia desde el editor.

Clipper define `clipper-revision-adoption/1` como ACK de su operación explícita
`library adopt-revision`, separada de relink/import y del retitle productor.
Sólo cambia project del manifest Stalker: mismo asset/session/proyecto/perfil,
recibos completos, bytes/membresía intactos salvo manifest, CAS editorial y
operation ID idempotente. El consumidor no escribe manifests ni catálogo.
La publicación de una revisión verificada bajo un asset lógico existente es
responsabilidad del catálogo y todavía requiere cerrar su recorrido operativo;
el fixture cruzado simula ese registro, sin alegar una UI implementada.

Deps puede devolver filas adicionales de la revisión/ruta histórica de un mismo
asset_id mientras una cola las fije (`status=historical_revision`,
`queued_exports` no vacío). No deduplicar únicamente por asset_id. El guard
actual de nüün ya bloquea estas filas por la cola; no se transforma un ACK de
adopción en un ACK de relink ni se permite liberar esa copia.

Retitle productor sigue bloqueado con referencias de Clipper, incluso inactivas.
La nueva adopción no autoriza retitle ni cambia el coordinador/locks de Stalker.
Audio por contribuciones y refinamiento PCM pertenecen a Clipper; no modifican
reloj, eventos, captura ni bytes archivados. Corrección afín de drift, delivery
multiarchivo, UI de adopción y aceptación larga/física siguen separados.
Autoridad y evidencia: [entrega de Clipper](../../../../ai/clipper/docs/2026-10-29-contract-coordination.md).
