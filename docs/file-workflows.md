# Archivos y flujos entre n-creator-utils, Stalker y Clipper

Revisión: 24 de octubre de 2026. Alcance: almacenamiento, identidad, referencias, movimientos, entregas y puntos de integración. No es una auditoría de efectos, calidad de render o modelos de IA.

Se revisaron los checkouts de Clipper (`7075eb9`) y Stalker (`46161c7`), sus contratos y el código de lectura/escritura de archivos. Los cambios de esta revisión son documentales: no se movieron grabaciones, proyectos o finales, ni se configuraron automatizaciones.

## Qué le corresponde a cada repo

| Repo | Responsabilidad | Qué conserva como autoridad |
| --- | --- | --- |
| **n-creator-utils** | Ingresar, organizar, respaldar y archivar media; descargar material y separar stems | Archivos originales en sus destinos, sidecars Sony y bundles Insta360. Hoy no hay un catálogo general que conecte todas las operaciones. |
| **[Stalker](https://github.com/nu9ve/stalker)** | Grabar pantalla y señales asociadas | La **carpeta `.stalker` completa**, con su manifest, pistas, voz derivada y eventos opcionales. |
| **[Clipper](https://github.com/nu9ve/clipper)** | Editar, sincronizar, componer y producir entregas | `clipper-project.json`, artefactos editoriales y las fuentes externas a las que apunta. El proyecto por sí solo no es un backup de esos originales. |

El propietario confirmó que el editor de `/Users/nu9ve/code/ai/clipper` reemplaza al `clipper/` antiguo de este repo. No hay que mejorar dos editores en paralelo. El código antiguo todavía está presente y conectado a `nu9ve clip`; retirarlo es un cambio pendiente separado. Su JSON de rangos no es el contrato del editor nuevo.

Stalker y Clipper ya tienen un intercambio implementado. Lo que falta aquí es integrar **ingreso, organización y archivo** alrededor de ese intercambio, conservando sus archivos y relaciones.

## Ubicaciones y configuración actuales

| Datos | Ubicación actual o default | Cómo se cambia / límite |
| --- | --- | --- |
| Capturas Stalker | `~/Downloads/Stalker_FECHA_HORA.stalker/` | Configuración de Stalker → carpeta de destino. El cambio afecta nuevas sesiones, no relocaliza automáticamente las anteriores. |
| Presets de Stalker | `~/Library/Application Support/Stalker/presets.json` | Guarda `outputDirectory` global y presets. No es una carpeta de media. |
| Biblioteca de proyectos Clipper CLI/TUI | `~/Downloads/clipper-projects/` | `CLIPPER_PROJECTS_ROOT`. Hay herramientas de movimiento/migración de proyectos; cambiar la variable por sí solo cambia dónde busca/crea. |
| Finales Clipper CLI/TUI | `~/Downloads/` | `CLIPPER_FINAL_OUTPUTS_ROOT`. No mueve finales ya publicados. |
| Proyecto Clipper | `<biblioteca>/<slug>/clipper-project.json`, `analysis/`, `transcripts/`, `reports/`, `logs/`, `exports/` | Edición/metadata y derivados por proyecto. Las fuentes agregadas normalmente siguen fuera. |
| Cola de variantes Clipper | `<biblioteca>/.export-queue/` | Guarda snapshots, logs e intermedios. Los snapshots siguen dependiendo de media externa. |
| Caché compartida Clipper | `~/.cache/clipper/` | `CLIPPER_CACHE_ROOT`; incluye derivados de media, renders y otras cachés. No confundir con originales o archivo permanente. |
| Mezcla derivada de Stalker en Clipper | `<proyecto>/cache/stalker/*.wav` | Mezcla PCM reutilizable. No es una nueva grabación original. Su política de limpieza específica está pendiente. |
| Temporales de render Clipper | Según operación; los nativos pueden usar la carpeta de salida | `CLIPPER_RENDER_TEMP_ROOT` en rutas que lo soportan. Un temporal no es un final publicado. |
| Archivo principal de n-creator-utils | `/Volumes/MEDIA502/001_raw` por default | `root_dir` / `NU9VE_ROOT_DIR`; organiza por categoría y fecha. |
| Segunda copia de audio | `/Volumes/SSD1` por default | `root_recordings_dir` / `NU9VE_ROOT_RECORDINGS_DIR`. |
| Descargas y stems de n-creator-utils | `audio_songs_dir`, `audio_samples_dir`, `tmp_video_dir`, `audio_splits_dir` | [Referencia de configuración](configuration.md). No hay registro automático en Clipper. |

Se leyó únicamente el campo de destino del preset local de Stalker: está configurado en `/Users/nu9ve/Downloads`. En el shell de revisión no estaban definidas las variables de raíz de Clipper de esta tabla; esto **no prueba** el entorno de un TUI ya abierto ni el workspace de ClipperStudio. Studio tiene configuración/bookmarks propios y no tiene toda la integración Stalker expuesta en su interfaz.

Downloads actualmente puede contener originales de captura, proyectos y entregas terminadas. Una carpeta allí no es desechable sólo por estar en Downloads.

## Qué archivos forman una sesión Stalker

```text
Stalker_FECHA_HORA.stalker/
  manifest.json
  screen.mov
  screen-2.mov                  # pantallas adicionales, si existen
  mic.wav                      # o mic.caf para una voz derivada grande
  events.ndjson                # opcional
  highlights.json              # opcional
  sources/
    system.mov                 # audio de computadora independiente
    mic.mov                    # micrófono original sin procesar
    mic-2.mov                  # segmentos adicionales si cambia formato/dispositivo
```

Los nombres internos son parte del contrato, no nombres cosméticos que el organizador pueda normalizar. El manifest contiene `sessionID` y rutas relativas; Clipper guarda además la raíz y rutas absolutas al importar.

- `screen.mov` tiene video y puede incorporar audio de computadora. `sources/system.mov` conserva esa misma señal por separado; no son dos audios que deban sumarse.
- `mic.wav`/`mic.caf` es una voz derivada alineada al reloj de sesión. `sources/mic*.mov` conserva el audio crudo; no es basura de caché.
- Los eventos y highlights no son videos, pero pertenecen a la captura y pueden alimentar la edición. Su ausencia puede ser intencional porque son opcionales.
- La sesión se escribe en su destino durante la grabación; no hay un MP4 final suelto que sustituya toda la carpeta. Al detener se terminan writers y se generan derivados.
- El nombre de la carpeta usa fecha/hora local; las colisiones reciben `_2`, `_3`, etc. La identidad de la sesión es el UUID, no el nombre de carpeta.
- `completed` es condición de importación normal. Sesiones incompletas se conservan para revisión/recuperación, no deben tratarse como terminadas por detectar sólo `screen.mov`.

## Qué hace Clipper al importar

El contrato productor es [`stalker-clipper/1`](https://github.com/nu9ve/stalker/blob/46161c7/docs/clipper-contract.md), con revisión compatible 1.1 y manifest de captura v3. La guía operativa del consumidor es [Stalker en CLI/TUI](https://github.com/nu9ve/clipper/blob/7075eb9/docs/stalker-integration.md). Son distintos del contrato JSON de servicios entre ClipperStudio y su backend.

El comando existente `clipper stalker import PROYECTO CAPTURA.stalker`:

1. Lee y valida manifest, estado, nombres y archivos dentro de la carpeta.
2. Registra la sesión en `capture_sessions` del proyecto, con UUID, raíz, digest del manifest, pistas y voz asociada.
3. Agrega las pantallas como fuentes por **referencia**. No copia el bundle a la carpeta del proyecto ni encola cada WAV/JSON como un clip independiente.
4. Conserva la asociación de voz para sincronización y audio de computadora para mezcla. La selección depende del modo tutorial/cámara externa.

Reimportar la misma raíz/manifest no duplica las fuentes. Si cambió el manifest o la raíz de una sesión conocida, se rechaza la sustitución silenciosa. Cambiar de carpeta puede requerir relink aunque los archivos tengan los mismos bytes.

El reloj v3 es común para pistas y eventos; al sincronizar con una cámara se aplica el placement correspondiente. Al archivar no hay que renombrar pistas, eliminar segmentos ni reconstruir su relación usando fechas del filesystem. El `start_time` de un stream puede ser distinto de cero aunque comparta reloj.

**Resultado práctico:** guarda el bundle en una ubicación estable antes de incorporarlo a una edición que quieras conservar. Llevar sólo el proyecto Clipper a otro disco no lleva sus originales.

## Copiar, mover, renombrar y limpiar: diferencias que importan

| Operación | Comportamiento actual | Consecuencia para el flujo |
| --- | --- | --- |
| `nu9ve save` sobre media de cámara | Copia a destinos normalizados y elimina la fuente; mueve sidecars/soporte según perfil | Conviene hacerlo antes de importar esas fuentes a Clipper. Después las rutas/firma de la edición pueden quedar obsoletas. |
| `nu9ve save` sobre `.stalker` o proyecto Clipper | No hay adaptador: recorre archivos sueltos, puede renombrar MOV/WAV y dejar JSON/eventos atrás; un MOV sólo-audio tampoco sigue el supuesto de video genérico | **No es un flujo compatible.** Necesitamos manejo de paquete completo, no pasarle estas carpetas al clasificador actual. |
| Agregar media/importar Stalker en Clipper | Registra referencias a archivos existentes | Importar no equivale a respaldar ni consolidar media. |
| Mover el proyecto Clipper | Existen helpers para mover carpeta de proyecto y reescribir algunas referencias internas | No relocalizan automáticamente originales externos. No asumir portabilidad completa de sesiones Stalker o cola. |
| Relink genérico de Clipper | Reasigna rutas de items; los registros `capture_sessions` incluyen además raíz, voz y pistas como strings | Falta un relink completo de bundle. Cambiar sólo `item.path` no resuelve todas sus dependencias. |
| Encolar un export | Persiste snapshot editorial, algunos assets/transcripts y firmas de fuentes | No copia toda la media. La cola todavía necesita que los originales sigan disponibles en sus rutas. |
| Publicar un final desde la cola | Mantiene el master en el snapshot y copia otro archivo al destino de finales; usa creación exclusiva | Puede haber dos copias legítimas. Quitar la entrada de cola no borra sus renders retenidos. |
| `Storage / Cleanup` de Clipper | Revisión de categorías/propiedad; protege fuentes, metadata editorial y assets referenciados; finales requieren selección explícita | Es limpieza de artefactos gestionados, no un archivador global de Downloads. Borrar derivados puede perder reanudación aunque no elimine edición. |
| `nu9ve split` sobre un audio local | Normaliza su nombre y, tras éxito, mueve el original a `audio_splits_dir/splitted/` | Si Clipper ya lo referencia, puede romper el vínculo. Para un asset compartido, preparar una copia de trabajo antes. |
| `nu9ve save` sobre un final exportado | Lo trata como media genérica y puede cambiar nombre/ruta | No preserva automáticamente su asociación con thumbnails, copy, subtítulos, reportes ni proyecto. Una entrega completa necesita otro contrato. |

La revisión de relink se basa en `remap_project_media_paths`, `relink_project_media_path` y `_rewrite_project_paths`: las dos primeras actualizan items; la última no reasigna `capture_sessions` al objeto resultante y su reescritura tipada tampoco convierte esos strings en rutas. No se ejecutó un traslado de proyectos personales para verificarlo.

## Flujos que ya pueden conectarse manualmente

Son escenarios candidatos para contrastar con tus hábitos, no una afirmación de que ya trabajas exactamente así. No hay automatización entre los tres repos.

### Cámara → archivo estable → edición

1. Importar una tarjeta con `n-creator-utils` usando sus reglas de cámara.
2. Verificar el resultado y seleccionar en Clipper las rutas definitivas del archivo.
3. Mantener esos originales disponibles mientras existan edición, cola o variantes que los necesiten.
4. Exportar desde el editor; tratar la entrega como salida derivada, no como otro original de tarjeta.

Esto evita organizar/renombrar una fuente después de que la edición ya depende de ella. En Insta360 sigue siendo importante conservar el bundle original; las conversiones que necesite el editor serían derivados aparte.

### Stalker + cámara → proyecto Clipper

1. Elegir un destino de captura estable en Stalker, o copiar la carpeta `.stalker` completa a ese destino antes de importar.
2. Importar cámara y bundle en el mismo proyecto Clipper, usando para la cámara su ubicación definitiva.
3. Conservar toda la carpeta `.stalker`, incluidas pistas de respaldo y eventos. No registrar sólo `screen.mov` si se quiere la integración de sesión.
4. Mantener las referencias disponibles durante la edición y los exports pendientes.

La oportunidad para este repo es organizar/verificar esos bundles y registrar su entrega a Clipper, sin asumir las funciones de sincronización o edición del editor.

### Pantalla/tutorial sin cámara

Stalker produce el bundle y Clipper ya tiene un flujo tutorial con voz derivada y pantalla. `n-creator-utils` puede aportar archivo y retención de la sesión, no necesita recortar ni mezclarla aquí. La copia de archivo debe conservar el bundle, no repartir audio/video por categorías.

### Descarga o stems → asset de edición

Las descargas de este repo pueden entregar un archivo desde una biblioteca estable para agregarlo manualmente al editor. Para stems, primero trabajar sobre una copia si el original es compartido, conservar qué asset produjo cada derivado y decidir cuál se importará. Hoy no hay registro de procedencia común ni actualización automática del proyecto Clipper.

### Export → entrega → archivo

Clipper genera master, variantes y thumbnails/copy según lo solicitado. Un flujo de archivo tendría que conservar juntos los archivos elegidos y un vínculo al proyecto/variante, distinguiendo:

- original de captura;
- proyecto y artefactos editoriales para reeditar;
- derivados regenerables/intermedios;
- archivos efectivamente entregados/publicados.

Un MP4 terminado no sustituye los originales o el proyecto si después se quiere reeditar. Tampoco todo cache debe ir al archivo permanente.

## Integraciones posibles, todavía no implementadas

| Prioridad candidata | Capacidad | Qué resolvería y qué debe respetar |
| --- | --- | --- |
| Alta | Inventario de entrada que reconozca archivos y paquetes | Distinguir `.stalker`, bundle Insta360, media suelta, proyecto y entrega antes de mover nada. Evita desarmar relaciones. |
| Alta | Plan de transferencia de bundle + verificación | Copiar a staging, verificar manifest/archivos/checksums, publicar carpeta y conservar origen hasta éxito. Un log incremental permite saber qué quedó a medias. |
| Alta | Entrega de ubicaciones estables a Clipper | Registrar UUID/identidad, origen y destino. Importar después de organizar; sin automatizar cortes o efectos desde este repo. |
| Alta | Relocalización coordinada con Clipper | El editor actualiza bundle, voz, pistas, referencias y artefactos de cola relacionados. El organizador no edita por su cuenta el JSON editorial. |
| Media | Paquete de entrega de outputs | Master, variantes, imágenes/textos/subtítulos elegidos y un manifest con proyecto, variante, fechas y rutas. Sin renombrar a ciegas con `save`. |
| Media | Política de retención por etapa | Separar original, trabajo, caché y publicado; conservar los archivos que sostienen proyectos/colas activos antes de liberar disco. |
| Media | Registro de procedencia de assets/stems | Relacionar descarga/original y derivados sin mover una fuente que otro proyecto sigue usando. |
| Después | Automatizar llegada de sesiones terminadas | Detectar `completed`, pedir/asignar proyecto según el flujo acordado y registrar resultado. Detectar un MOV no basta para autorizar importación/archivo. |

Estas son propuestas de archivo/integración. No implican watcher, daemon, edición automática del proyecto, cambio de carpetas ni implementación ya acordada. Primero hay que describir tus flujos reales para decidir qué simplifica trabajo y qué sólo añade pasos.

## Qué definir con los flujos reales

Para cada caso basta describir: qué grabas/descargas; dónde llega; cuándo lo nombras u organizas; cuándo lo importas; qué produces al exportar; qué conservas al terminar y en qué disco; qué tareas repites o te hacen perder tiempo. Con eso podremos elegir ubicaciones y transferencias concretas sin imponer una estructura de carpetas antes de entender el uso.

## Referencias de código y contratos

- Stalker: [destino y presets](https://github.com/nu9ve/stalker/blob/46161c7/Sources/Stalker/AppModel.swift), [layout/manifest](https://github.com/nu9ve/stalker/blob/46161c7/Sources/StalkerCore/SessionManifest.swift), [contrato de intercambio](https://github.com/nu9ve/stalker/blob/46161c7/docs/clipper-contract.md).
- Clipper: [importación de bundles](https://github.com/nu9ve/clipper/blob/7075eb9/src/clipper_tui/stalker.py), [rutas/proyectos/relink](https://github.com/nu9ve/clipper/blob/7075eb9/src/clipper_tui/projects.py), [snapshots y publicación de cola](https://github.com/nu9ve/clipper/blob/7075eb9/src/clipper_tui/export_queue.py), [storage y limpieza](https://github.com/nu9ve/clipper/blob/7075eb9/docs/storage-resume.md), [contrato entre clientes](https://github.com/nu9ve/clipper/blob/7075eb9/apps/APP_CONTRACT.md).
- Este repo: [comportamiento de importación](importing.md), [descargas y stems](commands.md), [configuración](configuration.md), [inventario y sucesores](modules.md).

Los enlaces fijados a commits describen la revisión; una implementación posterior puede cambiar el comportamiento. El contrato técnico de Stalker sigue siendo propiedad de ese repo y no se duplica aquí como un segundo schema.
