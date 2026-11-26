# Registro de motores v1

Estado: **propuesta** (2026-11-19). Define cómo un motor que corre se anuncia a los demás procesos del mismo usuario, para que un cliente de la suite (hoy `nu9ve mcp`, [diseño](../docs/suite-mcp.md)) sepa qué apps están abiertas y cómo hablarles sin adivinar rutas. Nace de nove [data-hub §6](../../nove/docs/data-hub.md#6-una-sola-puerta-registro-de-motores-y-mcp-de-la-suite) y de lo que nove ya hace para publicar su pipe (`runtime/ipc-pipe.json`, [motor §9](nu9ve-engine-v1.md#9-seguridad-sólo-local-sólo-el-usuario)). Ejemplos: [`data/registry-examples-v1.json`](../data/registry-examples-v1.json).

**Debe** es obligatorio. **Conviene** es lo recomendado. «Motor» y «cliente» como en [`nu9ve-engine-v1`](nu9ve-engine-v1.md). «Registro» es el archivo de un motor; «carpeta», donde viven todos.

## 0. Para qué y para qué no

- **Para** descubrir: qué app tiene su motor vivo, por dónde se le habla, qué versión y qué capacidades tiene.
- **No** es un canal: nada se pide ni se responde por el registro. Se habla con el motor por su transporte, con las reglas y comprobaciones de [`nu9ve-engine-v1`](nu9ve-engine-v1.md).
- **No** arranca motores. Un cliente que no encuentra un registro vivo dice que la app no está abierta, como ya pide [MCP v1 §0](nu9ve-mcp-v1.md#0-principios).
- **No** sustituye lo que cada app publica dentro de su raíz privada (nove `runtime/ipc-pipe.json`): eso sigue para sus propios clientes. El registro es la copia pública para la suite, del mismo usuario.

## 1. Dónde

| Sistema | Carpeta |
| --- | --- |
| macOS | `~/Library/Application Support/nu9ve/engines/` |
| Windows | `%LOCALAPPDATA%\nu9ve\engines\` |

- **Debe:** la variable `NU9VE_ENGINES_DIR` (ruta absoluta) reemplaza la carpeta, para el motor y para el cliente. Las pruebas y la QA la usan **siempre**: un motor con raíz de QA (`<APP>_QA_ROOT` o similar) **no debe** escribir en la carpeta de la persona. Si la variable no es absoluta, el motor no se registra y el cliente no lee nada.
- **Debe** en Windows: una app MSIX escribe la carpeta **sin virtualizar**, igual que su biblioteca ([Windows §9](windows-app-v1.md#9-paquete-y-distribución), `unvirtualizedResources`). Si no, el archivo queda en la copia privada del paquete y ningún otro proceso lo ve. Sin probar todavía en un paquete real (§9).
- Otros sistemas: sin carpeta en v1. Ninguna app de la suite corre ahí.

## 2. Archivo

Un archivo por app: `<app>.json`, con `<app>` que cumple `^[a-z][a-z0-9-]{0,31}$` y es el **nombre corto de la app en minúsculas y sin acentos** (`nove`, `nuun`, `clipper`). Es el mismo que va en el prefijo de sus herramientas en `nu9ve mcp`.

```json
{
  "contract": "nu9ve-registry/1",
  "app": "nove",
  "app_version": "0.12.0",
  "pid": 4242,
  "process_start_us": 1795100645123456,
  "transport": {"kind": "unix", "path": "/Users/ana/Library/Application Support/nove-development/engine/runtime/ipc"},
  "engine": {"contract": "nu9ve-engine/1", "protocol": "nove/1", "engine": "rust", "version": "0.12.0"},
  "capabilities": ["data_catalog_v1", "mcp_tools_v1"],
  "written_at": "2026-11-19T15:04:05Z"
}
```

| Campo | Debe | Qué es |
| --- | --- | --- |
| `contract` | Sí | `"nu9ve-registry/1"` |
| `app` | Sí | Igual al nombre del archivo sin `.json` |
| `app_version` | Sí | La versión de la app que lanzó el motor (la de su «Acerca de») |
| `pid` | Sí | El proceso que **sirve** el transporte (el motor, no la app), entero entre 1 y 2^32 − 1 |
| `process_start_us` | Sí | Cuándo empezó ese proceso, en microsegundos desde 1970 UTC, **leído del sistema** (§4), nunca del reloj del motor al arrancar |
| `transport` | Sí | §3 |
| `engine` | Sí | `contract`, `protocol`, `engine` y `version` copiados de su `engine_info` ([motor §4](nu9ve-engine-v1.md#4-engine_info)) |
| `capabilities` | Sí | Las de su `engine_info`, tal cual. Puede ir vacío. Las que lee la suite: `data_catalog_v1` ([`nu9ve-data-v1`](nu9ve-data-v1.md)), `mcp_tools_v1` ([diseño](../docs/suite-mcp.md#4-herramientas-de-cada-app)) |
| `written_at` | Sí | Cuándo se escribió, RFC 3339 en UTC con `Z`. Sólo para personas: la vida la decide §4 |

- **Debe:** UTF-8 sin BOM, un objeto JSON, como mucho **16 KiB**. Todos los enteros caben en 2^53 − 1.
- **Debe** al cliente: ignorar campos de primer nivel que no conoce; tratar como inválido (§5) un registro al que le falta uno de la tabla o lo trae de otro tipo.
- **No debe:** secretos, tokens, la ruta de la biblioteca de la persona más allá de lo que exige el transporte, ni datos de sus proyectos. El registro dice que la app está abierta y cómo llamarla; nada más.
- **Conviene:** `library` (cadena corta: `personal`, `development`) cuando la app tiene varias bibliotecas, para que la persona sepa cuál está abierta. No cambia cómo se le habla.
- **Versión menor del 2026-11-25:** `profile_id` (opcional) es el perfil de [`nu9ve-profile-v1`](nu9ve-profile-v1.md#2-archivo) de la biblioteca que sirve el motor (`^[a-z0-9][a-z0-9-]{0,31}$`). Falta = el motor no tiene perfil. Un valor que no es una cadena con esa forma hace inválido el registro (`bad_profile_id`). Un cliente anterior lo ignora, como cualquier campo que no conoce. El registro dice el id, nunca la ruta de la biblioteca del perfil.

## 3. Transporte

El de [motor §1](nu9ve-engine-v1.md#1-transporte) en modo **marcos, un marco por conexión**, que es lo único que un proceso ajeno puede usar (stdio es del padre).

| `kind` | Campos | Comprobaciones antes de enviar ([motor §9](nu9ve-engine-v1.md#9-seguridad-sólo-local-sólo-el-usuario)) |
| --- | --- | --- |
| `unix` | `path`: ruta absoluta del socket | `lstat`: es un socket, del usuario actual, `mode & 0o077 == 0`; tras conectar, el uid del par es el del usuario |
| `pipe` | `name`: `\\.\pipe\<nombre>` | El servidor del pipe es el proceso `pid` del registro y es del usuario actual |

- **Debe:** un motor que sólo habla por stdio con su app (nüün `rpc-serve`, Clipper `serve`) **no se registra** hasta que sirva un socket o un pipe con esas reglas. El registro no inventa otro canal.
- **Debe** al cliente: un `kind` desconocido hace inválido el registro (§5); no se intenta otro.

## 4. Vida: pid con hora de inicio

Un pid se reutiliza. Por eso un registro está **vivo** sólo si, en este momento:

1. existe un proceso con ese `pid`;
2. es del usuario actual;
3. su hora de inicio, leída del sistema y truncada a microsegundos, es **igual** a `process_start_us`.

Si falla una, el registro está **viejo** y se ignora: la app no está abierta.

| Sistema | Hora de inicio del proceso |
| --- | --- |
| macOS | `proc_pidinfo(pid, PROC_PIDTBSDINFO)`: `pbi_start_tvsec * 1_000_000 + pbi_start_tvusec` |
| Windows | `GetProcessTimes`: `CreationTime` (FILETIME, 100 ns desde 1601) → `(ft − 116444736000000000) / 10` |

- **Debe:** el motor obtiene su propia hora con la **misma** llamada que el cliente usará, sobre su propio pid. Así la igualdad es exacta, sin tolerancia.
- **Debe:** el cliente comprueba la vida **cada vez** que va a usar un registro, no una vez al arrancar: el motor puede haberse ido entre dos llamadas.
- Pasar la vida no prueba quién es el proceso, sólo que es el mismo que escribió el archivo. Lo que prueba el canal es §3.

## 5. Escribir, reemplazar, borrar

El motor:

- **Debe:** escribir su registro cuando **ya acepta conexiones** en su transporte, no antes. Escribe con [`nu9ve-fs`](nu9ve-fs-v1.md) (temporal hermano y rename; durabilidad `Plain` basta), crea la carpeta si falta y la deja `0700` en macOS. El archivo, `0600`. En Windows, carpeta y archivo heredan la DACL del perfil del usuario.
- **Debe:** si ya hay un `<app>.json` **vivo** de otro proceso, no lo pisa: no se registra y lo dice en su registro local. Hay a lo más un motor registrado por app. Si está viejo o es inválido, lo reemplaza.
- **Debe:** al salir de forma ordenada, borrar su registro **sólo si** el archivo sigue siendo suyo (mismo `pid` y `process_start_us`). Otro motor pudo haberlo reemplazado.
- **Debe:** reescribirlo si cambia algo de lo que publica (otro socket). Nunca deja un registro que no corresponde a lo que sirve.
- Si el motor muere sin limpiar, el registro queda y §4 lo declara viejo.

El cliente:

- **Debe:** antes de leer, comprobar en macOS que la carpeta es del usuario y sin permisos de grupo ni otros, y que el archivo es regular (no enlace), del usuario y con `mode & 0o077 == 0`. Si no, lo ignora y no sigue enlaces.
- **Debe:** leer con tope (16 KiB) antes de interpretar JSON.
- **No debe:** borrar registros. Un registro viejo lo reemplaza el siguiente motor de esa app; borrarlo desde fuera compite con un motor que lo está escribiendo.
- **Debe:** listar sólo archivos `*.json` cuyo nombre cumple §2; los temporales y lo demás se ignoran.

## 6. Qué ve el cliente

Por cada `<app>.json`, uno de cuatro estados, que es lo que dicen las herramientas de la suite («nove no está abierto»):

| Estado | Cuándo |
| --- | --- |
| `live` | Válido (§2, §3) y vivo (§4) |
| `stale` | Válido, pero el proceso no existe, es de otro usuario o tiene otra hora de inicio |
| `invalid` | No se pudo leer, pasa de 16 KiB, no es JSON, le falta un campo, el `app` no coincide con el archivo, `contract` o `transport.kind` desconocidos, `profile_id` mal formado, o permisos inseguros |
| (ausente) | No hay archivo: la app nunca se abrió o salió en orden |

- **Debe:** un `contract` con otro mayor (`nu9ve-registry/2`) es `invalid` para un cliente v1. Un motor v2 que quiera seguir visible para clientes v1 escribe v1.
- Después de `live`, el cliente igual pide `engine_info` antes de usar el motor ([motor §4](nu9ve-engine-v1.md#4-engine_info)): el registro dice cómo llegar, el motor dice qué es.

## 7. Ejemplos

[`data/registry-examples-v1.json`](../data/registry-examples-v1.json): registros válidos e inválidos con su motivo (también con `profile_id`), la vida contra una tabla de procesos (pid ausente, pid reutilizado, otro usuario) y la conversión de la hora de inicio en Windows. `python3 -I scripts/registry-examples.py --check` comprueba el archivo contra una implementación de referencia de §2 a §6.

## 8. Adopción

Quién adoptó qué está en [adopción por app](../docs/adoption.md#nu9ve-registry-v1).

## 9. Decisiones abiertas

1. **Windows sin probar.** La carpeta sin virtualizar desde una app MSIX y la hora de inicio con `GetProcessTimes` están escritas, no probadas en un paquete real. Lo prueba la primera app que se registre en Windows (nove).
2. **Una instancia por app.** Dos bibliotecas abiertas de la misma app no se registran a la vez (§5). Si hace falta, sería un `<app>~<instancia>.json` en una versión menor.
