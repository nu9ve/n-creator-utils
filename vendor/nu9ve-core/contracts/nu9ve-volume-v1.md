# Identidad de volumen v1

Estado: **propuesta** (2026-11-22). Es la segunda pieza de F0 en [varias máquinas, discos y nube](../docs/suite-devices.md#62-identidad-de-volumen--nu9ve-volume-v1-contrato-nuevo-cierra-a3s6) y responde a la [auditoría A3](../docs/audits/fable-audit-2026-11-10.md#a3--la-identidad-de-un-disco-no-viaja-entre-equipos-ni-entre-sistemas) (brief S6). Define cómo se reconoce un disco en cualquier máquina y sistema, aunque se monte con otro nombre, y cómo varias apps escriben en él sin pisarse. Las decisiones del usuario del 2026-11-22 están en la [§12](#12-decisiones). Ejemplos: [`data/volume-examples-v1.json`](../data/volume-examples-v1.json).

**Debe** es obligatorio. **Conviene** es lo recomendado. «Volumen» o «disco» es un sistema de archivos montado (una partición). «Marcador» es `.nu9ve/volume.json`. «Observador» es el proceso que mira los discos conectados: una app, o el agente de la suite más adelante.

## 0. Para qué y para qué no

- **Para** saber que el MEDIA501 que se conecta hoy a la M1 es el mismo que ayer estuvo en la M4, aunque se monte como `/Volumes/MEDIA501 1` o como `E:`; para que una raíz de datos apunte a un disco y no a una ruta ([`nu9ve-data-v1` §3](nu9ve-data-v1.md#3-respuesta)); y para que lo que describe al disco viaje con él (§8).
- **No** protege contra alguien que copia el disco a propósito: el marcador es un archivo. El segundo factor (§3) detecta las copias de archivos habituales, no un clon por bloques (§6).
- **No** reemplaza el catálogo de nüün ni dice qué hay en el disco. Eso lo dice el índice de cada app (§8).
- **No** decide qué se puede borrar. Liberar espacio sigue exigiendo los recibos verificados de la app dueña.

## 1. Marcador

En la raíz del volumen: `.nu9ve/volume.json`.

```json
{
  "contract": "nu9ve-volume/1",
  "volume_id": "2b7d4c1e-8f3a-4e6b-9c0d-1a2b3c4d5e6f",
  "role": "archive",
  "label": "MEDIA501",
  "registered_at": "2026-11-22T21:00:00Z",
  "registered_by": {"device_id": "6f0c2a4e-9b1d-4c3a-8e5f-0a1b2c3d4e5f", "app": "nuun"},
  "fs": {"os": "macos", "kind": "exfat", "serial": "5A1C-93F2"}
}
```

| Campo | Debe | Qué es |
| --- | --- | --- |
| `contract` | Sí | `"nu9ve-volume/1"` |
| `volume_id` | Sí | UUID 4 en minúsculas, con guiones. Es la identidad del disco en la suite |
| `role` | Sí | `archive` (archivo: MEDIA), `work` (disco de trabajo: ESSD, XSSD) u `other` |
| `label` | Sí | El nombre del volumen al registrarlo, 1 a 64 caracteres sin control. Es histórico: el nombre actual lo da el sistema |
| `registered_at` | Sí | RFC 3339 en UTC con `Z` |
| `registered_by` | Sí | `device_id` del [dispositivo](nu9ve-device-v1.md) que lo registró y `app` (como en el [registro](nu9ve-registry-v1.md#2-archivo): `^[a-z][a-z0-9-]{0,31}$`) |
| `fs` | Sí | El segundo factor tal como lo leyó quien registró (§3): `os` (`macos`, `windows`, `linux`), `kind` y `serial` |
| `role_changed_at` | No | Cuándo nüün cambió el rol (§2) |
| `previous_volume_id` | No | El disco del que éste es copia, si se registró como copia (§6) |

- `fs.kind`: `apfs`, `hfsplus`, `exfat`, `fat32`, `ntfs`, `ext4` u `other`. `fs.serial`: 1 a 128 caracteres, el texto que da el sistema (§3).
- **Debe:** UTF-8 sin BOM, un objeto JSON, como mucho **16 KiB**. Al lector: ignorar campos que no conoce; inválido si falta uno obligatorio o tiene otro tipo o formato.
- **No debe:** rutas de la persona, nombres de proyectos ni secretos. Lo que hay en el disco va en los índices de cada app (§8).

## 2. Crear y cambiar el marcador

- **Debe:** lo crea **la primera app que registra el disco**, y sólo cuando la persona lo pide («registrar este disco») o acepta la sugerencia de su app. Lo crea con `nu9ve-volumes`, que tiene este contrato, en cuanto la pieza esté en el core.
- **Debe:** quien registra tiene un dispositivo en estado `ok` ([`nu9ve-device-v1` §4](nu9ve-device-v1.md#4-leer)). Sin dispositivo no hay registro.
- **Debe:** crear `.nu9ve/` si falta y publicar el marcador **sin reemplazar** ([`nu9ve-fs` §2](nu9ve-fs-v1.md#2-publicar-sin-reemplazar)). Si ya hay un marcador, gana el que está: el proceso lo relee y lo usa. Después de publicar, relee el archivo y comprueba que es el suyo.
- **Debe:** una app que no es nüün registra sólo con `role` `work` u `other`. `archive` lo pone sólo nüün.
- **Debe:** nadie reescribe un marcador, con dos excepciones:
  1. nüün cambia `role`. Reescribe el archivo con todo igual salvo `role` y `role_changed_at`.
  2. La persona confirma que el disco es una **copia** de otro (§6). Entonces se registra de nuevo, con otro `volume_id` y `previous_volume_id`. En un disco `archive` eso lo hace sólo nüün.
- **No debe:** borrar un marcador, ni uno inválido. Un marcador inválido se deja y se dice (§5).
- **Conviene** en Windows: marcar `.nu9ve` como oculto (`FILE_ATTRIBUTE_HIDDEN`). En macOS y Linux ya lo oculta el punto.

## 3. Segundo factor: el serial del sistema de archivos

El segundo factor es lo que el **sistema** dice del volumen, no lo que dice el marcador. Se compara sólo **dentro del mismo sistema**: macOS y Windows dan el serial en formatos distintos y no se comparan entre sí.

| Sistema | APFS, HFS+ | exFAT, FAT32 | NTFS |
| --- | --- | --- | --- |
| macOS | `VolumeUUID` de Disk Arbitration (`kDADiskDescriptionVolumeUUIDKey`), en mayúsculas | La misma clave, que Disk Arbitration deriva del número de serie del volumen | La misma clave |
| Windows | No se lee (§9) | `VolumeSerialNumber` de `GetVolumeInformationW` como `XXXX-XXXX` en hexadecimal mayúscula | Igual |
| Linux | `UUID` de `blkid` | `UUID` de `blkid` (`XXXX-XXXX`) | `UUID` de `blkid` |

- **Debe:** si el sistema no da serial, el observador no puede comprobar el segundo factor y el estado es `unverified` (§5), nunca `ok`.
- exFAT en macOS, **comprobado** el 2026-11-22 por nüün con MEDIA502: Disk Arbitration y `diskutil info -plist` dan la misma clave, que se mantiene al volver a montar y cambia al cambiar el serial (probado en una imagen desechable). FAT32 y el cotejo con Windows siguen pendientes (§13).

## 4. Lo que recuerda cada observador

Para comprobar el segundo factor en otro sistema, cada dispositivo recuerda lo que vio:

```text
(volume_id, os) → serial visto, primera vez, última vez
```

- **Debe:** se guarda en la máquina, en la carpeta de la app que observa (o del agente), nunca en el disco: el marcador no se reescribe.
- **Debe:** la primera vez que un dispositivo ve un `volume_id` en un sistema distinto del de `fs.os`, recuerda el serial que ve ahí. Desde entonces lo compara con ése.
- Su formato es de cada observador en F0. En F1, el agente lo publica como avistamientos en la carpeta de intercambio.

## 5. Estados al conectar un disco

Un observador da a cada volumen montado uno de estos estados:

| Estado | Cuándo | Qué hace la app |
| --- | --- | --- |
| `unreadable` | El sistema no puede leer sus archivos (HFS+ en Windows) | Lo muestra como disco que no se puede leer aquí |
| `unmarked` | No hay `.nu9ve/volume.json` | Lo usa como hoy, sin identidad de suite. Puede ofrecer registrarlo (§2) |
| `invalid` | Hay marcador, pero no se lee, pasa de 16 KiB, no es JSON o no cumple §1 | Lo dice y no lo toca |
| `unverified` | Marcador válido, pero el sistema no da serial | Usa el `volume_id`, con aviso |
| `ok` | Marcador válido y segundo factor de acuerdo | Usa el `volume_id` |
| `suspect` | Marcador válido, pero el serial contradice lo conocido | §6: pregunta a la persona antes de usarlo |
| `duplicate` | Dos volúmenes montados a la vez en esta máquina con el mismo `volume_id` | Los dos quedan así. §6 |

El orden de decisión, con `os` el sistema de este observador y `serial` el que da ahora:

1. Si no se pueden leer sus archivos: `unreadable`. Si no hay marcador: `unmarked`. Si el marcador no cumple §1: `invalid`.
2. Si otro volumen montado tiene el mismo `volume_id`: `duplicate`.
3. Si `serial` falta: `unverified`.
4. Si este observador recuerda un serial para `(volume_id, os)`: `ok` si es igual, `suspect` si no.
5. Si no recuerda nada y `fs.os` es `os`: `ok` si `serial` es `fs.serial`, `suspect` si no.
6. Si no recuerda nada y `fs.os` es otro sistema: `ok`, y recuerda este serial (§4).

En `ok` y `unverified`, el observador actualiza su «última vez».

## 6. Copias, clones y formateos

| Qué pasó | Qué ve la suite |
| --- | --- |
| Se copiaron los archivos de un disco a otro, `.nu9ve` incluido | El disco nuevo tiene otro serial: `suspect` en todo observador que conozca el original o en el mismo sistema que lo registró |
| Se clonó por bloques (el serial también se copia) | No se distingue sola. Se nota cuando los dos se conectan a la vez (`duplicate`) o, en F1, cuando dos máquinas los ven al mismo tiempo |
| Se formateó | El marcador desaparece: `unmarked`. El `volume_id` viejo queda en lo que las apps recuerdan, como un disco que ya no se ve |
| Se cambió el serial a mano | `suspect`. La persona dice que es el mismo disco, y el observador recuerda el serial nuevo |

Ante `suspect` o `duplicate`, la app pregunta y la persona elige:

- **«Es el mismo disco».** El observador recuerda el serial actual y el estado pasa a `ok`.
- **«Es una copia».** El disco se registra de nuevo (§2, excepción 2) con otro `volume_id` y `previous_volume_id`. Lo que las apps sabían del original sigue siendo del original.
- Mientras no elija, ninguna app escribe en ese disco ni lo usa para resolver una raíz.

## 7. Roles y quién escribe

| `role` | Quién escribe en el disco |
| --- | --- |
| `archive` | **Sólo nüün**, en todo el disco (decisión del usuario, §12) |
| `work` | Cada app en su carpeta: la de sus datos (la que ya usa hoy) y la suya dentro de `.nu9ve/apps/` (§8) |
| `other` | Igual que `work` |

- **Debe:** una app no escribe ni borra en la carpeta de otra, ni en `.nu9ve/` fuera de la suya. Leer, sí.
- **Debe:** dentro de su carpeta, una app escribe desde una sola máquina a la vez. Es así por construcción, porque el disco está montado en una sola máquina. Llevar una copia del proyecto a otra máquina por red es F3a ([varias máquinas §6.4](../docs/suite-devices.md#una-app-en-dos-máquinas)).
- **No debe:** abrir por red (SMB, AFP) una base de datos SQLite de un disco. En el disco conectado a esta máquina, sí.

## 8. Carpeta de cada app e índice

```text
<volumen>/.nu9ve/
├─ volume.json              §1
└─ apps/
   ├─ nuun/                 sólo la escribe nüün
   │  ├─ index.json         lo que nüün tiene en este disco
   │  └─ …                  recibos, manifiestos, snapshot del catálogo: formato de nüün
   ├─ atlas/
   └─ clipper/
```

- **Debe:** `<app>` cumple `^[a-z][a-z0-9-]{0,31}$`, el mismo nombre del registro. Otros nombres en `apps/` se ignoran.
- **Debe:** `.nu9ve/` sólo tiene `volume.json` y `apps/`. Cualquier otro nombre queda reservado para la suite.
- **Puede:** `index.json`, el índice de lo que la app tiene en el disco. Es una respuesta completa de `data_catalog` ([`nu9ve-data-v1` §3](nu9ve-data-v1.md#3-respuesta)) guardada en un archivo:
  - `app` es el de la carpeta y `next_cursor` es `null`: todo en un documento, como mucho **8 MiB**;
  - las raíces de lo que está en el disco llevan `volume_id` igual al del marcador y su `volume_path` ([data §3](nu9ve-data-v1.md#3-respuesta)); una raíz con otro `volume_id` es de otro disco y vale como cualquier raíz;
  - `generated_at` dice de cuándo es. Quien lo lee lo muestra con esa fecha, no como si fuera de ahora;
  - un dataset `personal` va entero sólo si la persona decidió guardarlo en ese disco (un respaldo es esa decisión). Si no, sólo con su forma ([data §6](nu9ve-data-v1.md#6-privacidad));
  - **Debe** al lector: leer con tope, validar como catálogo y descartar un índice cuyo `app` no es el de la carpeta (`app_mismatch`) o que no cumple `nu9ve-data-v1`. Un índice inválido no invalida el disco ni los índices de otras apps.
- **Adoptar este disco en otra máquina** es leer sus índices (y, para nüün, su snapshot) y, si la persona lo pide, importarlos al catálogo local. Lo decide cada app.

## 9. Sistemas y discos que no se pueden escribir

- **HFS+ en Windows:** no se lee. Estado `unreadable`. La app lo dice; no ofrece registrarlo.
- **NTFS en macOS:** se lee y no se escribe. Si tiene marcador, se reconoce; si no, queda `unmarked` y no se puede registrar desde la Mac.
- **Cualquier disco de sólo lectura:** igual que NTFS en macOS.
- **exFAT y FAT** no tienen diario y guardan hora local ([auditoría A5](../docs/audits/fable-audit-2026-11-10.md#a5--nombres-fechas-y-durabilidad-dependen-del-sistema-de-archivos)). El marcador lleva su hora en UTC, escrita por la app, y no depende de la hora del archivo.
- **Sin Windows a mano:** las reglas de Windows están escritas y en los ejemplos, sin probar en una máquina real (decisión del usuario, §12).

## 10. Ejemplos

[`data/volume-examples-v1.json`](../data/volume-examples-v1.json): marcadores válidos e inválidos con su motivo; quién puede crear con qué rol; los estados de la §5 en escenarios (otro sistema, serial distinto, sin serial, dos montados, sin marcador, ilegible); nombres de carpeta de app; e índices válidos e inválidos. `python3 -I scripts/volume-examples.py --check` comprueba el archivo contra una implementación de referencia de §1 a §8. Para validar los índices usa la referencia de `scripts/data-examples.py`.

## 11. Adopción

Quién registra, observa o escribe su carpeta está en [adopción por app](../docs/adoption.md#nu9ve-volume-v1).

## 12. Decisiones

1. **Decidido (el usuario, 2026-11-22):** marcador `.nu9ve/volume.json` y segundo factor. En los discos de trabajo también escriben otras apps (nove, Clipper, Atlas), cada una en su carpeta. En MEDIA (`archive`) sólo escribe nüün.
2. **Decidido (el usuario, 2026-11-22):** dos apps no escriben la misma carpeta. Lo que sí ocurre es la misma app en dos máquinas, en momentos distintos, llevando el disco. Copiarlo por red con su lease es la etapa siguiente.
3. **Decidido (el usuario, 2026-11-22):** Windows entra en el contrato desde ahora, aunque se pruebe cuando haya un PC a mano.
4. Sustituye el `.ncreator/volume.json` que proponía la auditoría A3. nüün decide en su sesión cómo migra lo que ya escribe en sus discos.

## 13. Decisiones abiertas

1. **Serial de exFAT y FAT:** en macOS, exFAT confirmado con MEDIA502 (2026-11-22, §3). Falta FAT32 en la Mac y leer MEDIA502 en Windows. En Windows el serial tiene otro formato, así que no se compara con el de la Mac, sólo consigo mismo (§4).
4. **Tope del índice:** el de nüün en MEDIA502 ya pesa 5,2 MiB de 8 MiB. Antes de que lo pase hay que decidir entre subir el tope, partir el índice en varios archivos o dejar en el índice sólo un resumen por proyecto.
2. **Un disco con varias particiones:** cada partición es un volumen con su marcador. Si hace falta agrupar las particiones de un mismo disco físico, va en una versión menor.
3. **Discos de red y carpetas en la nube** (un NAS, iCloud Drive) no son volúmenes de este contrato en v1.
