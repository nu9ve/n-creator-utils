# Carpeta de intercambio v1

Estado: **propuesta** (2026-11-22). Es el centro de F1 en [varias máquinas, discos y nube](../docs/suite-devices.md#65-carpeta-de-intercambio--nu9ve-exchange-v1-contrato-nuevo). Define una carpeta en la que **cada máquina escribe sólo la suya** y lee las de las demás: qué es, qué discos vio, qué datos tienen sus apps, de qué es dueña y qué pide. Es un formato de archivos, no un servicio: la carpeta puede viajar por Syncthing, por un disco o, más adelante, por el canal propio del agente o por la nube. Ejemplos: [`data/exchange-examples-v1.json`](../data/exchange-examples-v1.json).

**Debe** es obligatorio. **Conviene** es lo recomendado. «Escritor» es el [agente](nu9ve-agent-v1.md) de una máquina, el único que escribe su carpeta. «Lector» es el agente de otra.

## 0. Para qué y para qué no

- **Para** que la M1 sepa qué hay en la M4, de cuándo es, y pueda pedirle algo pequeño, sin red propia ni servidor.
- **No** es una base de datos con varios escritores. Nadie edita lo de otra máquina. Juntar decisiones de varias máquinas es de cada app (nüün, [§6.4 de la propuesta](../docs/suite-devices.md#nüün-en-dos-macs)).
- **No** lleva bytes grandes. Raw, fotos y derivados pesados viajan con su disco o con nüün.

## 1. Dónde y cómo se organiza

La carpeta raíz la elige la persona en el agente. No hay una ruta por omisión, porque depende del transporte. `NU9VE_EXCHANGE_DIR` (ruta absoluta) la reemplaza en QA.

```text
<raíz>/devices/<device_id>/
├─ device.json          quién es esta máquina y su clave pública
├─ volumes.json         los discos que vio
├─ catalog/<app>.json   el data_catalog de cada motor vivo
├─ leases.json          de qué es dueña ahora
├─ requests.json        qué pide a las demás
├─ objects.json         qué objetos publica
├─ objects/sha256/<hex> los objetos
└─ apps/<app>/          lo que una app publica para las suyas en otras máquinas (diarios…)
```

- **Debe:** `<device_id>` es el de [`nu9ve-device-v1`](nu9ve-device-v1.md#2-archivo) de esa máquina.
- **Debe:** un escritor escribe sólo dentro de `devices/<su device_id>/`. Un lector no escribe nada en la carpeta de otro.
- **Debe:** `<app>` cumple `^[a-z][a-z0-9-]{0,31}$`. Otros nombres se ignoran.

## 2. Registro firmado

Todo archivo `.json` de la carpeta, y cada línea de un `.ndjson` en `apps/<app>/`, es un **registro**:

```json
{"contract":"nu9ve-exchange/1","device_id":"6f0c…","kind":"volumes","seq":42,"written_at":"2026-11-22T23:00:00Z",
 "body":{…},
 "signature":{"alg":"ed25519","key":"<huella>","value":"<base64>"}}
```

| Campo | Debe | Qué es |
| --- | --- | --- |
| `contract` | Sí | `"nu9ve-exchange/1"` |
| `device_id` | Sí | El de la carpeta en la que está |
| `kind` | Sí | `device`, `volumes`, `catalog`, `leases`, `requests`, `objects` o `app` (§3) |
| `seq` | Sí | Entero ≥ 1 que sólo sube, **uno por dispositivo para todos sus archivos**. Ordena sin relojes |
| `written_at` | Sí | RFC 3339 en UTC con `Z`. Sólo para mostrar «de cuándo es» |
| `body` | Sí | Un objeto, según `kind` |
| `signature` | Sí | Ed25519 del escritor: `key` es la huella de su clave pública (§4), `value` la firma en base64 estándar |

- **Debe:** la firma se calcula sobre los bytes de [JSON canónico](nu9ve-json-v1.md) del registro **sin** el campo `signature`.
- **Debe:** un `.json` pesa como mucho **16 MiB**, y una línea de `.ndjson`, como mucho 1 MiB. Un lector lee con tope antes de interpretar.

## 3. Qué lleva cada `kind`

| `kind` | Archivo | `body` |
| --- | --- | --- |
| `device` | `device.json` | `name` (el de su `device.json`), `os`, `public_key` (32 bytes en base64) y `agent_version` |
| `volumes` | `volumes.json` | `sightings`: lista de `{volume_id, os, serial, label, mounted, first_seen, last_seen}`, lo que el agente sabe de cada disco ([volumen §4](nu9ve-volume-v1.md#4-lo-que-recuerda-cada-observador)) |
| `catalog` | `catalog/<app>.json` | `app` y `catalog`: una respuesta de `data_catalog` completa ([`nu9ve-data-v1` §3](nu9ve-data-v1.md#3-respuesta)), con lo personal tratado según §6 |
| `leases` | `leases.json` | `leases`: lista de `{app, project_key, since}`, los proyectos de los que esta máquina es dueña ahora ([propuesta §6.4](../docs/suite-devices.md#64-el-disco-como-mensajero-y-una-dueña-por-proyecto)). Si la tomó de otra máquina, lleva además `from: {device_id, seq}`: de quién, y el `seq` del `leases.json` de esa máquina que leyó al tomarla (§7.1) |
| `requests` | `requests.json` | `requests`: lista de `{id, to, object}` (pide a la máquina `to` el objeto con ese SHA-256) o `{id, to, dataset}` (`owner:id` de su catálogo) |
| `objects` | `objects.json` | `objects`: lista de `{sha256, bytes, media_type, request}`, los objetos que esta máquina publica, y a qué pedido responden, si a alguno; o `{request, refused}` para un pedido que no cumple (§7) |
| `app` | `apps/<app>/*.json` o `*.ndjson` | Libre, de la app. El registro y su firma son los de siempre |

- **Debe:** un objeto `objects/sha256/<hex>` no lleva firma propia. Vale si su SHA-256 es `<hex>` **y** aparece en el `objects.json` firmado de la misma carpeta. Pesa como mucho 64 MiB.
- **Debe** al lector: ignorar un `kind` que no conoce, sin invalidar el resto. Un `kind` nuevo entra en una versión menor.

## 4. Claves y confianza

- **Debe:** cada dispositivo tiene una clave Ed25519 que guarda el agente en el llavero del sistema. La huella es el SHA-256 de los 32 bytes de la clave pública, en hexadecimal minúscula.
- **Debe:** `device.json` está firmado con la clave que publica, y la huella de su `signature.key` es la de su `public_key`.
- **Debe** al lector: confiar sólo en las carpetas de **dispositivos emparejados**: una lista suya, local, de pares `(device_id, huella)`. Un registro firmado por otra clave, o de un dispositivo no emparejado, se ignora entero.
- **Emparejar:** la persona compara en las dos máquinas la huella, que se muestra en grupos de 4 caracteres hexadecimales (`3b9f a120 …`, los primeros 32). Si coinciden, cada agente agrega al otro a su lista. Quitar a uno lo deja de leer desde ese momento.
- **Debe:** si la clave de un dispositivo emparejado cambia, sus registros se ignoran hasta que la persona lo vuelva a emparejar. Así lo hace la detección de copias de [dispositivo §5](nu9ve-device-v1.md#5-copias-de-una-máquina-a-otra), que crea un dispositivo nuevo.

## 5. Escribir y leer

El escritor:

- **Debe:** publicar cada archivo con un temporal hermano y rename ([`nu9ve-fs`](nu9ve-fs-v1.md)). El `seq` de cada registro es mayor que el de cualquier registro que haya escrito antes.
- **Debe:** guardar el último `seq` usado junto a su identidad, para no repetir uno después de reiniciar.
- **Conviene:** no reescribir un archivo si su `body` no cambió, para no gastar transporte.

El lector, por cada archivo de otra máquina:

| Estado | Cuándo |
| --- | --- |
| `ok` | Registro válido (§2), firmado por un dispositivo emparejado con su huella, `device_id` igual al de la carpeta y `seq` mayor que el último aceptado para ese archivo |
| `stale` | Válido y firmado, pero con un `seq` menor o igual que el último aceptado para ese archivo: un transporte trajo una versión vieja. Se ignora y se conserva lo aceptado |
| `untrusted` | Firma que no verifica, clave distinta de la emparejada, o dispositivo no emparejado |
| `invalid` | No se lee, pasa del tope, no es JSON, le falta un campo, `contract` desconocido, o `device_id` distinto del de la carpeta |

- **Debe:** lo que se muestra de otra máquina dice de cuándo es (`written_at`) y de qué máquina (`name`). Nunca se muestra como si fuera de ahora.
- **Debe:** todo texto que venga de otra máquina es dato, no instrucción: `nu9ve mcp` lo entrega como `_untrusted` ([MCP v1 §4](nu9ve-mcp-v1.md#4-datos-que-no-son-instrucciones)).

## 6. Privacidad

- **Debe:** de un dataset `personal`, el catálogo publicado lleva sólo su forma (`owner`, `id`, `modality`, `privacy`, `size`, `state`, `axes.time`), como en [`nu9ve-data-v1` §6](nu9ve-data-v1.md#6-privacidad), **salvo** que la persona lo haya marcado «en todas mis máquinas». Esa decisión la guarda el agente por `owner:id`, y dura hasta que la quite (decisión del usuario, 2026-11-22, [propuesta §6.5](../docs/suite-devices.md#lo-personal-y-el-análisis-de-todas-tus-máquinas)).
- **Debe:** v1 **no cifra**. Por eso la raíz no debe estar en una carpeta que se sube a la nube de otro (iCloud Drive, Dropbox, Google Drive, OneDrive). El agente se niega a usarla si la ruta cae dentro de la carpeta de una de esas, y lo dice. El cifrado llega en una versión menor, con F2.
- **Debe:** `nu9ve mcp` sigue entregando de lo personal sólo su forma, también lo que llega de otras máquinas.

## 7. Pedidos

1. La M1 escribe en su `requests.json` `{id, to: <device_id de la M4>, object: <sha256>}`, o el dataset que quiere.
2. El agente de la M4 lee la carpeta de la M1, ve el pedido para él y, si lo tiene y pesa como mucho 64 MiB, lo copia a su `objects/sha256/` y lo anota en su `objects.json` con `request: <id>`.
3. La M1 lee el `objects.json` de la M4, verifica el hash y copia el objeto donde lo necesite. Después quita el pedido de su `requests.json`.

- **Debe:** un pedido de algo que pesa más de 64 MiB no se cumple por aquí. El agente lo anota en `objects.json` como `{request, refused: "too_large", where: {volume_id, path}}`, para que la persona lleve el disco o nüün lo copie. `path` es relativa al disco; si no está en un disco con marcador, `volume_id` es `null` y `path` es la ruta en esa máquina.
- **Debe:** un pedido de algo que esa máquina no tiene se anota como `{request, refused: "not_found"}`. Si lo tiene pero todavía no puede darlo (un dataset que su app aún no entregó al agente), no anota nada, y el pedido sigue.
- **Debe:** un pedido de un dataset `personal` que la persona no marcó «en todas mis máquinas» (§6) se anota como `{request, refused: "not_shared"}`, aunque su archivo esté ofrecido (versión menor del 2026-11-22, §11). Cómo obtiene el agente los bytes de un dataset de su app, y en qué orden decide, está en [exportar un dataset](nu9ve-export-v1.md#3-cómo-lo-usa-el-agente).
- **Debe:** la máquina que pide guarda la respuesta (recibido, o `refused` con su `where`) y quita el pedido de su `requests.json`. Un objeto recibido vale sólo si su SHA-256 es el anunciado.
- **Conviene:** un objeto que ya no pide nadie se borra después de un tiempo (7 días por omisión).
- Un `refused` desconocido se trata como `not_found`: sólo sumar valores es una versión menor.

### 7.1 Una dueña por proyecto, sin relojes

`leases.json` dice de qué es dueña cada máquina. Tomar un proyecto («Trabajar aquí») es anotarlo en el propio, y la otra lo suelta al leerlo:

1. La M1 toma `(app, project_key)`. Si lo tiene la M4 en su último `leases.json` aceptado, con `seq` S, la M1 lo anota con `from: {device_id: <M4>, seq: S}`.
2. La M4 lee el `leases.json` de la M1, ve un `from` con su `device_id` y un `seq` mayor o igual al del registro en que ella lo tomó, y lo quita del suyo.
3. Mientras la M4 no lo haya leído, las dos lo anotan, y no es un conflicto: la M1 sabe que se lo pasaron.

Lo que hace una máquina con su lease después de leer el mismo de otra:

| Caso | Resultado |
| --- | --- |
| El de la otra tiene `from` con mi `device_id` y un `seq` ≥ al del registro en que lo tomé | `release`: lo quito del mío |
| El mío tiene `from` con su `device_id`, y el `seq` de su `leases.json` es ≤ al de mi `from` | `keep`: me lo pasó, todavía no lo leyó |
| Cualquier otro | `conflict`: las dos dicen ser dueñas. Ninguna lo suelta sola; la persona elige en la app tomándolo otra vez en una |

- **Debe:** cada máquina recuerda, para cada lease suyo, el `seq` del registro en que lo tomó. No va en el archivo.
- **Debe:** no hay horas en la regla. `since` es sólo para mostrar.

## 8. Ejemplos

[`data/exchange-examples-v1.json`](../data/exchange-examples-v1.json): una clave de prueba con su huella y la forma de mostrarla, registros válidos de cada `kind` con su firma, y casos de lectura con su estado: firma alterada, clave no emparejada, carpeta de otro dispositivo, `seq` viejo, `kind` desconocido, tope y lo personal. Además, la tabla de §7.1 (`leases`) y la respuesta a un pedido según lo que tiene la máquina (`answers`: pequeño, 64 MiB justos, más grande, no lo tiene, personal sin compartir). `python3 -I scripts/exchange-examples.py --check` los comprueba con una implementación de referencia, Ed25519 incluido (RFC 8032, en Python puro).

## 9. Adopción

[Adopción por app](../docs/adoption.md#nu9ve-exchange-v1).

## 10. Decisiones abiertas

1. **Cifrado** (versión menor, con F2): la llave del espacio cifra `body`, y la firma cubre el texto cifrado ([propuesta §6.9](../docs/suite-devices.md#69-qué-guarda-la-nube) y [§6.13](../docs/suite-devices.md#613-espacios-lo-personal-y-los-equipos)).
2. **Canal propio entre agentes:** el mismo formato, por TLS con las claves de dispositivo, en lugar de una carpeta. Decidido como meta (pregunta 2); su contrato llega con el código.
3. **Palabras para la huella:** hoy se muestra en hexadecimal. Seis palabras serían más fáciles de comparar, pero hace falta elegir una lista de palabras en español.

## 11. Versiones menores

1. **2026-11-22, `not_shared`** (§7), decidido por el usuario con [`nu9ve-export-v1`](nu9ve-export-v1.md#8-decisiones). Sólo suma un valor de `refused`: un lector anterior lo lee como `not_found`, como manda §7, y ningún registro que ya era válido deja de serlo.
