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

`save` presenta resumen y pide `GUARDAR`; `--dry-run` muestra el plan y `--yes` omite sólo esa confirmación estándar. Para conservar fuente: `--keep-source`. Formato antiguo `save all mx TEXTO` ya no se interpreta: usar flags; el antiguo motor quedó como `legacy-save`, con sus riesgos intactos.

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

```sh
swift test --package-path native/MediaMenu
python3 scripts/build_media_app.py --output dist/Media-dev-NUEVO.app \
  --configuration debug --skip-build --python /ruta/absoluta/a/python3
open dist/Media-dev-NUEVO.app --args --show
```

El builder no sobrescribe outputs existentes. `--skip-build` exige binario ya compilado; omitirlo para compilar. Arranque normal sólo deja menubar; `--show` abre la ventana de forma explícita. El launcher/instalador/autostart definitivo están en [backlog](features/dashboard-launcher.md).

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
