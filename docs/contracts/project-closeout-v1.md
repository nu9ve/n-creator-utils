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
o renders, y no termina el proyecto. **Revisar liberación** mantiene las puertas
del cierre completo: requisitos, publicación y declaración de entregas completas.
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

## Autoridades y alcance

- nüün guarda requisitos por proyecto, con valor inicial por perfil: `9logs`
  sólo horizontal; `e64` horizontal, al menos un vertical y thumbnail. La
  persona confirma «ya publiqué» y «ya no habrá más entregas». No se certifican
  canales ni URLs. Marcar terminado y guardar decisiones no borran media.
- Los nombres sólo filtran candidatos. La persona vincula finales de trabajo
  o archivo a un proyecto existente; una ubicación no se reasigna en silencio.
  No se renombran carpetas ni cambia el mapping `e64` / `erikiado64`.
- Stalker conserva manifests y reloj. Se reutiliza el ingreso de bundles y
  los recibos existentes; este cierre no vuelve a implementar el ingreso.
- Clipper conserva edición, registro y colas. Se usan deps, exclusión de
  kernel, reenlace y ACK por su CLI. **No existe comando público de borrado
  con desregistro:** el editable se conserva y se indica eliminarlo desde
  Clipper. No se editan sus JSON ni se importa código de su repo.
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
| `closure_progress` | `plan_id`; etapa y conteos reales del retiro. Sin promesa de porcentaje global ni ETA. |

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

Pendiente por Clipper: API pública de retiro del proyecto que compruebe
uso/colas y quite su propio registro. Publicación por canal, recetas editables,
cámaras y progreso global/agrupación de Ingreso son trabajos separados.

Las pruebas usan volúmenes fixture y media temporal. La aceptación en la app
con un proyecto real pequeño sigue siendo una operación explícita del usuario;
instalación y fixtures no sustituyen los [gates de release](../release.md).
