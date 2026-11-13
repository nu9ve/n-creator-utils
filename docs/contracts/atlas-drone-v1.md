# Fotos de dron para Atlas · v1 (propuesta)

Estado: **propuesta 2026-11-05, actualizada 2026-11-08** (ver
[«Primer ingreso real»](#actualización-2026-11-08--primer-ingreso-real-y-exports)), escrita desde una sesión de Atlas
(`/Users/nu9ve/code/data/atlas`, working tree sobre `ba60ffa`) tras inventariar la
SD `MAVIC200`, el ESSD y MEDIA502. Lado nüün implementado y aplicado el 2026-11-08: ver
[estado del lado nüün](#estado-del-lado-nüün--2026-11-08). Atlas es
consumidor; subaereo (app iOS de vuelo) es productor de los exports JSON. Índice:
[contratos](README.md). Contrato de captura del lado Atlas/subaereo:
[`atlas/docs/contracts/subaereo-capture-v1.md`](../../../../data/atlas/docs/contracts/subaereo-capture-v1.md).

## Qué hay hoy (evidencia)

| Lugar | Contenido observado el 2026-11-05 |
| --- | --- |
| SD `MAVIC200` (exFAT, 16 GB usados) | `DCIM/101MEDIA` `DJI_0911–0999` (89 pares) y `DCIM/102MEDIA` `DJI_0001–0483` (483 pares). `DCIM/100CANON` y `CANONMSC` **vacías** (residuo de otra cámara: no clasifican nada). `MISC/GIS/dji.gis` (43 MB, base de geocercas DJI), `MISC/THM/101`, `MISC/IDX` (soporte). |
| ESSD `001_proyectos/atlas-subaereo` | Raíz de imágenes de Atlas (`config/atlas.json`). Reorganizada a mano en `101MEDIA/` (623 pares, mayo–nov) y `102MEDIA/` (483 pares, nov). 1,106 JPG = 5.8 GB; 1,106 DNG = 27.3 GB. ESSD al 91 % (46 GB libres). |
| ESSD `001_proyectos/atlas-derived` | Derivados de Atlas: catálogo, tiles ODM, proyectos ODM (2–8 GB cada uno), estado humano `atlas_state.sqlite3`. |
| MEDIA502 `001_raw/vdron/` | Videos históricos ya nombrados `25OCT02-182637_MAVIC2_00-01_MX-26DRONSTART[].MP4`. No hay fotos de Atlas archivadas en MEDIA. |

Todas las fotos son `Make=DJI`, `Model=FC3170` (cámara del Mavic Air 2),
`SerialNumber=1TCLH9N03BJ5N9`, 4000×3000, pares JPG (8 bit, ~5.3 MB) + DNG
(16 bit, ~24.7 MB). El DNG ocupa el 82 % de los bytes.

**Riesgo confirmado:** DJI reinicia la numeración por carpeta. `101MEDIA` termina en
`DJI_0999` y `102MEDIA` empieza en `DJI_0001`; 142 nombres (`DJI_0342–0483`)
existen en ambas carpetas con fotos distintas (mayo vs noviembre). Un consumidor
que identifique por nombre mezcla vuelos. Atlas ya lo resolvió de su lado
(catálogo v2: IDs estables, sufijo de fecha en colisiones); nüün no debe
aplanar carpetas DCIM ni identificar por nombre.

## Actualización 2026-11-08 · primer ingreso real y exports

Escrito desde Atlas (`/Users/nu9ve/code/data/atlas`, sobre `10ba32a`) tras leer la SD
montada (sólo lectura: EXIF y miniaturas en un directorio temporal; nada copiado ni
escrito en la tarjeta), el ESSD y MEDIA502.

### Evidencia

| Lugar | Estado 2026-11-08 |
| --- | --- |
| SD `MAVIC200` (exFAT, UUID `8486AB6C-…-F4B3A87CD60A`, 37 GB usados) | `101MEDIA` 89 pares (31 ago) · `102MEDIA` 999 pares (4–7 nov) · `103MEDIA` 254 pares (7 nov) · `100CANON`/`CANONMSC` vacías · `MISC` 58 MB · `LOST.DIR` 128 KB. 1,342 pares, todos `FC3170` / `1TCLH9N03BJ5N9`. El nombre del volumen no es identidad: puede cambiar. |
| Fotos sólo en la SD | **770 pares** (6–7 nov, 10 sesiones, todas nadir a 45 m) que no están en el ESSD. Una sesión del 7 nov cruza `102MEDIA` → `103MEDIA`: la sesión no es la carpeta. |
| Fotos sólo en el ESSD | **534 pares, 16.3 GB** (30 may – 3 jun) en `atlas-subaereo/101MEDIA`. Ya no están en la SD y **no hay copia en MEDIA502**: hoy son copia única. |
| ESSD | 92 % (41 GB libres); `atlas-subaereo` 28 GB (JPG + DNG), `atlas-derived` 18 GB. |
| MEDIA502 | 54 % (2.1 TB libres). Sin fotos de Atlas archivadas. |
| Exports de subaereo | 22 JSON sueltos en `atlas/coverage_exports/` (may–nov, 20 KB – 9 MB). Desde el 8 nov la app exporta **24 h / 7 días / todo** con `exportWindow.mergeKeys`; el completo ya pesa 9 MB y crece 1–2 MB por día de vuelo. |

### Decisiones del usuario (2026-11-08)

Reemplazan lo que diga lo contrario en las secciones de 2026-11-05/06 de abajo
(sobre todo «Rutas y nombres: Atlas conserva los originales»).

1. **Nombres de nüün, no los de DJI.** Las fotos de dron usan la gramática de
   [naming v1](media-naming-v1.md) y viven en una carpeta de dron por fecha. El nombre
   original (`DJI_NNNN`, carpeta DCIM, UUID de la tarjeta) queda como **metadato** del
   catálogo de nüün y en el mapa que se publica para Atlas: nadie identifica por nombre.
2. **Misión contra no misión.** Lo que pertenece a una misión de subaereo se separa de
   las demás fotos de dron, ambas por fecha.
3. **Un solo registro de misiones.** Los exports de subaereo llegan por AirDrop a
   `~/Downloads`; nüün los detecta, incorpora sólo lo nuevo a un registro unificado y
   quita el archivo de Descargas. Atlas lee ese registro y se entera de los puntos nuevos.
4. **DNG:** el archivo guarda JPG+DNG por ahora. El trabajo sólo JPG; los DNG que ya están
   en el ESSD se quedan como semilla de la prueba de Atlas. Después de A11–A15, Atlas pide
   algunos DNG extra al archivo (prueba también la extracción) y decide si conservar DNG
   vale la pena o basta el JPG.

### Decisiones del usuario, desde nüün (2026-11-08)

5. **Proyecto por defecto de las misiones.** Una foto que coincide con un registro de
   misión toma el proyecto por defecto del flujo de misiones (para el usuario,
   `SUBAEREO`): `…_MAVICAIR2_MX-SUBAEREO[].JPG`. Las fotos de dron sin misión usan el
   proyecto que se elija en el ingreso.
6. **Export antes del plan.** Lo ideal es mandar el JSON a `~/Downloads` antes de revisar
   el plan, y que nüün lo detecte en ese momento para clasificar bien. Si al insertar la SD
   hay fotos de dron posteriores al último export registrado y parecen de misión por
   evidencia barata de EXIF/XMP (nadir, `GimbalPitchDegree` ≤ −80°, altura relativa
   constante, intervalos regulares), el Flujo sugiere «Probablemente de misión: falta el
   export de subaereo». Es una **sugerencia**, no una clasificación.
7. **Sin export, se archiva igual.** Las unidades que quedan `pending: export` se pueden
   archivar verificadas, para que dejen de ser copia única. Cuando llega el export, se
   resuelven: pasan de `fotos/` a `misiones/…` como un renombrado dentro del archivo, sin
   copiar otra vez. La SD no se libera mientras falte (A14).
8. **Nada automático por ahora.** Ninguna copia empieza sola mientras los flujos no estén
   identificados y probados; siempre hay un plan revisado. Automatizar los flujos que se
   puedan automatizar queda como prioridad futura de nüün ([PLAN](../../PLAN.md)).
9. **Recetas, no código fijo.** Dispositivos, categorías, carpetas, proyecto por defecto,
   tolerancias de misión (±2 s, ≤ 40 m) y patrones de exports viven como datos de una
   **receta**, con la del usuario (`nu9ve-default`) como plantilla inicial. Otros usuarios
   no tendrán subaereo ni esos nombres; la receta se podrá editar por nodos después.
10. **DNG en trabajo, sólo una muestra (2026-11-08).** Sustituye «los DNG que ya están en el ESSD se quedan como
   semilla». El ESSD conserva los DNG de las fotos etiquetadas por Atlas, de la sesión más reciente de cada
   iluminación, de una sesión mala (para medir si el DNG rescata) y de una misión vieja buena; el resto se libera
   tras verificarlo contra MEDIA502. `units.ndjson` (`work.dng`) dice dónde está cada DNG en trabajo. Atlas puede
   pedir otros con A16 (`dng_plan` con `keep`).

### Layout (propuesta concreta)

| Destino | Ruta | Contenido |
| --- | --- | --- |
| Archivo, misiones | `001_raw/dron/misiones/AAAA/MM/<YYMONDD-HHMMSS>_<PLAN>/` | Fotos ligadas a una misión. `<YYMONDD-HHMMSS>` = primera foto de la sesión; `<PLAN>` = slug del nombre del plan (`CALLESHUECOS-45M`). JPG y DNG **en la misma carpeta y con el mismo stem** |
| Archivo, otras fotos de dron | `001_raw/dron/fotos/AAAA/MM/` | Fotos del dron sin registro en subaereo (manuales, otras apps) |
| Archivo, video de dron | `001_raw/vdron/AAAA/MM/` | Sin cambio |
| Archivo, soporte | `001_raw/metadata/dji/support/AAAA/MM/<…>/MISC/…` | Sin cambio (`dji.gis` se puede omitir) |
| Trabajo (Atlas) | `ESSD/001_proyectos/atlas-subaereo/misiones/AAAA/MM/<YYMONDD-HHMMSS>_<PLAN>/` | Sólo JPG de misiones; DNG por pedido (A16). Las carpetas `101MEDIA/`, `102MEDIA/` existentes no se tocan |
| Mapa para Atlas | `ESSD/001_proyectos/atlas-subaereo/_nuun/units.ndjson` | Una línea por unidad: `asset_id`, rutas de archivo y trabajo, `original_name`, `dcim_folder`, `card_uuid`, `captured_at` (+ UTC), `make`, `model`, `serial`, SHA-256 de JPG y DNG, y `mission` (`planID`, `sessionID`, `captureID`) o `null` |

Nombre de cada foto: `YYMONDD-HHMMSS_MAVICAIR2_<PAIS>-<PROYECTO>[].JPG|.DNG` (fotos sin
duración; `-N` si dos caen en el mismo segundo). `FUENTE` sale del registro de
dispositivos (`FC3170` → `MAVICAIR2`).

**Misión o no:** una foto es de misión si coincide con un registro de `captureLog` del
registro unificado: misma hora (`mediaCreatedAt` = EXIF `DateTimeOriginal`, ±2 s),
mismo nombre de la SD y posición a ≤ 40 m (la regla de
`subaereo/tools/subaereo_ingest.py`). Sin export que cubra la fecha, la unidad queda
`pending: export` en vez de adivinar.

### Pedidos en orden

| # | Pedido | Por qué ahora |
| --- | --- | --- |
| A11 | **Archivar lo que es copia única**, con el layout de arriba: (a) los 534 pares de `ESSD/atlas-subaereo/101MEDIA` (30 may – 3 jun); (b) los 770 pares nuevos de la SD (6–7 nov). JPG+DNG, SHA-256 verificado | Pérdida de un disco = pérdida de esos vuelos |
| A12 | **Trabajo sólo JPG** de las misiones que faltan en el ESSD (~4 GB) y `units.ndjson` con el mapa de todo lo ingresado. Deduplicar contra lo presente por hora + serial + nombre original o por SHA, nunca por nombre solo | Atlas no analiza desde la SD; el ESSD está al 92 % |
| A13 | **Registro unificado de misiones.** Vigilar `~/Downloads` por `coverage-*.json` y `JSON-*.json` (validar `schemaVersion` y listas). Unir en `004_datos/subaereo/coverage/history.json` con las reglas del export: upsert por `exportWindow.mergeKeys`, gana el `exportedAt` más nuevo, procedencia por registro, tombstones cuando la app los publique. Agregar `changes.ndjson` (cuándo, qué export, lista, id, `new`/`updated`). Guardar el crudo comprimido en `004_datos/subaereo/coverage/raw/AAAA/MM/` (unos MB; permite rehacer la unión si hay un error) y **después** borrarlo de Descargas. Espejo en `ESSD/…/atlas-subaereo/_missions/` (`history.json` + `changes.ndjson`) para que Atlas lo lea sin MEDIA montado. Importar también una vez los 20 JSON de `atlas/coverage_exports/` | Hoy hay exports completos que repiten los mismos puntos y archivos sueltos en dos lugares |
| A14 | **Liberar la SD sólo si** (1) el archivo JPG+DNG está verificado y (2) el registro unificado cubre la última `DateTimeOriginal` de la tarjeta. Si falta (2): `pending: export`. Formatear desde el dron (`camera_action_required`) | La SD es la única fuente de las fotos y el export sólo vive en el iPhone hasta compartirse |
| A15 | **Identidad por evidencia**: tarjeta = UUID del volumen (el nombre `MAVIC200` puede cambiar); dispositivo = `Make+Model+Serial`; dron contra 360/acción por XMP de vuelo. `100CANON`, `CANONMSC` vacías y `LOST.DIR` no clasifican | El nombre es de la tarjeta, no del dron; la cámara 360 también es DJI |
| A16 | **Extracción de DNG a pedido** (después de A11–A15): Atlas entrega una lista de `asset_id` (fotos etiquetadas, mejores misiones recientes, crepúsculo profundo) y nüün copia esos DNG del archivo al trabajo, junto a su JPG, con recibo | Probar la extracción desde el archivo y medir si el DNG aporta |

### Registro de misiones en nüün (A13)

En el sandbox de macOS, Rust ejecuta A13 y A16. A13 verifica el export descomprimido y la fuente antes de limpiar el buzón. A16 confirma SHA-256 y el stamp del respaldo antes de mover el DNG a la Papelera. El diario permite reanudar entre el renombre y el registro del evento, verificando el archivo en la Papelera. Los resultados de ambos flujos se comparan con Python en fixtures temporales.


Implementado el 2026-11-08. Desde entonces `atlas/coverage_exports/` ya no existe (se movió a la Papelera a pedido del usuario): Atlas debe leer `_missions/history.json` ([pedido](next-sessions.md#pedido-para-atlas--2026-11-08--leer-misiones-del-registro-de-nüün)). Primera importación real el mismo día: los 20 exports de `atlas/coverage_exports` (no se borraron) y 4 de Descargas, 22 fuentes y 65 planes; registro y espejo verificados ([evidencia](../releases/status-log-2026-10-25_2026-11-08.md#dron-para-atlas-paso-2-registro-unificado-de-misiones--2026-11-08)). Rutas, patrones y reglas salen
del flujo `missions` de la receta (`media/recipes.py`).

| Lugar | Ruta |
| --- | --- |
| Archivo | `MEDIA502/004_datos/subaereo/coverage/history.json`, `changes.ndjson`, `raw/AAAA/MM/<export>.gz` |
| Espejo para Atlas | `ESSD/001_proyectos/atlas-subaereo/_missions/history.json` y `changes.ndjson`, byte a byte iguales al archivo |

`history.json` tiene la misma forma que la salida de `subaereo_merge_exports.py`: `schemaVersion`, las listas por
`mergeKeys`, `exportWindow.kind = "merged"` y `mergedFrom` con `sha256`. Además trae un bloque aditivo `nuun`:

```json
{"nuun": {"schema": "nu9ve-mission-history/1", "flow": "subaereo", "changes_count": 3,
  "sources": {"<sha256>": {"file": "JSON-24h.json", "exportedAt": "2026-11-08T08:00:00Z", "kind": "24h",
              "raw": "004_datos/subaereo/coverage/raw/2026/11/JSON-24h.json.gz", "changes": {"new": 1, "updated": 2, "deleted": 0}}},
  "records": {"captureLog": {"C3": "<sha256>"}}, "tombstones": {"plans": {}}}}
```

`changes.ndjson`, una línea por registro nuevo, actualizado o borrado. Atlas puede leer sólo las líneas
posteriores a la última que procesó:

```json
{"at":"2026-11-08T20:01:02Z","flow":"subaereo","list":"captureLog","id":"plan-9E4E…","change":"new","export":"JSON-24h.json","export_sha256":"…","exportedAt":"2026-11-08T08:00:00Z"}
```

Gana el `exportedAt` más nuevo aunque el export viejo llegue después. Un tombstone (`deletedPlanIDs`, cuando subaereo
lo publique) borra el plan y un export más viejo no lo revive. Los archivos fuera de `~/Downloads` (por ejemplo
`atlas/coverage_exports/`) se incorporan pero nunca se borran.

### `units.ndjson` (A12)

`ESSD/001_proyectos/atlas-subaereo/_nuun/units.ndjson`: una línea por unidad de dron, reemplazada por `asset_id`
en cada ingreso y ordenada por `captured_at`. Las rutas de `work` son relativas a `atlas-subaereo/`; las de
`archive`, a la raíz del volumen.

```json
{"schema":"nu9ve-drone-unit/1","asset_id":"…","job_id":"…","project":"subaereo",
 "archive":{"volume":"MEDIA502","jpg":"001_raw/dron/misiones/2026/11/26NOV07-153422_CALLESHUECOS-45M-1107-1533/26NOV07-154516_MAVICAIR2_MX-SUBAEREO[].JPG","dng":"…[].DNG"},
 "work":{"volume":"ESSD","jpg":"misiones/2026/11/26NOV07-153422_CALLESHUECOS-45M-1107-1533/26NOV07-154516_MAVICAIR2_MX-SUBAEREO[].JPG","dng":null},
 "original_name":"DJI_0001.JPG","dcim_folder":"103MEDIA","card_uuid":"8486AB6C-…",
 "captured_at":"2026-11-07T15:45:16","captured_at_utc":"2026-11-07T22:45:16Z","utc_basis":"host_zone",
 "make":"DJI","model":"FC3170","serial":"1TCLH9N03BJ5N9","sha256":{"jpg":"…","dng":"…"},
 "gps":{"lat":29.09,"lon":-110.96,"alt":43.8},"flight":{"gimbal_pitch":-89.9,"relative_altitude":43.8},
 "mission":{"planID":"…","sessionID":"…","captureID":"…"},"mission_state":"matched","plan_name":"CallesHuecos-45m-1107-1533"}
```

`mission_state` puede ser `matched`, `none` o `pending_export`. En `pending_export`, `mission` es `null` y el JPG
no está todavía en el trabajo. `utc_basis: host_zone` significa que el EXIF de DJI no trae zona horaria y se usó la del Mac.

### Estado del lado nüün · 2026-11-08

| Pedido | Estado | Evidencia |
| --- | --- | --- |
| A15/A1 identidad | Hecho: EXIF `Make+Model+Serial`, XMP de vuelo, receta `nu9ve-default`, `device-registry` | [STATUS](../releases/status-log-2026-10-25_2026-11-08.md#dron-para-atlas-paso-1-registro-de-dispositivos-y-receta--2026-11-08) |
| A13 registro de misiones | Hecho y aplicado: 22 exports, 65 planes, espejo `_missions/`; Inicio avisa cuando cae un JSON en Descargas | [STATUS](../releases/status-log-2026-10-25_2026-11-08.md#dron-para-atlas-paso-2-registro-unificado-de-misiones--2026-11-08) |
| A12 misión o no + trabajo JPG + `units.ndjson` | Hecho y aplicado: SD `MAVIC200` (job `cc8bd128`), 1,276 de misión en 25 sesiones | [STATUS](../releases/status-log-2026-10-25_2026-11-08.md#primer-ingreso-real-de-dron-aplicado-mavic200--2026-11-08) |
| A11 copias únicas | Hecho: SD (6–7 nov) y `ESSD/101MEDIA` mayo–junio (job `77bdcb73`, 534 pares) | [STATUS](../releases/status-log-2026-10-25_2026-11-08.md#a11-aplicado-mayojunio-del-essd-respaldado-en-media502--2026-11-08) |
| A14 liberar la SD | Hecho: SD vacía tras verificar; un plan con limpieza se rechaza si alguna foto espera export (`mission_export_missing`). Falta formatear desde el dron | idem |
| A16 DNG a pedido | Hecho: muestra de 128 en trabajo y 999 liberados tras verificar (`dng-d9549817`); `dng-plan` con `keep` para pedir más | [STATUS](../releases/status-log-2026-10-25_2026-11-08.md#dng-del-essd-muestra-conservada-y-el-resto-a-la-papelera-a16--2026-11-08) |

Pendiente del lado nüün:
- **Reclasificar al llegar el export:** pasar `pending_export` → `misiones/` renombrando dentro del archivo. Hoy sólo puede pasar con la limpieza apagada.
- **Confirmar un dispositivo nuevo desde la app.**
- **Interfaz para el plan de DNG.** Hoy es CLI/servicio.

### Retención de lo que produce Atlas

| Clase | Qué | Respaldo |
| --- | --- | --- |
| Estado humano | `atlas-derived/atlas_state.sqlite3`, `labels/cells/*/manifest.json` | Siempre (A6); irreemplazable |
| Productos caros | Ortofotos ODM + `opensfm/reconstruction.json` (A9), `surface/*/session.json`, `traces/`, `ortho-cells.npz`, `reports/*/report.json` | Sí: días de cómputo, pocos GB |
| Regenerable | Miniaturas, recortes de etiquetas, `frames/`, mosaicos, tiles, intermedios ODM, `surface/_backup-*` | No; Atlas los borra (`odm clean`, limpieza por versión) |

## Clasificación DJI: dron frente a 360/acción

Regla propuesta, por archivo/unidad y con evidencia, nunca por marca ni por
carpeta residual:

| Evidencia | Categoría | Fuente (`FUENTE` de naming) |
| --- | --- | --- |
| `Make=DJI` y `Model=FC3170` | `dji-drone` (Mavic Air 2) | `MAVICAIR2` (histórico de video: `MAVIC2`; decidir uno y conservar el otro como alias) |
| `Make=DJI`, otro `Model` con XMP `drone-dji:GimbalPitchDegree` / `FlightYawDegree` / `RelativeAltitude` | `dji-drone`, modelo nuevo → pedir confirmación una vez y registrar `Model→categoría` | `Model` en mayúsculas |
| `Make=DJI` sin XMP de vuelo, nombres `CAM_*`/`.OSV`/`.INSV`, o proyección equirectangular | `dji-360` | según modelo |
| `Make=DJI` sin XMP de vuelo, video de acción `DJI_YYYYMMDDHHMMSS_NNNN_D.MP4` | `dji-action` | según modelo |
| Árbol `MISC/GIS`, `MISC/THM`, `MISC/IDX` junto a `DCIM/1xxMEDIA` | pista de tarjeta de dron (`hint`), **no** clasificación | — |

La identidad del dispositivo es `Make + Model + SerialNumber`. Con el registro
de dispositivos pendiente en [PLAN](../../PLAN.md), basta guardar la decisión
confirmada `FC3170/1TCLH9N03BJ5N9 → dji-drone` para no volver a preguntar.
`ExifTool -fast2` sobre 1,106 JPG tarda ~6 s: leer EXIF para clasificar es barato.

## Unidad de ingreso: captura de dron

- Una **unidad** = `JPG + DNG` con el mismo stem **en la misma carpeta DCIM**. Nunca
  emparejar entre carpetas. Un JPG sin DNG o un DNG sin JPG se ingresa igual y se
  reporta.
- Agrupación sugerida: **sesión de vuelo** = fotos consecutivas con huecos
  `< 8 min` en `DateTimeOriginal`. En noviembre eso dio 8 sesiones claras (p. ej.
  `2026-11-05 17:33–17:51`, 82 fotos a 45 m). Si existe export de subaereo, su
  `sessionID`/`planID` manda sobre la heurística.
- Videos del dron (`.MP4`) siguen el flujo `vdron` existente; este contrato cubre fotos.

## Rutas y nombres: Atlas conserva los originales

> **Sustituido el 2026-11-08** por [decisiones del usuario y layout](#decisiones-del-usuario-2026-11-08): nombres de nüün, carpeta de dron con misiones separadas, nombre original como metadato. Se conserva como historia.

Atlas, subaereo y ODM referencian `DJI_NNNN.JPG`. El naming v1
(`YYMONDD-HHMMSS_FUENTE_…[]`) **no** se aplica a fotos de Atlas:

| Destino | Ruta propuesta | Contenido |
| --- | --- | --- |
| Archivo MEDIA | `001_raw/atlas/AAAA/MM/<YYMMDD>_<SERIAL6>/<DCIM folder>/DJI_NNNN.{JPG,DNG}` | Ambos formatos, árbol DCIM original. `<SERIAL6>` = últimos 6 caracteres del serial (`1TCLH9N03BJ5N9` → `3BJ5N9`); evita mezclar dos drones. |
| Archivo MEDIA, soporte | `001_raw/metadata/dji/support/AAAA/MM/<YYMMDD-HHMMSS>_<SERIAL6>/MISC/…` | `MISC/THM`, `MISC/IDX`. `dji.gis` se puede omitir (es base de geocercas de DJI, reproducible, 43 MB). |
| Trabajo ESSD (Atlas) | `001_proyectos/atlas-subaereo/<YYMMDD>_<SERIAL6>/<DCIM folder>/DJI_NNNN.JPG` (+ DNG según política) | Atlas lee recursivo desde la raíz configurada. Las carpetas `101MEDIA/`, `102MEDIA/` ya existentes se dejan como están; no hacer `relayout`. |
| Exports subaereo | `atlas/coverage_exports/*.json` (repo de Atlas) | Llegan por AirDrop a `~/Downloads`; nüün puede ofrecer moverlos junto con el ingreso de la SD del mismo día. |

Atlas tolera cualquier profundidad bajo la raíz; lo que no tolera es renombrar
archivos o separar JPG/DNG en carpetas distintas (`images/` vs `rimages/` del
layout actual rompe el par).

## Política JPG / DNG (actualizada 2026-11-06)

Respuestas cortas a las preguntas del usuario:

- **¿Hacen falta los DNG?** Para el análisis de hoy, no: Atlas, ODM y el etiquetado
  leen JPG. Para mejorar el análisis, sí: el DNG (16 bit, sin nitidez ni reducción de
  ruido de DJI) es lo único que permite revelar de nuevo crepúsculo profundo y noche
  con menos ruido, que es donde se pierden grietas finas. Se archivan siempre.
- **¿Se puede sacar el JPG del DNG?** Técnicamente sí (`rawpy`/`dcraw`), pero no es el
  mismo JPG de DJI: otra curva, otro color, otra nitidez. Cambia las huellas de ODM y
  los resultados de `surface`. Por eso **nunca se conserva sólo el DNG**.
- **¿Se puede conservar sólo el JPG?** En trabajo, sí. En archivo, no: se perdería el
  margen de mejora nocturno.

| Lugar | JPG | DNG | Notas |
| --- | --- | --- | --- |
| Archivo MEDIA | Siempre | Siempre | Réplica única aceptada ([política](../features/replication-policy.md)) |
| Trabajo ESSD (Atlas) | Siempre, sólo fotos útiles | Sólo sesiones que Atlas pida (A3) | Hoy Atlas no lee DNG |
| Fotos no útiles (horizonte, pruebas) | En `_no-nadir/` o `_pruebas/` del trabajo, o sólo en archivo | Sólo archivo | Atlas ignora carpetas que empiezan con `_` |

### Qué fotos sirven para qué (catálogo Atlas, 1,106 fotos)

| Tipo | Criterio | Fotos | JPG | DNG | En trabajo |
| --- | --- | ---: | ---: | ---: | --- |
| Nadir con buena luz | pitch ≤ −80°, `direct-sun` o `diffuse-dusk` | 462 | 2.9 GB | 11.5 GB | JPG (+ DNG si se pide) |
| Nadir con luz media | `low-sun`, `deep-dusk` | 147 | 0.8 GB | 3.6 GB | JPG; DNG útil en crepúsculo profundo |
| Nadir de noche | `artificial-night` | 421 | 1.8 GB | 10.3 GB | JPG (iluminación); DNG útil |
| No nadir | pitch > −80° (horizonte, oblicuas; p. ej. las 65 de `Calles-45m-1104-2225`) | 76 | 0.4 GB | 1.9 GB | `_no-nadir/` o sólo archivo |

La clasificación sale de EXIF (`GimbalPitchDegree`, exposición, ISO) y nüün puede
calcularla sin visión: `ev100 = log2(N²/t) − log2(ISO/100)`; bandas en
`atlas/docs/surface-pipeline.md` §1. Con esta política el working set baja de 33 GB a
~6 GB de JPG útiles más los DNG que Atlas pida.

### Qué respaldar de Atlas (además de los originales)

| Qué | Tamaño | Por qué |
| --- | ---: | --- |
| `atlas-derived/atlas_state.sqlite3` | < 1 MB | Revisiones y etiquetas de celdas: irreemplazable |
| `atlas/coverage_exports/` | ~8 MB | Exports de subaereo; también viven en el iPhone sólo hasta borrarlos |
| `atlas-derived/odm/projects/*/odm_orthophoto/odm_orthophoto.tif` + `opensfm/reconstruction.json` | ~1.5 GB | Recalcular cuesta 10–40 min por sección con Docker |
| `atlas-derived/labels/cells/*/manifest.json` | KB | Define los sets etiquetados (los recortes se regeneran) |
| Resto de `atlas-derived/` | decenas de GB | Regenerable: no respaldar |

Limpieza hecha el 2026-11-06: `atlas_vision odm clean --all --apply` borró 38.4 GB de
intermedios ODM regenerables (ESSD de 95 % a 87 %, 63 GB libres).

## Liberación de la SD

`save` completo para dron: copiar → verificar archivo (MEDIA, JPG+DNG) y trabajo
(ESSD, JPG + DNG solicitados) → limpieza journalizada de `DCIM/1xxMEDIA`. Reglas
específicas:

- No borrar `MISC/` ni `DCIM/` vacíos: el dron recrea su índice; formatear desde el
  dron (como Sony) es lo recomendado. Estado final sugerido `camera_action_required`
  con texto «formatear desde DJI Fly / control».
- No liberar si el export de subaereo del día no se ha copiado todavía **y** la
  ingesta lo esperaba (opción de perfil): la SD es la única fuente de las fotos,
  pero el JSON sólo vive en el iPhone hasta que se exporta.
- La herramienta de subaereo `tools/subaereo_ingest.py` también copia y nunca borra;
  dos herramientas no deben limpiar la misma tarjeta. Propuesta: nüün es el único
  que libera; la ingesta de subaereo queda para su dataset de análisis o se
  sustituye por este flujo.

## Pedidos de Atlas a nüün (para tabla de pendientes)

| # | Pedido | Motivo |
| --- | --- | --- |
| A1 | Categoría `dji-drone` por EXIF `Model`/XMP de vuelo, con registro de `Make+Model+Serial` confirmado. | Diferenciar dron de 360/acción sin preguntar cada vez. |
| A2 | Perfil «Atlas» con layout que conserva nombres y árbol DCIM; JPG y DNG en la misma carpeta. | Atlas, subaereo y ODM referencian `DJI_NNNN`. |
| A3 | Copia de trabajo parcial: JPG siempre, DNG sólo para sesiones solicitadas. | ESSD al 91 %; Atlas no usa DNG todavía. |
| A4 | Consulta read-only `sources --project atlas --purpose edit` que devuelva unidades con `{asset_id, jpg, dng, dcim_folder, captured_at, captured_at_utc, model, serial, session_hint}`. | Atlas podría resolver rutas por asset sin depender del symlink `images/`. |
| A5 | Mover/copiar el export JSON de subaereo del mismo día junto con la ingesta (opcional, sugerencia confirmable). | Hoy llegan sueltos a `~/Downloads`. |
| A6 | Incluir `atlas_state.sqlite3` en respaldos de estado humano. | Revisiones y etiquetas no se regeneran. |
| A7 | Lease/consumidor: no liberar trabajo de una sesión que Atlas tenga en un proyecto ODM activo (`atlas-derived/odm/manifest.json`, estado `running`). | Evitar borrar insumos durante horas de ODM. |
| A8 | Al copiar a trabajo, poner las fotos no nadir (pitch > −80°) en `<sesión>/_no-nadir/` (o dejarlas sólo en archivo). | Atlas las ignora; quedan a mano para borrarlas o usarlas en otra cosa. |
| A9 | Respaldar ortofotos ODM + `reconstruction.json` de Atlas (tabla de arriba) como «productos caros», separados de derivados regenerables. | Evitar horas de recálculo tras un fallo de disco. |
| A10 | Mostrar el espacio del ESSD y sugerir `atlas_vision odm clean --all` cuando pase del 90 %. | El ESSD llegó a 95 % con proyectos ODM. |

## Lo que Atlas aporta de vuelta

- Puede publicar por sesión `light_class` medida desde EXIF (`ev100`:
  `direct-sun`, `low-sun`, `diffuse-dusk`, `deep-dusk`, `artificial-night`) y
  qué sesiones necesitan DNG (crepúsculo profundo y noche). Nüün puede leerlo para
  la política A3 sin interpretar visión.
- Atlas no escribe el catálogo de nüün ni sus recibos; nüün no reescribe GPS,
  vuelo, calidad ni fotogrametría.

## Adopción

| App | Estado | Fecha |
| --- | --- | --- |
| Atlas | Catálogo v2 recursivo con IDs estables; lee cualquier árbol bajo la raíz; `ev100`/`exposure_light_class` por foto. | 2026-11-05, working tree sobre `ba60ffa` |
| Atlas | Ignora carpetas `_*` (para A8); `odm clean` para intermedios; docs de respaldo. | 2026-11-06, working tree sobre `0e4d0d2` |
| nüün | **A16 (muestra de DNG):** 128 DNG en trabajo (etiquetadas, una sesión por iluminación, malas, una vieja) y 999 liberados tras verificar contra MEDIA502 (operación `dng-d9549817`) | 2026-11-08 · working tree |
| nüün | **A11 aplicado:** 534 pares de mayo–junio de `ESSD/atlas-subaereo/101MEDIA` respaldados en MEDIA502 (job `77bdcb73`, 16.31 GB), sin tocar el ESSD; `units.ndjson` con 1,810 líneas | 2026-11-08 · build `e799494274d5` |
| nüün | **Primer ingreso real:** SD `MAVIC200`, job `cc8bd128`. 1,276 pares de misión en `dron/misiones` (25 sesiones), 769 JPG nuevos en `atlas-subaereo/misiones`, 507 ya presentes, `units.ndjson` con 1,276 líneas, SD liberada | 2026-11-08 · build `0e06154caaf5` |
| nüün | **A12 y UI:** misión o no al planear (nombre + hora ±2 s/±8 s + ≤ 40 m), `dron/misiones|fotos`, trabajo sólo JPG, `units.ndjson`, sección en Inicio y aviso en la revisión del plan. Fixtures y planes de sólo lectura; instalado en `a8a92513f456` (descartar, «parece de misión» sin registro, revisión en tabla), con trabajo sin duplicar lo que ya está en `atlas-subaereo` (por nombre, tamaño, hora EXIF y serial); falta la aceptación desde la app | 2026-11-08 · working tree |
| nüün | **A13:** registro de misiones con espejo (`missions-review`/`missions-incorporate`/`missions-status`), primera importación real (22 exports) verificada | 2026-11-08 · working tree sobre `3998dc9`, sin commit |
| nüün | **A15/A1:** identidad de dispositivo por EXIF `Make+Model+Serial` (lector propio, sin dependencias), XMP de vuelo para dron frente a 360/acción, receta `nu9ve-default` como plantilla inicial (`FC3170` → `dji-drone`, `MAVICAIR2`) y registro `device-registry` (`devices`, `confirm-device`). Fotos de dron: nombre de nüün con la hora EXIF y `dron/fotos/AAAA/MM/` con JPG y DNG juntos. Tarjeta: ya se identifica por UUID del volumen (`source_identity`). Sólo fixtures; A11–A14 y A16 pendientes | 2026-11-08 · `3998dc9` |
| subaereo | Ver contrato de captura en Atlas. Exports 24 h / 7 d / todo con `mergeKeys` (8 nov) | 2026-11-08 · subaereo `e4894e1` |
| Atlas | Inventario SD/ESSD/MEDIA; pedidos A11–A16 con las decisiones del usuario (nombres de nüün, misiones separadas, registro unificado de exports, trabajo sólo JPG, DNG a pedido). Atlas adaptará su catálogo para identificar por hora EXIF + serial y leer `units.ndjson` y `_missions/` | 2026-11-08 · atlas sobre `10ba32a` |
