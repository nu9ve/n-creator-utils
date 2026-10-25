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

## Prioridad 3: límites entre repos y herramientas externas

El propietario confirmó que [nu9ve/clipper](https://github.com/nu9ve/clipper) reemplaza al clipper interno. El trabajo futuro de este repo se centra en ingreso, organización y archivo; el editor externo mantiene cortes, composición, subtítulos, IA y export. El análisis de rutas, referencias y escenarios está en [archivos y flujos](docs/file-workflows.md).

- Retirar el comando/módulo `clipper` histórico, sus opciones y dependencias de empaquetado en un cambio separado. Identificar trabajos antiguos que aún necesiten consulta; no inventar compatibilidad automática de su JSON con el proyecto nuevo.
- Incorporar reconocimiento de paquetes antes de cualquier movimiento: `.stalker`, bundles Insta360, proyectos Clipper y entregas no son carpetas arbitrarias de archivos clasificables por extensión.
- Definir transferencia/archivo de bundle completo, ubicaciones estables y registro de origen/destino para incorporar fuentes a Clipper después de organizarlas.
- Coordinar un contrato de relocalización con Clipper: sus referencias de sesión, voz, pistas y cola no quedan resueltas cambiando sólo la ruta del video. El organizador no debe modificar JSON editorial de forma independiente.
- Diseñar archivo de entregas que conserve master/variantes y assets de publicación elegidos con referencia al proyecto. No pasar finales/proyectos al `save` genérico.
- Adaptadores de descarga con resultados estructurados y contrato de múltiples archivos/playlists; decidir biblioteca de assets con el flujo real del usuario.
- Ciclo de vida de stems explícito: `split` actualmente renombra y mueve el audio local. Proteger fuentes compartidas y registrar procedencia de derivados antes de automatizar su entrega al editor.

Estas integraciones son candidatas, no funciones implementadas. Se elegirán después de describir los flujos comunes del usuario. No se continuará desarrollando aquí otro renderer/editor ni se migrarán funciones del editor externo de vuelta a este repo.

## Prioridad 4: integraciones y plataformas

- API Python con modelos de entrada/salida y excepciones estables; CLI como adaptador del mismo servicio.
- Reportes/eventos JSON con IDs de operación para gestores de tareas, servicios o interfaces futuras. No iniciar un API server antes de estabilizar estas operaciones.
- Matriz de Python/macOS/Linux y empaquetado reproducible; verificar wheel, herramientas externas y dependencias opcionales por ambiente. Evaluar Windows con pruebas propias.
- `inspect-sd`, reportes Markdown/CSV y expulsión opcional sólo tras éxito verificable.
- Inventario de usos manuales de hardware/Bash y evaluación de dependencias/procedencia antes de integrar o archivar código.

## Confirmaciones de producto pendientes

El sucesor de `clipper/` ya está confirmado: `/Users/nu9ve/code/ai/clipper`. Stalker es el productor de capturas asociado. Para las otras áreas, completar la tabla de [repos sucesores](docs/modules.md#duplicación-y-repos-sucesores) con equivalencias y usos reales. Queda describir los flujos habituales para concretar qué transferencias y política de archivo conviene automatizar.

El propietario debe revocar las credenciales que estuvieron incrustadas y provisionar nuevas mediante el ambiente. Retirarlas del código actual no las elimina del historial ni cambia su estado en los proveedores.
