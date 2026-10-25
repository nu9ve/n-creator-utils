# Naming y carpetas de media v1

Estado: implementado en el productor (`media/layout.py`, planes esquema 2) el 2026-10-25 y usado en el primer respaldo real (`debloateando-mi-tele`). **Stalker y Clipper aún no lo adoptan**; las secciones de adopción son la propuesta para sus repos. Índice: [contratos](README.md).

## Por qué

Una carpeta de proyecto debe poder recorrerse con flecha abajo y ver todo lo grabado en orden: clips de cámara y sesiones de pantalla intercalados por hora de inicio. Los nombres son para humanos y orden; **la identidad nunca es el nombre** (ver abajo).

## Gramática

```text
YYMONDD-HHMMSS_<FUENTE>_<MM-SS>_<PAIS>-<PROYECTO>[]<-N>.<EXT>
26OCT24-194650_SONYA6400_08-24_MX-DEBLOATEANDO-MI-TELE[].MP4
26OCT24-195850_STALKER_03-48_MX-DEBLOATEANDO-MI-TELE[].stalker
```

| Parte | Regla |
| --- | --- |
| `YYMONDD-HHMMSS` | Inicio de captura en hora de pared local, mes en inglés de 3 letras mayúsculas. Ordena alfabéticamente = cronológicamente dentro de un mes/año. |
| `FUENTE` | Dispositivo/productor: `SONYA6400` (XML `ILCE-6400`), `STALKER`, `H6TR1`/`H6LR`, categoría en mayúsculas si no hay modelo. Tabla en `MODELS` de `layout.py`. |
| `MM-SS` | Duración redondeada a segundos; minutos pueden pasar de 59. Fotos no llevan duración. `00-00` si no hay fuente fiable. |
| `PAIS-PROYECTO` | Slug en mayúsculas. País y proyecto son campos distintos del plan; ciudad no entra al nombre. |
| `[]` | Hueco histórico de etiquetas; se conserva por compatibilidad con años de archivo existente. |
| `-N` | Sólo si dos archivos del mismo plan producen el mismo nombre (mismo segundo y fuente): `…-2[].MP4`. Determinista dentro del plan. |

Fuentes de tiempo, en orden: Sony `CreationDate` + `Duration`/`captureFps` del XML M01; Stalker `createdAt` (UTC → local) + `duration.value/timescale` del manifest; `ffprobe` (`creation_time`, `duration`); WAV por cabecera; por último `mtime` sin duración. El plan registra el nombre resultante y entra en su digest: revisar el plan es revisar los nombres.

## Carpetas

| Destino | Contenido |
| --- | --- |
| Trabajo `<prefix>/<perfil>_raw/<proyecto>/` | Plano: clips editables y bundles Stalker completos. Sin XML, miniaturas ni índices de cámara. |
| Archivo `001_raw/videos|v4k|vdron|360/videos/AAAA/MM/` | Video por tipo/fecha de captura. |
| `001_raw/recordings|recordingslong/AAAA/MM/` | Audio (>10 min a `recordingslong`). |
| `001_raw/images|rimages|360/images/AAAA/MM/` | Fotos y RAW. |
| `001_raw/stalker/AAAA/MM/<nombre>.stalker/` | Sesiones Stalker completas. |
| `001_raw/metadata/<familia>/AAAA/MM/<nombre del clip>.<EXT>` | Sidecars del clip (XML Sony, LRV/THM). |
| `001_raw/metadata/<familia>/support/AAAA/MM/<DDMM-HHMMSS>_<PAIS>-<PROYECTO>_<FUENTE>/<árbol original>` | Índices, miniaturas, `AVF_INFO`, `PRIVATE/SONY`: árbol de la tarjeta para reconstruirla. |

Copias aterrizan como `.<nombre>.ncpartial`, se verifican por lectura y se renombran sin reemplazar nada. Un archivo idéntico ya presente se adopta; uno distinto bloquea el plan en revisión.

## Identidad y bundles Stalker

- Un bundle archivado cambia **sólo el nombre de su carpeta**. `manifest.json`, pistas, `mic.wav`, `events.ndjson` y `sources/` quedan byte a byte.
- Identidad de sesión: `sessionID` del manifest. Identidad de unidad: `asset_id` + `content_revision` del catálogo/handoff. Ningún consumidor debe inferir identidad, pertenencia o duplicado por nombre.
- El original en la biblioteca de Stalker conserva su nombre (`Stalker_AAAA-MM-DD_HH-MM-SS.stalker`) hasta que Stalker adopte este naming.

## Renombre en sitio (`media relayout JOB_ID`)

Para jobs terminados con sesiones Stalker copiadas con el nombre anterior. Renombra la carpeta del bundle en cada volumen con copias (rename atómico, sin copiar bytes). Después actualiza las filas `copies`, guarda `layout_override:<job>` para entregas futuras y reescribe el recibo portable con un bloque `relayout`. Es idempotente tras una interrupción. Verificado en las 3 sesiones reales y con `recover-catalog` desde MEDIA.

Las copias de trabajo se eliminan al terminar de editar (ver [política](../features/replication-policy.md)). Reeditar después requiere `nu9ve media deliver`/`restore` desde MEDIA y relink en el editor.

