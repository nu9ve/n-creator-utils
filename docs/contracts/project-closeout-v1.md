# Cierre de proyectos · primera entrega · 2026-11-04

Decisión del usuario: conservar fuentes originales y finales. Los intermedios
identificados se descartan sin respaldo, con consentimiento explícito; es la
excepción nueva a la regla de exigir copia de todo. Lo desconocido se conserva.

## Actualización vigente · 2026-11-05

**Sin gestión por canal**, por decisión del usuario. Estar en la carpeta de
publicados (`subidos`, o la elegida por perfil) es su declaración de publicación.
También puede marcarse `published` por final desde el proyecto, sin moverlo
primero a esa carpeta. Guardar sólo registra decisiones; publicación y respaldo
verificado son estados independientes. Un archivo modificado exige revisión.

**Respaldar publicados…** prepara sólo los finales elegidos publicados: copia,
verifica y retira sus copias de trabajo. No exige «ya no habrá más» ni toca raws
o renders, y no termina el proyecto. **Revisar cierre** mantiene los requisitos
del proyecto y la declaración de entregas completas.
En la revisión, el botón final aplica el plan sin un segundo modal. Desde el
7 nov, por decisión del usuario, pulsar **Completar y liberar** sobre la lista
revisada es el consentimiento del descarte de intermedios: ya no hay casilla.
La revisión avisa que se descartan sin respaldo y el motor sigue registrando
`closure-consent` con el digest.

`closure_save.finals[]` añade `published` opcional; el booleano global anterior
sigue siendo compatible como declaración de todas las elegidas. `closure_plan`
añade `release_work` (por defecto true). Los planes nuevos registran
`closure_finish`; false sólo libera finales. `closure_apply` devuelve también
`closure_settings` con las asociaciones actualizadas a las ubicaciones de archivo.
El estado terminal, esas asociaciones y el evento de cierre se publican en una
sola transacción SQLite, para poder reanudar tras una interrupción sin confundir
un cambio propio con una nueva decisión humana. Los recibos previos se conservan. Guardar decisiones registra `project_delivery_decision` con el digest previo y la selección nueva en la misma transacción. El movimiento independiente de finales mantiene la exclusión de Clipper durante la consulta de dependencias y todo el retiro.

En el sandbox de macOS, Rust ejecuta ambas variantes. El cierre editorial vuelve a verificar el inventario de raws y renders, confirma los respaldos antes del retiro y conserva la copia de trabajo de los finales pendientes de publicar. La limpieza de regenerables se delega a Clipper después del cierre y se reintenta con el digest original si falla.

La ruta por rol de una entrega seleccionada se resuelve antes del digest y
preflight, también cuando un vertical y un horizontal comparten nombre. Un
contenido distinto en el destino bloquea; nunca se reemplaza silenciosamente.

## Entregas agrupadas e Inicio · 2026-11-07

El diálogo agrupa horizontal, vertical, miniatura y auxiliares. La selección
por grupo o de todos los visibles conserva las correcciones de tipo y de
publicación ya hechas. Los requisitos del perfil quedan en un desplegable.
En una revisión nueva, sólo se preseleccionan entregas publicadas con
coincidencia exacta de título o alias según las convenciones de nombre.
Las coincidencias amplias quedan detrás de **Ver otras coincidencias**.
El usuario sigue confirmando la pertenencia en el plan.

Se muestra publicación por carpeta elegida, decisión explícita de esa
versión o recibo de una transferencia terminada desde publicados. El recibo
debe corresponder a la ubicación y al stamp actual del final. Una ubicación
en MEDIA por sí sola deja la publicación por confirmar. Se puede confirmar
un grupo o corregir cada entrega desde su menú.

