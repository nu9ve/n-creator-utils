# Inventario y arquitectura

## Responsabilidad entre repos

`n-creator-utils` se ocupa de ingreso, organización, descargas, stems y archivo. [Stalker](https://github.com/nu9ve/stalker) produce capturas y [Clipper](https://github.com/nu9ve/clipper) es el editor vigente. El clipper interno de este repo quedó reemplazado; el diagrama siguiente documenta lo que todavía está conectado en el código, no una recomendación de mantener dos editores. Consulta [archivos y flujos entre repos](file-workflows.md).

## Flujo principal

```text
nu9ve.py (CLI, logging y códigos de salida)
  ├─ doctor → configuración + inventario de ejecutables/directorios
  ├─ save → manager.file.save_files → save_tag_files
  │    ├─ manager.video / manager.audio → metadata y duración
  │    ├─ Sony → sidecars + soporte
  │    └─ Insta360 → bundle + manifiesto
  ├─ download → manager.file.downloader → yt-dlp / scdl
  ├─ split → manager.file.splitter → descarga opcional → demucs
  └─ clip → clipper.clipper_cli → Clipper (histórico, reemplazado)
       ├─ clip_settings → JSON y metadata
       ├─ ffmpeg → cortes, filtros y renders
       └─ --openai → clientes de IA opcionales

configuration.py → defaults + JSON local + entorno
constants.py → compatibilidad de nombres y derivación de destinos
```

El núcleo no requiere paquetes Python externos. Los programas de media se invocan por `subprocess` con listas de argumentos. Sólo el modo IA carga sus SDKs. La configuración efectiva sigue siendo global por proceso a través de `constants.py`.

## Inventario por archivo/directorio

| Parte | Responsabilidad real | Conexión y estado |
| --- | --- | --- |
| `nu9ve.py` | Entrada CLI, ayuda, carga de comandos, diagnóstico y errores comunes | Activo. Expone `main(argv=None)` y entry point instalable. |
| `configuration.py` | Cargar/validar opciones y resolver rutas | Activo, sin crear archivos ni iniciar procesos. |
| `constants.py` | Exportar nombres históricos y destinos derivados | Activo; incluye algunos restos como `FINAL_DIRS` y tablas de calendario. |
| `manager/file.py` | Detección, clasificación, naming, transferencias, Sony/Insta360; descarga y stems | Activo. Concentra demasiadas responsabilidades; candidato principal a separar. |
| `manager/video.py` | Consultar FFprobe en JSON, duración y umbral de resolución | Activo; compartido por importación y clipper. |
| `manager/audio.py` | Duración WAV/MP3 e identificador de micrófono | Activo. `metatag_updater` es un stub vacío. |
| `clipper/clipper_cli.py` | Argumentos de `clip` | Histórico/reemplazado, aún conectado al CLI por compatibilidad. |
| `clipper/clipper.py` | Orquestación antigua de cortes/renders y experimento IA | Reemplazado por el editor externo; sin nuevas funciones previstas aquí. |
| `clipper/clip_settings.py` | Parseo antiguo de JSON, defaults, herencia y rangos | Histórico/reemplazado. No es el formato de proyecto del editor actual. |
| `clipper/ffmpeg.py` | Construcción antigua de filtros y ejecución FFmpeg | Histórico/reemplazado. El renderer actual se mantiene en el repo Clipper. |
| `clipper/config*.json` | Ejemplos de trabajos de recorte | Los archivos full incluyen claves antiguas `bottom_text`/`top_text` que no se leen. Usa la guía para campos actuales. |
| `clipper/openai_clipper.py` | Experimento standalone de transcripción y selección | No importado por el CLI; sólo ejecuta al llamarlo como script. Usa `./lower.mp3`, SDK legado y modelos fijados en código. |
| `hardware/novation.py` | Biblioteca MIDI copiada/adaptada para Launchpad, LaunchControlXL, LaunchKeyMini y Dicer | No importada por el flujo normal. Referencias de origen en su cabecera; depende de `rtmidi` y `novation_launchpad.charset`. |
| `hardware/control.py` | Borrador de `hardware_control()` | Su función no implementa un flujo; importaba el driver, pero el comando principal nunca estuvo conectado. |
| `hardware/test.py` | Instanciar `LaunchControlXL` y mostrar el objeto | Prueba manual de hardware, no una prueba automatizada. |
| `utils/youtube.py` | Viejo descargador con `youtube-dl` y parsing manual | No usado por el CLI. Le faltan imports y tiene sustituciones de un título concreto. |
| `utils/spleeter.py` | Experimentos de stems con Spleeter | No usado. Imports y `Separator` incompletos; rutas personales. El flujo actual usa Demucs. |
| `playground/ideas.py` | Variantes de las funciones de `utils` | Experimento sin integrar; incluye rutas/títulos fijos y piezas incompletas. |
| `playground/erikiado` | Colección Bash histórica de imágenes, GoPro, audio y tareas S3 | No invocada por Python; depende de herramientas externas, perfiles/rutas personales y operaciones con efectos reales. |
| `playground/gopro-utils` | Variante Bash más pequeña del flujo GoPro | No invocada por Python. Comparte lógica con el script anterior y el importador actual. |
| `social/` | Carpeta vacía en este checkout | Sin implementación ni API; Git no versiona directorios vacíos por sí solos. |
| `tests/` | Regresión de configuración, CLI, importación, clipper e integraciones simuladas/locales | Sin cuentas ni dispositivos; ver [desarrollo](development.md). |

“No conectado” significa que no se llama desde la ruta principal del repo. No prueba que nadie ejecute el script por separado. No se borraron estas partes.

## Duplicación y repos sucesores

Duplicaciones observables aquí:

- `utils/youtube.py`, `utils/spleeter.py` y `playground/ideas.py` comparten parsing manual de `youtube-dl`, nombres concretos y lógica de stems. `manager/file.py` contiene los adaptadores actuales.
- Los dos scripts Bash de `playground/` comparten operaciones GoPro e imágenes; el flujo Python ya organiza media, pero no equivale a todas sus funciones (por ejemplo S3/optimización).
- `clipper/openai_clipper.py` anticipa métodos de IA de `Clipper`; el experimento no es una implementación independiente validada.
- La consulta de video ahora usa un adaptador común `manager.video.probe_video`, aunque aún hay consultas específicas de fecha y audio.

La sustitución del clipper local está confirmada por el propietario y por la revisión del editor externo. Para el resto aún hay que registrar equivalencias antes de retirar código:

| Capacidad | Repo sucesor | Comando/API equivalente | Diferencias, usuarios y decisión |
| --- | --- | --- | --- |
| Clipper local/IA | [nu9ve/clipper](https://github.com/nu9ve/clipper), checkout `/Users/nu9ve/code/ai/clipper` | CLI `clipper`, TUI y ClipperStudio; proyecto `clipper-project.json` | Sucesor confirmado. Retirar el módulo antiguo como cambio separado; no duplicar nuevas funciones de edición aquí. Su JSON antiguo no tiene migración automática. |
| Importador de cámaras | Por confirmar | Por confirmar | Comparar Sony, Insta360, fechas, borrados y segunda copia de audio. |
| Descargas y stems | Por confirmar | Por confirmar | Comparar destinos, proveedores, normalización y modelos. |
| MIDI, scripts Bash y tareas S3 | Por confirmar | Por confirmar | Confirmar usos manuales y herramientas que no cubre el CLI actual. |

Stalker, en `/Users/nu9ve/code/ai/stalker`, es el productor de capturas `.stalker`, no otro editor. La revisión de sus archivos y del contrato compartido está en [file-workflows.md](file-workflows.md). El retiro del código antiguo y las integraciones propuestas todavía no se han ejecutado.

## Integrar con otros sistemas hoy

Para automatizar, el límite más sencillo es ejecutar `nu9ve` con argumentos separados, configuración por entorno y comprobar el código de salida. `doctor` tiene salida JSON; el resto usa logs en stderr y las salidas propias de procesos externos. No hay un esquema estable de eventos ni reporte JSON de importación/render.

También hay funciones Python utilizables, con límites explícitos:

```python
import os
os.environ['NU9VE_CONFIG'] = '/ruta/config.json'  # antes de importar módulos

from configuration import load_settings
settings = load_settings()  # no escribe ni procesa media

from manager.file import resolve_src_name, save_tag_files
# Tiene efectos sobre el filesystem y elimina fuentes:
# source = '/ruta/copia-tarjeta'
# profile = resolve_src_name(source, 'AUTO')
# if profile:
#     save_tag_files(source, profile, 'mx', 'hermosillo')
```

`save_tag_files` recibe un **perfil ya resuelto**: para detección llama primero a `resolve_src_name(ruta, 'AUTO')` y comprueba que retorne un nombre; pasarle `AUTO` directamente sólo lo trata como etiqueta. Las funciones de CLI `save_files`, `downloader`, `splitter` y `clipper` aceptan listas de argumentos opcionales; las funciones de trabajo lanzan excepciones en errores. No existe una API de librería versionada ni garantiza uso concurrente.

`download_file` devuelve la ruta descargada; `move_to_file_type_dir` y `splitter` devuelven el destino. `save_tag_files` y `Clipper.clip_video` operan por efectos y todavía no devuelven un resultado estructurado. Para una integración robusta falta separar planificación, ejecución, adaptadores externos y reporte.
