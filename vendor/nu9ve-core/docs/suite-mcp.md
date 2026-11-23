# `nu9ve mcp`: un servidor MCP para toda la suite

Estado: **diseño con un primer corte de código** (2026-11-19): [`rust/nu9ve-ipc`](../rust/nu9ve-ipc/) y [`rust/nu9ve-mcp`](../rust/nu9ve-mcp/), sólo con socket Unix (macOS; Linux sólo para pruebas). Qué falta, en §10. Un solo servidor MCP que lee el [registro de motores](../contracts/nu9ve-registry-v1.md), habla con cada motor abierto y expone sus herramientas con el prefijo de su app, con las reglas de [MCP v1](../contracts/nu9ve-mcp-v1.md). Pedido en nove [data-hub §6](../../nove/docs/data-hub.md#6-una-sola-puerta-registro-de-motores-y-mcp-de-la-suite); el usuario decidió en [A49](../../nove/docs/decisions.md#a49--datos-de-toda-la-suite-en-nove-siete-decisiones-y-modelo-v1-2026-11-19) (decisión 4) que **convive** con `nove mcp` y el MCP de nüün hasta cubrir lo mismo con pruebas. El usuario aprobó el primer corte de código (§9).

## 1. Qué es

- Un binario de la suite, `nu9ve`, con el subcomando `mcp`. JSON-RPC 2.0 por stdio, sin red ([MCP v1 §0](../contracts/nu9ve-mcp-v1.md#0-principios)).
- **Cliente de motores, no otro motor.** No abre bases de datos ni archivos de las apps. Todo lo pide al motor `live` del registro, por su transporte ([registro §3](../contracts/nu9ve-registry-v1.md#3-transporte)), con las comprobaciones de [motor §9](../contracts/nu9ve-engine-v1.md#9-seguridad-sólo-local-sólo-el-usuario).
- **No conoce ninguna app.** Las herramientas de cada app las publica su motor (§4). El código del servidor no nombra apps, igual que el resto del core (`scripts/check-boundary.sh`).
- **No arranca nada.** Si una app no está abierta, sus herramientas no aparecen y `suite_engines` lo dice.

```bash
claude mcp add nu9ve -- nu9ve mcp
claude mcp add nu9ve-acciones -- nu9ve mcp --allow-actions
```

## 2. Convivencia con los MCP de cada app

| | `nove mcp`, `nu9ve media mcp` | `nu9ve mcp` |
| --- | --- | --- |
| Herramientas | Las de su app, sin prefijo | Las de todas las apps abiertas con `mcp_tools_v1`, con prefijo, más las de la suite (§3) |
| Cómo llega al motor | Su propio cliente (nove: socket o pipe de su raíz) | El registro |
| Permiso de edición | Su diálogo, su ventana de 60 min | El suyo, por app (§5) |

- Los dos pueden estar registrados en el mismo cliente. Cada proceso pide su propio permiso: aceptar en uno no autoriza al otro.
- Un MCP de app se retira **sólo** cuando `nu9ve mcp` cubre sus herramientas con las pruebas de [MCP v1 §6](../contracts/nu9ve-mcp-v1.md#6-validación) y el usuario lo decide. Hasta entonces la app no cambia su MCP.

## 3. Herramientas de la suite

Nivel Leer. Son las únicas que el servidor define él mismo.

| Herramienta | Qué hace |
| --- | --- |
| `suite_engines` | Las apps del registro con su estado (`live`, `stale`, `invalid`, ausente), versión y capacidades. Sin rutas ni pids |
| `suite_data_catalog` | El catálogo agregado ([`nu9ve-data-v1` §5](../contracts/nu9ve-data-v1.md#5-agregar-catálogos)): por app, por proyecto (`project_key`) o por modalidad, paginado. Texto como `_untrusted`; lo `personal`, reducido (§6) |
| `suite_projects` | Las claves de proyecto de todas las apps abiertas, con el título y el padre de cada app. No une árboles |

## 4. Herramientas de cada app

Para que el servidor no conozca apps, el **motor** publica su catálogo de herramientas por su protocolo. Capacidad `mcp_tools_v1`, dos métodos:

```json
{"id":1,"method":"mcp_tools"}
→ {"id":1,"ok":true,"data":{"tools":[
    {"name":"status","title":"Estado","description":"…","level":"read","input_schema":{"type":"object","properties":{},"additionalProperties":false}},
    {"name":"strategy_save","title":"Guardar estrategia","description":"…","level":"edit","input_schema":{…}}],
   "instructions":"…"}}

{"id":2,"method":"mcp_call","params":{"tool":"strategy_save","arguments":{…},"agent":"Claude Code","grant":"edit"},"idempotency_key":"…"}
→ {"id":2,"ok":true,"data":{…}}
```

- **Nombre:** `<app>_<name>`, con `<app>` del registro. Si la herramienta ya empieza por `<app>_` (`nove_status`), no se repite. El resultado cumple `^[a-z0-9_]{1,64}$`; una herramienta que no cabe se omite y se cuenta en `suite_engines`. Sólo el motor registrado como `suite` (el agente, [`nu9ve-agent-v1` §5](../contracts/nu9ve-agent-v1.md#5-cómo-lo-ve-una-app)) da nombres que empiezan por `suite_`; de otro motor se omiten. Ningún motor puede dar `suite_engines`, `suite_data_catalog` ni `suite_projects`, que son del servidor.
- **Nivel:** `level` es `read`, `show`, `edit` o `act` ([MCP v1 §2](../contracts/nu9ve-mcp-v1.md#2-cuatro-niveles-de-herramientas)). Las `annotations` las pone el servidor según el nivel, no el motor. Un nivel desconocido omite la herramienta.
- **Sin `--allow-actions`,** `tools/list` lleva sólo `read` y `show`, y un `mcp_call` de otro nivel ni se envía.
- **`instructions`** de cada app se juntan en las del servidor, bajo su nombre, después de las de la suite.
- **Cambios:** el servidor vigila la carpeta del registro y, cuando una app se abre o se cierra, emite `notifications/tools/list_changed` (`capabilities.tools.listChanged: true`). Pide `mcp_tools` otra vez cuando el registro de una app cambia de `pid` o de `process_start_us`.
- **Errores:** el `error.code` del motor llega como «`código`: explicación» con `isError: true` ([MCP v1 §1](../contracts/nu9ve-mcp-v1.md#1-protocolo)). Un motor que dejó de estar `live` entre `tools/list` y `tools/call` da «`not_running`: <app> no está abierta».

Alternativa descartada (decisión 1): lanzar el MCP de cada app como hijo y reenviar. Funciona sin tocar las apps, pero exige ejecutar un comando leído de un archivo y deja el catálogo fuera del motor.

## 5. Niveles y permiso por sesión

Lo de [MCP v1 §2 y §3](../contracts/nu9ve-mcp-v1.md#3-permiso-por-sesión-y-registro-de-cambios), por app:

- **Editar:** la primera edición de **cada app** en la sesión abre un diálogo nativo con el nombre de la app, el del cliente (`clientInfo.name`) y que se puede deshacer. Por defecto «No». Si se acepta, vale 60 minutos para esa app en este proceso, y cada respuesta lleva `edit_permission_minutes_left`. El servidor envía `grant: "edit"` sólo dentro de esa ventana.
- **Actuar:** un diálogo por acción, con el nombre de la app y lo que pide. `grant: "act"` sólo tras un «Sí».
- **El motor sigue mandando:** valida, registra en su «Cambios de agente» con `agent` y responde. Un motor que no tiene registro de cambios no publica herramientas `edit`.
- En builds de depuración, `NU9VE_QA_MCP_CONFIRM=yes|no` contesta los diálogos para las pruebas. En release siempre pregunta.
- Como hoy en nove, el motor confía en el `grant` de un cliente del mismo usuario: el diálogo prueba que alguien pulsó «Sí» en el escritorio, no quién.

## 6. Datos que no son instrucciones y privacidad

- Todo campo `*_untrusted` de una respuesta del motor se vuelve a limpiar en el servidor (control y bidi fuera, 300 caracteres), aunque el motor ya lo haga: es la última puerta antes del modelo. Origen: `text` y `clean` de nove `core/src/mcp.rs`.
- Salida acotada a 200 KiB por respuesta; si se recorta, se dice.
- `suite_data_catalog` y `suite_projects` entregan el texto como `title_untrusted`, `description_untrusted`, `tags_untrusted` (también el título de cada proyecto), y de un dataset `personal` sólo su forma; su referencia no aparece en las listas de los proyectos, para no revelar en qué proyecto está ([`nu9ve-data-v1` §6](../contracts/nu9ve-data-v1.md#6-privacidad); vectores en `mcp` de [`data/data-examples-v1.json`](../data/data-examples-v1.json)).

## 7. Qué sale de nove y qué no (mudar la pieza)

Origen: nove `core/src/mcp.rs` (918 líneas, leído el 2026-11-19 en `64119a9`) y el cliente de `local.rs` y `pipe.rs`. Según la regla de [AGENTS](../AGENTS.md), mudar es un cambio propio: el crate nace aquí con lo genérico, y nove, en su repo, deja un enlace y fija la primera copia en el mismo paso cuando lo adopte.

| Pieza de nove | Destino | Qué cambia |
| --- | --- | --- |
| Bucle JSON-RPC: líneas, lotes, tope de 1 MiB, −32700/−32600/−32601/−32602/−32000, versiones | `rust/nu9ve-mcp` | Suma `2025-11-25` (la de nüün). `serverInfo` por parámetro |
| `text`, `clean`, tope de 200 KiB | `nu9ve-mcp` | Igual |
| `confirm` (osascript en Mac, diálogo en Windows), ventana de 60 min, `<APP>_QA_MCP_CONFIRM` | `nu9ve-mcp` | Título y variable por parámetro |
| Cliente de socket con `lstat` y uid del par (`local.rs` `call`, `validate_socket`, `same_user`) | `rust/nu9ve-ipc` | Ruta por parámetro, no `root/runtime/ipc` |
| Cliente de pipe con PID y usuario del servidor (`pipe.rs`) | `nu9ve-ipc`, segundo corte | Windows; se prueba allá |
| Lectura del registro, vida por pid y hora de inicio | `nu9ve-ipc` | Nuevo |
| Lista de herramientas de nove, `INSTRUCTIONS`, `library_root` | Se quedan en nove | Pasan a su motor como `mcp_tools` cuando nove adopte `mcp_tools_v1` |

`nu9ve-ipc` y `nu9ve-mcp` son la Ola 3 de [qué se extrae](suite-adoption.md). Su condición era que nüün portara su MCP a Rust; `nu9ve mcp` la cumple de otra manera: es el segundo usuario del cliente de IPC y del servidor MCP, al lado de nove.

## 8. Pruebas

Con **dos motores falsos** (un binario de prueba del crate, en dos procesos con nombres de app distintos), cada uno con su socket en una carpeta temporal y su registro en `NU9VE_ENGINES_DIR`:

- `tools/list` sin `--allow-actions`: las `read` y `show` de los dos, con prefijo, todas con `readOnlyHint`.
- Un registro viejo (pid vivo con otra hora de inicio) no aparece; un registro inválido tampoco y `suite_engines` dice por qué.
- Cerrar un motor emite `list_changed` y sus herramientas desaparecen; `tools/call` a una de ellas da `not_running`.
- Edición: con `NU9VE_QA_MCP_CONFIRM=no` el motor no recibe nada; con `yes`, recibe `grant: "edit"` una vez por app.
- `_untrusted` con caracteres bidi llega limpio y acotado.
- `suite_data_catalog` sobre los dos motores da lo de `aggregate` de los vectores, y lo `personal` sólo con su forma.
- Socket de otro usuario o con permisos de grupo: rechazado antes de enviar.

Más, como pide [MCP v1 §6](../contracts/nu9ve-mcp-v1.md#6-validación), una prueba con un cliente real (Claude Code `/mcp`) registrada aparte.

## 9. Decisiones abiertas

1. **Decidido (el usuario, 2026-11-19): las herramientas salen del motor** (`mcp_tools`, §4), no del reenvío al MCP de cada app.
2. **Decidido (el usuario, 2026-11-19): primer corte de código** en el core: `nu9ve-ipc` con socket Unix y registro, `nu9ve-mcp` con el bucle y las herramientas de la suite, binario `nu9ve`, pruebas con dos motores falsos.
3. **Decidido (el usuario, 2026-11-19): de lo `personal`, sólo su forma** ([`nu9ve-data-v1` §6](../contracts/nu9ve-data-v1.md#6-privacidad)).
4. ~~**Abierta: quién instala `nu9ve`.**~~ **Decidido (el usuario, 2026-11-22):** se instala una vez por máquina como agente propio ([`nu9ve-agent-v1`](../contracts/nu9ve-agent-v1.md)), desde un instalador o desde la app que lo traiga, empezando por nüün. Gana la versión más nueva. `nu9ve mcp` es el mismo binario. La propuesta de [varias máquinas, discos y nube §6.6](suite-devices.md#66-agente-de-la-suite--nu9ve-agent) haría de este binario también un agente que corre siempre, lo que inclina la balanza hacia una instalación propia.

## 10. Primer corte (2026-11-19)

| Pieza | Qué hace | Falta |
| --- | --- | --- |
| `nu9ve-ipc` | Lee, escribe y borra registros (§1 a §6 del contrato), vida con `proc_pidinfo` (macOS) y `/proc` (Linux), cliente de socket con `lstat`, uid del par y un marco por conexión | Windows: `GetProcessTimes`, usuario del proceso, cliente de pipe y carpeta sin virtualizar. Hoy en Windows ningún registro está vivo |
| `nu9ve-mcp` | Bucle JSON-RPC, `tools/list` con prefijo y niveles, `tools/call` con permiso por app (60 min) y diálogo por acción, `list_changed` cada segundo, `suite_engines`, `suite_data_catalog` (con páginas), `suite_projects`, `engine_info` antes de usar un motor, `_untrusted` limpio, tope de 200 KiB. `catalog` valida y agrega `nu9ve-data-v1` igual que `scripts/data-examples.py` | Diálogo en Windows. Prueba con un cliente real (Claude Code `/mcp`). Ninguna app sirve todavía `mcp_tools` ni `data_catalog` ni se registra |
| Binario `nu9ve` | `nu9ve mcp [--allow-actions]`, `nu9ve --version` | Quién lo instala (§9, decisión 4) |

Pruebas (`cargo test -p nu9ve-ipc -p nu9ve-mcp`): los ejemplos de los dos contratos en Rust, y `tests/two_engines.rs`, con el binario real y dos motores falsos que sirven marcos de [`nu9ve-engine`](../rust/nu9ve-engine/) en sockets privados, más un registro de pid reutilizado. Cubren todo lo de §8 menos el socket de otro usuario, que no se puede crear sin privilegios (sí se prueba uno con permisos de grupo).
