# Exportar un dataset v1

Estado: **propuesta** (2026-11-22). Define `data_export`, el método con el que el agente de una máquina pide al motor dueño de un dataset sus bytes, **listos en un archivo**, para darlos a otra máquina que los pidió por la [carpeta de intercambio](nu9ve-exchange-v1.md#7-pedidos). Es lo que hace que `suite_pull {dataset}` de la M1 reciba el GeoJSON de baches que una app procesó en la M4 sin que nadie lo ofrezca a mano. Ejemplos y vectores: [`data/export-examples-v1.json`](../data/export-examples-v1.json).

**Debe** es obligatorio. **Conviene** es lo recomendado. «Motor» es el de la app dueña ([motor](nu9ve-engine-v1.md)); «agente», el de la máquina ([agente](nu9ve-agent-v1.md)).

## 0. Para qué y para qué no

- **Para** lo pequeño: un archivo de hasta 64 MiB, el tope de un objeto del intercambio ([§3](nu9ve-exchange-v1.md#3-qué-lleva-cada-kind)). Tablas, GeoJSON, un resumen, un paquete chico.
- **No** para lo grande. Raw, fotos y derivados pesados viajan con su disco o con nüün; el motor dice en qué disco están (§2).
- **No** es una lectura del catálogo. [`data_catalog`](nu9ve-data-v1.md) describe sin leer contenido; `data_export` sí entrega contenido, y por eso tiene sus propias reglas de privacidad (§4).
- **Local.** Viaja por el transporte del motor ([motor §1](nu9ve-engine-v1.md#1-transporte)). Lo que sale de la máquina sale por la carpeta de intercambio, con sus reglas, nunca por este método.

## 1. Método

Un motor que exporta lista `data_export_v1` en `capabilities` de su `engine_info` y en su [registro](nu9ve-registry-v1.md#2-archivo). **Debe** servir también `data_catalog_v1`: se exporta lo que el catálogo lista.

```json
{"id":1,"method":"data_export","params":{"dataset":"baches-2026-11","max_bytes":67108864}}
```

| Parámetro | Tipo | Qué hace |
| --- | --- | --- |
| `dataset` | cadena, obligatoria | El `id` del dataset **en el catálogo de este motor** ([data §4.1](nu9ve-data-v1.md#41-identidad-y-dueño)), sin `owner:` |
| `max_bytes` | entero 1..2^53 − 1 | El tope de quien pide. Por omisión 67108864 (64 MiB) |

- **Debe:** otro parámetro, un tipo equivocado, un `dataset` que no cumple el patrón de id o un `max_bytes` fuera de rango es `invalid_params`.
- **Debe:** responde dentro del plazo del cliente (5 s en [`nu9ve-ipc`](../rust/nu9ve-ipc/)). Si preparar el archivo tarda más, lo prepara aparte y responde `pending` (§2); quien pide vuelve a preguntar después.
- **Debe:** no cambia el dataset. Puede escribir el archivo exportado y guardarlo para la próxima vez.

## 2. Respuesta

```json
{"id":1,"ok":true,"data":{"contract":"nu9ve-export/1","dataset":"baches-2026-11","status":"ready",
 "path":"/Users/ana/Library/Caches/una-app/export/baches-2026-11.geojson",
 "bytes":20480,"media_type":"application/geo+json","sha256":"9f86…"}}
```

| Campo | Debe | Qué es |
| --- | --- | --- |
| `contract` | Sí | `"nu9ve-export/1"` |
| `dataset` | Sí | El mismo `dataset` de la petición |
| `status` | Sí | `ready`, `too_large`, `pending` o `not_found` |

Según `status`:

| `status` | Campos | Qué quiere decir |
| --- | --- | --- |
| `ready` | `path`, `bytes`, `media_type`, `sha256` (los cuatro obligatorios) | El archivo está listo. `path` es absoluta (`/…`, o `C:\…` en Windows), de un archivo regular, hasta 4096 bytes; `bytes` es su tamaño, ≤ `max_bytes`; `sha256`, el de sus bytes, 64 hex en minúsculas; `media_type`, su tipo, que cumple `^[a-z0-9][a-z0-9.+-]{0,63}/[a-z0-9][a-z0-9.+-]{0,127}$` |
| `too_large` | `where` obligatorio; `bytes` si se sabe | El dataset no cabe en `max_bytes`, o no se exporta como un archivo. `where` es `{volume_id, path}` como en [intercambio §7](nu9ve-exchange-v1.md#7-pedidos), sin otros campos: el `volume_id` del disco con marcador (UUID 4) y la ruta dentro de él, relativa, con `/`, sin `.` ni `..` (`""` es la raíz del disco); o `volume_id: null` y la ruta absoluta en esta máquina |
| `pending` | — | Lo está preparando. Se pide otra vez después |
| `not_found` | — | Este motor no tiene ese dataset, o ya no |

- **Debe:** con `ready`, el archivo no cambia después de responder y sigue en `path` al menos 10 minutos. Un dataset que cambió se exporta a **otro** archivo; nunca se reescribe el que se entregó.
- **Debe:** `path` es una ruta que elige el motor, en su propia carpeta (caché, exportaciones). Nunca una que venga de la petición.
- **Debe:** el contenido es el del dataset en el formato que la app elige y declara en `media_type`. **Conviene** un formato abierto (GeoJSON, CSV, NDJSON, Parquet) y, para una carpeta pequeña, un zip.
- **Debe** al cliente: un `status` que no conoce es `not_found`; cualquier respuesta que no cumple esta tabla es inválida y se trata como `pending` (se vuelve a pedir, no se responde nada a nadie con ella). Sólo sumar valores es una versión menor.
- **Debe** al cliente: copia el archivo con tope de `max_bytes` y verifica su SHA-256 antes de usarlo. Si no coincide, es `pending`.

## 3. Cómo lo usa el agente

Cuando una máquina emparejada pide un dataset `owner:id` ([intercambio §7](nu9ve-exchange-v1.md#7-pedidos)), el agente de la máquina pedida decide, en este orden:

| Paso | Si… | Responde |
| --- | --- | --- |
| 1 | El dataset es `personal` en el catálogo publicado de esta máquina y la persona no lo marcó «en todas mis máquinas» (§4) | `refused: "not_shared"` |
| 2 | Hay un archivo ofrecido (`offer`) para ese dataset | Ese archivo, o `too_large` con su disco |
| 3 | No está en ningún catálogo publicado de esta máquina | `refused: "not_found"` |
| 4 | El catálogo dice que pesa más de 64 MiB | `too_large`, con el disco de su raíz |
| 5 | El motor de `owner` está `live` y tiene `data_export_v1` | Lo que diga `data_export`: `ready` se copia a `objects/sha256/` y se publica; `too_large` y `not_found` se anotan como tales; `pending`, una respuesta inválida o un error del motor, nada (el pedido sigue) |
| 6 | Si no | Nada: el pedido sigue hasta que el motor esté vivo |

- **Debe:** el agente pide sólo al motor cuyo `app` es el `owner` del dataset, por el [registro](nu9ve-registry-v1.md#6-qué-ve-el-cliente), y comprueba antes su `engine_info` ([motor §4](nu9ve-engine-v1.md)).
- **Debe:** el agente pasa `max_bytes` = 64 MiB.
- **Debe:** una vez anotada la respuesta a un pedido, no se vuelve a pedir `data_export` por él.

## 4. Privacidad

- **Debe:** `data_export` es una salida de contenido. Sólo lo llama el agente, para responder un pedido de una máquina emparejada. `nu9ve mcp` **no** lo ofrece como herramienta ni lo llama: lo que pasa por MCP llega al proveedor del modelo ([data §6](nu9ve-data-v1.md#6-privacidad)).
- **Debe:** de un dataset `personal`, el agente da los bytes sólo si la persona lo marcó «en todas mis máquinas» (`nu9ve agent share`, o `share` del motor `suite`), como pide [intercambio §6](nu9ve-exchange-v1.md#6-privacidad). Lo comprueba en cada pedido, también si el archivo se ofreció con `offer`: dejar de compartirlo corta lo que no se envió todavía. Lo que ya llegó a la otra máquina queda allá hasta que se borre.
- **Debe:** un dataset que no está en ningún catálogo de esta máquina, y que alguien ofreció con `offer`, vale como lo decidió quien lo ofreció: la persona a mano, o la app según §5.
- **Debe** al motor: la clase que declara en su catálogo es la que manda. Si no la sabe, es `personal` ([data §6](nu9ve-data-v1.md#6-privacidad)).

## 5. Ofrecer sola lo compartido

`data_export` necesita el motor vivo cuando llega el pedido. Una app que se cierra puede, además, **ofrecer sola** lo pequeño que la persona marcó «disponible en todas mis máquinas», para que el agente lo dé aunque la app no esté abierta. Lo hace con los métodos del motor `suite` ([agente §5](nu9ve-agent-v1.md#5-cómo-lo-ve-una-app)):

1. `shared {"app": "<su app>"}` da `{"app", "shared": ["<app>:<id>", …]}`: los datasets suyos que la persona marcó. Lo llama al arrancar, después de cambiar un dataset, y cada tanto mientras corre (cada 5 minutos conviene).
2. Por cada uno que puede exportar en hasta 64 MiB, escribe el archivo como en §2 y llama `offer {"path", "dataset": "<app>:<id>", "media_type"}`. El archivo no cambia mientras esté ofrecido.
3. Si el dataset cambió, escribe un archivo nuevo, lo ofrece y llama `unoffer {"path"}` del anterior. Si dejó de estar en `shared`, `unoffer`.
4. Si la persona lo marca desde la app («disponible en todas mis máquinas»), la app llama `share {"dataset": "<app>:<id>"}` o `unshare`, y la marca queda en el agente, la misma que `nu9ve agent share`.

- **Debe:** una app ofrece sola sólo datasets suyos que están en `shared`. Ofrecer otra cosa es una decisión de la persona, a mano.
- **Debe** al agente: `unshare` también deja de ofrecer los archivos de ese dataset.
- **Conviene:** una app con `data_export_v1` ofrece sola sólo lo que tarda en exportar o lo que quiere dar con la app cerrada; lo demás lo responde `data_export` al momento.

## 6. Ejemplos

[`data/export-examples-v1.json`](../data/export-examples-v1.json):

- peticiones con su respuesta esperada (`invalid_params` por parámetro de más, id inválido o `max_bytes` fuera de rango);
- respuestas válidas de cada `status` y respuestas inválidas con su motivo (ruta relativa, `bytes` sobre el tope, hash mal escrito, `ready` sin `media_type`, `too_large` sin `where`…);
- la tabla de §3: qué anota el agente en `objects.json` según la clase, si está compartido, lo ofrecido, el catálogo y la respuesta del motor.

`python3 -I scripts/export-examples.py --check` comprueba el archivo contra una implementación de referencia de §1 a §4. Con `--server <motor> …` envía las peticiones de ejemplo a un motor real por marcos en stdio y valida su respuesta.

## 7. Adopción

[Adopción por app](../docs/adoption.md#nu9ve-export-v1).

## 8. Decisiones

1. **Contrato propio,** no una versión menor de [`nu9ve-data-v1`](nu9ve-data-v1.md): el catálogo describe sin leer contenido, y este método entrega contenido, con otras reglas de privacidad y otro cliente (sólo el agente).
2. **Decidido (el usuario, 2026-11-22): un personal no compartido se rechaza con `not_shared`,** un valor nuevo de `refused` en [intercambio §7](nu9ve-exchange-v1.md#7-pedidos) (versión menor). Así la máquina que pide sabe que hay que compartirlo en la otra. No revela nada nuevo: el catálogo ya publica la forma del dataset.
3. **Un archivo por dataset.** Una carpeta pequeña se entrega empaquetada. Si hace falta traer varias partes de un dataset, se piden como objetos (`object`), por su SHA-256.