El primer vertical publicado con asociación explícita o nombre exacto
genera una sugerencia en Inicio durante la consulta de finales. Cuenta
entregas y no sus réplicas. La sugerencia abre la revisión y no suma al badge
ni emite una notificación. Queda pendiente decidir el aviso opcional para
varios verticales con más de un día y cómo registrar su antigüedad.
**Estas son todas las entregas definitivas** y el consentimiento de descarte se mantienen
para el cierre completo. La revisión separa finales, raws e intermedios.

## Completar antes de publicar · 2026-11-07

Elegir archivos los vincula a la selección. La publicación se declara por
carpeta o desde el menú **Publicación**. Marcar la selección como definitiva
autoriza revisar el cierre editorial aunque haya entregas pendientes de subir.
Si el proyecto requiere menos tipos que el perfil, se pueden usar los tipos
elegidos como excepción sólo para el proyecto.

Un final pendiente de publicar se copia a MEDIA cuando falta y se verifica.
Su copia de ESSD se conserva para publicar y el estado de publicación
permanece pendiente. Un final publicado de trabajo conserva la política
de retirar esa copia sólo tras verificar el respaldo y sus dependencias.
El plan identifica estas acciones mediante `keep_work`, `copy_required`
y el stamp de la selección conservada. Un cambio posterior de esa copia
invalida el plan antes del retiro de otros archivos.

El plan congela rutas raw y renders conocidas incluso cuando están vacías o
ausentes. La revisión muestra **Sin acción** en esas categorías. Un archivo
nuevo en una ruta revisada invalida el plan. Se siguen verificando los
originales catalogados y las entregas, incluso si el cierre retira cero
archivos y sólo respalda entregas pendientes. El registro del cierre
puede terminar con cero bytes liberados. La exclusión con Clipper se
requiere para planes con retiro.

## Preparar la revisión y efectos visibles · 2026-11-07

El botón del formulario dice **Preparar revisión…**. Prepararla no copia ni retira nada: lee los archivos para verificar sus huellas (fuentes en el respaldo, intermedios, entregas elegidas y su par en el respaldo), y por eso tarda según los bytes. La hoja muestra la fase, el archivo actual y los bytes leídos (`closure_plan` con `review_id`, que reporta en `closure_progress`), y se puede cancelar. Los hashes siguen siendo completos. Durante la revisión, la identidad de cada disco se consulta una sola vez.

En la revisión, cada entrega y grupo lleva un badge con su efecto y el color de estado del tema:
- Verde: ya está en el respaldo, sin cambios o ya retirado.
- Azul: respaldar y conservar en edición.
- Ámbar: retirar una copia de edición.
- Rojo: descartar intermedios sin respaldo.
- Gris: sin acción.

Aplicar el plan sigue siendo el paso que copia y libera.

## Aplicar con progreso total y reabrir un cierre aplicado · 2026-11-07

