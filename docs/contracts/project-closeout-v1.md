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
En la revisión, el botón final aplica el plan sin un segundo modal; el descarte
de intermedios y dependencias externas requiere la casilla explícita de revisión.

`closure_save.finals[]` añade `published` opcional; el booleano global anterior
sigue siendo compatible como declaración de todas las elegidas. `closure_plan`
añade `release_work` (por defecto true). Los planes nuevos registran
`closure_finish`; false sólo libera finales. `closure_apply` devuelve también
`closure_settings` con las asociaciones actualizadas a las ubicaciones de archivo.
El estado terminal, esas asociaciones y el evento de cierre se publican en una
sola transacción SQLite, para poder reanudar tras una interrupción sin confundir
un cambio propio con una nueva decisión humana. Los recibos previos se conservan. Guardar decisiones registra `project_delivery_decision` con el digest previo y la selección nueva en la misma transacción. El movimiento independiente de finales mantiene la exclusión de Clipper durante la consulta de dependencias y todo el retiro.

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

## Proyectos relacionados · propuesta pendiente de confirmación · 2026-11-07

Caso: `terra-6-1` y su regrabación `terra-6-1-extra`. El horizontal publicado y las miniaturas son de la regrabación. No se implementa hasta que el usuario confirme este diseño.

1. **Sugerir y confirmar.** En la hoja de cierre, **Incluir proyecto relacionado…** lista proyectos del mismo perfil cuyo título normalizado empieza por el del otro seguido de `-` (`terra-6-1` ↔ `terra-6-1-extra`). La persona confirma «es una regrabación del mismo trabajo». Se guarda en ambos `project-closure:` como `related[{project_id, kind: 'rerecording'}]`, con el evento `project_relation_confirmed`. Quitar la relación es otra decisión registrada.
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
