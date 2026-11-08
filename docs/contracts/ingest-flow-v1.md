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

## Copias de trabajo y nombres · actualización 2026-11-06

Ingresos nuevos arrancan con **Todo el lote** para trabajo; elegir Ninguno o una selección es explícito por ingreso, sin heredarlo silenciosamente a otra tarjeta/fuente. Restaurar un borrador conserva su elección. Finales mantiene su política propia. Sin SSD requerido online se bloquea hasta conectarlo o confirmar archive-only con pending/skipped.

Flujo muestra DestinationsStep siempre visible, sin disclosure que oculte la política. El proyecto general, la asignación por unidad y el editor masivo muestran el nombre normalizado antes de ejecutar mediante `project_preview {name} → {name}` read-only, que reutiliza `common.slug`; no hay otra gramática Swift. No renombra proyectos anteriores ni transforma texto mientras se escribe.

Las sugerencias de Ingreso incluyen proyectos de planes revisados en esta sesión, borradores y capturas pendientes además de recientes. Se aplican por elección, no asignan ni sincronizan automáticamente cámara/Stalker. **Ingreso único de todas las fuentes pendientes sigue sin implementar**: cámara y bibliotecas Stalker usan planes por fuente/raíz; elegir varias no autoriza un nuevo cleanup global.

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

## Seguimiento Sony · 2026-11-07

El formato pendiente de Sony se muestra en Resultado y Actividad. Sale
de los avisos de Inicio y del contador de atención. Un fallo de archivo
ausente al finalizar el retiro también queda como seguimiento en Actividad
si todos los miembros removibles están journalizados como retirados, los
índices quedan conservados en el journal y existen los recibos requeridos
con SHA coincidente. Esta clasificación no cambia el estado del job ni
acredita formato o integridad actual del respaldo. Los fallos con evidencia
incompleta siguen requiriendo atención.

Al reconectar, nüün comprueba en segundo plano los trabajos Sony pendientes
de esa identidad de volumen. Consulta sólo las rutas registradas. Cuando
una ruta y tamaño se reutilizan, compara el SHA-256 con el miembro del
recibo. Si los originales están ausentes y un índice Sony existente
cambió de tamaño o fecha de modificación, registra `sony_card_checked`
y cierra el trabajo. La observación acredita que la SD se comprobó tras
reconectar. El formato sigue a cargo de la cámara.

El seguimiento sólo actualiza el catálogo y no interrumpe un ingreso.
Si reaparecen bytes originales, Actividad los señala y mantiene el trabajo
pendiente. Una identidad distinta, índices iguales o lectura inconclusa
conservan el estado. El botón de confirmación se retira de la app. El CLI
`confirm-cleanup` sigue disponible con las mismas comprobaciones para Sony.

La lectura final de Sony registra la fase `checking_card`. Si un archivo
desaparece durante la lectura, se valida la identidad de la tarjeta y se
reintenta una vez la lectura. Una segunda falla se registra y no se repite
ningún borrado dentro de ese reintento. El incidente físico de noviembre
sigue pendiente de reproducir con una grabación descartable.

### Etapas observadas del retiro Sony · 2026-11-07

El motor da a cada trabajo Sony una etapa (`sony_stage` en `job` y `jobs`, de
`transfer.sony_stage`). Sale sólo de observaciones; nunca pide confirmar:

| Etapa | Qué se observó | Actividad |
| --- | --- | --- |
| `backup_verified` | Copias verificadas; aún sin retiro | Respaldo verificado |
| `removal_observed` | Este ingreso retiró los clips verificados y no los vio en su lectura final | Retiro observado · formatea y vuelve a conectar |
| `removal_recorded` | Retiros journalizados, pero la lectura final falló (`failed`) | Retiro de clips registrado · falta comprobar la SD |
| `removal_durable` | Al volver a montar la misma tarjeta, los clips siguen ausentes y los índices sin cambios (`waiting_camera`) | Retiro comprobado al reconectar · falta formatear |
| `formatted` | Clips ausentes y un índice Sony reescrito (`ready`, trabajo `completed`) | Formato observado |
| `originals_reappeared` | Un clip respaldado volvió a la tarjeta con el mismo SHA-256 | Aviso con los archivos y **Volver a revisar la SD…** |

Al reaparecer originales se guardan sus rutas (`sony-card-reappeared:<job>`) y el
evento `sony_card_checked` las incluye. El trabajo sigue `camera_action_required`
y cuenta como aviso. **Volver a revisar la SD…** sólo abre Ingreso con esa
tarjeta: un plan nuevo se revisa y nada se retira hasta aplicarlo. Sin la
tarjeta conectada, el botón queda desactivado.

**Diagnóstico de `060b585d` (Baches, 6 nov):** lectura del catálogo en modo de
sólo lectura. Seis miembros removibles (dos MP4, dos XML y dos miniaturas,
7,027,220,534 bytes) constan `cleaned=1`; los nueve índices, `cleaned=0`. El
progreso quedó en fase `cleaning` con 99.9 % y el fallo llegó un segundo
después de la última escritura de progreso. El orden del motor de esa build
deja una sola operación tras el último retiro: la lectura final de la tarjeta.
Por eso el `FileNotFoundError` de `C0001.MP4`, retirado minutos antes, encaja
con una entrada de directorio todavía listada que ya no existía al leerla, el
caso que la fase `checking_card` reintenta desde `a439bb378645`. Es coherente
con la reaparición observada el 7 nov tras la interrupción de FSKit, pero no
la prueba: la tarjeta ya se formateó y reutilizó. Los recibos en MEDIA502 y la
entrega posterior a ESSD constan verificados. El trabajo sigue `failed` y la
comprobación al reconectar no lo incluía. Decisión del usuario (7 nov): la
comprobación al reconectar observa también los trabajos `failed` con
`removal_recorded`. Con la tarjeta formateada (clips ausentes y un índice
reescrito) los cierra como `completed` con `sony_card_checked`, sin
reanudarlos ni tocar la tarjeta. Un fallo sin todos los retiros journalizados
queda fuera.

## Pruebas y límites

Fixtures temporales: asignaciones/auxiliares, SHA y recuperación, progreso entre
archivos/fases/sesiones, fallos y replay. QA nativa usa RPC con identidades
fixture y simula avisos de montaje; no monta ni expulsa SD personales. Quedan
aceptación física de reconexión/popup, rendimiento con media grande y los
[gates de distribución](../release.md). No se añade daemon ni dependencia.
