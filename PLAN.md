# Plan de evolución

La meta es convertir la colección en una herramienta configurable, modular e integrable conservando sus capacidades. Este archivo separa cambios realizados de propuestas; el comportamiento actual se documenta en [README](README.md) e [importación](docs/importing.md).

## Base implementada en esta revisión

- Configuración local ignorada por Git, JSON seleccionable y variables de entorno; rutas personales basadas en el home real, con defaults de archivo preservados.
- Entry point instalable y uso directo por Python, ayuda por comando y diagnóstico JSON sin operaciones sobre media.
- Carga diferida de IA; el recorte local no necesita los SDKs cloud. Credenciales incrustadas retiradas del código de trabajo, sin reescribir historial.
- Root de archivo existente y rutas sin solapamiento como precondiciones; comparación de tamaño antes de borrar en los helpers de movimiento/copia. Esto no es verificación por checksum.
- Colisiones protegidas en segunda copia de audio, descargas y originales de stems; conservación de manifest original de un bundle.
- FFprobe estructurado y con errores explícitos; WAV minúsculo, MP3 y nombres de audio genéricos tolerados.
- Defaults del JSON del clipper, prioridad de config explícita, inputs independientes por clip, rangos fraccionarios, extensión mayúscula y propagación de fallos FFmpeg corregidos. Fuente configurable y fallback del sistema.
- URLs como argumentos completos; ruta final de yt-dlp reportada de forma explícita; split sin cambiar el cwd global, archivando el original sólo tras éxito.
- Documentación por área y suite de regresión con archivos temporales/media sintética.

Estos cambios son una base. No certifican servicios externos, hardware, compatibilidad Windows o todas las combinaciones de render.

## Prioridad 1: importación confiable y revisable

| Trabajo | Criterio de aceptación |
| --- | --- |
| Plan de importación + `save --dry-run` | La misma planificación alimenta simulación y ejecución; simulación no crea ni borra archivos. Incluye auxiliares que hoy se eliminan directamente. |
| Políticas explícitas de copy/move/delete | El usuario puede preservar tarjetas sin editar código; comportamiento histórico sigue disponible y claramente identificado. |
| Transferencia verificada y recuperación | Copia temporal, checksum origen/destino, publicación final y borrado sólo tras validación; fallo deja estado reconocible y recuperable. |
| Identidad/espacio de volúmenes y locks | No importar en una carpeta que suplanta un disco ausente; detectar destinos con poco espacio y procesos concurrentes. |
| Manifiesto incremental y reporte final | Estados por archivo, errores y acciones completadas, incluidos Sony y archivo genérico; el registro sobrevive a fallos intermedios. |
| Idempotencia/reintentos | Repetir una importación no duplica todo ni desasocia video/sidecar; recuperación documentada y probada. |

La nota antigua que proponía Sony copy-only no estaba implementada. Hoy se mueve media y se archiva soporte para liberar la SD. Una futura política debe preservar esa opción y ofrecer un modo de conservación de tarjeta; no cambiar silenciosamente el comportamiento.

## Prioridad 2: modularidad y configuración

| Trabajo | Criterio de aceptación |
| --- | --- |
| Extraer configuración por operación | Se puede ejecutar con dos perfiles de usuario en el mismo proceso sin tocar constantes globales. |
| Separar `manager/file.py` | Módulos de detección/perfiles, metadata, naming, planificación, transferencia, manifiesto y adaptadores externos con contratos explícitos. |
| Reglas/presets de cámara | Sony, Insta360, GoPro, Canon, Nikon, DJI y grabadoras tienen fixtures y responsabilidades delimitadas; etiquetas no se confunden con detectores. |
| Política de fecha y nombres | Locale/zona/EXIF explícitos; tags y caracteres válidos por plataforma. Sustituir corrección GoPro fija por ajuste configurable. |
| Enrutamiento configurable | Categorías, umbral de resolución, audio largo, detección de drone/pantalla y retención de auxiliares dejan de depender de substrings/fijos. |
| Sidecars adicionales | XMP y otros acompañantes conservan relación con el master; operación consistente ante fallos. |

## Prioridad 3: clipper y herramientas externas

- Esquema de clips completo y versionado: tipos, recursos existentes, colores, texto, rangos dentro de duración del video y claves desconocidas.
- Presets de salida, carpeta de resultados y política de colisión/intermedios, sin perder los encuadres disponibles.
- Pruebas reales de fuente vertical, horizontal, cuadrada, logo, fondo multimedia, ambos recursos y texto con caracteres especiales. Filtros con escape correcto y selección explícita de streams/audio.
- Separar la generación IA de la ejecución de un trabajo; JSON persistente, modelos/SDK actualizados y configurables, límites/reintentos y cache JSON en lugar de pickle.
- Adaptadores de descarga con versiones probadas y resultados estructurados; eliminar dependencia del texto de logs `scdl` y definir el contrato de múltiples archivos/playlists.
- Flags/modelos de Demucs configurables, ciclo de vida explícito del audio fuente y pruebas optativas de ML con datos pequeños.

## Prioridad 4: integraciones y plataformas

- API Python con modelos de entrada/salida y excepciones estables; CLI como adaptador del mismo servicio.
- Reportes/eventos JSON con IDs de operación para gestores de tareas, servicios o interfaces futuras. No iniciar un API server antes de estabilizar estas operaciones.
- Matriz de Python/macOS/Linux y empaquetado reproducible; verificar wheel, herramientas externas y dependencias opcionales por ambiente. Evaluar Windows con pruebas propias.
- `inspect-sd`, reportes Markdown/CSV y expulsión opcional sólo tras éxito verificable.
- Inventario de usos manuales de hardware/Bash y evaluación de dependencias/procedencia antes de integrar o archivar código.

## Confirmaciones de producto pendientes

Completar la tabla de [repos sucesores](docs/modules.md#duplicación-y-repos-sucesores) con enlaces, capacidades equivalentes, diferencias y usos reales. No hay base para declarar módulos reemplazados únicamente por antigüedad o por no ser importados desde el CLI.

El propietario debe revocar las credenciales que estuvieron incrustadas y provisionar nuevas mediante el ambiente. Retirarlas del código actual no las elimina del historial ni cambia su estado en los proveedores.