Un renombre cambia rutas. Un consumidor que guardó la ruta vieja debe resolver por `sessionID`/`asset_id` (handoff nuevo) o hacer relink. No se renombra nada que un editor esté usando sin ese paso.

## Estado de archivo para productores

`nu9ve media captures <biblioteca>` (sólo lectura) lista las sesiones `completed` de una biblioteca Stalker con `archived: true|false`. `true` significa que hay copia de archivo verificada con recibo para ese `sessionID` + digest del manifest. Es la única señal válida para ofrecer liberar el original local; un manifest modificado después de archivar vuelve a `false`.

## Adopción propuesta: Stalker

1. Opcional: si la sesión tiene proyecto/país, nombrar la carpeta de la sesión con esta gramática desde su creación (`…_STALKER_<duración final>_…[].stalker`), renombrando al finalizar, cuando la duración se conoce, y antes de marcar `completed`. Así archivo y trabajo no necesitan `relayout`.
2. Proyecto/país como metadata opcional versionada del manifest, propiedad de Stalker; el archivador no la escribe.
3. Biblioteca durable configurable fuera de `~/Downloads`. Señal estable de finalización, sin reescribir un bundle `completed`.
4. Fixture compartida: una sesión con `createdAt`/`duration` conocidos debe producir exactamente `26OCT24-195850_STALKER_03-48_MX-…[].stalker`, y su contenido no debe cambiar.
5. «Liberar sesiones archivadas»: sólo sesiones con `archived: true` según `captures`, por selección explícita, hacia la Papelera (recuperable), nunca por evento. El archivador no borra la biblioteca de Stalker.

## Agrupar cámara y pantalla (caso para el autogroup de Clipper)

La relación no es 1:1. Caso real, `debloateando-mi-tele` (horas locales de `sources`):

| Fuente | Inicio | Fin |
| --- | --- | --- |
| C0003 cámara | 20:41:10 | 20:43:27 |
| C0004 cámara | 20:44:01 | 20:48:48 |
| Sesión Stalker | 20:44:16 | 20:52:00 |
| C0005 cámara | 20:54:15 | 21:07:51 |
| Sesión Stalker | 20:56:33 | 21:10:56 |

- **Varias cámaras por pantalla:** una sesión puede abarcar varios clips (la cámara se detuvo y se reanudó). Un clip puede empezar antes que la sesión o terminar después, y entre clips hay huecos de segundos a minutos. El usuario no sabe de antemano cuáles van juntos; agruparlos es el trabajo del autogroup.
- **Sugerencia:** componentes conectados de intervalos que se solapan o quedan a menos de una tolerancia configurable (p. ej. 90 s). Sin solape claro, el grupo queda como candidato y no como hecho. El usuario confirma o deshace; no hay sync silencioso.
- **Relojes:** `captured_at` tiene precisión de segundo y viene de relojes distintos (cámara frente a Mac), que pueden diferir segundos o minutos. Sirve para proponer grupos. La alineación fina es del reloj Stalker y del sync de audio de Clipper. Un desfase medido por dispositivo podría guardarse y reutilizarse más adelante (decisión abierta; si se adopta, va a «Pendientes para n-creator-utils»).

## Copias de trabajo borradas a mano

El usuario borra de la carpeta de trabajo, en Finder y sin avisar, lo que no sirve (p. ej. un clip de 2 s) o un proyecto entero al terminar de editar. Eso es normal y no debe dar error.

- `sources` comprueba presencia y tamaño de cada copia. Si falta la de trabajo, la fuente sale en `removed_from_work`, apuntando a la copia de archivo, y no en `sources`.
- Si falta una copia de archivo, sale en `warnings` (`archive_copy_missing`).
- Un consumidor muestra lo removido como removido: ni error ni re-importación automática. Volver a traerlo es una acción explícita (`deliver`/`restore` o relink a la copia de archivo).

## Adopción propuesta: Clipper

1. Selector de biblioteca que lea `<perfil>_raw/<proyecto>/` y respete el orden por nombre: clips y sesiones intercalados por inicio de captura. Comprobado 2026-10-25: `clipper_tui.stalker.expand_inputs` sobre la carpeta real devuelve los 7 clips y 3 bundles en ese orden, y reconoce `.stalker` por sufijo. Falta comprobar import, sync, render y export con `[]` en las rutas (filtros de ffmpeg, listas concat, glob).
2. Reconocer `*.stalker` con cualquier nombre de carpeta: validar por `manifest.json` (`producer`, `sessionID`), no por el patrón `Stalker_…`.
3. Guardar en el proyecto el `sessionID`/`asset_id` además de la ruta, y ofrecer relink cuando la ruta cambie (relayout, otro SSD, restauración).
4. Consumir `creator-media-handoff/1` ([spec](media-library-v1.md)): ubicaciones por volumen/rol. La copia de trabajo es la preferida para editar; el archivo es el respaldo.
5. No escribir dentro de bundles ni renombrar archivos de media; las ediciones viven en el proyecto Clipper.

## Fuera de alcance

- No se renombra el archivo histórico de MEDIA ya organizado.
- Final Cut, Atlas y las entregas (`002_finales`) conservan sus convenciones.
- Fotos sin EXIF fiable usan `mtime`. DJI/GoPro/H6 tienen reglas de fuente sólo a nivel fixture hasta validarlas con hardware.
