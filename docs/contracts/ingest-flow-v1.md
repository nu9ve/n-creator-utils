# Ingreso directo · 2026-11-05

Decisión del usuario: **Entrada → Flujo → Resultado**, con destinos y revisión
juntos. El botón final autoriza el plan visible, incluyendo la limpieza
verificada indicada. No hay otro «ejecutar / ¿estás seguro?». El cierre de
proyecto mantiene la revisión explícita del descarte sin réplica.

## Resolución y soporte de cámara

- El proyecto general es un fallback: puede estar vacío si cada unidad de
  media ya tiene uno. Se valida después de resolver las asignaciones, igual
  en CLI/RPC y Swift. Una unidad principal sin proyecto efectivo se bloquea.
- RAW/JPG y XML asociado mantienen su unidad. Soporte sin media principal se
  agrupa con `support: true`, sin copia de trabajo ni proyecto editorial.
  Sus bytes, stamps, hashes, limpieza y recibos se verifican normalmente.
- El plan mantiene un tag técnico `camera-support` para compatibilidad de
  campos; no se crea un proyecto/asset editorial con ese nombre. Los recibos
  con soporte usan **`nu9ve-transfer-receipt/2`** y llevan `project_id: null`
  sólo para esas unidades. Los demás siguen en `/1`; no se cambia la semántica
  de UUID de `/1`. Recovery actual admite ambas versiones, rechaza majors
  futuros y verifica que el flag coincida
  con el plan y restaura sus copias/miembros sin crear proyectos ficticios.
- Soporte nuevo conserva su árbol bajo `metadata/<familia>/support/...` con
  identificador estable de la unidad de ingreso, en vez de exigir un proyecto.
  No se renombran ni reorganizan copias históricas. Bundles Stalker conservan
  todos sus miembros/rutas originales y la propiedad de su manifest.
- `preview[].source_action` añade `retain`, `release_verified`,
  `release_clipper` o `camera_format` al documento firmado por digest. La UI
  muestra la decisión del planner; no replica reglas de limpieza por extensión.
  `planner_revision: ingest-flow/1` participa en el ID antes de generar preview:
  evita reutilizar el ID de un job anterior con un digest de revisión distinto.

## Pantallas y confirmación

Entrada integra fuente, asignación y contenido. Por defecto muestra media
principal; «Todos los archivos» y «Auxiliares» permiten inspeccionar el soporte
que sigue incluido en el respaldo. En compacto la cabecera se puede plegar.

Flujo integra opciones/destinos y plan, con una fila desplegable por bundle o
unidad. Preparar/recalcular no copia ni limpia. El plan se prepara al entrar
si hay datos válidos y se invalida al cambiar entradas, asignaciones o política.
Sólo el botón **Respaldar y liberar origen** / **Respaldar y conservar origen**
ejecuta el documento revisado original, sin reserializarlo desde Swift.

Elegir sólo respaldo queda expresado en el plan y aceptado por ese botón por
operación. Si se esperaba un SSD que falta, hay confirmación de excepción
inline y decisión pending/skipped; no se convierte en el default del perfil.
DJI ambiguo, conflictos y destinos desconocidos siguen bloqueando. Sony
conserva índices y termina en `camera_action_required` hasta el formato manual.

## Progreso aditivo

`progress` añade `global_total`, `global_done`, `global_fraction`,
`backup_budget`, `release_budget`, `cleanup_bytes`, `eta_seconds` y `eta_scope`.
Son trabajo lógico del plan, no bytes físicos únicos almacenados:

- Presupuesto de respaldo: bytes de fuente + verificación por cada destino
  requerido + relectura de origen cuando corresponde limpiar.
- Stalker reserva además el tamaño de los originales a retirar. La fracción
  no vuelve a cero al pasar a rehash, Clipper, snapshot o a otra sesión; el
  detalle mantiene los contadores reales de cada paso. Un original retenido
  no se representa como liberado.
- No se publica 100 % antes del resultado terminal del trabajo automático.
  Para Sony, 100 % de esa operación **no** significa tarjeta lista: el formato
  requerido sigue indicado por el estado y el resultado.
- ETA aproximada a partir de trabajo medido/tiempo transcurrido, tras al menos
  tres segundos. Con Stalker se etiqueta sólo como tiempo de respaldo; se
  oculta durante deps/rehash/retiro, cuya duración no se finge conocer.
- Se conserva el máximo registrado al reanudar. Se vuelven a comprobar los
  bytes requeridos; el porcentaje no autoriza ninguna limpieza.

## Barra y montaje

Las pestañas principales se posicionan en coordenadas de la ventana, compensando
el inicio real del toolbar tras los controles nativos. Discos y actividad no
mueven ese centro; reducen su detalle antes de invadir la navegación.

Actualización 2026-11-06: apertura automática **activada por defecto**, desactivable desde Ajustes → «Mostrar el importador rápido al conectar una tarjeta». Un `false` guardado se respeta; registrar el default no lo sobrescribe. La ventana compacta usa nivel flotante y no se oculta al desactivar la app; selección/completo usan nivel normal. Cerrar retira la ventana; volver a compacto recupera el nivel flotante. Entrar a fullscreen pasa al modo completo/nivel normal. No es una ventana modal ni impide usar otras apps.

Un montaje nuevo vuelve a habilitar su aviso por ruta. El importador se abre
con la preferencia activada y destinos registrados online aunque exista un
resultado anterior. No interrumpe una operación activa. Un borrador previo se
puede retomar desde **Retomar otro ingreso de esta sesión**: esos borradores
son locales a la sesión de la app, no una persistencia prometida tras salir.
Sin destinos online no hay popup ni lectura profunda. Notificación del sistema
y apertura del importador son independientes. Los eventos son pistas y el
motor siempre revalida identidad/contenido antes de aplicar.

## Pruebas y límites

Fixtures temporales: asignaciones/auxiliares, SHA y recuperación, progreso entre
archivos/fases/sesiones, fallos y replay. QA nativa usa RPC con identidades
fixture y simula avisos de montaje; no monta ni expulsa SD personales. Quedan
aceptación física de reconexión/popup, rendimiento con media grande y los
[gates de distribución](../release.md). No se añade daemon ni dependencia.
