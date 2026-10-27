# Operación, instalación y debug

Estado y límites: [STATUS](STATUS.md). Estos comandos describen el código actual; no son una autorización para probar sobre tu única copia. El catálogo nuevo es independiente de `config.local.json` del legado.

## Arranque desde código

```sh
python3 -m venv .venv
source .venv/bin/activate
python -m pip install -e .
nu9ve --help
nu9ve media --help
```

Python 3.9+; el nuevo motor no necesita SDK cloud ni FFmpeg para copiar/verificar. FFprobe/FFmpeg siguen siendo herramientas de algunas utilidades/pruebas antiguas. App nativa: macOS 14+ y Swift 6 para compilar.

macOS guarda estado en `~/Library/Application Support/n-creator-utils/media/`. `NU9VE_MEDIA_HOME` o `media --state-dir` eligen otro catálogo. Es local: no poner una SQLite WAL activa compartida en MEDIA. Usar paths físicos sin symlinks para estado/raíces; en pruebas, `/private/tmp` evita el alias `/tmp`.

## Registrar destinos

```sh
nu9ve media volumes register MEDIA502 /Volumes/MEDIA502 --role archive --prefix 001_raw
nu9ve media volumes register ESSD /Volumes/ESSD --role work --prefix 001_proyectos
nu9ve media volumes list
```

Registrar SSD1 de la misma forma para audio. La raíz ya debe existir en el disco correcto; no crear una carpeta para suplir un externo ausente. El registro modifica el catálogo local; no migra tu archivo. El default requiere volumen externo. `--allow-local` es una elección explícita para carpetas locales/fixtures, no redundancia adicional.

## Plan y ejecución

```sh
nu9ve media plan /Volumes/TARJETA --archive MEDIA502 --work ESSD \
  --project intro --profile e64 --country mx --city hermosillo --cleanup > plan.json
# Revisar data.preview, destinos, limpieza y digest de la respuesta.
nu9ve media apply plan.json --digest DIGEST_REVISADO
```

`plan` no copia/borra media. El CLI JSON devuelve `{ok, command, data}` o un error; `apply` acepta el plan crudo o la respuesta completa de `plan`. Cambios entre revisión y ejecución pueden bloquear el job. El planner decide nombres históricos fechados y carpetas por tipo/año/mes (ver [CONTEXT](CONTEXT.md)); revisa `data.layout`/`data.preview` antes de aplicar. `nu9ve media job JOB_ID` muestra `progress` (fase, bytes copiados/verificados, MB/s). `nu9ve media relayout JOB_ID` renombra en sitio bundles Stalker de jobs terminados al naming [v1](contracts/media-naming-v1.md).

Flujo humano equivalente:

```sh
nu9ve save /Volumes/TARJETA --archive MEDIA502 --work ESSD \
  --project intro --profile e64 --country mx --city hermosillo
nu9ve save all --archive MEDIA502 --work ESSD --project intro
```

`save` presenta resumen, rutas de destino por archivo y pide `GUARDAR`; `--dry-run` muestra el plan y `--yes` omite sólo esa confirmación estándar. Para conservar fuente: `--keep-source`. El atajo `save all mx TEXTO` equivale a `save all --country mx --project TEXTO`: el último texto ahora es **proyecto**, no ciudad. No combinar esos posicionales con `--country`/`--project`; ciudad y perfil se seleccionan con flags separados. El antiguo motor quedó como `legacy-save`, con sus riesgos intactos.

### Sólo respaldo, sin archivos de trabajo

```sh
nu9ve save all mx kasakarla-electroritual --archive MEDIA502 --no-work
# Para elegir una sola tarjeta:
nu9ve save /Volumes/EOS_DIGITAL mx kasakarla-electroritual --archive MEDIA502 --no-work
# Vista previa: no copia ni limpia. La confirmación explícita permite construir el plan archive-only.
nu9ve save /Volumes/EOS_DIGITAL mx kasakarla-electroritual --archive MEDIA502 \
  --no-work --confirm-archive-only --dry-run
```

