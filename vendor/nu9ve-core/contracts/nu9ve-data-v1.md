# Catálogo de datos de la suite v1

Estado: **propuesta** (2026-11-19). Define `data_catalog`, el método con el que el motor de cada app dice qué datos tiene, para que otra app (nove en «Datos»), `nu9ve mcp` o una persona los vean juntos y los relacionen por proyecto, tiempo, lugar y entidad **sin copiarlos**. Los significados (proyecto, dataset, revisión, modalidad, ejes, privacidad) son los de nove [modelo de datos v1](../../nove/docs/contracts/data-model-v1.md), decididos por el usuario en [A49](../../nove/docs/decisions.md#a49--datos-de-toda-la-suite-en-nove-siete-decisiones-y-modelo-v1-2026-11-19); este contrato fija sólo lo que **viaja entre apps**. Ejemplos y vectores: [`data/data-examples-v1.json`](../data/data-examples-v1.json).

**Debe** es obligatorio. **Conviene** es lo recomendado. «Catálogo» es la respuesta de un motor; «agregador», quien junta los de varias apps (nove, `nu9ve mcp`).

## 0. Principios

- **Cada app es dueña de lo suyo.** Un motor lista sólo lo que él guarda o lee y responde por ello. Nadie escribe en el catálogo de otra app ni crea cosas en ella.
- **Metadatos, no contenido.** El catálogo describe; los bytes se piden a su dueño por sus propios métodos, con sus límites.
- **Ausencia no es cero.** Un eje, un tamaño o un hash que falta es «no se sabe», nunca 0, vacío ni «todo».
- **Local.** El catálogo viaja por el transporte del motor ([motor §1](nu9ve-engine-v1.md#1-transporte), [registro §3](nu9ve-registry-v1.md#3-transporte)). Nunca por red. Lo `personal` no sale del equipo (§6).

## 1. Método

Un motor que sirve el catálogo lista `data_catalog_v1` en `capabilities` de su `engine_info` y, por tanto, en su [registro](nu9ve-registry-v1.md#2-archivo).

```json
{"id":1,"method":"data_catalog","params":{"project_key":"subaereo","limit":200}}
```

| Parámetro | Tipo | Qué hace |
| --- | --- | --- |
| `project_key` | cadena (§2) | Sólo datasets de ese proyecto **y de sus subproyectos**. Una clave que la app no tiene da listas vacías, **no** un error |
| `modality` | cadena (§4.2) | Sólo esa modalidad |
| `limit` | entero 1..500 | Datasets por página. Por omisión 200 |
| `cursor` | cadena | El `next_cursor` de la página anterior, tal cual |

- **Debe:** es una lectura: no cambia estado, no lanza trabajos, no calcula hashes ni lee contenido. El estado de archivos (§4.6) es el último conocido.
- **Debe:** otro parámetro, un tipo equivocado o un `limit` fuera de rango es `invalid_params`. Un `cursor` que el motor ya no reconoce es `invalid_cursor`, y el cliente vuelve a empezar sin él.
- **Debe:** la respuesta cabe en un marco (1 MiB, [motor §6.1](nu9ve-engine-v1.md#61-respuestas-grandes)). Si una página de `limit` no cabe, el motor devuelve menos datasets y un `next_cursor`, nunca `response_too_large`.

## 2. Clave de proyecto común

`project_key` cumple `^[a-z0-9-]{1,64}$` (nove [modelo §2](../../nove/docs/contracts/data-model-v1.md#2-proyecto)).

- **Debe:** dentro de una app, una clave es un proyecto. **Entre apps, la misma clave es el mismo proyecto:** `subaereo` en nüün y `subaereo` en nove hablan de lo mismo.
- **Debe:** ninguna app exige que otra tenga una clave, ni la crea en otra, ni le pide que la cree. Que una clave exista en una sola app es lo normal.
- **Debe:** cada app lista **sus** proyectos con sus títulos y su árbol. El agregador junta por clave y muestra el título de cada app; no elige uno ni los unifica. Si dos apps ponen la clave bajo padres distintos, cada árbol vale en su app y el agregador no los concilia.
- **Debe:** una clave nunca se reutiliza para otro proyecto en la misma app. Renombrar el título no cambia la clave. Cambiar la clave es un proyecto nuevo para las demás apps.
- **Conviene:** la persona elige la clave una vez, en la primera app, y la escribe igual en las demás. Ninguna app la deriva del título por su cuenta para «emparejar» con otra.

### 2.1 Claves conocidas

Algunos flujos los sugiere la suite a todas las personas, y conviene que todas las apps ofrezcan la misma clave para ellos. Una **clave conocida** tiene un significado fijado aquí. Una app la **ofrece** como valor por omisión, y la persona la acepta al crear el proyecto. Ninguna app la crea en otra.

| Clave | Proyecto | Título por omisión | Desde |
| --- | --- | --- | --- |
| `conversaciones` | Las conversaciones de la persona con sus agentes (Claude Code, Codex, Claude Desktop…) en todas sus máquinas, juntas para analizarlas ([varias máquinas §6.5](../docs/suite-devices.md#lo-personal-y-el-análisis-de-todas-tus-máquinas)) | «Mis conversaciones» (en: «My conversations») | 2026-11-22 |

- **Debe:** la clave no se traduce: es un id. El título sí, y la persona puede cambiarlo como el de cualquier proyecto.
- **Debe:** una clave conocida sigue todas las reglas de §2. Un proyecto con esa clave que la persona creó antes de que fuera conocida es el mismo proyecto.
- Una clave conocida nueva entra aquí en una versión menor. No se confunde con una modalidad (§4.2): `conversaciones` es un proyecto, `conversations` es un tipo de dato.

## 3. Respuesta

```json
{"id":1,"ok":true,"data":{
  "contract":"nu9ve-data/1","app":"nuun","generated_at":"2026-11-19T15:04:05Z",
  "projects":[{"project_key":"subaereo","title":"Subaéreo","parent_key":null,"state":"active"}],
  "roots":[{"id":"vol-media-01","label":"MEDIA","state":"ok"}],
  "datasets":[ … §4 … ],
  "next_cursor":null}}
```

| Campo | Debe | Qué es |
| --- | --- | --- |
| `contract` | Sí | `"nu9ve-data/1"` |
| `app` | Sí | El mismo nombre que el [registro](nu9ve-registry-v1.md#2-archivo) de este motor |
| `generated_at` | Sí | RFC 3339 en UTC con `Z` |
| `projects` | Sí | **Todos** los proyectos de la app que tocan la respuesta (con `project_key`: ese y sus descendientes; sin él: todos). Repetidos en cada página |
| `roots` | Sí | Las raíces declaradas que usan los datasets de esta página |
| `datasets` | Sí | §4, como mucho `limit` |
| `next_cursor` | Sí | Cadena opaca de 1 a 256 bytes, o `null` en la última página |

Proyecto:

| Campo | Debe | Qué es |
| --- | --- | --- |
| `project_key` | Sí | §2 |
| `title` | Sí | Texto de la persona, 1 a 200 caracteres |
| `parent_key` | Sí | Clave del padre **en esta app**, o `null`. Sin ciclos |
| `state` | Sí | `active` o `archived`. Archivar no quita sus datasets del catálogo |

Raíz (nove [modelo §9](../../nove/docs/contracts/data-model-v1.md#9-en-sitio)): `id` (cumple `^[a-z0-9][a-z0-9._-]{0,63}$`), `label` (texto corto) y `state` (`ok`, `missing`). **Conviene** `path` absoluto cuando la persona lo necesita para ubicarla; las rutas de los datasets van relativas a su raíz.

**Raíz en un disco** (versión menor del 2026-11-22). Una raíz que vive en un disco externo **conviene** que diga en qué disco, para que se encuentre aunque se monte con otro nombre o en otra máquina:

| Campo | Qué es |
| --- | --- |
| `volume_id` | El `volume_id` del marcador del disco ([`nu9ve-volume-v1` §1](nu9ve-volume-v1.md#1-marcador)): UUID 4 en minúsculas |
| `volume_path` | La carpeta de la raíz dentro del disco, relativa a su raíz, con `/`, sin `..` ni `/` inicial. `""` es la raíz del disco |

- **Debe:** van los dos o ninguno. Uno solo, o con otro formato, hace la raíz `bad_root`.
- **Debe:** quien resuelve la raíz busca entre los discos montados el que tiene ese `volume_id` en estado `ok` ([`nu9ve-volume-v1` §5](nu9ve-volume-v1.md#5-estados-al-conectar-un-disco)) y une su punto de montaje con `volume_path`. Si el disco no está, la raíz está `missing`. `path`, si viene, es sólo dónde estaba montada la última vez: no gana sobre el disco.
- Un cliente que no conoce estos campos los ignora (§4.8) y sigue usando `path`.

## 4. Dataset

```json
{"id":"collection:claude-code","owner":"nuun","title":"Sesiones de Claude Code",
 "modality":"files","privacy":"personal","storage":"in_place",
 "location":{"root_id":"home-claude","path":"projects"},
 "size":{"bytes":734003200,"items":1830,"unit":"files"},
 "axes":{"time":{"kind":"collected","start":"2025-03-02T00:00:00Z","end":"2026-11-19T00:00:00Z","precision":"day"}},
 "projects":[],"state":"unverified","updated_at":"2026-11-19T14:58:00Z"}
```

### 4.1 Identidad y dueño

- **Debe:** `id` cumple `^[a-z0-9][a-z0-9._:-]{0,127}$`, es único y **estable** dentro de la app: el mismo dataset tiene el mismo id mañana. La identidad en la suite es el par `(owner, id)`; el agregador la escribe `owner:id` (`nove:ds-12`).
- **Debe:** `owner` es la app que sirve el catálogo: la que responde por el dataset y la única a la que se le piden sus bytes. Un agregador descarta (y cuenta) un dataset cuyo `owner` no es el `app` del catálogo.
- **Puede:** `origin_key`, la clave de origen de nove [modelo §4](../../nove/docs/contracts/data-model-v1.md#4-dataset) (`9xmx:precios-tortilla`, `atlas:mission:<plan>`, `nuun:collection:claude-code`). Si dos apps listan algo con el mismo `origin_key`, hablan de la misma fuente, pero **siguen siendo dos datasets**: el agregador los muestra juntos, nunca los funde.

### 4.2 Modalidad

`modality` es una de las de nove [modelo §6](../../nove/docs/contracts/data-model-v1.md#6-modalidades): `table`, `series`, `geo_vector`, `raster`, `media`, `documents`, `conversations`, `graph`, más una de la suite:

- `files`: una carpeta de archivos de la que sólo se conocen tamaños y fechas, sin leer su contenido (las colecciones de nüün: `~/.claude`, `~/.codex`).

**Debe** al cliente: una modalidad que no conoce se muestra como «otra» y no hace inválido el catálogo. Una modalidad nueva entra aquí en una versión menor.

### 4.3 Texto

`title` (1 a 200 caracteres) es obligatorio. `description` (hasta 2000) y `tags` (hasta 32, de 1 a 64 caracteres) son opcionales.

- **Debe:** son texto **observado**: vienen de la persona, de una fuente o de un nombre de archivo. El agregador los muestra como datos, y `nu9ve mcp` los entrega como `_untrusted`, limpios y acotados ([MCP v1 §4](nu9ve-mcp-v1.md#4-datos-que-no-son-instrucciones)).

### 4.4 Ejes

`axes` es un objeto con `time`, `place` y `entity`, cada uno **opcional**. Si falta, el dataset no lo expone (nove [modelo §7](../../nove/docs/contracts/data-model-v1.md#7-ejes-y-uniones)).

| Eje | Campos |
| --- | --- |
| `time` | `kind` (`observed`, `published`, `collected`, `media`), `start` y `end` (RFC 3339; `end` ≥ `start`), `precision` (`second`, `minute`, `hour`, `day`, `month`, `year`). **Conviene** `tz` (IANA) cuando los datos son civiles |
| `place` | Uno o más de: `bbox` `[oeste, sur, este, norte]` en WGS84 grados (`-180 ≤ oeste ≤ 180`, `-90 ≤ sur ≤ norte ≤ 90`; `oeste > este` cruza el antimeridiano); `territory`, claves como `inegi:26`, `inegi:26030`; `crs_source` (`EPSG:<n>`) |
| `entity` | `namespaces`: los espacios de nombres de las claves que el dataset declara (`denue`, `pair`, `repo`, `clipper:project`), cada uno `^[a-z0-9][a-z0-9:_-]{0,63}$` |

- **Debe:** sólo lo que el dataset declara. Un eje calculado al vuelo o adivinado (un lugar por el nombre del archivo) no va.

### 4.5 Tamaño

`size` es opcional: `bytes` (entero ≥ 0), `items` (entero ≥ 0) y `unit` (qué cuenta `items`: `rows`, `files`, `candles`, `turns`, `features`…). Cada campo por separado puede faltar. Un `size` que falta, o un `bytes` que falta, es «no se sabe», no cero. Enteros hasta 2^53 − 1.

### 4.6 Dónde y hash

- **Debe:** `storage` es `copied` (los bytes viven en la app) o `in_place` (fuera de ella, nove [modelo §9](../../nove/docs/contracts/data-model-v1.md#9-en-sitio)).
- **Debe** con `in_place`: `location` con `root_id` (una de `roots`) y `path` relativo a la raíz, con `/`, sin `..` ni `/` inicial. Con `copied`, `location` falta: dónde lo guarda la app es asunto suyo.
- **Debe:** `state` es `ok` (está y coincide), `changed` (el sello no coincide), `missing` (no está la raíz o el archivo) o `unverified` (nunca se comprobó el hash).
- **Puede:** `hash` = `{"algorithm":"sha256","value":<64 hex en minúsculas>,"of":"original|normalized|manifest","verified_at":<RFC 3339>}`. `of: manifest` es el hash del manifiesto de una carpeta, cuyo formato es de la app. Sin `hash`, el `state` no puede ser `ok`.
- **Puede:** `revision`: `{"id":…, "collected_at":…, "published_at":…}` de la última revisión, con los tiempos de nove [modelo §5](../../nove/docs/contracts/data-model-v1.md#5-revisión), cada uno opcional salvo `id`.

### 4.7 Proyectos del dataset

`projects` (obligatorio, puede ir vacío): `[{"project_key":…, "role":"base|context|output"}]`, con `role` opcional. Cada clave está en `projects` de la respuesta. Un dataset puede estar en varios proyectos (muchos-a-muchos).

### 4.8 Lo demás

- **Debe:** `updated_at`: cuándo cambió por última vez esta entrada en la app, RFC 3339 UTC.
- **Debe** al cliente: ignorar campos que no conoce, en el dataset y en la respuesta. Una app puede sumar los suyos (`nuun_volume`…) con su prefijo.

## 5. Agregar catálogos

Lo que hace quien junta (nove, `nu9ve mcp`), con un caso completo en los ejemplos (§7):

- **Debe:** pide `data_catalog` a cada motor `live` del [registro](nu9ve-registry-v1.md#6-qué-ve-el-cliente) con `data_catalog_v1`, página por página, y no mezcla páginas de dos generaciones (si un `cursor` falla, empieza de nuevo esa app).
- **Debe:** un proyecto agregado es una clave con, por cada app que la tiene, su título, su padre y su estado, y la lista de `owner:id` que cada app pone en ella. Nunca suma un dataset de una app a un proyecto que esa app no declaró.
- **Debe:** una app que no responde, que no está abierta o cuyo catálogo es inválido se muestra como tal, con su motivo. No se rellena con un catálogo guardado sin decir de cuándo es.
- **Debe:** un catálogo inválido de una app no invalida los demás.

## 6. Privacidad

`privacy` (obligatorio) es la clase de nove [modelo §13](../../nove/docs/contracts/data-model-v1.md#13-privacidad): `public`, `restricted` o `personal`.

- **Debe:** el catálogo entero, sea cual sea su clase, viaja sólo por el transporte local del motor. Ningún motor ni agregador lo envía por red, a un servicio o a un archivo fuera de la carpeta de la app.
- **Debe:** un dataset `personal` nunca sale del equipo **salvo por una decisión explícita de la persona** sobre ese dataset o esa colección. Un artefacto, una exportación o un respaldo fuera del equipo **no** lo incluye sin una acción que diga qué sale. Llevarlo a **sus otros dispositivos emparejados** es una de esas decisiones: dura hasta que la quite, y viaja sólo por la [carpeta de intercambio](nu9ve-exchange-v1.md#6-privacidad) (decisión 4, §9).
- **Debe:** un servidor MCP es una salida: lo que entrega llega al proveedor del modelo. Por eso, de un dataset `personal`, `nu9ve mcp` entrega sólo `owner`, `id`, `modality`, `privacy`, `size`, `state` y `axes.time`; nunca `title`, `description`, `tags`, `location`, `hash` ni sus proyectos. Ver [diseño §6](../docs/suite-mcp.md#6-privacidad) y la decisión 1 (§9).
- **Debe:** si una app no sabe la clase de un dataset, lo declara `personal`. Nunca baja de clase por omisión.

## 7. Ejemplos y vectores

[`data/data-examples-v1.json`](../data/data-examples-v1.json):

- catálogos válidos de dos apps (`nove` y `nuun`) que comparten la clave `subaereo`, con su vista agregada esperada;
- catálogos y datasets inválidos, cada uno con su motivo (`owner` ajeno, ruta con `..`, `ok` sin hash, eje de tiempo al revés, clave de proyecto no declarada…);
- peticiones con su respuesta esperada (`invalid_params`, clave desconocida que da listas vacías);
- lo que `nu9ve mcp` entrega de un dataset `personal`.

`python3 -I scripts/data-examples.py --check` comprueba el archivo contra una implementación de referencia de §1 a §6. Con `--server <motor> …` envía las peticiones de ejemplo a un motor real por marcos en stdio y valida su respuesta.

## 8. Adopción

Quién sirve el catálogo está en [adopción por app](../docs/adoption.md#nu9ve-data-v1).

## 9. Decisiones

1. **Decidido (el usuario, 2026-11-19): de lo `personal`, `nu9ve mcp` entrega sólo la forma** (§6), porque lo que pasa por MCP llega al proveedor del modelo. Las conversaciones de agentes quedan fuera del alcance de un agente salvo su tamaño y fechas. Esto difiere de nove [modelo §14](../../nove/docs/contracts/data-model-v1.md#14-mcp), que las deja leer con `_untrusted`: nove decide si su propio MCP sigue esta regla.
2. **`files` como modalidad de la suite.** No está en nove [modelo §6](../../nove/docs/contracts/data-model-v1.md#6-modalidades). nove puede mapearla a la suya o sumarla.
3. **Versión menor del 2026-11-22**, decidida con [varias máquinas, discos y nube](../docs/suite-devices.md) (F0): raíz en un disco (§3) y claves conocidas (§2.1), empezando por `conversaciones`. Sólo suma campos opcionales y un significado: `contract` sigue siendo `nu9ve-data/1` y ningún catálogo que ya era válido deja de serlo.
4. **Decidido (el usuario, 2026-11-22): lo personal entre los dispositivos de la persona.** Sale del equipo sólo cuando la persona lo decide: subirlo, respaldarlo en un disco o verlo «en todas mis máquinas». Una vez decidido, se analiza junto desde todas ([varias máquinas §6.5](../docs/suite-devices.md#lo-personal-y-el-análisis-de-todas-tus-máquinas)). Cambia la regla de §6 aflojándola: un motor o un agregador que ya la cumplía la sigue cumpliendo, así que es una versión menor. Lo que entrega `nu9ve mcp` no cambia.
