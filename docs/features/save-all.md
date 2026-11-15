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

Carpetas vivas de producción y liberación de colecciones musicales se operan desde sus runbooks. Sigue pendiente incorporar sus revisiones y los planes explícitos de raws a este panel. [Música](music-flows.md) · [Cierre editorial](../contracts/project-closeout-v1.md)

## Recorrido de la primera entrega

Inicio completo usa el espacio libre y estado de los destinos del snapshot. Compacto y selección ofrecen el mismo acceso en la barra. Abrir la selección usa datos ya observados y no escanea tarjetas. Después de elegir una fila, el planeador existente abre esa fuente. «Agregar al lote» conserva el documento original revisado. Se puede volver a por otra fuente y mantener los planes preparados durante la sesión.

«Calcular impacto» consulta Rust para un conjunto de planes de ingreso de SD, Stalker o finales. El resumen muestra el espacio disponible y el estimado posterior por destino. Reserva las copias antes de contar cualquier retiro. Considera el retiro de finales de un disco registrado como potencial. Un retiro condicionado de Stalker o el formato en cámara nunca financian otra copia. La estimación puede sobrecontar archivos que el motor termine adoptando como idénticos.

## Contrato Rust

- `save_all_review {plans: [documento_json_original, ...]}` comprueba digest y registro local de cada plan, identidades de fuentes y destinos, cambios del registro DJI, solapamiento y destinos repetidos. Devuelve IDs en orden, digest del lote y capacidad estimada por volumen. El tamaño total sigue el límite de 16 MiB del RPC one-shot.
- `save_all_apply {plans, digest}` repite la revisión de destinos y ejecuta en serie los planes exactamente en el orden aprobado, con el mismo transfer que el ingreso individual. Cada hijo revalida los archivos, mantiene su journal y conserva su seguimiento de cámara/Clipper. El resultado indica qué hijos se intentaron y sus estados. Si falla o se pausa un hijo, los siguientes quedan por iniciar. La reanudación de un hijo usa Actividad.
- El lote de esta entrega vive en la sesión de la app. Los hijos empezados son durables en el catálogo. Queda guardar la selección y los planes por iniciar, mostrar el avance de cada hijo y autorizar en el lote operaciones con otros contratos (misiones, copias de edición, cierres, liberaciones y carpetas vivas). Incorporar misiones desde Inicio invalida los planes de tarjeta afectados preparados en la sesión.