`--no-work` elige archive-only con trabajo `skipped`: no copia al SSD, no crea entrega pendiente ni comprueba su disponibilidad. Sigue usando el planner, naming por tipo/año/mes, SHA-256 y journal del motor nuevo. Pide `SOLO MEDIA` para esta operación y después `GUARDAR`; `--confirm-archive-only` sustituye sólo la primera confirmación. `--yes` por sí solo no autoriza archive-only. No cambia la política de trabajo de próximas operaciones ni el SSD predeterminado. Es incompatible con `--work` o `--work-decision pending`. Tras verificar el archivo requerido, limpia la tarjeta según su perfil; `--keep-source` conserva los originales. `all` considera todas las tarjetas elegibles: usar una ruta si sólo quieres una.

Archivo sin SSD, explícito por operación:

```sh
nu9ve save /Volumes/TARJETA --archive MEDIA502 --work ESSD --project intro \
  --archive-only --confirm-archive-only --work-decision pending
```

`pending` registra entrega posterior al SSD; `skipped` la omite deliberadamente. La SD puede liberarse tras verificar los requeridos de ese plan. En Sony, tras verificar se borran de la SD los clips y fotos (no sus índices) y el job queda en «formatea en la cámara»; después del formateo, `media confirm-cleanup JOB TARJETA` lo cierra. País/ciudad no son campos de proyecto.

Para SD mixta, `--assignments archivo.json` acepta rutas relativas → `{project, profile, camera}`. Pares/bundles se asignan juntos. Categorías DJI: `dji-drone`, `dji-360`, `dji-action`; no hay aún registro persistente de modelos. La app permite revisar/asignar unidades y muestra rutas del plan.

## Jobs, biblioteca y pendientes

```sh
nu9ve media jobs
nu9ve media job JOB_ID
nu9ve media resume JOB_ID
nu9ve media library --work-state pending
nu9ve media deliver ASSET_ID --work ESSD
nu9ve media work-state ASSET_ID --state skipped
nu9ve media resolve ASSET_ID
nu9ve media restore ASSET_ID /ruta/destino-existente
nu9ve media handoff ASSET_ID > handoff.json
nu9ve media rename-project PROJECT_ID nuevo-titulo
```

Reintento por archivo, no rollback general. `paused`/`failed` requieren revisar error y reanudar; una operación completada no vuelve a limpiar. Recovered jobs se usan para restore/delivery, no para reanudar limpieza de una SD.

```sh
nu9ve media recover-catalog MEDIA502
```

Recovery lee recibos de archivo y vuelve a verificar bytes. No sustituye una estrategia de backup para todas las decisiones posteriores del catálogo; ver bloqueadores en STATUS. No borrar staging/contendedores de jobs a mano como solución rutinaria.

Stalker: `save CARPETA --source-kind stalker --keep-source`; sólo sesiones terminadas, bundle completo. Entregas: `--source-kind delivery --archive-only --confirm-archive-only --work-decision skipped --keep-source`. Publicación explícita: `media publication ASSET_ID RUTA_RELATIVA --channel CANAL --state published`; es metadata local, no un upload. Clipper sigue aceptando rutas por su propio flujo; el handoff nuevo necesita consumidor.

## App de desarrollo y launcher

Instalación local y actualización al código actual (no ejecuta backups):

```sh
python3 scripts/install_media_app.py --python /usr/bin/python3
```

Reconstruye Swift en release y empaqueta el motor Python actual; instala en `~/Applications/nuun.app` (nombre visible **nüün**) y registra esa ruta en LaunchServices para Spotlight/Raycast. Si existe la instalación anterior `n creator utils.app` o `Nu9ve Media.app`, la migra al respaldo de builds fuera de Applications. Cierra la app desde su menú antes de actualizar: el instalador rechaza cualquier instancia abierta, sin cancelarla ni interrumpir un job. Conserva el bundle anterior fuera de Applications, en `~/Library/Application Support/n-creator-utils/app-backups/`. El catálogo y las preferencias se conservan; el reemplazo del bundle usa staging y restaura el anterior si falla el intercambio. Las copias anteriores no se borran automáticamente.

