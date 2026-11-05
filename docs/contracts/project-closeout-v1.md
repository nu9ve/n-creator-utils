# Cierre de proyectos · primera entrega · 2026-11-04

Decisión del usuario: conservar fuentes originales y finales. Los intermedios
identificados se descartan sin respaldo, con consentimiento explícito; es la
excepción nueva a la regla de exigir copia de todo. Lo desconocido se conserva.

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
| `closure_save` | `project_id`, `required[]`, `no_more`, `published`, `finals[{volume_id,path,role}]`, `profile_default?`; guarda decisiones, no mueve archivos. |
| `closure_plan` | `project_id`, `archive`, `work`; comprueba hashes y registra el plan. |
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