Al aplicar, la hoja reemplaza el plan por la vista de acción ([interacción, «Acciones largas»](nu9ve-interaction-v1.md#acciones-largas)). El motor emite en `closure_progress`:
- `overall_done`/`overall_total` en bytes: copiar y verificar cada final cuenta dos veces su tamaño, y retirar cada copia de edición cuenta su lectura;
- `files_done`/`files_total`;
- `current` (nombre, origen, destino, acción);
- un `log` de hasta 60 pasos terminados (`copied`, `released`).

Solo informa: no cambia qué se copia ni qué se retira.

Al reabrir un proyecto con un cierre aplicado, la hoja empieza con su resumen: fecha, archivos retirados, bytes liberados y las entregas que siguen en edición sin publicar. Estas ya están respaldadas. Al publicarlas, **Respaldar publicados…** retira su copia de edición sin preparar todo el cierre otra vez.

## Carpeta de Clipper y caché · decisión del usuario · 2026-11-07

Medido en ESSD, en sólo lectura: las carpetas `clipper/` de cuatro proyectos `e64` suman 9.9 GB y sus transcripciones unos 16 MB. Dos de esos proyectos ya están retirados en Clipper, `terra-6-1-extra` (634 MB, 509 de caché) y `debloateando-mi-tele` (833 MB, 740 de caché). `buscando-baches-con-el-dron` guarda 4.6 GB de exports, 2.7 GB de caché y 314 MB de temporales `.clipper-multicam-*` que quedaron en `logs/`. Ningún export coincide en tamaño con un final de ESSD o MEDIA502. El usuario ya había limpiado la caché desde Clipper en los demás proyectos; lo que queda (268–740 MB por proyecto) es siempre `cache/stalker/*.wav`, el audio que Clipper extrae de las sesiones Stalker y que su Storage / Cleanup no ofrece borrar.

Decisión:
- A largo plazo se conservan las transcripciones y lo necesario para recuperar la edición. Al retirar un proyecto no deben quedar archivos inaccesibles acumulándose.
- **Qué es regenerable dentro de `clipper/` lo decide Clipper.** Pedido registrado en su repo: [retiro con limpieza](../../../../ai/clipper/docs/2026-11-07-retire-cleanup-from-nuun.md). Pide limpieza revisada al retirar, cerrar la fuga de temporales, una política de caché y una lista legible por máquina.
- **Excepción anterior (retirada el 7 nov):** nüün retiraba por su cuenta los `.wav` de `clipper/cache/stalker/` de un proyecto ya retirado en Clipper (`origin: clipper_cache`, grupo «Audio de caché de Clipper»). Ya no se ofrece. Esos archivos son ahora la categoría `project_cache` de la lista de Clipper. Un plan revisado antes del cambio con entradas `clipper_cache` se rechaza al aplicar (`plan_changed`) y hay que preparar la revisión otra vez.
- **Lista de regenerables de Clipper (implementada en Clipper y consumida por nüün, 7 nov).** Clipper es la única fuente de lo regenerable dentro de `clipper/`:
  - `clipper library cleanup-plan <carpeta> --json` (proyecto activo o retirado) y la clave `regenerable` de `clipper library retire-plan --json` devuelven `schema: clipper-regenerable-files/1`, `project_root`, `project_state` (`active`/`retired`), `inventory_complete`, `categories[]` (`id`, `label`, `files`, `bytes`, `default_selected`), `preserved[]` y `entries[]` con `path`, `category`, `bytes`, `hardlink_duplicate` y `stamp` (`device`, `inode`, `size`, `mtime_ns`, `ctime_ns`), más `digest`. Generarla no borra nada.
  - `cache/stalker/*.wav` y `cache/sync-clock/*.mov` forman la categoría `project_cache`, preseleccionada sólo en proyectos retirados o en la revisión de retiro. `exports` y `queue` se ofrecen sin preseleccionar. Transcripciones, manifest o `clipper-retired-project.json`, reports, JSON de análisis, locks, fuentes y finales nunca aparecen.
  - Para borrar: `clipper library cleanup --plan <doc> --digest <digest> [--category …] --json` (respuesta `clipper-regenerable-cleanup/1` con `deleted_files`, `deleted_bytes`, `skipped[]`) o `clipper library retire … --cleanup-digest <digest>`. Clipper revalida categoría, dueño y stamp bajo su lease de Storage y omite lo cambiado (`skipped[]`). Una entrada sin dueño único (fuente compartida con otro proyecto) no se lista.
  - Los proyectos retirados siguen en Storage / Cleanup de Clipper (por `clipper-retired-project.json` y los recibos de retiro). Las carpetas `.clipper-multicam-*` ya no se acumulan en `logs/`: Clipper las borra al terminar, fallar o cancelar el export, y barre las de ejecuciones muertas en el siguiente export multicam.
- **Cómo la usa el cierre (`media/closure.py`, `clipper_regenerables` / `clipper_cleanup`):**
  - Sólo en un cierre completo (no en **Respaldar publicados…**). Por cada carpeta de edición del grupo, si `<editor>/clipper/` (o la propia carpeta) tiene `clipper-project.json` o `clipper-retired-project.json`, la revisión pide `cleanup-plan`. nüün valida el documento: esquema, `project_root` igual a esa carpeta, tipos de cada categoría y entrada, y digest de 64 caracteres.
  - Se ofrecen sólo las categorías que Clipper preselecciona (`default_selected`). En un proyecto retirado incluye `project_cache`. En uno activo, `project_cache`, `exports` y `queue` no se ofrecen. Las no preseleccionadas aparecen en «Conservar» con sus archivos.
  - Si Clipper no responde, el comando falla, el documento no encaja o `inventory_complete` es false, no se ofrece nada de esa carpeta y «Conservar» dice por qué.
  - El plan guarda `clipper_regenerable[]`: por carpeta, `editor`, `project_root`, `project_state`, `digest`, las categorías elegidas, sus `entries`, `files`, `bytes` y el documento completo. También guarda `clipper_files` y `clipper_bytes`. El digest del plan de cierre cubre todo eso. Esas entradas no van en `entries` ni en `closure_scopes`: nüün no las borra ni las hashea.
  - La revisión las muestra como intermedios, un grupo por categoría («Clipper · <categoría>», carpeta, archivos y bytes según Clipper) con la etiqueta «Clipper los borra». Los totales de la cabecera las suman.
  - Con el consentimiento de **Completar y liberar** (`confirm_discard`), y sólo después de que el release terminó bien, nüün escribe el documento revisado en `<catálogo>/clipper-cleanup/<plan>-<n>.json` y llama a `cleanup --plan … --digest <revisado> --category <elegidas>`. Comprueba la respuesta (esquema, `plan_digest`, categorías y contadores) y la registra en `clipper_cleaned[project_root]`: `deleted_files`, `deleted_bytes`, `skipped` (hasta 200) y `skipped_count`. Si falla, guarda `error` sin deshacer el cierre, que ya terminó. Volver a aplicar el mismo plan sólo repite lo que falló, con el mismo digest. Si algo cambió, Clipper lo omite.
  - `project-released` sigue contando sólo los bytes que retiró nüün. Lo borrado por Clipper está en `clipper_cleaned` y la hoja de cierre lo suma al resumen.
  - Pendiente en Clipper: un archivo durable de transcripciones (propuesta `CLIPPER_TRANSCRIPT_ARCHIVE_ROOT`, sin implementar). Cuando exista, el cierre podría verificarlo en el respaldo. Hoy las transcripciones se conservan dentro de `clipper/`.
- **Duplicados de finales (por implementar):** un archivo en `renders/` o `clipper/exports/` idéntico por SHA-256 a un final ya verificado en MEDIA se puede retirar en el cierre, con su propia línea en la revisión. Un nombre parecido sólo sirve para sugerir candidatos; la decisión la da el hash. Hoy no hay ninguno en esos proyectos.
- **Proyectos vinculados (implementado):** se vinculan desde Proyectos y «Estas son todas las entregas definitivas» del proyecto que se cierra autoriza los intermedios de todo el grupo. Ver la sección «Proyectos relacionados».

## Transcripciones durables en MEDIA · 2026-11-07

El usuario quiere conservar las transcripciones de Clipper ligadas a los videos ya respaldados, en un área de datos por programa dentro de MEDIA. Clipper ofreció escribirlas ahí. Para respetar las fronteras vigentes (nüün es el único que escribe en MEDIA, con copia verificada y recibos; Clipper es dueño de su contenido), se propone:
- **Clipper** exporta, sin escribir en MEDIA, la lista de transcripciones de un proyecto: ruta, formato y la fuente a la que pertenecen (SHA-256 del miembro o `asset_id` de nüün).
- **nüün** las copia y verifica a un área nueva de MEDIA, por ejemplo `004_datos/clipper/transcripts/<asset>/`, con recibo y ligadas al asset respaldado. Lo hace en el ingreso posterior o en el cierre, y el cierre comprueba que estén antes de liberar.

El usuario pidió avanzar el 7 nov: el intercambio quedó definido en [transcripciones de Clipper v1](clipper-transcripts-v1.md). Clipper implementa la exportación y nüün el área, la copia y la comprobación en el cierre.

## Una sola verificación al aplicar · 2026-11-08

Medido en el cierre de Terra: aplicar rehasheaba dos veces todas las fuentes y entregas del grupo en MEDIA502 (unos 10 GB a ~117 MB/s) aunque nüün no retiraba nada. Ahora:
- **Una verificación del respaldo por aplicación.** `apply_close` la hace y pasa sus guardas a `apply_release`, que ya no relee todo. Antes de cada retiro se siguen vigilando por stamp, y un cambio obliga a rehashear. Se conserva la relectura por archivo previa a cada retiro de nüün.
- **Sin retiros de nüün** (el plan sólo copia transcripciones, conserva entregas o deja que Clipper borre sus regenerables), una copia del respaldo cuyo stamp coincide con el que la revisión registró al hashearla (`archive_stamp`) no se vuelve a leer. Una copia cambiada, una hecha en esta misma aplicación o una sin stamp de revisión se hashea completa. Es la misma vigilancia por stamp que Transfer usa con destinos verificados.
- **Con retiros de nüün** se hashea cada copia del respaldo una vez antes del primer retiro, como exige «liberación con hash actual».
- La verificación entra en la medida global (`verify_total`), con su archivo y su avance, así que la barra no se queda quieta.

## Revisión con medida global y proyecto ya completo · 2026-11-07

**Preparar revisión…** ya no muestra el avance de cada paso. `closure.review_estimate` calcula al empezar, sólo con tamaños, los bytes y archivos que la revisión va a leer:
- raws en edición dos veces (leer y comparar con el respaldo);
- fuentes catalogadas una vez en el respaldo;
- intermedios una vez;
- cada entrega elegida una vez, y otra más si el respaldo tiene un archivo con el mismo nombre y tamaño.

`ReviewMeter` emite en `closure_progress` estos campos: `overall_done`/`overall_total`, `overall_files_done`/`overall_files_total`, el archivo actual (`current`, `file_done`/`file_total`) y la etapa. La medida se detiene en 99 % hasta terminar, así una estimación baja nunca hace retroceder la barra. Los hashes siguen completos.

Si el proyecto ya está liberado y la revisión no encuentra nada que copiar ni retirar, el plan trae `already_complete`. La hoja dice «Ya está completo» y no ofrece aplicarlo: aplicarlo sólo reescribiría el registro del cierre.

## Por publicar en Proyectos · 2026-11-07

El motor lee dónde está cada entrega guardada (`closure.delivery_states`). Proyectos (`capture_overview` → `projects[].deliveries`) y la hoja al reabrir (`closure_candidates.deliveries`, `closure_deliveries`) usan esa misma lectura, así que la fila y el resumen coinciden:

| Estado | Significado | Cuenta en la fila |
| --- | --- | --- |
| `archived` | La selección ya apunta al respaldo | — |
| `awaiting` | En el disco de edición, sin publicar | «Terminado · N por publicar» |
| `published` | Publicada, todavía en el disco de edición | «Publicadas por respaldar» |
| `moved` | El mismo archivo (nombre, tamaño, mtime e inode) está ahora en la carpeta de publicados; trae `current_path` | Igual que `published` |
| `published_archived` | Ya no está en edición y hay un recibo de transferencia desde publicados con ese nombre y tamaño | — |
| `missing` | Ya no está y no hay evidencia de dónde quedó | «Entregas no encontradas»; no cuenta como espera |

Con el disco desconectado se conserva la decisión guardada (`offline`). La lectura hace un `stat` por entrega y sólo recorre la carpeta de publicados si falta un archivo; no lee contenido ni calcula hashes.

Proyectos agrega el filtro **Por publicar**: proyectos terminados que todavía tienen entregas en el disco de edición. Al reabrir, si una entrega se movió a la carpeta de publicados, la hoja la propone con su nueva ubicación y como publicada. Se guarda sólo cuando la persona guarda o respalda; el motor no reescribe la selección. Tras **Respaldar publicados…**, el cierre actualiza la selección al respaldo y la marca baja sola.

## Proyectos relacionados · implementado · 2026-11-07

Caso: `terra-6-1` y su regrabación `terra-6-1-extra`. El horizontal publicado y las miniaturas son de la regrabación. El usuario confirmó el diseño y pidió hacer el vínculo desde Proyectos.

Implementación:
- `closure_link` (`project_id`, `other_id`, `linked`) y `closure_link_candidates`. Proyectos ofrece **Vincular con otro proyecto…** y, en el detalle, la sugerencia por nombre con un botón **Vincular**.
- `project-closure:<id>.related` es simétrico y se conserva al guardar entregas.
- El plan lleva `closure_group` y `closure_group_titles`, y cada final su `owner`. `closure_settings_digest` cubre las decisiones de todo el grupo; un proyecto solo conserva su digest anterior. Vincular o desvincular invalida una revisión abierta.
- El commit terminal escribe estado, liberación y selección de cada proyecto del grupo.

Diseño original, que sigue vigente:

1. **Sugerir y confirmar.** Desde Proyectos (antes se pensó en la hoja de cierre), se listan proyectos del mismo perfil cuyo título normalizado empieza por el del otro seguido de `-` (`terra-6-1` ↔ `terra-6-1-extra`). La persona confirma «es una regrabación del mismo trabajo». Se guarda en ambos `project-closure:` como `related[{project_id, kind: 'rerecording'}]`, con el evento `project_relation_confirmed`. Quitar la relación es otra decisión registrada.
2. **Una lista de entregas.** La selección muestra las entregas de los dos proyectos en un solo grupo por tipo. Cada final conserva su proyecto dueño: la regla `final_already_linked` no cambia y nada se reasigna. Los requisitos de cada proyecto se cumplen con la unión: un horizontal de `terra-6-1-extra` cubre el horizontal de `terra-6-1`, y la revisión dice «cubre el requisito de terra-6-1 · pertenece a terra-6-1-extra».
3. **Fuentes y acciones por tanda.** El plan es uno solo, con un digest, pero `closure_scopes` lleva el proyecto de cada scope. La revisión muestra una sección por tanda: raws, intermedios y badges de efecto. Una tanda sin trabajo en ESSD sale con **Sin acción**.
4. **Sin fusionar historiales.** Cada proyecto conserva su catálogo, sus alias, recibos y eventos. Al terminar, el commit terminal escribe `project-state`, `project-released` y `project-closure` de cada uno en la misma transacción.
5. **Terra hoy.** `terra-6-1` ya está cerrado (siete archivos retirados). Al incluir la regrabación, su tanda saldría **Sin acción** y la de `terra-6-1-extra` mostraría lo que aún tenga en ESSD.

La alternativa más corta, dos cierres separados en los que el requisito acepta finales de un proyecto relacionado confirmado, ahorra la vista por tandas pero obliga a revisar dos veces las mismas entregas.

## Autoridades y alcance

- nüün guarda requisitos por proyecto, con valor inicial por perfil: `9logs`
  sólo horizontal; `e64` horizontal, al menos un vertical y thumbnail. La
  persona confirma la selección definitiva. La publicación tiene su propia
  declaración. Marcar terminado y guardar decisiones no borran media.
- Los nombres sólo filtran candidatos. La persona vincula finales de trabajo
  o archivo a un proyecto existente; una ubicación no se reasigna en silencio.
  No se renombran carpetas ni cambia el mapping `e64` / `erikiado64`.
- Stalker conserva manifests y reloj. Se reutiliza el ingreso de bundles y
  los recibos existentes; este cierre no vuelve a implementar el ingreso.
- Clipper conserva edición, registro y colas. Se usan deps, exclusión de
  kernel, reenlace y ACK por su CLI. **Clipper ya ofrece retiro editorial con desregistro** mediante
  `library retire-plan` / `retire` y TUI (5 noviembre). Conserva un snapshot y
  todos los archivos; nüün sigue conservando el editable por defecto y la persona
  decide el retiro desde Clipper. No se editan sus JSON ni se importa código de
  su repo. Su siguiente consulta deps reconoce la ausencia de referencias activas;
  el recibo aditivo no autoriza retirar media ni sustituye comprobación de respaldo.
- Final Cut queda fuera: no se abre ni modifica una biblioteca, incluso dentro
  de `renders`. La persona confirma que ninguna otra edición necesita los
  archivos a retirar; nüün no afirma haber inspeccionado referencias FCP.

## Operación

Owner: [closure.py](../../media/closure.py), boundary [service.py](../../media/service.py),
retiro compartido [release.py](../../media/release.py).

1. Resolver carpetas raw desde recibos, rutas congeladas, aliases y perfil.
   Verificar SHA-256 de las fuentes en archivo, aunque su copia de trabajo ya
   se retiró. Sin fuentes catalogadas, con operación pendiente o respaldo
   faltante/alterado, bloquear. No se repite un ingreso ya resuelto.
2. Inventariar raws y `renders/` de las carpetas editoriales conocidas. Fuera
   de esos límites, incluidos editables, bibliotecas y material desconocido,
   se conserva todo. La revisión lista archivos y bytes por acción.
3. Para finales archivados, verificar la ubicación elegida sin duplicarla.
   Para elegidos de trabajo, adoptar un idéntico por hash o copiar mediante
   Transfer: masters por año, verticales a `shorts`, thumbnails a `thumbnails`.
   Se conservan nombres y sólo se copian los archivos elegidos.
4. Congelar decisiones, stamps, hashes, destinos y categorías en un plan
   registrado con digest. Confirmar descarte y dependencias externas.
5. Bajo exclusión con Clipper: copiar/verificar finales, revalidar fuentes y
   finales, reenlazar con ACK, snapshot de catálogo y retiro journalizado por
   archivo. Los destinos verificados se vigilan por stamps; cambios obligan
   rehash. Colas o dependencias desconocidas conservan el origen.
6. Cambios de archivos/configuración/identidad, corrupción o desconexión
   detienen la operación. Reanudar el mismo ID/digest continúa el journal;
   no adopta desapariciones sin intención registrada ni retira archivos nuevos.
7. Sólo al terminar se registra el proyecto terminado/liberado. Inicio muestra
   **Falta liberar trabajo** para cierres declarados completos pendientes; es
   invitación a revisar, no prueba de integridad actual.

Los planes, decisiones, consentimiento y resultados entran en snapshots SQLite.
Fuentes y finales mantienen recibos portables del motor. Un render descartado
no tiene réplica prometida. Nunca se ofrece borrar la última copia de MEDIA.

## Boundary JSON/RPC

Accesible tanto desde la app como por `media rpc`:

| Método | Parámetros / efecto |
| --- | --- |
| `closure_candidates` | `project_id`, `query?`; nombres/tamaños, roles sugeridos y decisiones. Máximo 1000 resultados con `truncated`; sin hashes. |
| `closure_overview` | Decisiones y recordatorios, sólo metadata. |
| `closure_save` | `project_id`, `required[]`, `no_more`, `published`, `finals[{volume_id,path,role,published?}]`, `profile_default?`; guarda decisiones, no mueve archivos. |
| `closure_plan` | `project_id`, `archive`, `work`, `release_work?` (true por defecto); comprueba hashes y registra el plan. |
| `closure_apply` | `plan_id`, `digest`, `confirm_discard: true`; ejecuta el cierre. Sin consentimiento también se bloquea un intento por `apply_release`. |
| `closure_progress` | `plan_id`; etapa y conteos reales del retiro. Al aplicar trae `overall_done`/`overall_total`; sin ETA. |
| `closure_deliveries` | `project_id`; estado actual de cada entrega guardada (ver «Por publicar»). Sólo lectura. |

El lector persistente sólo admite candidates/overview/progress. Swift aplica
por ID/digest, sin reserializar el plan. Actividad y Proyectos permiten reanudar.

`nu9ve-release/1` conserva compatibilidad y añade `closure_project_id`,
`closure_settings_digest`, `closure_scopes`, `closure_finals`, `closure_copies`
y `retained_editors`. Cada entrada de cierre lleva categoría `raw`, `final` o
`intermediate`; sólo esta última admite `archive_path: null`. Los planes raw
anteriores mantienen su semántica. Transfer admite selección explícita de
archivos sólo para entregas sin limpieza; conserva digest/preflight/verificación.

## Límites y aceptación

Material histórico con fuentes sin catálogo exige adopción/auditoría antes de
este cierre. Sí pueden vincularse finales ya movidos a la colección `finales`
de MEDIA sin devolverlos a ESSD ni reorganizar terabytes. No se localiza ni
borra cualquier caché del disco ni carpetas editoriales arbitrarias.

API de retiro Clipper disponible en su checkout CLI/TUI, con plan/digest,
comprobación de uso/colas y desregistro propio. [Contrato y prueba real pendiente](../../../../ai/clipper/docs/2026-11-05-project-retirement.md).
No se añade invocación automática desde el cierre de nüün ni notificación push;
la coordinación sigue por deps frescos bajo exclusión. Publicación por canal está fuera por decisión del usuario. Recetas editables y registro de cámaras siguen pendientes; progreso global/agrupación de Ingreso ya se implementaron el 5 noviembre (ver contrato de ingreso).

Las pruebas usan volúmenes fixture y media temporal. La aceptación en la app
con un proyecto real pequeño sigue siendo una operación explícita del usuario;
instalación y fixtures no sustituyen los [gates de release](../release.md).


## Cambios externos y borrados previos · 2026-11-06

El usuario reporta haber eliminado algunos intermedios fuera de nüün y algunos proyectos desde Clipper. No se han inventariado esos cambios ni verificado sus desregistros. El siguiente recorrido se hace íntegro desde la app: no preparar proyectos/planes por CLI ni modificar registros para que la prueba pase.

- Un plan nuevo inventaría sólo raws/renders presentes dentro de sus scopes conocidos. Intermedios ya ausentes no requieren recreación para borrarlos, pero tampoco declaran el proyecto terminado ni prueban respaldo de originales/finales. La política sólo identifica `renders/`, no cualquier archivo que el usuario llame intermedio.
- Un archivo desaparecido **después** de revisar un plan no se acredita como retiro de nüün sin intención journalizada. Revisar/replanificar; no reutilizar un digest para ocultar cambios externos.
- `sources --purpose edit` informa copias de trabajo ausentes en `removed_from_work`; no las restaura ni cambia a archivo silenciosamente. Un archivo faltante en el respaldo es otra situación y bloquea cierre cuando falta la evidencia requerida.
- Borrar/retirar desde Clipper debe reflejarse en deps frescos y su propio registro. Si queda un proyecto enlazado no disponible, un disco offline o una cola que fija rutas, nüün puede bloquear la liberación; no borrar su JSON/registro desde aquí. El proyecto del catálogo nüün conserva su historia y no se termina por desaparecer el editable Clipper.

Pendiente operativo: observar el estado desde Biblioteca/Actividad y el siguiente plan de la app; si bloquea, registrar razón y proyecto/cola concretos. No afirmar reconciliación de los borrados reales sólo por esta lectura de código.