Busca **nuun**; la app se presenta como **nüün**. El nombre de archivo ASCII y los keywords nuun/nun se registran al instalar; la aceptación real de alias/ranking en Spotlight/Raycast sigue pendiente (el índice local no devolvió metadata durante esta sesión). No se crean ejecutables duplicados para los alias. Abrir o reabrir conserva el modo visible; cerrar la ventana deja la menubar. **⌘I** cambia la misma ventana a compacto; expandir abre completo y «Elegir archivos» abre selección intermedia. Completo admite fullscreen. `--background` inicia sólo en menubar. [Estados y límites](features/nuun-window.md).

La instalación es un snapshot: para incorporar cambios futuros vuelve a ejecutar el instalador. Ajustes → Diagnóstico muestra el build y las rutas del motor. Python externo, firma de desarrollo y ausencia de notarización siguen siendo límites de esta instalación local.

En «Ingreso», elige fuente/proyecto/MEDIA y **Copiar también al SSD para editar**:

- **Ninguno · sólo respaldo**: omite el ESSD incluso conectado, sin pendientes. Es la selección inicial.
- **Todo el lote**: respalda todo y copia también los archivos editables al SSD; índices/auxiliares siguen sólo en archivo.
- **Elegir archivos**: abre directamente selección intermedia desde compacto; las miniaturas se cargan automáticamente. Marca las unidades que quieras editar y asigna proyecto por unidad si hace falta. Todo se respalda, incluidas las unidades sin marcar. Pares RAW/JPG y bundles se mantienen juntos. Sin marcas se requiere confirmar sólo respaldo.

Selección y completo inspeccionan la fuente automáticamente con loader y cancelación de lecturas obsoletas; no hay botón de cargar. El grid pide miniaturas a Quick Look bajo demanda, con hasta cuatro solicitudes activas y caché propia en memoria de 24 MB/160 entradas; no decodifica todo el lote en el hilo principal ni copia videos al disco interno. Los auxiliares tienen placeholder y no se seleccionan para editar. Proyecto/perfil se asignan por unidad; cámara avanzada y clasificación DJI siguen disponibles.

Activa **Limpiar SD tras verificar**, revisa el plan y confirma sólo respaldo cuando corresponda. La confirmación prepara el plan; **Ejecutar plan revisado** copia, verifica y después limpia. Si falla la verificación, no se autoriza la limpieza. Sony conserva su tratamiento especial. Revisa bytes, copias para editar y rutas por archivo antes de ejecutar.

En ajustes, **Apariencia** ofrece Claro/Oscuro/Sistema y matiz/saturación/brillo (HSV) del color base. Las nuevas preferencias comienzan en **Claro + #FFBB5C**. La paleta deriva fondo, tarjetas, campos, bordes, botones y barra de título; dashboard e importador comparten la misma superficie. El texto de las acciones principales elige negro/blanco según luminancia. Los cambios se aplican a SwiftUI y NSWindow, no sólo al tint. Las preferencias existentes se conservan; **Restaurar claro · #FFBB5C** aplica el nuevo preset. El modo compacto usa opciones verticales; selección y completo usan controles directos e imágenes sin tarjeta de fondo. **Al conectar una tarjeta** permite activar el popup del importador rápido; está desactivado inicialmente. Sin destinos registrados online no aparece ni escanea la SD. **Probar vista del importador…** abre la misma vista compacta sin simular una conexión ni iniciar un job. La vista compacta permite nombre de lote, último proyecto y ninguna/todas/algunas copias para editar; la selección visual abre el tamaño intermedio y la revisión/ejecución puede continuar allí, sin crear otra ventana. La reconexión física y sus permisos/notificaciones requieren validación separada. En Stalker se inspeccionan sólo bundles del primer nivel de la biblioteca (o el bundle elegido directamente), sin recorrer otros proyectos de Downloads.

### Perfiles y espacios de trabajo

