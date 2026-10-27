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

Estado 2026-10-25: Clipper CLI/TUI hace relink **local y a pedido** de una fuente importada desde la biblioteca (misma revisión; `sessionID` + digest del manifest o SHA-256), reescribe items, `capture_sessions`, `sync_reference_path` y firmas de placement, e invalida previews. Cada relink publica un ACK (`clipper-relink-ack/1`: `operation_id` idempotente, asset/revisión, `from`/`to`, revisión del proyecto antes/después, `still_pinned` con los exports en cola que conservan la ruta vieja) en `clipper library deps --json`, junto con las dependencias por asset. n-creator-utils aún no lo consume y no hay leases. Ver [adopción Clipper](media-naming-v1.md#adopción-clipper-implementada-2026-10-25).

Un contrato de leases registra uso en proyecto/queue/Final Cut/Atlas, responsable y revisión. Un consumidor offline o no integrado no autoriza limpieza; ofrecer retención manual. Hash igual permite resolver nueva copia, pero el consumidor confirma su propia consistencia. Un asset cambiado invalida sync/transcripts/previews según el editor, no según el archivador.

## Finalización de Stalker y metadata común

Acuerdo de inmovilidad adoptado por Stalker 0.4.0 (2026-10-25, `ecb18f1`): `status=completed` es la última escritura, durable y atómica, después de cerrar writers y derivados; un bundle `completed` no se regenera ni renombra en sitio (la CLI de recuperación escribe fuera). La «revisión verificable» es el digest del manifest: si alguien edita el bundle, cambia y `captures` lo reporta `archived: false`. Detalle en [naming v1](media-naming-v1.md#adopción-stalker-implementada-2026-10-25). Una notificación del SO es sólo una pista; el archivador vuelve a leer y verifica manifest/membresía antes y después.

Stalker 0.4 guarda en el manifest `project: {version: 1, name, country}` (texto del usuario, sin IDs). `project_id`/`profile_id` comunes siguen siendo del catálogo; Stalker no los conoce todavía. Stalker mantiene ownership y compatibilidad. Nunca exigir nombrar un proyecto para poder grabar una prueba rápida. El organizador puede clasificar esa captura más tarde sin cambiar la identidad/reloj.

## Entregas y publicación

Entrega contiene ID, proyecto/perfil, variantes elegidas, hashes y roles (master, short, thumbnail, subtitle, copy). Estado por variante/canal: `candidate`, `approved`, `published`, con evidencia manual/URL opcional; cobertura de archivo se guarda aparte. Archivar sólo publicados puede ser política, pero entregas aprobadas aún no publicadas siguen siendo datos que pueden requerir respaldo.

No deducir publicación por existencia en Downloads o por terminar render. `subidos` es una convención configurable que propone un estado. Preservar nombres finales del usuario; cambios de nombre son ubicación/metadata, no contenido. Proyectos editables, bibliotecas FCP y assets externos necesitan cobertura aparte de las entregas.

## Transporte y fases

Fase 1: archivos JSON + helper local de una sola ejecución, stdout estructurado/stderr logs. Futuro NDJSON de progreso con versión, `event_id`, `job_id`, secuencia, fecha, fase y bytes por destino; el cliente recupera estado por job y tolera eventos repetidos/faltantes. No prometer exactly-once por notificaciones.

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
