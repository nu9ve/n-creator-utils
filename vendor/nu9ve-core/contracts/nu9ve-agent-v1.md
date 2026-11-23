# Agente de la suite v1

Estado: **propuesta** (2026-11-22). Es la primera pieza de F1 en [varias máquinas, discos y nube](../docs/suite-devices.md#66-agente-de-la-suite--nu9ve-agent), con la decisión del usuario del mismo día (pregunta 9). Define cómo se instala, se actualiza y se encuentra el agente `nu9ve`: un proceso por usuario y máquina que guarda la identidad del dispositivo y su clave, escribe y lee la [carpeta de intercambio](nu9ve-exchange-v1.md) y, desde F2, la sesión de la cuenta. Ejemplos: [`data/agent-examples-v1.json`](../data/agent-examples-v1.json).

**Debe** es obligatorio. **Conviene** es lo recomendado. «Agente» es el binario `nu9ve` corriendo como servicio de inicio de sesión. «Instalador» es quien lo pone en la máquina: su propio instalador o una app que lo trae dentro.

## 0. Para qué y para qué no

- **Para** que haya **un solo** agente por máquina, sea quien sea el que lo instaló, y que las apps sepan si está, qué versión es y qué puede hacer.
- **No** es el motor de una app: no guarda datos de ninguna. Les pide su catálogo por su transporte, como `nu9ve mcp`.
- **No** hace falta para que una app funcione. Sin agente, una app hace todo lo suyo y no muestra lo de dispositivos ni nube.

## 1. Dónde

| Sistema | Carpeta del agente | Arranque |
| --- | --- | --- |
| macOS | `~/Library/Application Support/nu9ve/agent/` | LaunchAgent `~/Library/LaunchAgents/xyz.nu9ve.agent.plist`, etiqueta `xyz.nu9ve.agent` |
| Windows | `%LOCALAPPDATA%\nu9ve\agent\` | Tarea programada «nu9ve agent» al iniciar sesión, sólo para ese usuario |

En la carpeta: el binario (`nu9ve` o `nu9ve.exe`) y `agent.json` (§2). Es hermana de `engines/` ([registro §1](nu9ve-registry-v1.md#1-dónde)) y de `device.json` ([dispositivo §1](nu9ve-device-v1.md#1-dónde)).

- **Debe:** `NU9VE_AGENT_DIR` (ruta absoluta) reemplaza la carpeta. En QA se usa **siempre**, y un instalador en QA **no** registra el arranque en el sistema: sólo copia y escribe `agent.json`. Si la variable no es absoluta, no hay agente.
- **Debe:** un instalador con sandbox o en un paquete MSIX escribe la carpeta real, sin virtualizar. Si no puede, no instala: un agente en el contenedor de una app sería otro agente.

## 2. `agent.json`

```json
{"contract":"nu9ve-agent/1","version":"0.3.0","build":"public","installed_at":"2026-11-22T22:00:00Z","installed_by":"nuun","binary":"nu9ve"}
```

| Campo | Debe | Qué es |
| --- | --- | --- |
| `contract` | Sí | `"nu9ve-agent/1"` |
| `version` | Sí | `MAYOR.MENOR.PARCHE`, enteros sin ceros a la izquierda |
| `build` | Sí | `public` (sin nube) o `cloud` (con `nu9ve-cloud` de `inner-core`, [apps con y sin la parte privada §2](../docs/suite-modules.md#2-la-regla-principal-las-apps-piden-lo-privado-no-lo-enlazan)) |
| `installed_at` | Sí | RFC 3339 en UTC con `Z` |
| `installed_by` | Sí | `installer`, o el nombre de la app que lo instaló (como en el [registro](nu9ve-registry-v1.md#2-archivo)) |
| `binary` | Sí | El nombre del ejecutable dentro de la carpeta: `nu9ve` o `nu9ve.exe` |

- **Debe:** UTF-8 sin BOM, un objeto JSON, como mucho 4 KiB. El lector ignora campos que no conoce y trata como ausente un archivo inválido.

## 3. Versión: gana la más nueva

Un instalador trae una versión `(v, build)` y encuentra la instalada `(i, build_i)`, o ninguna. Lo que hace:

| Caso | Acción |
| --- | --- |
| No hay agente, o `agent.json` es inválido | `install` |
| `v` mayor que `i`, y no baja de `cloud` a `public` | `offer_update`: una app lo ofrece en Ajustes («Actualizar»); el instalador propio, a quien lo corrió |
| `v` igual que `i`, y `build` es `cloud` sobre `public` | `offer_update` |
| `v` igual, mismo `build` | `keep` |
| `v` menor que `i` | `keep`: **nunca** se baja de versión |
| `build` `public` sobre un `cloud` instalado, de cualquier versión | `keep`: la nube se actualiza con su propio instalador |

- **Debe:** las versiones se comparan por sus tres números, en ese orden.
- **Debe:** una app **nunca** actualiza en silencio. Instala sin preguntar sólo si la persona ya pidió en Ajustes «Instalar el agente» o «Actualizar».

## 4. Instalar, actualizar, quitar

- **Debe:** copiar el binario a un temporal dentro de la carpeta, comprobar que `nu9ve --version` da la versión que se espera, y publicarlo con rename ([`nu9ve-fs`](nu9ve-fs-v1.md)). Después se escribe `agent.json`. Si algo falla, queda el agente anterior.
- **Debe:** al actualizar, pedir al agente que pare (cooperativo, como [motor §8](nu9ve-engine-v1.md)) antes del rename, y arrancarlo de nuevo con el arranque del sistema (`launchctl kickstart` o la tarea). No hay dos agentes a la vez.
- **Debe:** quitar el agente borra el arranque, el binario y `agent.json`. **Nunca** borra `device.json`, la clave del dispositivo ni la carpeta de intercambio: son de la persona, y otro agente que se instale después los usa.
- **Debe** en macOS: carpeta `0700`, `agent.json` `0600`, binario `0700`.

## 5. Cómo lo ve una app

Al arrancar, y cada vez que abre Ajustes, una app calcula uno de estos estados:

| Estado | Cuándo | Qué muestra |
| --- | --- | --- |
| `absent` | No hay `agent.json` válido | Ajustes ofrece «Instalar el agente», una vez, sin aviso que interrumpa |
| `installed` | Hay `agent.json`, pero su registro en `engines/` no está vivo | «El agente está instalado pero no corre», con «Arrancar» |
| `running` | Su registro está `live` ([registro §6](nu9ve-registry-v1.md#6-qué-ve-el-cliente)) | Usa lo que diga su `engine_info`: `devices_v1`, `exchange_v1`, `cloud_v1`… |
| `outdated` | Corre o está instalado, y la app trae una versión que §3 resuelve como `offer_update` | Además, «Actualizar» en Ajustes |

- **Debe:** el agente se registra como un motor más ([registro](nu9ve-registry-v1.md)), con el nombre de app `suite`. Así sus herramientas aparecen en `nu9ve mcp` como `suite_*` ([varias máquinas §7](../docs/suite-devices.md#7-mcp-qué-se-habilita-y-cuándo)).
- **Métodos del motor `suite`** (protocolo `nu9ve-suite/1`, un marco por conexión como todo motor registrado). Con `devices_v1`: `devices` y `volumes`. Con `exchange_v1`: `where`, `leases`, `lease_take`, `lease_release` y `pull` (con `drop` lo deshace) ([intercambio §3 y §7](nu9ve-exchange-v1.md#3-qué-lleva-cada-kind)), `offer` y `unoffer` para lo que la máquina da cuando se lo piden, `shared`, `share` y `unshare` para lo que la persona marcó «en todas mis máquinas» ([exportar §5](nu9ve-export-v1.md#5-ofrecer-sola-lo-compartido)), y `app_publish`, `app_append` y `app_read` para `apps/<app>/`. Con `mcp_tools_v1`: `mcp_tools` y `mcp_call`, que dan los mismos como `suite_*`; por MCP, `lease_take`, `lease_release` y `pull` son de nivel editar, piden `grant: "edit"` y quedan en el registro de cambios del agente (`changes.ndjson`). Además `engine_shutdown`, que lo para en orden (§4). Lo que viene de otra máquina sale en campos `_untrusted`, y de lo personal, sólo su forma, también por los métodos. Para responder el pedido de un dataset, el agente pide sus bytes al motor dueño con `data_export` ([exportar un dataset](nu9ve-export-v1.md#3-cómo-lo-usa-el-agente)).
- **Conviene:** una app calcula el estado con `nu9ve agent state`, corriendo el binario que trae (sólo consulta: no es correr su propia copia), o en Rust con `nu9ve_mcp::agent::install::state_of`. Así ninguna repite las reglas de §3 y §5.
- **Debe:** **ninguna app arranca su propia copia** del agente ni lo corre desde su bundle. Si no está, no hay agente.
- **Debe:** sin agente, la app funciona completa en lo suyo. Lo que pide al agente no aparece.

## 6. Ejemplos

[`data/agent-examples-v1.json`](../data/agent-examples-v1.json): `agent.json` válidos e inválidos, la comparación de versiones, la acción de §3 en cada caso y el estado de §5. `python3 -I scripts/agent-examples.py --check` los comprueba contra una implementación de referencia.

## 7. Adopción

[Adopción por app](../docs/adoption.md#nu9ve-agent-v1).

## 8. Decisiones

1. **Decidido (el usuario, 2026-11-22):** un solo agente por máquina, suyo. Lo instala su instalador o cualquier app que lo traiga, empezando por nüün. Gana la versión más nueva, y las apps comprueban y ofrecen en Ajustes.
2. **Abierta:** si el instalador propio es un paquete (`.pkg`) firmado y notarizado, o un ejecutable que se copia a sí mismo. Se decide con la firma de las apps.
3. **Abierta:** el nombre de la etiqueta del LaunchAgent usa el dominio `xyz.nu9ve`. Si las apps se firman con otro identificador de equipo, se cambia antes de la primera instalación real.
