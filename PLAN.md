# Plan de trabajo

Actualizado: 2026-11-07, America/Hermosillo. Éste es el backlog priorizado;
[STATUS](docs/STATUS.md) conserva la evidencia por entrega y
[CONTEXT](docs/CONTEXT.md) las decisiones. Las propuestas históricas no son otra lista de pendientes vigente.

## P0 · aceptar el flujo actual

| Orden | Trabajo | Criterio de cierre / referencia |
| --- | --- | --- |
| 1 | Continuar aceptación de los dos multicams; usuario ya está editando | Compacto flotante implementado/instalado el 6 noviembre (`88fa9b79a571`); preferencia popup local habilitada. Flotante aceptado por el usuario; detección/popup sigue incierto (posible tardanza/apertura manual). Menubar → insertar SD → Entrada → Flujo → revisión → copiar/verificar → limpieza Sony → formato/confirmación; descubrir sesiones terminadas y entregar a Clipper sin perder miembros. Usuario reportó Stalker correcto y Sony fallido al finalizar. Dos clips Sony ya entregados/verificados a ESSD por petición explícita; defaults/nombres/destinos instalados en `17acd51df25c`. [Recorrido](docs/sony-validation.md#aceptación-acompañada-sony-y-dos-multicams--2026-11-06). |
| 2 | Aclarar el aviso recurrente Sony y cambios externos | Prioridad de diagnóstico: Sony terminó failed con FileNotFoundError de C0001.MP4 pese a seis retiradas journalizadas; no reanudar a ciegas ni afirmar doble borrado. Observar desde la app los intermedios/proyectos Clipper eliminados y cualquier referencia residual, sin preparar planes por CLI ni cambiar catálogo. Identificar job, texto y estado real: clips retirados, índices conservados, formato pendiente o aviso histórico. Confirmar no vuelve a borrar. Si hay defecto, corregir mensaje/reconciliación con fixtures y sin eliminar guardas. |
| 3 | Aceptar el cierre editorial con Terra | Revisar selección definitiva y requisitos del proyecto, respaldar los verticales pendientes conservándolos en ESSD y comprobar la indicación Sin acción para raw/render ausentes. Con trabajo presente, el retiro exige rehash, deps/ACK y consentimiento. [Contrato](docs/contracts/project-closeout-v1.md). |
| 4 | Recuperación física y rendimiento | Tras asegurar material de prueba, desconexión/reconexión y reanudación controladas, popup/remount, ExFAT/HFS+, tamaños reales y contención con captura. No interrumpir deliberadamente la única copia de las grabaciones nuevas. |
| 5 | Cobertura de almacenamiento real | Analizar MEDIA502 y `coverage --no-hash` de ESSD contra MEDIA; informe orientativo antes de elegir carpeta/proyecto. Liberación exige hash actual, no coincidencia de tamaño. [Política](docs/features/replication-policy.md). |

Última build instalada registrada `face3d2b2657` (7 noviembre), con cierre editorial y comprobación Sony automática. Se conserva el estado de los trabajos históricos hasta comprobar cada SD.

La entrega del 7 noviembre deja el seguimiento Sony en Actividad cuando el
retiro ya está registrado o falta el formato. En la prueba física de un clip
el primer retiro se revirtió al reconectar. El retiro repetido persistió tras
desmontar y montar en el mismo lector. El usuario confirmó que el formato
en cámara funcionó. La comprobación al reconectar quedó automatizada en
la build instalada. Falta reconectar la misma SD para
aceptar el cierre en Actividad. La durabilidad del retiro en el flujo normal
queda como prueba física aparte.
[Evidencia Sony](docs/sony-validation.md#prueba-física-con-un-clip--2026-11-07).
El cierre agrupa
entregas, propone coincidencias exactas y sugiere revisar desde Inicio al
detectar el primer vertical publicado. Falta aceptación con un proyecto real,
incluida la selección por grupo. La notificación opcional por varios verticales
con más de un día queda por definir. [Evidencia](docs/STATUS.md).

Pendiente de producto solicitado el 6 noviembre: selector de todas las fuentes/capturas pendientes para un mismo proyecto, con planes por fuente y reglas claras; no está resuelto por las sugerencias de nombres.

## P1 · siguiente entrega de producto

El recorrido de arriba tiene prioridad. Después, elegir una entrega pequeña según uso;
no se ha decidido adelantar dron frente a carpetas vivas.

| Área | Pendiente real | Dueño / aceptación |
| --- | --- | --- |
| Carpetas vivas | Ingreso incremental sin renombrar, limpiar origen ni propagar borrados; música/samples, editables y adopción de carpetas antiguas | Planner/índice/catálogo; destino de música en MEDIA por decidir. [Colecciones](docs/features/product-flows.md#carpetas-vivas-música-y-samples). |
| Atlas / fotos de dron | Propuesta actualizada 6 noviembre, **A1–A10**, sin implementación nüün | Primero clasificación confirmada, pares por carpeta y layout original; después trabajo selectivo/handoff, respaldo de estado/exports/productos costosos y coordinación de uso. [Contrato propietario](docs/contracts/atlas-drone-v1.md). No ejecutar ODM ni limpiar al montar. |
| Fotos / Kyū | Consumidor de `camera_bundle` RAW+JPG y aceptación real | Kyū es dueño del importador; nüün mantiene unidades, selección y recibos. [Fotos](docs/features/product-flows.md#fotos-y-kyū). |
| Audio / cámaras | H6/SSD1, DJI dron frente a 360/acción, GoPro; registro persistente de modelo/categoría y reloj | Evidencia por dispositivo y confirmación; sin inferir por marca/carpeta residual. |
| Stalker | Casos reales recording/finalizing/recovery; nombre genérico y tamaño de micrófono reportados; sugerencia SD desde sesión reciente | Stalker conserva manifests/reloj/audio. Reconciliar filas antiguas con código/pruebas antes de reimplementar. [Pedidos](docs/contracts/README.md#pendientes-para-n-creator-utils). |
| Trabajo y estado humano | Aceptar entrega posterior desde archivo, restore sin SD, retención de snapshots, respaldo de editables y rehash periódico | Catálogo/library/recovery; preservar decisiones humanas y ubicaciones. |

**Clipper:** sources/handoff, reenlace integral y ACK ya adoptados en CLI/TUI.
Desde el 5 noviembre también dispone de `library retire-plan` / `retire` con desregistro.
Nüün conserva el editable y observa deps frescos; invocar automáticamente ese retiro desde nüün sería otra entrega, no un bloqueo por API inexistente. Studio y Final Cut fuera del cierre actual.

## P2 · configuración, fluidez y distribución

- Recetas editables y registro de cámaras: la pestaña Flujos actual es sólo lectura. Una receta genera parámetros del mismo planner; no sustituye digest, revisión ni verificación.
- Onboarding de discos/perfiles, diagnósticos redactados, salud del runtime y acciones claras sobre pendientes.
- Medir antes de optimizar: bibliotecas grandes, miniaturas, idle CPU/RSS/IO, accesibilidad, fullscreen/pantallas y contención. DiskArbitration, progreso empujado y agrupación de grilla siguen pendientes; no requieren reescritura de plataforma ni daemon.
- Revisar validación/autenticación de planes, ownership/leases y durabilidad/replay según la frontera actual; exclusión host-local ya existe, no es lease distribuido.
- Si se prepara distribución: snapshot final y suites vigentes, wheel fuera del checkout, versiones alineadas, runtime/licencias empaquetados, firma Developer ID/notarización/Gatekeeper. [Gates completos](docs/release.md). La instalación ad-hoc con Python externo sigue siendo desarrollo.

## P3 · expansión

MCP actual de sólo lectura. Mutaciones necesitan aprobación/cancelación y contrato común de plan/digest/progreso. Windows/Linux, catálogo multiusuario, nube y segunda réplica son entregas futuras. Descargas/stems pueden incorporarse como derivados; MIDI, iluminación, IoT y Raspberry Pi siguen exploratorios.

## Retomar en otra sesión

Leer CONTEXT → primera entrada de STATUS → fila de este PLAN → contrato/módulo dueño.
Para la prueba actual basta [el recorrido acompañado](docs/sony-validation.md#aceptación-acompañada-sony-y-dos-multicams--2026-11-06).
Registrar sólo resultados observados, job/build y siguiente paso; no heredar aprobación física de otra build o de fixtures. Historial de entregas en STATUS, no volver a copiarlo aquí.
Respetar `git status`, sin commits/restage ni mensajes a otras sesiones como efecto lateral.
[Formato de handoff](docs/development.md#cerrar-y-retomar-una-sesión).
