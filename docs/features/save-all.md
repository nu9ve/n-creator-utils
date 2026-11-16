# Poner al día

## Qué reúne Inicio

| Pendiente | Revisión disponible | Efecto |
| --- | --- | --- |
| SD con video, fotos o audio | Una fuente por tarjeta. El proyecto general sirve de fallback. Ingreso permite asignar proyecto, perfil, país, cámara y copia de edición por unidad | Copia verificada a los destinos elegidos. La limpieza, si está en el plan, respeta la receta de la tarjeta |
| Stalker | Una fuente por biblioteca pendiente. Ingreso muestra bundles completos | Respalda sesiones terminadas. El retiro local sigue los ajustes, las dependencias y el ACK de Clipper |
| Exports de misiones | Revisión existente de Descargas | Incorpora y respalda el registro. Luego se replantean las tarjetas que lo usan. A14 sigue bloqueando la limpieza con fotos pendientes |
| Finales publicados | Un grupo por carpeta y perfil. El plan permite conservar la copia de edición | Respalda entregas y, si está autorizado, libera la copia de trabajo |
| Copias de edición pendientes | Un grupo por proyecto. Abre Biblioteca con los assets seleccionados | Prepara trabajo desde el archivo |
| Cierres y originales Stalker ya archivados | Revisión editorial o de liberación existente | Relee el respaldo y comprueba dependencias antes del retiro |
| Jobs pausados, fallidos o con acción de cámara | Actividad | Reanuda el job existente cuando corresponde. La acción física de cámara conserva su estado propio |

Las carpetas vivas ya registradas se pueden revisar desde el panel. El primer registro de una carpeta y la liberación de colecciones musicales se operan desde sus runbooks. El retiro de raws tiene revisión propia. [Música](music-flows.md) · [Cierre editorial](../contracts/project-closeout-v1.md)

## Recorrido de la primera entrega

Inicio completo usa el espacio libre y estado de los destinos del snapshot. Compacto y selección ofrecen el mismo acceso en la barra. Abrir la selección usa datos ya observados y no escanea tarjetas. Después de elegir una fila, el planeador existente abre esa fuente. «Agregar al lote» conserva el documento original revisado. Se puede volver a por otra fuente y mantener los planes preparados entre sesiones.

«Calcular impacto» consulta Rust para un conjunto de planes de ingreso de SD, Stalker o finales. El resumen muestra el espacio disponible y el estimado posterior por destino. Reserva las copias antes de contar cualquier retiro. Considera el retiro de finales de un disco registrado como potencial. Un retiro condicionado de Stalker o el formato en cámara nunca financian otra copia. La estimación puede sobrecontar archivos que el motor termine adoptando como idénticos.

## Contrato Rust

- `save_all_review {plans: [documento_json_original, ...]}` comprueba digest y registro local de cada plan, identidades de fuentes y destinos, cambios del registro DJI, solapamiento y destinos repetidos. Devuelve IDs en orden, digest del lote y capacidad estimada por volumen. El tamaño total sigue el límite de 16 MiB del RPC one-shot.
- `save_all_apply {plans, digest}` repite la revisión de destinos y ejecuta en serie los planes exactamente en el orden aprobado, con el mismo transfer que el ingreso individual. Cada hijo revalida los archivos, mantiene su journal y conserva su seguimiento de cámara/Clipper. El resultado indica qué hijos se intentaron y sus estados. Si falla o se pausa un hijo, los siguientes quedan por iniciar. La reanudación de un hijo usa Actividad.
- Los hijos iniciados conservan su estado en el catálogo. La selección y los hijos pendientes se guardan en el borrador local. Misiones, cierres y liberaciones conservan su revisión específica.

## Reapertura y copias sin retiro · 2026-11-15

Rust guarda la selección y los planes pendientes en un único `save-all.json` local con reemplazo atómico. Está fuera de SQLite para que los snapshots de cada copia no repliquen el borrador. Al abrir nüün se recupera el lote. Los ingresos ya iniciados se retoman por su job en Actividad. Una carpeta viva o entrega de trabajo interrumpida queda en el lote y puede volver a revisarse; el motor reutiliza su copia verificada. Tras aplicar, el borrador descarta las entradas atendidas y guarda un solo resumen acotado. Los recibos y journals que todavía sirven para recuperar o reanudar quedan en sus módulos propietarios.

Las copias de edición ahora se pueden preparar por proyecto y agregar al lote. Rust congela el destino, las rutas de trabajo y los recibos de archivo, y vuelve a compararlos bajo el lock de escritura antes de copiar. También se pueden agregar respaldos incrementales de carpetas vivas ya registradas. Cuando la carpeta está al día no se crea un plan nuevo. Al completar un plan de carpeta viva creado aquí, se conservan el índice, el manifiesto y un resumen en vez de la lista de trabajo y sus intenciones de versión.

En Proyectos, «Ver pendientes» filtra por proyecto usando las asignaciones que ya existen. Una SD sin inspeccionar sigue requiriendo selección e inspección explícitas. «Dependencias de Clipper» consulta por ID editorial bajo demanda. Separa cola, fuentes en uso, rutas ausentes en discos conectados y retiros registrados. El plan de liberación sigue siendo quien comprueba respaldo y dependencias para autorizar un retiro.

El export DJI aparece antes de las tarjetas. La selección de pendientes permite incorporarlo dentro de «Poner al día» con una decisión explícita. «Recalcular DJI» vuelve a preparar el plan guardado con sus parámetros originales y muestra el cambio de pendientes y ejemplos de rutas. «Usar el nuevo plan» reemplaza sólo el documento que se comparó. Después se vuelve a calcular el impacto del lote. Los planes anteriores que aún no guardaron los parámetros se revisan desde Ingreso. Si cambia el registro fuera de la app, Rust exige una nueva revisión antes de iniciar el job. Los jobs ya empezados conservan su plan y su journal.
