# Identidad de dispositivo v1

Estado: **propuesta** (2026-11-22). Es la primera pieza de F0 en [varias máquinas, discos y nube](../docs/suite-devices.md#61-identidad-de-dispositivo--nu9ve-device-v1-contrato-nuevo-local). Define cómo cada máquina de la persona recibe una identidad estable y local, para que la suite pueda decir «visto en la M4» sin usar el nombre del equipo, el usuario ni un número de serie. Ejemplos: [`data/device-examples-v1.json`](../data/device-examples-v1.json).

**Debe** es obligatorio. **Conviene** es lo recomendado. «Dispositivo» es una instalación de la suite para un usuario del sistema en una máquina. «Archivo» es `device.json`.

## 0. Para qué y para qué no

- **Para** nombrar una máquina en lo que la suite guarda o compara: avistamientos de discos ([`nu9ve-volume-v1`](nu9ve-volume-v1.md)), quién registró un disco y, más adelante, la carpeta de intercambio entre máquinas (F1).
- **No** es una credencial. Saber el `device_id` no da acceso a nada. La clave del dispositivo, el emparejado y el registro en la cuenta llegan en F1 y F2, con sus propios contratos.
- **No** identifica a la persona ni al hardware ante terceros: es un seudónimo aleatorio que sólo tiene sentido dentro de la suite.

## 1. Dónde

| Sistema | Archivo |
| --- | --- |
| macOS | `~/Library/Application Support/nu9ve/device.json` |
| Windows | `%LOCALAPPDATA%\nu9ve\device.json` |
| Linux | `$XDG_DATA_HOME/nu9ve/device.json` (`~/.local/share` si falta la variable). Sólo para pruebas: ninguna app de la suite corre ahí |

- Es la carpeta padre de la del [registro de motores](nu9ve-registry-v1.md#1-dónde).
- **Debe:** la variable `NU9VE_DEVICE_FILE` (ruta absoluta) reemplaza el archivo. Las pruebas y la QA la usan **siempre**: un proceso con raíz de QA no lee ni crea el archivo de la persona. Si la variable no es absoluta, no hay dispositivo (§4), no se adivina otra ruta.
- **Debe** con sandbox o paquete: un proceso que no alcanza la carpeta real (una app de Mac con sandbox redirigida a su contenedor, una app MSIX virtualizada) **no** crea otro archivo en su contenedor. Trabaja sin dispositivo y lo dice. Dos archivos en la misma máquina serían dos dispositivos falsos. En Windows, como el registro, la carpeta se declara sin virtualizar ([Windows §9](windows-app-v1.md#9-paquete-y-distribución)).

## 2. Archivo

```json
{
  "contract": "nu9ve-device/1",
  "device_id": "6f0c2a4e-9b1d-4c3a-8e5f-0a1b2c3d4e5f",
  "name": "M4",
  "created_at": "2026-11-22T20:00:00Z",
  "os": "macos",
  "host_check": "3b9f…(64 hex)"
}
```

| Campo | Debe | Qué es |
| --- | --- | --- |
| `contract` | Sí | `"nu9ve-device/1"` |
| `device_id` | Sí | UUID 4 en minúsculas, con guiones (`^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$`). Se crea una vez y **nunca cambia** |
| `name` | Sí | El nombre que da la persona: 1 a 64 caracteres, sin caracteres de control. Puede repetirse entre dispositivos; la identidad es el id |
| `created_at` | Sí | RFC 3339 en UTC con `Z` |
| `os` | Sí | `macos`, `windows` o `linux` |
| `host_check` | No | §5. Si falta, no se pueden detectar copias |
| `previous_device_id` | No | §5: el dispositivo del que salió una copia detectada |

- **Debe:** UTF-8 sin BOM, un objeto JSON, como mucho **4 KiB**.
- **Debe** al lector: ignorar campos que no conoce; tratar como inválido (§4) un archivo al que le falta un campo obligatorio, lo trae de otro tipo o con otro formato.
- **No debe:** número de serie, dirección MAC, nombre del equipo, nombre del usuario, rutas ni secretos. La clave privada de F1 vive en el llavero del sistema, no aquí.
- **Debe:** el nombre lo elige o lo confirma la persona. Un proceso que crea el archivo **no** usa el nombre del equipo del sistema por su cuenta: suele llevar el nombre de la persona («MacBook de Ana»). Mientras la persona no elige, el nombre es el sistema y los cuatro primeros caracteres del id (`Mac 6f0c`, `PC 6f0c`, `Linux 6f0c`).

## 3. Crear y cambiar

- **Debe:** lo crea el primer proceso de la suite que lo necesita y no lo encuentra. Escribe un temporal hermano y lo publica **sin reemplazar** ([`nu9ve-fs` §2](nu9ve-fs-v1.md#2-publicar-sin-reemplazar)). Si otro proceso ganó la carrera, usa el archivo del otro y descarta el suyo. Nunca hay dos ids en la misma máquina.
- **Debe** en macOS y Linux: crea la carpeta `nu9ve/` si falta, con modo `0700`, y el archivo con `0600`. En Windows, carpeta y archivo heredan la DACL del perfil.
- **Debe:** cambiar el nombre reescribe el archivo entero con el mismo `device_id`, `created_at` y `host_check`, por temporal y rename. Ningún otro campo cambia nunca, salvo por §5.
- **No debe:** borrar el archivo ni reemplazarlo por uno nuevo porque «no le gusta». Un archivo inválido se deja como está (§4).

## 4. Leer

Un lector obtiene uno de tres estados:

| Estado | Cuándo | Qué hace |
| --- | --- | --- |
| `ok` | Archivo válido (§2) y la comprobación de §5 no dice que es una copia | Usa el `device_id` |
| `copied` | Archivo válido, pero su `host_check` no coincide con esta máquina (§5) | Sigue §5 |
| `none` | No hay archivo y este proceso no lo puede crear, la variable de §1 no es absoluta, la carpeta no se alcanza, o el archivo es inválido | Trabaja sin dispositivo. Si el archivo es inválido, lo dice y no lo toca |

- **Debe:** leer con tope (4 KiB) antes de interpretar JSON. En macOS, ignorar el archivo si no es regular, no es del usuario o tiene permisos de grupo u otros.
- **Debe:** trabajar sin dispositivo es un estado normal. Lo que necesita un `device_id` (registrar un disco, por ejemplo) lo dice y no inventa uno.

## 5. Copias de una máquina a otra

El Asistente de Migración, una restauración de Time Machine o copiar el perfil de Windows llevan `device.json` a otra máquina. Sin más, dos máquinas tendrían el mismo id. `host_check` lo detecta sin guardar un identificador del hardware:

```text
host_check = sha256_hex(utf8("nu9ve-device/1\n" + device_id + "\n" + platform_id))
```

| Sistema | `platform_id` |
| --- | --- |
| macOS | `IOPlatformUUID` del registro de I/O, tal cual (mayúsculas) |
| Windows | `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid`, tal cual |
| Linux | `/etc/machine-id` sin el salto de línea final |

- El hash lleva el `device_id`, así que no sirve para reconocer la misma máquina en otro contexto, y no se puede invertir para obtener el `platform_id`.
- **Debe:** al crear el archivo, escribir `host_check` si el `platform_id` se puede leer. Si no se puede, el campo falta y no hay detección.
- **Debe** al leer: si hay `host_check` y el `platform_id` de esta máquina se puede leer y el hash calculado **no** coincide, el estado es `copied`. El proceso:
  1. no usa ese `device_id`;
  2. crea un dispositivo nuevo, con `previous_device_id` igual al viejo, y lo publica en lugar del copiado (es la única vez que el archivo se reemplaza);
  3. propone a la persona el nombre del viejo y le pide que confirme o cambie, porque la máquina nueva no es la vieja.
- Si falta `host_check` o el `platform_id` no se puede leer, el estado es `ok`: no hay forma de saberlo.
- Reinstalar Windows cambia `MachineGuid` y el dispositivo pasa a ser otro. Es lo esperado: lo que la suite vio con el viejo sigue siendo del viejo.

## 6. Privacidad

- El `device_id` y el nombre se guardan en la máquina y en lo que la suite escribe a propósito: el marcador de un disco que esta máquina registró ([`nu9ve-volume-v1`](nu9ve-volume-v1.md)) y, en F1, la carpeta de intercambio.
- **No debe** aparecer en registros de errores que se envían fuera, en telemetría ni en un registro de benchmarks ([`nu9ve-bench-v1` §9](nu9ve-bench-v1.md)), salvo que la persona lo pida al compartirlo.
- `nu9ve mcp` puede mostrar el nombre y el id: no son secretos, y una herramienta como `suite_devices` los necesita.

## 7. Ejemplos

[`data/device-examples-v1.json`](../data/device-examples-v1.json): dónde está el archivo según el sistema y las variables, archivos válidos e inválidos con su motivo, el cálculo de `host_check` y los estados de §4 y §5 contra un `platform_id` dado. `python3 -I scripts/device-examples.py --check` comprueba el archivo contra una implementación de referencia de §1 a §5.

## 8. Adopción

Quién crea o lee el archivo está en [adopción por app](../docs/adoption.md#nu9ve-device-v1).

## 9. Decisiones abiertas

1. **La clave del dispositivo** (F1): algoritmo (Ed25519 en archivo protegido, o P-256 del Secure Enclave), y dónde se publica su huella. Este contrato deja libre un campo `key` para entonces; hoy ningún lector lo usa.
2. **Quién lo crea en la práctica.** Hasta que exista el agente `nu9ve` ([decisión 9](../docs/suite-devices.md#12-preguntas-para-el-usuario)), lo crea la primera app que registra un disco, que hoy es nüün.
