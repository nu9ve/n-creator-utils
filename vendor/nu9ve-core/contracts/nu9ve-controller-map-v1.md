# Controladores: perfil físico v2 y mapa v1

Estado: **propuesta** (2026-11-27). Dice cómo se describe un controlador físico (qué manda cada control y cómo se encienden sus luces) y cómo una app guarda qué hace cada control. Nace de M0 de Stalker: el Launchpad Pro con sus LEDs, probado en el aparato ([diseño](../../stalker/docs/2026-11-27-diseno-m0-midi-y-e1-escena.md#2-m0-midi)). La forma del mapa es la del preset de curado, con la acción como id de catálogo. Lo implementan [`Nu9veMIDI`](../swift/README.md#nu9vemidi) en Swift y, desde el 2026-11-27, [`nu9ve-midi`](../rust/nu9ve-midi/README.md#perfil-físico-v2-y-luces) en Rust, con los mismos casos ([`data/midi/controller-vectors-v1.json`](../data/midi/controller-vectors-v1.json)).

**Debe** es obligatorio. **Conviene** es lo recomendado. «Perfil» es el archivo de un modelo de aparato en [`data/midi/`](../data/midi/). «Mapa» es lo que guarda una app: control → acción. «Catálogo» es el de [interacción §1](nu9ve-interaction-v1.md#1-un-solo-catálogo).

## 0. Para qué y para qué no

- **Para** que un aparato se describa una vez y lo use cualquier app: direcciones, modos, luces y la cara del aparato para dibujarlo.
- **Para** que el mapa de una app tenga la misma forma en todas, y el configurador común (C4) y la vista de conjunto de nüün lo lean sin saber de qué app es.
- **No** dice qué hace una acción: eso es de cada app y su catálogo.
- **No** dice cómo se reparte un aparato entre varias apps, ni quién lo abre. Eso es [`nu9ve-surface-v1`](nu9ve-surface-v1.md). Este contrato es lo que se guarda y lo que se pinta.

## 1. Perfil físico v2

El v2 es el [v1 del crate](../rust/nu9ve-midi/README.md#perfil-físico-v1) con tres cosas más. Todo lo del v1 sigue igual: `id`, `name`, `vendor`, `ports` (alias del puerto), `evidence`, `notes` y los controles con `id`, `label`, `kind` (`note`, `cc`, `pitchBend`), `channel` desde 1, `number` y `lsb` en decimal o hex, `mode`, `deadband`, `verified`, `identified` y `mirrorOf`.

| Campo | Dónde | Qué es |
| --- | --- | --- |
| `schemaVersion` | Perfil | `2`. Un perfil v1 **no debe** traer `feedback` |
| `layout` | Perfil | `[columnas, filas]` de la cara del aparato, para dibujarlo |
| `grid` | Control | `[columna, fila]` desde arriba a la izquierda (la fila 0 es la de arriba). Sin `grid`, el control no se dibuja en la cara |
| `lit` | Control | `false`: no tiene luz; `true`: sí. Sin él, manda `feedback.allControls` |
| `feedback` | Perfil | Cómo se encienden las luces (abajo). Sin él, el aparato no tiene salida |
| `pageControl` | Perfil | Id del control principal que la suite reserva para cambiar de página en este modelo ([`nu9ve-surface-v1` §5](nu9ve-surface-v1.md#5-páginas-zonas-y-modos)); `suite.json` lo reemplaza o lo quita. Versión menor del 2026-11-27 |

`feedback`:

| Campo | Debe | Qué es |
| --- | --- | --- |
| `kind` | Sí | `palette`: el valor del mensaje es un color de la paleta (Launchpad, APC). `onOff`: un solo color, encendido o apagado (controladores de DJ) |
| `allControls` | Sí | Todos los controles tienen luz salvo los `"lit": false`; si es `false`, sólo los `"lit": true` |
| `init` | No | Mensajes en hex que se mandan al conectar, por ejemplo el modo que las direcciones suponen (el modo programmer del Launchpad, `F0 00 20 29 02 10 2C 03 F7`) |
| `palette` | Con `palette` | Color → valor (0–0x7F). Los nombres son `red`, `orange`, `yellow`, `green`, `cyan`, `blue`, `purple`, `pink` y `white`, cada uno con su `<color>.dim` opcional |
| `on` | No | El valor encendido de un `onOff`; por omisión 0x7F |
| `flash`, `pulse` | No | Plantillas en hex con `{number}` y `{value}` que el aparato anima solo (Launchpad: SysEx `23` y `28`) |
| `evidence` | No | Qué se vio en el aparato y qué viene del manual |

- **Debe:** una luz quieta es el mismo mensaje que el control manda, con el valor del color: Note On (`0x90 | canal`) o CC (`0xB0 | canal`) en su dirección.
- **Debe:** un color sin su `.dim` usa el brillante; uno que falta en la paleta usa `white`, y si tampoco está, 0x7F.
- **Debe:** con `flash` o `pulse`, se manda la luz quieta y después la plantilla. Sin plantilla, la app parpadea por software, **sólo** mientras algo parpadea.
- **Debe al conectar:** `init`, después **todas** las luces apagadas (el aparato puede mostrar restos de otra app) y luego el estado. Desde ahí, sólo lo que cambia. Al soltar el aparato, se apaga lo que se encendió.
- **Debe:** los controles que son copia (`mirrorOf`) no tienen luz propia.
- **Conviene:** `grid` dentro de `layout`, y `evidence` que diga qué se comprobó en el aparato y qué viene del manual.

Primer perfil v2: [`novation-launchpad-pro-v2.json`](../data/midi/novation-launchpad-pro-v2.json), el Launchpad Pro original en modo programmer: 96 controles, colores, parpadeo y pulso vistos en el aparato el 2026-11-27. Su `pageControl` es «Session» (`top-5`). El DDJ-SB sigue en v1, sin luces, hasta comprobar su salida; cuando entre, será `onOff`.

## 2. Luces que pide una app

Una app no escribe bytes: pide una luz por su sentido y el perfil la dibuja.

| Parte | Valores |
| --- | --- |
| Color | Uno de los nueve de la §1, o ninguno (apagada) |
| Tenue | Sí o no |
| Movimiento | `solid`, `flash` o `pulse` |

Las luces siguen el estado de cada acción (`off`, `available`, `active`, `warning`, [`nu9ve-surface-v1` §3](nu9ve-surface-v1.md#3-estados-y-luces)). Por convención: `off` apagada, `available` tenue, `active` brillante y `warning` naranja parpadeando. Una app puede elegir otro color por acción, y un mapeo puede cambiarlo (`color`, §3).

## 3. Mapa

```json
{
  "schemaVersion": 1,
  "enabled": true,
  "devices": [
    {
      "match": "Launchpad Pro",
      "profile": "novation-launchpad-pro",
      "mappings": [
        {"control": "left-8", "action": "recording.toggle"},
        {"control": "right-1", "action": "preset.select", "argument": "6f1c2a4e-9b1d-4c3a-8e5f-0a1b2c3d4e5f"},
        {"key": {"kind": "note", "channel": 1, "number": "0x30"}, "action": "preset.next", "color": "cyan"}
      ]
    }
  ]
}
```

| Campo | Debe | Qué es |
| --- | --- | --- |
| `schemaVersion` | Sí | `1`. Otro valor no se lee |
| `enabled` | Sí | Interruptor general. Apagado, la app no abre el sistema MIDI |
| `devices[].match` | Sí | Subcadena del nombre del puerto, sin distinguir mayúsculas, como los alias del perfil |
| `devices[].profile` | Sí | Id del perfil, o `null` para un aparato sin perfil |
| `mappings[].control` | Uno de los dos | Id del control en el perfil. De una copia (`mirrorOf`) se guarda el principal |
| `mappings[].key` | Uno de los dos | Dirección cruda: `kind`, `channel` desde 1 y `number` en decimal o hex. Para un aparato sin perfil, o un control que el perfil no tiene |
| `mappings[].action` | Sí | Id del catálogo de la app |
| `mappings[].argument` | No | Argumento de la acción (el UUID de un preset, el id de una escena) |
| `mappings[].color` | No | Color de su luz, en lugar del de la acción |
| `mappings[].range`, `curve` | No | Sólo acciones continuas: `[mín, máx]` y `linear` o `log`. Reservados; todavía nadie los escribe |

- **Debe:** un control (o una dirección) tiene **una sola** acción. Asignarle otra quita la anterior y la app dice cuál quitó. Una acción puede tener varios controles.
- **Debe:** un mapeo con una acción o un control que no se conocen **se conserva** y se muestra «no disponible» ([controles §9](nu9ve-controls-v1.md#9-estados)). Nunca se borra solo.
- **Debe:** si un archivo editado a mano repite un control, gana el primero y la app lo dice.
- **Debe:** se escribe con las reglas de [`nu9ve-fs-v1`](nu9ve-fs-v1.md) (temporal hermano y rename) en modo `Plain`, sin vaciar la caché del disco, dentro de la carpeta de la app (en QA, la de su raíz de QA).
- **Conviene:** «volver a fábrica», con el mapa de fábrica de la app.
- Hoy cada app guarda un mapa (Stalker: `~/Library/Application Support/Stalker/controllers.json`). Guardarlo por aparato, app, modo y perfil de la persona es [`nu9ve-surface-v1` §6](nu9ve-surface-v1.md#6-mapas-por-aparato-app-modo-y-perfil).

## 4. Lectura en vivo

- **Debe:** una acción de disparo ocurre al **bajar** el control (Note On con velocidad > 0, o CC > 0). Soltarlo no hace nada. Cualquier velocidad cuenta: un pad del Launchpad tocado suave manda 1.
- **Debe:** una acción continua recibe un valor de 0 a 1 ya normalizado y filtrado por la banda muerta. La banda muerta del mapeo gana a la del perfil, y la del perfil al valor por omisión (0,002 en 14 bits, 0 en 7).
- **Debe:** un control relativo (jog, browse) suma `delta × paso`. Un botón asignado a una acción continua la alterna entre 0 y 1.
- **Debe:** en un CC de 14 bits se guarda el último MSB por control y se combina al llegar el LSB. Un LSB sin MSB se ignora.
- **Debe:** se ignoran la presión de canal, el reloj y lo que no es voz de canal.
- **Debe:** nada corre en reposo. Sin mapa encendido, no se abre el sistema MIDI. Sin aparato, sólo se esperan las notificaciones del sistema. Los temporizadores existen sólo mientras se aprende o algo parpadea por software.

## 5. Ejemplos

- [`data/midi/novation-launchpad-pro-v2.json`](../data/midi/novation-launchpad-pro-v2.json): perfil v2.
- [`data/midi/pioneer-ddj-sb-v1.json`](../data/midi/pioneer-ddj-sb-v1.json): perfil v1.
- [`data/midi/controller-map-example-v1.json`](../data/midi/controller-map-example-v1.json): un mapa con un aparato con perfil v2, otro con v1 y una dirección cruda, y uno sin perfil.

- [`data/midi/controller-vectors-v1.json`](../data/midi/controller-vectors-v1.json): luces pedidas, el tablero de luces y la lectura en vivo, con los bytes y los gestos esperados.

`swift test --filter Nu9veMIDITests` valida los perfiles, comprueba que los embebidos son los mismos bytes que `data/midi/`, lee el mapa de ejemplo contra los perfiles y pasa los casos comunes; `cargo test -p nu9ve-midi` pasa los mismos casos.

## Adopción

Quién cumple qué está en [adopción por app](../docs/adoption.md#nu9ve-controller-map-v1).

## 6. Decisiones abiertas

1. ~~**El perfil v2 en Rust.**~~ Hecho el 2026-11-27 (C3a): `nu9ve-midi` lee el v2 y pinta con `LightBoard`, con los mismos casos que `Nu9veMIDI`.
2. **El DDJ-SB con luces.** Mixxx lo pinta con Note On `0x7F`/`0x00` en la misma dirección. Entra como `onOff` cuando se vea en el aparato.
3. **`range` y `curve`** se fijan cuando una app tenga su primera acción continua mapeada (la escena de Stalker, E1a).

## Cambios

Un campo opcional nuevo en el perfil o en el mapa es una versión menor, con su línea en el [CHANGELOG](../CHANGELOG.md). Un perfil que cambia de forma sube su `schemaVersion`; un mapa que cambia de significado es `nu9ve-controller-map-v2`.
