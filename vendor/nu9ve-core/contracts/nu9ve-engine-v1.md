# Protocolo de un motor detrás de una app v1

Estado: **propuesta** (2026-11-16). Escribe como regla de la suite lo que ya hablan tres motores: `rpc-serve` y `rpc` de nüün (Python `media/cli.py` y Rust `nuun-engine`), `clipper-engine/1` de Clipper (Python `engine_rpc.py` y Rust `clipper-core serve`) y el IPC enmarcado de nove (`serve-stdio`, `serve-unix`, `serve-pipe`). Ninguno cambia por este documento; lo que cada uno hace distinto y cómo llega a v1 está en la [§12](#12-diferencias-por-app-y-camino-a-v1). Ejemplos para probar clientes y servidores: [`data/engine-examples-v1.json`](../data/engine-examples-v1.json).

**Debe** es obligatorio. **Conviene** es lo recomendado. «Motor» es el proceso que responde; «cliente», la app (o la herramienta) que lo lanza y le pregunta; «oráculo», el motor de referencia contra el que se compara otro (hoy, el de Python).

## 0. Para qué

Las tres apps tienen la misma forma: una interfaz nativa que no guarda reglas de negocio y un motor en otro proceso que sí. nüün y Clipper además portan su motor de Python a Rust método por método, con la respuesta idéntica como criterio. Un protocolo común permite un solo cliente por plataforma (Swift, C#, Python), un solo servidor en Rust cuando haga falta (el crate viene después de este contrato) y las mismas herramientas de sombra. Lo que **no** define: el catálogo de métodos de cada app, que es suyo.

Fuera de este contrato: el MCP ([`nu9ve-mcp-v1`](nu9ve-mcp-v1.md), JSON-RPC 2.0), los mensajes nativos de una extensión de Chrome (nove `broker_host.rs`, formato fijado por Chrome) y los eventos de un trabajo supervisado (Clipper `clipper-job-events/1`, candidato a `nu9ve-jobs`).

## 1. Transporte

Tres modos. Un motor ofrece uno o más; cada modo es un subcomando o una opción explícita, nunca algo que el motor adivine.

| Modo | Cómo | Límite de petición | Quién lo usa hoy |
| --- | --- | --- | --- |
| **Líneas** | NDJSON por stdin/stdout: una petición por línea, una respuesta por línea | 1 MiB por línea | nüün `rpc-serve`, Clipper `serve` |
| **Marcos** | Cada mensaje es un marco: 4 bytes de largo **big-endian** sin signo y el JSON en UTF-8. Por stdin/stdout, por socket Unix o por named pipe de Windows | 1 MiB por marco, en las dos direcciones | nove `serve-stdio`, `serve-unix`, `serve-pipe` |
| **Una vez** | La petición entera por stdin hasta EOF; una respuesta por stdout; el proceso termina | 16 MiB (nüün), 1 MiB (nove) | nüün `rpc`, nove `call` |

Para todos:

- **Debe:** UTF-8 sin BOM. Un mensaje es un objeto JSON. Ni `NaN` ni `Infinity`.
- **Debe:** stdout lleva **sólo** el protocolo. Nada de banners, avisos ni trazas: eso va a stderr, que el cliente descarta o guarda en un registro local. stderr nunca es protocolo.
- **Debe:** el motor mide el tamaño **antes** de leer el JSON. Una línea o un marco de más se rechaza con `request_too_large` (§7) y el motor sigue sirviendo: en líneas lee con ese tope y descarta hasta el siguiente `\n` (Clipper; nüün rechaza la línea, pero la lee entera antes, ver §12).

Líneas:

- **Debe:** cada respuesta es una sola línea terminada en `\n`. JSON compacto: el escritor no emite saltos de línea crudos (los de las cadenas van escapados).
- **Debe:** una línea vacía o sólo de espacios se ignora y no tiene respuesta.
- **Debe:** toda línea que el motor escribe es una respuesta a una petición. En v1 no hay notificaciones ni eventos sin `id` en este canal (§6).

Marcos:

- **Debe:** un largo de 0 o de más de 1 MiB es un error (`frame_size` hoy en nove; `request_too_large` o `response_too_large` en v1, §7).
- Por socket o pipe, nove usa **un marco por conexión**: el cliente conecta, envía, lee y cierra. Es lo que v1 pide para socket y pipe. Por stdin/stdout los marcos van uno tras otro en la misma conexión.

Una vez:

- **Debe:** stdout tiene exactamente un sobre (§3), y nada más. Puede tener indentación o un salto de línea final.
- **Conviene:** salida 0 si `ok` es `true` y distinta de 0 si no. El cliente decide por el sobre, no por el código de salida; el código sirve cuando no hubo sobre (salida vacía: `backend_failed` en el cliente de nüün).

## 2. Petición

```json
{"id":7,"method":"project_summary","params":{"project":"/Users/…/clip"}}
```

- **Debe:** `method` es una cadena. Los nombres son `snake_case` y propios de cada app.
- **Debe:** `params` es un objeto o no está. Ausente equivale a `{}`. Un cliente **no debe** enviar `null` ni otro tipo; un motor responde `invalid_params` a lo que no sea objeto (nüün acepta `null` como `{}`: tolerado, no exigido).
- **Debe:** `id` sigue la §2.1.
- **Puede** llevar `idempotency_key` (cadena de 1 a 128 bytes), que pide la §6.3 para escrituras por un canal persistente.
- **Debe:** el motor ignora los demás campos de primer nivel. Así nove puede seguir enviando `protocol` durante su transición (§12.3) y una versión menor puede sumar campos.

### 2.1 Reglas del id

- **Debe:** `id` es un entero, una cadena o `null`. Falta de `id` equivale a `null`.
- **Debe:** el motor devuelve el `id` **tal cual**: mismo tipo y mismo valor. Un entero vuelve con los mismos dígitos, **también fuera de 64 bits**: `18446744073709551616` vuelve `18446744073709551616`, no `1.8446744073709552e+19`. Python lo hace solo; en Rust exige `serde_json` con `arbitrary_precision`, como Clipper (ver [`nu9ve-json-v1` §8](nu9ve-json-v1.md#8-límites-conocidos), que también dice qué cuidar si esa feature se une al binario). `7` y `"7"` son ids distintos.
- **Debe:** un `id` decimal (`1.5`, `1e3`), booleano, objeto o arreglo es `invalid_request`, y esa respuesta lleva `id: null` (Clipper).
- **Debe:** una petición que no se pudo leer (JSON roto, no es objeto, demasiado grande) se responde con `id: null`.
- **Conviene** al cliente: enteros entre 1 y 2^53 − 1, o cadenas de hasta 128 bytes. Así cualquier lector (Swift `Double`, JavaScript) los compara bien, y nove, que limita su `request_id` a 128 bytes, los acepta. Un motor puede rechazar un `id` de cadena más largo con `invalid_request`.
- **Debe** al cliente: comparar el `id` de la respuesta con el que envió, con su tipo (Clipper `_response`), y tratar una respuesta con otro `id` como error de protocolo, nunca como la respuesta de otra petición.

## 3. Respuesta

El sobre es el de [`nu9ve-json-v1` §9](nu9ve-json-v1.md#9-sobre-de-errores):

```json
{"id":7,"ok":true,"data":{"title":"…"}}
{"id":7,"ok":false,"error":{"code":"project_not_found","message":"No project manifest at the supplied directory."}}
```

- **Debe:** `id`, `ok` (booleano) y exactamente uno de `data` (con `ok: true`) o `error` (con `ok: false`). `data` puede ser cualquier valor JSON, `null` incluido.
- **Debe:** `error.code` cumple `^[a-z_]{1,64}$` (lo que valida el cliente de Clipper) y `error.message` es una cadena para personas, que puede cambiar.
- **Puede:** `error` lleva más campos, nunca metidos en el mensaje. Los que v1 nombra: `reason` (§5), `retryable` (booleano: repetir la misma petición puede funcionar; nove) y `retry_after_seconds` (entero; nove).
- **Debe** al cliente: ignorar campos de primer nivel que no conoce (Python `rpc` de nüün agrega `command`, §12).
- **Debe:** una respuesta por petición, en el orden en que llegaron. Los motores de líneas responden en secuencia; el cliente igual empareja por `id` (el de Swift de nüün encola varias y las empareja así).
- La línea no tiene por qué ser canónica. Si un sobre se guarda o se firma, sí ([`nu9ve-json-v1` §1](nu9ve-json-v1.md#1-la-regla-en-una-línea)).

## 4. `engine_info`

Todo motor responde `engine_info` con `params` vacíos, en cualquier modo, sin tocar estado y sin `not_ported`:

```json
{"id":"handshake","ok":true,"data":{
  "contract":"nu9ve-engine/1","protocol":"clipper-engine/1","engine":"rust","version":"0.1.0",
  "capabilities":[],"ported":["engine_info","capabilities","project_summary"],"native":["score_levels"]}}
```

| Campo | Debe | Qué es |
| --- | --- | --- |
| `contract` | Sí | `"nu9ve-engine/1"`: la versión de este contrato que habla |
| `protocol` | Sí | El protocolo de la app, `<nombre>/<mayor>`: el catálogo de métodos y sus formas (`clipper-engine/1`). Sube de mayor cuando un método cambia de forma sin compatibilidad |
| `engine` | Sí | Qué implementación responde: `"python"`, `"rust"`… |
| `version` | Sí | La versión del build (`CARGO_PKG_VERSION`), la misma que `--version` |
| `capabilities` | Sí | Cadenas `snake_case` con sufijo de versión (`web_watches_v1`): rasgos que no son un método. Puede ir vacío |
| `ported` | Sí | Los métodos que este motor responde **y** que el oráculo también tiene. Son los que la sombra compara (§10) |
| `native` | Sí | Los métodos que sólo este motor tiene. Sin oráculo, sin sombra |
| `limits` | Conviene | `{"request_bytes":…,"response_bytes":…}` de este modo |

- **Debe:** `ported` y `native` no se repiten entre sí. `engine_info` va en `ported` si el oráculo lo responde, si no en `native`. Un motor sin oráculo (nove) pone todo en `native` y `ported` vacío.
- **Debe:** la respuesta describe **este proceso en esta máquina** y no cambia mientras vive. Lo que depende de la plataforma (nüün: las lecturas de volúmenes sólo con un backend de volúmenes) ya va resuelto en las listas.
- **Puede:** campos propios de la app (nüün `writes`, `once`, `volume_backend`, `state_dir`; Clipper `queue_views`; nove `state_schema`, `tzdb_version`, `network`). El cliente ignora los que no conoce.
- **Conviene** al cliente: pedir `engine_info` primero, con un plazo de arranque (Clipper: 2 s), y no usar el motor si `contract` o `protocol` no son los que conoce: entonces responde el oráculo o la app avisa. Lo pide una vez por proceso y guarda las listas.

## 5. `not_ported`

```json
{"id":9,"ok":false,"error":{"code":"not_ported","message":"This method is available in the Python engine.","reason":"validator_input"}}
```

- **Significa:** «este motor todavía no sabe responder esta petición, y **no hizo nada**». Ni escribió, ni movió media, ni dejó un trabajo a medias. Por eso el cliente **puede** enviar la misma petición al oráculo.
- **Debe:** `reason` es un token que cumple `^[a-z_]{1,40}$` (lo que la sombra de Clipper acepta). Dice por qué, para contar en la sombra qué falta portar; nunca lleva datos de la persona. Dos tokens comunes:
  - `method_unavailable`: el método existe en el oráculo y este motor no lo tiene;
  - `platform_unavailable`: el motor lo tiene, pero no en esta plataforma (nüün sin backend de volúmenes).

  Los demás son de cada app (`validator_input`, `config_value`, `civil_datetime`… en Clipper).
- **Debe:** un método de `ported` **puede** contestar `not_ported` para algunos parámetros (un caso que el port aún no cubre). Uno que no está en ninguna lista es `unknown_method`, no `not_ported`.
- **Debe:** un motor que ya empezó a producir efectos no responde `not_ported`: responde el error real.

## 6. Respuestas grandes, progreso, cancelación y escrituras

### 6.1 Respuestas grandes

- **Puede:** el motor escribe `data` ya serializado, sin armar el valor en memoria (Clipper con `project_summary`: escribe `{"id":…,"ok":true,"data":` y pega los bytes). Los bytes **deben** ser un valor JSON válido y, en líneas, sin saltos de línea crudos.
- **Debe** al cliente de líneas: aceptar respuestas de al menos 16 MiB (Clipper acepta 16 MiB; nüün, 32 MiB en una vez). Un cliente que corta antes trata la respuesta como `response_too_large`.
- **Debe:** en marcos el tope es 1 MiB. Una respuesta mayor **debe** volver como `response_too_large` y **no debe** perderse en silencio (hoy nove no escribe nada, §12). Lo que puede crecer se pagina: `before`/`limit` en nove.

### 6.2 Progreso y cancelación

v1 no lleva progreso ni eventos dentro del canal: una petición, una respuesta. Lo que hacen hoy las apps, y que v1 recoge:

- **Debe:** un trabajo largo **no** corre dentro del bucle de un motor persistente, porque detendría las demás lecturas. Corre en un proceso de una vez (nüün: una mutación por proceso), en un trabajo supervisado (Clipper `clipper-core job run`) o en un worker del motor (nove).
- **El progreso se lee:** un método de lectura que el cliente sondea (nüün `action_progress`, `closure_progress`, `storage_progress`; nove el estado de sus trabajos y `events` con cursor `after`).
- **Cancelar:**
  - un proceso de una vez se cancela con `SIGINT` o `SIGTERM` al proceso (nüün), que termina con el código `cancelled`;
  - **en Windows**, donde no hay `SIGINT`, primero se cancela de forma cooperativa: un método `<cosa>_cancel` o un archivo de cancelación que el motor vigila, como el de Clipper. Después hay un plazo de gracia, y si el motor no terminó, el cliente lo corta con `TerminateJobObject` sobre el Job Object en el que lo arrancó, para que caigan también sus hijos. **No debe** depender de `GenerateConsoleCtrlEvent`, que no llega a un proceso sin consola. Cortado así, el cliente informa `cancelled` y no da por publicada ninguna salida;
  - un trabajo del motor se cancela con un método propio, `<cosa>_cancel` (nove `run_cancel`, `history_cancel`), que es una escritura (§6.3);
  - un trabajo supervisado, con señal o archivo de cancelación (Clipper).
- **Debe:** `cancelled` es el código de lo que se canceló, y lo que se canceló no publica salidas parciales.

### 6.3 Escrituras y respuestas perdidas

Un plazo vencido no dice si la petición se ejecutó. Clipper distingue si el mensaje llegó entero al motor (`dispatched`); nove contesta `response_timeout` con «consulta el estado antes de repetir».

- **Debe:** una escritura por un canal persistente lleva una clave de idempotencia: `idempotency_key` de primer nivel (nove) o un `request_id` en `params` (Clipper). La misma clave con los mismos parámetros devuelve el mismo resultado sin repetir el efecto; con otros parámetros, error.
- **Debe:** el motor ofrece cómo preguntar el resultado de una clave (Clipper `write_status`; nove el estado del trabajo o del comando).
- **Debe** al cliente: tras un plazo vencido o una conexión caída **después** de enviar, no reintentar a ciegas ni ir al oráculo: preguntar el estado. Sólo una lectura, o un error que llegó antes de enviar, pasa al oráculo.
- **Conviene** en una vez: la misma clave. nüün hoy no la lleva (§12).

### 6.4 Leases de sesión

Propuesta del 2026-11-22, tomada de las leases de reproducción de Clipper (M8, su `APP_SERVICES_CONTRACT.md`). Una *lease* es un método cuya respuesta dice «retenido»: el motor toma un recurso (candados de media, una fuente lista para reproducir) y lo mantiene mientras vive su proceso, después de responder.

- **Debe:** el motor lista el método de lease y su método de liberación en `ported` o `native`, y la app documenta cuáles son. Clipper: `project_output_lease`, `group_output_lease`, `project_source_lease` y `group_source_lease`, que se liberan con `playback_lease_release`.
- **Debe:** a lo sumo una lease activa por proceso. Pedir otra con una activa, o liberar sin ninguna, es un error del método con código de la app (Clipper `playback_lease_active`, `playback_lease_missing`).
- **Debe:** la liberación es un método con `params` vacíos en el mismo proceso, que responde cuando el recurso ya se soltó. Cerrar stdin (EOF) o la salida del proceso también lo sueltan.
- **Debe:** una lease que el motor no puede dar responde `not_ported` (§5) sin retener nada.
- **Debe** al cliente: un proceso propio para cada lease, que no se comparte con las demás peticiones y no se reinicia. Si ese proceso muere, la lease se perdió y la app se entera; nunca se readquiere sin que la app lo pida. Cerrar sin liberar es cerrar stdin (§8).
- **Conviene:** la adquisición y la liberación dicen su fase en `data` (Clipper `phase: "ready"` y `"released"`) y lo que la app necesita para verificar lo retenido (una `revision`).
- Qué se retiene y cómo se llaman los métodos es de la app.

## 7. Códigos de error

Un código es lo que el programa compara; por eso un error de transporte y uno del método no comparten nombre. Los de la tabla son de la suite; cada app suma los de su dominio (`project_not_found`, `queue_busy`, `read_only_volume`…).

**Los escribe el motor:**

| Código | Cuándo | Hoy |
| --- | --- | --- |
| `invalid_request` | JSON roto, no es objeto, `method` no es cadena, `id` de otro tipo | Clipper; nüün escribe `operation_failed` para JSON roto; nove `invalid_json` |
| `invalid_params` | `params` no es objeto, o falta o sobra un parámetro | Clipper; nove `invalid_json`; nüün `operation_failed` (`KeyError`) |
| `request_too_large` | Petición sobre el límite | nüün, Clipper; nove `frame_size` |
| `response_too_large` | Respuesta sobre el límite del modo | nove `frame_size`, sin respuesta |
| `unknown_method` | Ningún motor de la app lo tiene | nüün, Clipper; nove `invalid_json` |
| `not_ported` | §5 | nüün (sin `reason`), Clipper |
| `read_only` | Escritura en un modo que sólo lee (`rpc-serve`) | nüün |
| `protocol_version` | La petición pide otro mayor | nove |
| `engine_busy` | La cola del motor está llena; `retryable` | nove |
| `response_timeout` | El motor no terminó a tiempo; consultar el estado (§6.3) | nove |
| `idempotency_key` | Escritura sin clave válida | nove |
| `cancelled` | Se canceló (§6.2) | nüün (cliente), Clipper (trabajos) |
| `internal_error` | Falla que el motor no clasificó. El mensaje no lleva trazas. `operation_failed` no es un código de la suite (§13, decisión 1) | Clipper; nüün usa `operation_failed` |

**Los produce el cliente,** nunca vienen del motor: `engine_missing` (no hay binario o runtime; nüün `runtime_missing`), `startup_failed`, `startup_timeout`, `incompatible_engine` (§4), `timeout`, `connection_lost` (nüün `reader_unavailable`, nove `ipc_closed`), `engine_unavailable` (socket o pipe que no contesta; nove `ipc_unavailable`), `peer_user` y `unsafe_endpoint` (§9), `invalid_response` (el motor rompió el protocolo: un sobre inválido o un `id` que nadie pidió, §2.1; el cliente cierra ese proceso) y `lease_ended` (§6.4: liberar una lease que ya terminó). **Conviene** que cada cliente use estos nombres; uno que conserve los suyos documenta la equivalencia.

## 8. Ciclo de vida del proceso

- **Arranque:** el cliente lanza el motor como hijo suyo, con ruta absoluta, argumentos explícitos (la carpeta de estado: `--state-dir` en nüün, `--root` en nove; el modo: `rpc-serve`, `serve`, `serve-unix`) y el entorno que decide (nüün `EngineEnvironment` aísla el `HOME` en QA). stdin y stdout son pipes; stderr, nulo o un registro.
- **Debe:** `--version` escribe una línea `<binario> <versión>` y sale con 0 (`nuun-engine 0.1.0`, `clipper-core 0.1.0`). Un uso incorrecto sale con 2 y escribe el uso en stderr.
- **Listo** es cuando responde `engine_info`. Un cliente persistente **conviene** que lo arranque perezoso, en la primera petición.
- **Caída:** si el proceso termina, toda petición pendiente falla con `connection_lost` y la siguiente arranca otro. **Conviene** espaciar los reintentos (Clipper: 10 s, el doble cada vez, hasta 320 s).
- **Cierre de líneas y marcos por stdio:** el cliente cierra stdin. El motor termina la petición en curso y sale con 0. Si stdout se cierra (`EPIPE`), sale con 0 sin ruido. El cliente espera un plazo corto (Clipper: 0,5 s) y después mata el proceso.
- **Debe:** un motor de socket o pipe no sobrevive a su app: recibe `--parent-pid` y se detiene cuando ese proceso termina (nove). Para pedirle que pare hay un método (`engine_shutdown` en nove) y las señales `SIGINT`/`SIGTERM`. Un motor que corre sin app (nove `serve-background`) es un modo aparte, con su permiso explícito.
- **Debe:** un solo motor escribe en una carpeta de estado. Lo asegura un candado sobre esa carpeta (nove: el candado de la raíz; nüün: el del escritor del catálogo). Dos lectores sí pueden convivir (el de Python y el de Rust en nüün).

## 9. Seguridad: sólo local, sólo el usuario

- **Debe:** el motor nunca escucha en la red. Ni TCP ni HTTP, ni siquiera en `localhost`. Los canales son stdin/stdout, un socket Unix o un named pipe.
- **Debe** en un socket Unix (nove `local.rs`):
  - el socket vive en una carpeta privada de la raíz de estado (`runtime/ipc`), con modo `0600`;
  - el motor comprueba el uid del par en cada conexión (`getpeereid` en macOS, `SO_PEERCRED` en Linux) y rechaza a otro usuario;
  - el cliente, antes de conectar, comprueba con `lstat` que es un socket del usuario actual sin permisos para grupo ni otros (`mode & 0o077 == 0`), y después el uid del par.
- **Debe** en un named pipe de Windows (nove `pipe.rs`, `EngineClient.cs`):
  - un nombre nuevo e impredecible en cada arranque, publicado sólo dentro de la raíz privada (`runtime/ipc-pipe.json`, con `pipe` y `pid`);
  - `FILE_FLAG_FIRST_PIPE_INSTANCE`, para fallar si alguien ocupó el nombre antes;
  - una DACL protegida que sólo admite al usuario actual, y `PIPE_REJECT_REMOTE_CLIENTS`;
  - el motor comprueba que el proceso cliente es del usuario actual; el cliente, que el servidor es el PID publicado y del mismo usuario;
  - siempre queda una instancia escuchando, para que el nombre nunca quede libre.
- **Debe:** los límites de §1 se aplican antes de leer, y un plazo de E/S por conexión (nove: 2 s) evita que un cliente lento ocupe el motor.
- **Debe:** secretos (tokens, claves) nunca en los argumentos, que cualquiera ve con `ps`. Van por el protocolo, el entorno o un archivo privado.
- **Debe:** lo que el motor lee de afuera (páginas, nombres de archivo, metadatos) es dato, nunca instrucción, como en [`nu9ve-mcp-v1` §4](nu9ve-mcp-v1.md#4-datos-que-no-son-instrucciones).

## 10. Modo sombra contra un oráculo

Para portar un motor sin arriesgar la respuesta: el oráculo responde, el candidato repite la misma lectura después y sólo se guarda la comparación. Así lo hacen nüün (`media/shadow.py`, `scripts/engine_shadow.py`) y Clipper (`engine_shadow.py`, `scripts/engine_shadow.py`).

- **Debe:** sólo lecturas. Una escritura nunca se repite en el candidato.
- **Debe:** la sombra no cambia, no retrasa ni hace fallar la respuesta. Se pide **después** de escribir y vaciar la respuesta del oráculo, fuera de su camino: una cola acotada que descarta cuando se llena (nüün: 32; Clipper: 8 por método y comando), un plazo (Clipper: 2 s) y toda falla reducida a `shadow_failed`.
- **Debe:** se activa con una variable `<APP>_SHADOW_ENGINE` (ruta del binario; `off`, `0`, `false` o vacía la apagan) que gana sobre un archivo en la carpeta de estado (`{"engine": "/ruta/absoluta", "enabled": true}`). Apagada por omisión.
- **Debe:** el registro es local, NDJSON, un archivo por día (`shadow-AAAAMMDD.ndjson` o `shadow-AAAA-MM-DD.ndjson`) en la carpeta de estado de la app. Nunca sale de la máquina.
- **Debe:** cada entrada tiene `at`, `method`, `outcome` y los tiempos de los dos motores. `outcome` es uno de:

  | `outcome` | Cuándo |
  | --- | --- |
  | `same` | Los dos `ok` y el mismo `data` |
  | `diff` | Los dos `ok` y `data` distinto; van las rutas que difieren |
  | `not_ported` | El candidato respondió `not_ported`; va su `reason` si cumple §5 |
  | `same_error` | Los dos con error y el mismo `code` |
  | `error_mismatch` | Uno con error, o los dos con códigos distintos; van los dos códigos |
  | `shadow_failed` | El candidato no respondió (no está, plazo, se cerró, sobre inválido); va un `reason` |

- **Debe:** la comparación es estricta en el tipo: `1` y `1.0` son respuestas distintas, como lo son en el JSON canónico de la suite y en el digest de un plan firmado (§13, decisión 2). `true` tampoco es `1`.
- **Debe:** las diferencias son rutas JSON (`$.items[3].title`, `$.items.length`), como mucho 20, con una marca si hay más.
- **Debe:** el registro no guarda valores, sólo rutas, códigos y tiempos (Clipper), porque los valores son títulos y rutas de la persona (§13, decisión 3). Tampoco el mensaje de un error, que puede llevarlos.
- **Debe:** lo que cambia entre dos ejecuciones (la hora en que se pidió: nüün `checked_at`, Clipper `generated_at`) se declara por método y se quita de las dos respuestas antes de comparar. Que el campo exista sí se compara.
- **Conviene:** una herramienta con `enable`, `disable` y `report` (resultados por método y medianas de tiempo), y una con `replay`, que pide las lecturas sin parámetros sobre una carpeta de estado real a los dos motores (nüün).

**Del sombreado a responder.** Cuando un método da `same` en datos reales, el cliente puede pedírselo primero al candidato:

- **Debe:** sólo los métodos que el candidato lista en `ported` o `native`.
- **Debe:** pasa al oráculo sólo con `not_ported`, o con un error de transporte **antes** de enviar la petición (Clipper `before_dispatch`; nüün `not_ported`, `reader_unavailable`, `runtime_missing`). Cualquier otro error del candidato es la respuesta, como lo sería la del oráculo.
- **Conviene:** guardar qué motor respondió cada método, para el diagnóstico (nüün `answeredBy`).

## 11. Ejemplos

[`data/engine-examples-v1.json`](../data/engine-examples-v1.json): sobres válidos e inválidos para el lector del cliente, peticiones de entrada con la respuesta que un motor debe dar, ids que vuelven tal cual, un marco en hexadecimal, sobres viejos de nove con su forma v1 y comparaciones de sombra con su `outcome`. Todo va como **texto JSON**, para que cada lenguaje lo lea con su propio lector (así se prueba también que un entero de más de 64 bits no se vuelve decimal). `python3 -I scripts/engine-examples.py --check` comprueba el archivo contra una implementación de referencia de las reglas del cliente; los casos de motor los prueba cada motor.

## 12. Diferencias por app y camino a v1

Leído el 2026-11-16: nüün en `n-creator-utils` (`fc1d3bb`), Clipper (`41084ccb`), nove (`9935227`).

| Regla | nüün | Clipper | nove |
| --- | --- | --- | --- |
| Transporte (§1) | Líneas (`rpc-serve`) y una vez (`rpc`) | Líneas (`serve`) | Marcos por stdio, socket y pipe; una vez (`call`) |
| Petición (§2) | `{id, method, params}`; `params: null` vale `{}`; Rust acepta `params` que no son objeto | `{id, method, params}`; `null` es `invalid_params` | `{protocol, request_id, idempotency_key, method, params}` |
| id (§2.1) | Cualquier valor vuelve tal cual (`1.5`, `true`, objetos); Rust escribe los enteros fuera de 64 bits como decimal; sin `method`, la respuesta pierde el `id` | Entero, cadena o `null`; enteros grandes exactos | `request_id` cadena de hasta 128 bytes |
| Sobre (§3) | v1. `rpc` de Python sin `id`, con `command` e indentación | v1 | `{protocol, request_id, result, error}`, error con `retryable` |
| `engine_info` (§4) | Sólo Rust: `engine`, `version`, `ported`, `writes`, `once`… Sin `contract`, `protocol`, `native` ni `capabilities` | `engine`, `protocol`, `ported`, `native`, `queue_views`. Sin `contract`, `version` ni `capabilities` | `hello`: `build`, `protocol: 1`, `capabilities`… |
| `not_ported` (§5) | Sin `reason` | Con `reason` | No aplica: sin oráculo |
| Errores (§7) | `operation_failed` para JSON roto, parámetros e imprevistos; en `rpc-serve`, un método desconocido es `read_only`, no `unknown_method` | v1 | `invalid_json`, `frame_size` |
| Casos de motor de los ejemplos (§11), medido el 2026-11-16 | Rust: 9 de 20; Python: 2 de 20 (aún no tiene `engine_info`) | Python y Rust: 20 de 20 | Sin medir: su petición no es v1 |
| Respuesta grande (§6.1) | — | `data` ya serializado | Más de 1 MiB: no responde nada |
| Línea de más (§1) | Python y Rust la leen entera y la rechazan; sin descarte acotado | Descarta hasta `\n` | Marco: error de tamaño |
| Escrituras (§6.3) | Sólo en una vez, sin clave | `request_id`, `expected_revision`, recibos y `write_status` | `idempotency_key`, `response_timeout` |
| Sombra (§10) | Registra valores cortos; `1` y `1.0` son iguales; quita `not_ported` de `data` (respuesta parcial) | Sólo rutas; compara tipos | No aplica |
| Seguridad (§9) | stdio | stdio | Socket `0600` con uid del par; pipe con DACL y PID |

### 12.1 nüün

1. Rust: `engine_info` suma `contract`, `protocol` (propuesta: `nuun-media/1`), `native` (vacío) y `capabilities`. Python responde también `engine_info`, para que el cliente lo pida a cualquiera de los dos.
2. `not_ported` con `reason`: `method_unavailable` para «Method not ported…» y `platform_unavailable` para «No volume backend…».
3. Errores: `invalid_request` para JSON roto, `invalid_params` para parámetros y `unknown_method` antes que `read_only`, en Python y Rust a la vez, para que la sombra siga en `same_error`. Los imprevistos pasan a `internal_error`.
4. id: leer el `id` antes de validar `method`, aceptar sólo entero, cadena o `null` y, en el `nuun-engine`, conservar los enteros grandes (con `arbitrary_precision`, revisando antes lo que dice `nu9ve-json-v1` §8) o rechazarlos con `invalid_request`. Python hace lo mismo.
5. Línea de más: leer con tope y descartar hasta `\n`, como Clipper.
6. `rpc` de Python: el sobre con `id` (`null` si la petición no lo trae). `command` puede seguir mientras algún script lo lea.
7. Sombra: comparar tipos (`1` ya no es `1.0`) y dejar de guardar valores en el registro (hoy los valores de `python` y `rust` en `diffs`, y `rust_message`); sólo rutas, códigos y tiempos. Lo que hoy da `same` por la comparación laxa puede pasar a `diff`: es una diferencia real del port, no de la sombra.

### 12.2 Clipper

Ya habla casi todo v1. Falta:

1. `engine_info` suma `contract`, `version` y `capabilities` (vacío está bien), en Python y Rust.
2. El cliente (`engine_client.py`) acepta un `engine_info` con esos campos y valida `contract`.
3. Usar `method_unavailable` como ya hace, y documentar sus demás tokens de `reason` en su repo.

### 12.3 nove: transición con los dos sobres

nove no tiene oráculo ni sombra; adopta el transporte de marcos tal como está, el sobre y `engine_info`. Para no cambiar todo a la vez, el **motor elige el sobre por la petición** y los **clientes leen los dos**:

1. **Los clientes leen los dos sobres.** Swift (`Protocol.swift`), C# (`EngineClient.cs`), `broker_host.rs` y el cliente de `pipe-call`. Si el objeto tiene `ok` booleano, es v1. Si no, y tiene `request_id` y `result` o `error`, es el viejo: `result` se lee como `data` y `request_id` como `id`. Cualquier otra forma es error de protocolo. Ningún cambio de comportamiento: el motor sigue respondiendo el sobre viejo.
2. **El motor responde v1 a quien pregunta v1.** Una petición con `id` y sin `request_id` recibe `{id, ok, data | error}`; una con `request_id`, el sobre viejo. `protocol` deja de ser obligatorio cuando falta `request_id`. `engine_info` se suma a `hello`, con `contract`, `protocol: "nove/1"`, `engine: "rust"`, `version` (hoy `build`), `capabilities` (las mismas), `ported: []` y `native` con sus métodos. `idempotency_key` sigue en el primer nivel, en los dos sobres. `retryable` y `retry_after_seconds` siguen en `error`.
3. **Los clientes envían `id`.** Cada uno en su propio cambio. Los nombres de error pasan a v1 (`invalid_json` → `invalid_request` o `invalid_params`; `frame_size` → `request_too_large` o `response_too_large`), y una respuesta de más de 1 MiB vuelve como `response_too_large`.
4. **Se retira el sobre viejo** en la primera versión de nove en que todos sus clientes (Swift, C#, `broker_host.rs` y `pipe-call`) envían `id`. Va atado a ese hito, no a una fecha (§13).

Los mensajes nativos de la extensión de Chrome no cambian: su formato lo fija Chrome.

### 12.4 Cliente de Swift: `Nu9veEngine`

Desde el 2026-11-22 el paquete de Swift del core trae el cliente de este contrato ([`swift/README.md`](../swift/README.md#nu9veengine-cliente-de-nu9ve-engine-v1)). Parte del `LineServer` de nüün (`Protocol.swift`): un proceso persistente por líneas que empareja por `id`. Suma lo que ese no tenía: ids exactos (lee el JSON con su propio lector, que guarda los números como texto), handshake con `contract` y `protocol`, plazo por petición, lectura acotada, reinicio con espera creciente, `not_ported` como resultado y las leases de la §6.4. De nove no toma nada: su transporte es de marcos por socket o pipe, que este cliente todavía no habla.

| Regla | Cómo la cumple |
| --- | --- |
| Transporte (§1) | Líneas por stdin/stdout. Petición de más de 1 MiB: `request_too_large` sin enviarla. Respuesta de más de 16 MiB (configurable): se descarta hasta su `\n` y esa llamada falla con `response_too_large`; el proceso sigue |
| id (§2.1) | Enteros desde 1. Compara tipo y dígitos; un `id` que nadie pidió, o un sobre inválido, es `invalid_response` y cierra el proceso. Un error con `id: null` (petición ilegible) responde a la más antigua, porque el motor responde en orden |
| `engine_info` (§4) | Lo pide al arrancar, con plazo de 2 s; sin `contract` v1 o con otro `protocol`, `incompatible_engine` |
| `not_ported` (§5) | Resultado `.notPorted(reason:)`, no error. Un método que no está en `ported` ni `native` da `method_unavailable` sin preguntar |
| Escrituras (§6.3) | `idempotency_key` opcional; todo error de transporte dice si la petición llegó entera (`dispatched`) |
| Ciclo de vida (§8) | Arranque perezoso; plazo vencido o caída cierran el proceso y fallan lo pendiente. El primer reinicio es inmediato y los siguientes esperan 10 s, el doble cada vez, hasta 320 s, con `engine_unavailable` y `retry_after_seconds` mientras tanto. Cierre: stdin, 0,5 s, `SIGTERM`, 0,5 s, `SIGKILL` |
| Leases (§6.4) | `openLease(_:_:release:)`: un proceso propio, aviso si muere, `release()` y `close()` (EOF) |

Lo prueban los 20 casos de motor de la §11, enviados crudos por el transporte del cliente a un motor de prueba del paquete y, con `NU9VE_ENGINE_LIVE_PROBE`, al motor real de una app; qué pasó dónde está en [adopción](../docs/adoption.md#nu9ve-engine-v1). No cubre: marcos (nove), el modo de una vez (nüün `rpc`), la lectura de los dos sobres de nove y la sombra, que siguen en cada app.

**Neuf** es su primer usuario previsto: fija `swift` con su `sync-core.sh`, abre `clipper-core serve` con `expectedProtocol: "clipper-engine/1"` y llama a las vistas de M8 (`review_item`, `preview_item`, `effective_timeline`, `effective_transcript`, `project_summary`) sin Python; con `not_ported`, la app avisa o usa su respaldo.

## 13. Decisiones abiertas

Del usuario. Todas están decididas; cada una es regla en la sección que nombra.

1. **Decidido (el usuario, 2026-11-16): el código de un imprevisto es `internal_error`** (Clipper), no `operation_failed` (nüün), que suena a un fallo esperado del método. En nüün cambia en Python y Rust en el mismo paso.
2. **Decidido (el usuario, 2026-11-16): en la sombra, `1` y `1.0` son distintas** (Clipper). El JSON canónico de la suite los escribe distinto, y un plan firmado cambiaría de digest.
3. **Decidido (el usuario, 2026-11-16): el registro de la sombra guarda sólo rutas,** códigos y tiempos (Clipper), sin valores, porque los valores son títulos y rutas de la persona.
4. **Decidido (el usuario, 2026-11-16): en Windows se cancela primero de forma cooperativa,** con un método o un archivo de cancelación y un plazo de gracia, y al final con `TerminateJobObject` sobre su Job Object. Sin `GenerateConsoleCtrlEvent` (§6.2).
5. **Decidido (el usuario, 2026-11-16): no hay progreso ni eventos dentro del canal,** ni ahora ni en una v2 prevista. El progreso se sondea (§6.2). Si una app necesita un flujo de eventos, eso es de `nu9ve-jobs`, que ya los tiene en el supervisor de Clipper, no de este protocolo.
6. **Decidido (el usuario, 2026-11-16): nove retira su sobre viejo por hito, no por fecha,** en la primera versión en que todos sus clientes envían `id` (paso 4 de la §12.3).
7. **Decidido (el usuario, 2026-11-16): el crate de servidor `nu9ve-engine` nace desde Clipper,** que ya pasa los 20 casos de motor en Rust. nüün lo adopta cuando cierre sus brechas de la §12.1 y cambie a `internal_error`.

## Adopción

Quién adoptó qué está en [adopción por app](../docs/adoption.md#nu9ve-engine-v1).
