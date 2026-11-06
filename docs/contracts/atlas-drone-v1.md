# Fotos de dron para Atlas · v1 (propuesta)

Estado: **propuesta 2026-11-05**, escrita desde una sesión de Atlas
(`/Users/nu9ve/code/data/atlas`, working tree sobre `ba60ffa`) tras inventariar la
SD `MAVIC200`, el ESSD y MEDIA502. Nüün no la ha implementado. Atlas es
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
| nüün | Sin iniciar. | — |
| subaereo | Ver contrato de captura en Atlas. | — |