Ajustes se divide en **Perfiles, Discos, Apariencia, Capturas y Diagnóstico**. En Perfiles crea/edita ID (`9logs`), etiqueta, carpeta relativa (`9logs_raw`) y discos sugeridos. La ruta es `<raíz SSD>/<prefix registrado>/<raw_dir>/<proyecto>`. La selección del perfil en la app aplica discos sugeridos; CLI debe elegirlos con sus flags habituales. La carpeta se incorpora al planner compartido y queda congelada en cada plan. No mueve backups previos. País/ciudad, proyecto y perfil siguen separados.

Boundary local para agentes/otras superficies, mediante `media rpc` (una petición JSON por proceso):

```json
{"method":"profiles","params":{}}
{"method":"save_profile","params":{"id":"9logs","name":"9logs","raw_dir":"9logs_raw","work":"ESSD","archive":"MEDIA502"}}
```

La segunda petición cambia configuración local, no copia media. Volúmenes por ID/nombre registrado, roles validados y paths relativos sin traversal. No añade herramientas de escritura al MCP read-only.

Para construir un artefacto separado sin instalar:

```sh
swift test --package-path native/MediaMenu
python3 scripts/build_media_app.py --output dist/Media-dev-NUEVO.app \
  --configuration debug --skip-build --python /ruta/absoluta/a/python3
open dist/Media-dev-NUEVO.app --args --show
```

El builder no sobrescribe outputs existentes. `--skip-build` exige binario ya compilado; omitirlo para compilar. El script del motor se resuelve dentro de Resources al mover la app; las preferencias de runtimes antiguos no sustituyen ese snapshot. El updater/autostart de distribución siguen en [backlog](features/dashboard-launcher.md).

Build de distribución: runtime relocatable proporcionado con `--runtime`, licencia y firma explícita; todavía requiere [gates de release](release.md). No distribuir un bundle de desarrollo con Python/ruta del checkout externos.

## Debug aislado

Para una sesión de prueba, elegir un `NU9VE_MEDIA_HOME` nuevo bajo `/private/tmp` antes de iniciar CLI/app. Registrar sólo directorios de fixture con `--allow-local` y usar `--keep-source`; carpetas del mismo disco no pasan la política normal de dos discos físicos. Fallos/cleanup se prueban con `FixtureVolumes`, no relajando validaciones de producción.

| Síntoma | Comprobar |
| --- | --- |
| No popup al insertar SD | Es esperado sin destinos registrados online. Con destinos, verificar identidad y que la fuente sea una tarjeta elegible; manual inspect sigue disponible. |
| `volume_identity_unavailable` | `diskutil`/`df`, permisos y montaje. No saltar la comprobación por nombre. |
| `source_changed` / `checksum_mismatch` | Revisar job, conservar archivos y no forzar limpieza; reproducción pequeña con fixture. |
| `source_has_new_material` | Lo planeado quedó verificado y limpio; aparecieron archivos nuevos en la SD durante la limpieza y **no están respaldados**. Inspeccionar la SD de nuevo y crear otro plan. |
| `destination_conflict` | Ya existe un archivo distinto con el nombre planeado (o un contenedor de plan esquema 1 sin owner). No sobrescribir ni borrar para ocultarlo; revisar cuál es cuál. |
| App ve otro catálogo/runtime | Entorno de lanzamiento, preferencias del bundle, runtime externo frente a incluido; no mezclar el catálogo temporal de UI con el normal. |
| Swift no puede escribir su module cache | El ambiente puede necesitar autorización de build fuera del sandbox. No es una razón para desactivar validaciones del motor. |

No hay aún un comando que exporte un bundle completo de diagnóstico/redacción ni rotación integral de logs. Jobs/progress/eventos viven en SQLite; el JSON RPC da errores con código. Evitar compartir catálogos/planes crudos sin revisar paths y metadata personal.

MCP local: `nu9ve media mcp`; stdio JSON-RPC, sólo búsqueda/resolución/jobs/volúmenes. No abre un puerto ni expone escritura. [Boundary y herramientas](../media/mcp.py).
