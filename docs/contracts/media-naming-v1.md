# Naming y carpetas de media v1

Estado: implementado en el productor (`media/layout.py`, planes esquema 2) el 2026-10-25 y usado en el primer respaldo real (`debloateando-mi-tele`). **Stalker lo adoptó en 0.4.0** (2026-10-25, `d88eaf9`, ver [adopción](#adopción-stalker-implementada-2026-10-25)). **Clipper aún no**; su sección es la propuesta para su repo. Índice: [contratos](README.md).

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
| `MM-SS` | Duración redondeada a segundos con `round()` de Python (mitad a par: 0.5 → 0, 1.5 → 2); minutos pueden pasar de 59. Fotos no llevan duración. `00-00` si no hay fuente fiable. |
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
- Desde Stalker 0.4 una sesión con proyecto nace como `Stalker_AAAA-MM-DD_HH-MM-SS.stalker` y, al completarse, toma el nombre de esta gramática. Una sesión sin proyecto, o `interrupted`/`failed`, conserva `Stalker_…`. Las sesiones anteriores no se renombran en origen.
- `sessionID` se compara como UUID: Stalker lo escribe en mayúsculas, el catálogo en minúsculas.

## Renombre en sitio (`media relayout JOB_ID`)

Para jobs terminados con sesiones Stalker copiadas con el nombre anterior. Renombra la carpeta del bundle en cada volumen con copias (rename atómico, sin copiar bytes). Después actualiza las filas `copies`, guarda `layout_override:<job>` para entregas futuras y reescribe el recibo portable con un bloque `relayout`. Es idempotente tras una interrupción. Verificado en las 3 sesiones reales y con `recover-catalog` desde MEDIA.

Las copias de trabajo se eliminan al terminar de editar (ver [política](../features/replication-policy.md)). Reeditar después requiere `nu9ve media deliver`/`restore` desde MEDIA y relink en el editor.

Un renombre cambia rutas. Un consumidor que guardó la ruta vieja debe resolver por `sessionID`/`asset_id` (handoff nuevo) o hacer relink. No se renombra nada que un editor esté usando sin ese paso.

## Estado de archivo para productores

`nu9ve media captures <biblioteca>` (sólo lectura) lista las sesiones `completed` de una biblioteca Stalker con `archived: true|false`. `true` significa que hay copia de archivo verificada con recibo para ese `sessionID` + digest del manifest. Es la única señal válida para ofrecer liberar el original local; un manifest modificado después de archivar vuelve a `false`. Stalker nunca reescribe un bundle `completed`, así que el digest sólo cambia si alguien lo edita fuera de Stalker; esa sesión vuelve a quedar pendiente de archivo.

## Adopción propuesta: Stalker

Implementada el 2026-10-25 con las decisiones de [Adopción: Stalker](#adopción-stalker-implementada-2026-10-25). Diferencia con el punto 1: el nombre se aplica al completar, no desde la creación (la duración no se conoce antes).

1. Opcional: si la sesión tiene proyecto/país, nombrar la carpeta de la sesión con esta gramática desde su creación (`…_STALKER_<duración final>_…[].stalker`), renombrando al finalizar, cuando la duración se conoce, y antes de marcar `completed`. Así archivo y trabajo no necesitan `relayout`.
2. Proyecto/país como metadata opcional versionada del manifest, propiedad de Stalker; el archivador no la escribe.
3. Biblioteca durable configurable fuera de `~/Downloads`. Señal estable de finalización, sin reescribir un bundle `completed`.
4. Fixture compartida: una sesión con `createdAt`/`duration` conocidos debe producir exactamente `26OCT24-195850_STALKER_03-48_MX-…[].stalker`, y su contenido no debe cambiar.
5. «Liberar sesiones archivadas»: sólo sesiones con `archived: true` según `captures`, por selección explícita, hacia la Papelera (recuperable), nunca por evento. El archivador no borra la biblioteca de Stalker.

## Adopción: Stalker (implementada 2026-10-25)

Stalker 0.4.0, commits `ecb18f1` y `d88eaf9` (main). Decisiones que afectan a las tres apps:

| # | Decisión | Compatibilidad |
| --- | --- | --- |
| 1 | **Biblioteca por defecto `~/Movies/Stalker`**, configurable. Una biblioteca que seguía en el default de 0.3 (`~/Downloads`, no elegida a mano) pasa a ella una vez, con aviso. Las sesiones existentes no se mueven. | Las 3 sesiones archivadas siguen en `~/Downloads`. |
| 2 | **Cómo se entera n-creator-utils de la ruta:** Stalker publica `~/Library/Application Support/Stalker/library.json`, `{"schema": "stalker-library/1", "path", "previous": [...], "updatedAt"}`, y lo reescribe sólo cuando cambia la carpeta. `previous` lista carpetas anteriores (más reciente primero, máx. 20) que pueden tener sesiones. Lectura para otras apps; el campo manual de n-creator-utils sigue mandando. | Pedido en «Pendientes para n-creator-utils». Sin el archivo, todo funciona como hoy configurando la carpeta a mano. |
| 3 | **Metadata en el manifest:** `project: {"version": 1, "name": "<tal cual se escribió>", "country": "<p. ej. MX>"}`, opcional; ausente = sin proyecto. Propiedad de Stalker; el archivador no la escribe y la usa sólo como sugerencia. Con proyecto, el país es obligatorio (la gramática lo exige); el país se recuerda como default. **El proyecto se decide cuando el usuario lo sabe** (normalmente al final): grabar nunca pide nada; se escribe antes, durante (vale lo escrito al detener) o después. «Sugerencias» ofrece el último de Stalker y los de `nu9ve media projects`; nunca se aplica uno solo. | `stalker-clipper/1` rev. 1.2, adición opcional. |
| 4 | **Renombre al completar, no desde la creación:** durante la grabación la duración no existe, así que la carpeta es `Stalker_fecha`. Con `status=completed` pendiente y la duración final conocida: inicio = `createdAt` en la zona horaria del Mac al finalizar; duración = `duration` redondeada como `round()`; slug igual a `slug()` de `layout.py` (NFC, `[^\w-]+` → `-`, recorte de `-_`, máx. 160) en mayúsculas. | Fixture de contrato en `Tests/StalkerCoreTests/MediaContractTests.swift`: `26OCT24-195850_STALKER_03-48_MX-DEBLOATEANDO-MI-TELE[].stalker`. |
| 5 | **Colisión:** rename atómico que nunca reemplaza (`renamex_np` + `RENAME_EXCL`, insensible a mayúsculas en APFS); si existe, `-2[]`, `-3[]`…; si falla por otra causa conserva `Stalker_…` y lo anota en `warnings`. | Igual que `claim()` del plan. |
| 6 | **Señal de finalización:** `completed` es la última escritura de la grabación. Orden: writers y derivados cerrados → `fsync` de cada archivo + un `F_FULLFSYNC` → renombre (si hay proyecto) → manifest reemplazado con un rename atómico. Una interrupción deja `finalizing`, nunca `completed` a medias. **Única excepción posterior:** poner o cambiar el proyecto a mano, sólo si `captures` responde `archived: false` para esa ruta; renombra la carpeta (atómico, `-N[]`) y reemplaza el manifest (cambia su digest); media y `sessionID` intactos. Con `archived: true`, o si `captures` falla, no se toca. La CLI escribe fuera. | El watcher confía en `completed` + digest y revalida al aplicar (pedido en Pendientes). |
| 8 | **Proyecto reciente para la siguiente SD:** `library.json` lleva `recentSession` (`sessionID`, `path`, `project`, `country`, `createdAt`, `endedAt`) de la última sesión con proyecto, actualizado al detener o al nombrarla después. n-creator-utils puede proponerlo al importar una SD del mismo día/horas cercanas. | Campo opcional; pedido en Pendientes. |
| 7 | **Liberar sesiones archivadas:** botón explícito en Configuración. Consulta `captures` de la carpeta (o de otra elegida, p. ej. `~/Downloads`), ofrece sólo `archived: true` + `completed`, hijas directas de esa carpeta y cuyo manifest local siga `completed` con el mismo `sessionID`. Vuelve a consultar justo antes de mover a la Papelera y conserva lo que ya no figure archivado. Si el comando falla, no existe o responde algo inválido, no ofrece liberar. Nunca por evento. | Usa el catálogo del usuario; las pruebas usan `NU9VE_MEDIA_HOME` temporal. |

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
