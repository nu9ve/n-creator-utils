# nu9ve-mcp

El binario `nu9ve` de la suite, con dos órdenes:

- **`nu9ve mcp [--allow-actions]`:** el servidor MCP de la suite ([diseño](../../docs/suite-mcp.md)).
- **`nu9ve agent <orden>`:** el agente de la suite ([`nu9ve-agent-v1`](../../contracts/nu9ve-agent-v1.md)). Guarda la identidad del dispositivo y su clave, y escribe y lee la [carpeta de intercambio](../../contracts/nu9ve-exchange-v1.md).

## `nu9ve agent`

Primer corte (2026-11-22). Cada orden imprime su resultado en JSON.

| Orden | Qué hace |
| --- | --- |
| `init` | Crea `device.json` si no está ([`nu9ve-device`](../nu9ve-device/)) y la clave Ed25519 del dispositivo si no la hay. En macOS, la guarda en el llavero (servicio `xyz.nu9ve.agent`, cuenta `device_id`). Muestra la huella para emparejar |
| `name <NOMBRE>` | Cambia el nombre del dispositivo |
| `exchange <CARPETA>` | Elige la raíz de la carpeta de intercambio. Rechaza iCloud Drive, Dropbox, Google Drive, OneDrive y `~/Library/CloudStorage`, porque v1 no cifra |
| `publish` | Escribe la carpeta de esta máquina: `device.json`, `volumes.json` (los discos con marcador montados en `/Volumes`, con su serial de `diskutil`) y `catalog/<app>.json` de cada motor vivo con `data_catalog_v1`. Lo personal va sólo con su forma, salvo lo compartido. Sólo reescribe lo que cambió, y la «última vez» de un disco cuenta como cambio una vez por hora |
| `read` | Lee las carpetas de las otras máquinas y da a cada archivo su estado (`ok`, `stale`, `untrusted`, `invalid`), con el nombre de cada máquina, sus discos y cuántos datasets tiene cada app |
| `pair <DEVICE_ID> <HUELLA>` | Empareja otra máquina si su `device.json` está firmado por la clave que publica y su huella empieza por lo que la persona leyó en la otra pantalla (al menos 32 caracteres, con o sin espacios) |
| `unpair <DEVICE_ID>` | La deja de leer, y olvida los pedidos pendientes con ella, sus respuestas y lo recibido de ella |
| `share <OWNER:ID>`, `unshare <OWNER:ID>` | «Disponible en todas mis máquinas»: lleva un dataset personal entero a las demás, y su app puede ofrecerlo sola. `unshare` lo deja otra vez sólo con su forma y deja de ofrecer sus archivos |
| `status` | El dispositivo, la huella, la carpeta, los emparejados, el último `seq` y el pid del servicio, si corre |
| `sync` | Publica, lee y hace la pasada de pedidos y dueñas, como el servicio cada pocos minutos |
| `offer <ARCHIVO> [--dataset <OWNER:ID>]` | Ofrece un archivo para cuando otra máquina lo pida. Queda donde está. Sólo uno de hasta 64 MiB se puede enviar; uno más grande se ofrece por su dataset y se responde con su disco |
| `unoffer <ARCHIVO>` | Deja de ofrecerlo |
| `call <MÉTODO> [JSON]` | Corre un método del motor `suite` sin el servicio |
| `install [--by <APP>]` | Instala o actualiza el agente desde este binario, si la tabla de §3 lo dice (nunca baja de versión; un `public` no reemplaza un `cloud`). Para el que corre, publica con rename, escribe `agent.json` y, fuera de QA, el LaunchAgent `xyz.nu9ve.agent` (o la tarea «nu9ve agent» en Windows) |
| `uninstall` | Quita el arranque, el binario y `agent.json`. Nunca `device.json`, la clave, la carpeta de intercambio ni el estado del agente |
| `state` | Lo que ve una app (§5): `absent`, `installed`, `running` u `outdated`, con la acción de §3 si la app trae este binario. En Rust, `nu9ve_mcp::agent::install::state_of` |
| `serve` | El servicio de fondo (§4 y §5 del contrato). Publica cuando cambia el registro de motores o un disco se monta, y lee las emparejadas cada 3 minutos. Un solo `serve` por carpeta del agente (candado `serve.lock`). Para con `SIGTERM` o `SIGINT`. En macOS espera en kqueue y no gasta en reposo |

Los archivos del agente (`config.json`, `state.json` con el último `seq` y lo aceptado de cada archivo, `seen/` con la última copia aceptada de cada máquina, `paired.json` y `shared.json`) están en su carpeta, `~/Library/Application Support/nu9ve/agent/`.

### El motor `suite`

`serve` sirve `suite.sock` en la carpeta del agente y se registra como el motor `suite` (protocolo `nu9ve-suite/1`, capacidades `devices_v1`, `exchange_v1` y `mcp_tools_v1`). En `nu9ve mcp` aparecen:

| Herramienta | Nivel | Qué da |
| --- | --- | --- |
| `suite_devices` | Leer | Esta máquina (nombre, huella, apps vivas) y las demás de la carpeta: emparejada o no, nombre, última escritura, estado de cada archivo, catálogos y discos |
| `suite_volumes` | Leer | Cada disco con marcador: si está montado aquí y en qué máquina se vio, cuándo |
| `suite_where` | Leer | Dónde está un proyecto (`project_key`, con subproyectos), un dataset (`owner:id`) o un objeto (`sha256`) |
| `suite_leases` | Leer | Qué máquina es dueña de cada proyecto, y si hay conflicto |
| `suite_lease_take` | Editar | Pasa a esta máquina la dueña de `(app, project_key)` |
| `suite_lease_release` | Editar | Esta máquina deja de ser dueña de un proyecto, sin pasárselo a otra |
| `suite_pull` | Editar | Pide a una máquina emparejada un objeto o un dataset. Con `drop: true` quita el pedido, la respuesta y la copia recibida |

Una app llama los mismos métodos por su nombre (`devices`, `where`…), sin `grant`, y además `offer` y `unoffer`, `shared`, `share` y `unshare` ([exportar §5](../../contracts/nu9ve-export-v1.md#5-ofrecer-sola-lo-compartido)), y `app_publish`, `app_append` y `app_read` para su carpeta `apps/<app>/` en las otras máquinas.

**Pedidos y dueñas** ([intercambio §7 y §7.1](../../contracts/nu9ve-exchange-v1.md#7-pedidos)): tras cada lectura, el agente responde lo que le piden (copia lo ofrecido de hasta 64 MiB a `objects/sha256/`, o lo que exporta el motor dueño con `data_export` si su registro dice `data_export_v1` ([exportar un dataset](../../contracts/nu9ve-export-v1.md)), o anota `too_large` con el disco, `not_shared` para lo personal que la persona no compartió, o `not_found`), recibe lo que pidió en `received/<hex>` si el hash coincide, y suelta la dueña de un proyecto que otra máquina le tomó. Lo que viene de otra máquina sale en campos `_untrusted`; de lo personal, sólo su forma. Cada edición queda en `changes.ndjson`. `engine_shutdown` para el agente en orden.

**QA:** `NU9VE_AGENT_DIR` cambia la carpeta del agente y obliga a dar también `NU9VE_DEVICE_FILE`. Con eso, la clave va en un archivo de la carpeta y no en el llavero. `NU9VE_ENGINES_DIR` y `NU9VE_VOLUMES_DIR` cambian el registro y la carpeta de montajes. `tests/agent.rs` corre dos máquinas así, con una carpeta compartida.

**Falta** para cerrar F1:
- la clave en Windows;
- el canal propio entre agentes.
