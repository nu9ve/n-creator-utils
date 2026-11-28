# Un controlador para toda la suite v1

Estado: **propuesta** (2026-11-27), implementada en el agente `nu9ve` 0.3.0 (C3c, macOS) con pruebas de clientes falsos; ninguna app la usa todavía. Transporte decidido por el usuario el mismo día: un socket local con marcos (§2). Dice cómo un mismo controlador físico sirve a varias apps a la vez: el **agente `nu9ve`** ([`nu9ve-agent-v1`](nu9ve-agent-v1.md)) es el único que abre el aparato y pinta sus luces, y cada app le declara su catálogo, le avisa sus estados y recibe sus acciones. La pidió el usuario después de M0 de Stalker ([diseño §7](../../stalker/docs/2026-11-27-diseno-m0-midi-y-e1-escena.md#7-dirección-un-controlador-para-toda-la-suite)). El perfil del aparato y el mapa de cada app son de [`nu9ve-controller-map-v1`](nu9ve-controller-map-v1.md).

**Debe** es obligatorio. **Conviene** es lo recomendado. «Superficie» es el aparato visto por la suite: sus controles, sus luces y quién manda en cada uno. «Página» es la app que el aparato controla ahora. «Zona» son controles que pertenecen a una app aunque no tenga la página.

## 0. Lo que decidió el usuario (2026-11-27)

- Lo genérico va al core para que todas las apps lo reusen y lo mejoren: entrada y salida MIDI, perfiles, mapas, luces y sus animaciones, y el configurador. Lo que se acciona es de cada app.
- Un mismo aparato controla la app que está al frente y, a la vez, la que graba de fondo. Por ejemplo, Stalker graba mientras se usa SoundBender: se cambia de página desde el aparato y se controlan las dos.
- Hay modos y perfiles: según el perfil de la persona ([`nu9ve-profile-v1`](nu9ve-profile-v1.md)) y según si se está grabando ([`nu9ve-recording-v1`](nu9ve-recording-v1.md)).
- Sin un proceso por app: lo resuelve el agente para todas, lo más eficiente posible. Nada corre en reposo y todo va por eventos.
- Cada app configura a su manera, sobre piezas comunes. nüün ve los perfiles y mapas de todas.
- El primer controlador es el Launchpad Pro (el original, en modo programmer). El DDJ-SB va después.
- El transporte entre las apps y el agente es un socket local con marcos (§2).

## 1. Papeles

| Quién | Qué hace |
| --- | --- |
| Agente | Abre el aparato, manda su `init`, limpia las luces y las pinta. Lee los mapas de todas las apps (§6), decide de quién es cada control (§5) y le manda a esa app la acción. Sigue a la app que está al frente y la señal de grabación |
| App | Se conecta al agente, declara su catálogo y avisa cada cambio de estado (§4). Recibe acciones y las ejecuta como si vinieran de su menú. Escribe su propio mapa con su configurador |
| nüün | Lee todos los mapas y muestra aparatos, apps, perfiles y conflictos. No edita los de otras apps |

- **Debe:** las luces de un aparato tienen **un solo dueño**. CoreMIDI deja que varias apps lean el mismo aparato, pero si dos pintan, se pisan.
- **Debe — modo directo:** sin agente, o con un agente sin `surface_v1`, cada app abre el aparato sola, como Stalker en M0. Es la regla del agente: no hace falta para que una app funcione ([agente §0](nu9ve-agent-v1.md#0-para-qué-y-para-qué-no)).
- **Debe:** una app en modo directo que encuentra un agente con `surface_v1` **suelta** el aparato antes de conectarse: apaga lo que encendió y cierra su puerto. Después el agente lo abre y lo limpia entero.
- **Debe:** una conexión por app. Un `hello` con el nombre de una app ya conectada reemplaza la conexión anterior, que el agente cierra.

## 2. Transporte

**Decidido por el usuario (2026-11-27): socket local con marcos.** Las tres opciones que se compararon:

| | XPC | Socket local con marcos | Notificación más archivo |
| --- | --- | --- | --- |
| Latencia de una acción continua | Submilisegundo | Submilisegundo | Decenas de ms; además, el agente tiene que leer un archivo |
| Ida y vuelta (estados → agente, acciones → app) | Sí | Sí | No: el agente no tiene cómo mandar una acción a una sola app |
| Desde Rust (el agente) | Difícil: Mach services y bindings sin mantener | `std::os::unix::net`, lo que el agente ya usa | Fácil |
| Desde Swift | Nativo | `Network` o `FileHandle`, como `Nu9veEngine` | Fácil |
| Windows | No existe | Named pipe, igual que nove | Sí |
| Ya en la suite | No | Los marcos de [motor §1](nu9ve-engine-v1.md) y la seguridad de [motor §9](nu9ve-engine-v1.md#9-seguridad-sólo-local-sólo-el-usuario) | La [señal de grabación](nu9ve-recording-v1.md) |

**Cómo va el socket:**
- Una conexión persistente por app, con mensajes en los dos sentidos.
- Marcos de 4 bytes de largo más JSON UTF-8, de [motor §1](nu9ve-engine-v1.md), como mucho 64 KiB.
- En macOS, el socket va en `<carpeta del agente>/runtime/surface`: carpeta `0700`, socket `0600`. El agente comprueba el uid del par en cada conexión, y la app comprueba el socket con `lstat` antes de conectar ([motor §9](nu9ve-engine-v1.md#9-seguridad-sólo-local-sólo-el-usuario)).
- El agente lo anuncia con la capacidad `surface_v1` en el `engine_info` de su motor `suite` y en su registro, sólo mientras sirve el socket. La ruta va en el mismo `engine_info`, como `surface.socket`; el registro no cambia de forma. Sin agente, o si no responde, la app la deduce de la carpeta del agente ([agente §1](nu9ve-agent-v1.md#1-dónde)).
- No es una petición y su respuesta: es un canal de eventos aparte del motor `suite`, que sigue con una petición por conexión.

## 3. Estados y luces

Cada acción del catálogo de una app tiene un estado, y la app avisa cuando cambia:

| Estado | Qué quiere decir | Luz por omisión |
| --- | --- | --- |
| `off` | No se puede usar ahora | Apagada |
| `available` | Se puede usar | Su color, tenue |
| `active` | Está puesta (grabando, el preset elegido) | Su color, brillante |
| `warning` | Pide atención o confirmación | Naranja parpadeando |

- La app puede pedir otra luz para un estado (color, tenue, movimiento, [mapa §2](nu9ve-controller-map-v1.md#2-luces-que-pide-una-app)). Un mapeo puede cambiar el color.
- **Debe — confirmar en el aparato:** una acción que llega de un controlador **no** abre un diálogo modal, porque la app suele estar al fondo y el diálogo queda escondido. Si la acción necesita confirmación, la app pone esa acción en `warning` y espera una segunda pulsación del mismo control antes de un plazo corto (Stalker: 4 s). Si no llega, la acción vuelve a su estado. El aviso va también a un lugar que no interrumpa (el menú, la barra). Lo mismo dice [interacción §1](nu9ve-interaction-v1.md#1-un-solo-catálogo).
- **Debe:** el agente pinta con el perfil ([mapa §1](nu9ve-controller-map-v1.md#1-perfil-físico-v2)): al conectar, `init` y todo apagado; luego, sólo lo que cambia.

## 4. Mensajes

App → agente:

```json
{"type":"hello","contract":"nu9ve-surface/1","app":"stalker","pid":5120,"process_start_us":1795600000123456,
 "profile_id":"e64",
 "catalog":[
   {"id":"recording.toggle","title":"Grabar o detener","group":"Grabación","kind":"trigger"},
   {"id":"preset.select","title":"Elegir preset","group":"Presets","kind":"trigger",
    "arguments":[{"value":"6f1c2a4e-9b1d-4c3a-8e5f-0a1b2c3d4e5f","title":"Clean"}]},
   {"id":"scene.crossfade","title":"Mezcla A/B","group":"Escena","kind":"continuous"}],
 "states":[{"action":"recording.toggle","state":"available"},
           {"action":"preset.select","argument":"6f1c2a4e-9b1d-4c3a-8e5f-0a1b2c3d4e5f","state":"active"}]}
{"type":"states","states":[{"action":"recording.toggle","state":"active","light":{"color":"red","motion":"pulse"}}]}
{"type":"catalog","catalog":[…]}
{"type":"focus","active":true}
{"type":"bye"}
```

Agente → app:

```json
{"type":"welcome","contract":"nu9ve-surface/1","agent":"0.3.0","devices":[{"device":"novation-launchpad-pro-1","profile":"novation-launchpad-pro","port":"Launchpad Pro Standalone Port"}]}
{"type":"action","action":"preset.select","argument":"6f1c2a4e-9b1d-4c3a-8e5f-0a1b2c3d4e5f","device":"novation-launchpad-pro-1","control":"right-1","via":"page","host_ns":123469289000000}
{"type":"action","action":"scene.crossfade","value":0.42,"device":"novation-launchpad-pro-1","control":"fader-1","via":"page","host_ns":123469301000000}
{"type":"page","page":true,"device":"novation-launchpad-pro-1"}
{"type":"devices","devices":[]}
{"type":"released"}
```

- **Debe:** `hello` es el primer mensaje, y el agente contesta `welcome` o cierra. `app` es el nombre del [registro §2](nu9ve-registry-v1.md#2-archivo); `pid` y `process_start_us`, la vida del [registro §4](nu9ve-registry-v1.md#4-vida-pid-con-hora-de-inicio). `profile_id` es el perfil de la persona con el que corre la app, o `null`.
- **Debe:** `states` lleva sólo lo que cambió. El estado de una acción con argumento va por argumento. `catalog` reemplaza el catálogo entero, por ejemplo cuando cambian los presets. Una acción del catálogo sin estado avisado se pinta como `available`; una que no está en el catálogo queda apagada y no se manda.
- Cada acción del catálogo puede traer `color`, el de su luz. La luz de un control sale, en este orden, del `color` del mapeo, de la `light` que la app pidió para ese estado (color, tenue y movimiento), del `color` del catálogo y, si no hay ninguno, de `white` ([mapa §2](nu9ve-controller-map-v1.md#2-luces-que-pide-una-app)).
- **Debe:** `focus` con `"active": true` cuando la app pasa al frente (§5). `"active": false` puede mandarse y no quita la página.
- `welcome` y `devices` traen los aparatos abiertos: `device` (id del agente, estable mientras corre), `profile` (o `null`) y `port`. `page` dice de qué aparato es.
- **Debe:** `action` trae la acción del catálogo, su `argument`, y `value` (0 a 1, ya normalizado y filtrado) si es continua o `delta` si es relativa ([mapa §4](nu9ve-controller-map-v1.md#4-lectura-en-vivo)). Un botón en una acción continua manda `value` 1 y 0, alternando. `control` es el id del control en el perfil o, para una dirección cruda, su etiqueta (`CH 1 · NOTE 0x30`). `via` es `page` o `zone` (§5). `host_ns` es el reloj de host del mensaje MIDI, el mismo de la [señal de grabación §5](nu9ve-recording-v1.md#5-reloj-alinear-otra-línea-de-tiempo), para que una acción caiga en el instante exacto de la sesión.
- **Debe:** una acción de disparo nunca se pierde ni se junta con otra. De una continua, si la app va atrasada, el agente manda sólo el último valor de cada control.
- **Debe:** `released` dice que el agente soltó el aparato (va a actualizarse o a salir). La app no abre el aparato por su cuenta mientras la conexión siga viva.
- **Debe al lector:** ignorar campos y tipos que no conoce.

## 5. Páginas, zonas y modos

- **Página:** por omisión es la **app que está al frente**, si tiene mapa para ese aparato. **Decidido por el usuario (2026-11-27):** cada app le avisa al agente con `focus` (§4) cuando pasa al frente; el agente no usa `NSWorkspace`. La página es de la última app con mapa que avisó `focus`; si ninguna avisó, de la primera que se conectó. Pasar al frente una app que no es de la suite no mueve la página. Avisa con `page` a la app que gana o pierde la página.
- **Cambiar de página a mano:** un control reservado por aparato recorre las apps conectadas que tienen mapa. Cada pulsación fija la siguiente; después de la última, la página vuelve a seguir al frente. La página fijada gana a la del frente. El control se pinta blanco: tenue mientras la página sigue al frente, brillante mientras está fijada. Por omisión es el `pageControl` del perfil ([mapa §1](nu9ve-controller-map-v1.md#1-perfil-físico-v2)): en el Launchpad Pro, «Session» (`top-5`). La persona lo cambia, o lo quita con `"pageControl": null`, en `suite.json` (§6). Es reservado: si un mapa le da una acción, el agente la ignora y el conflicto se muestra (§5).
- **Zona:** un mapa puede marcar controles como zona de su app, con una condición: `always` o `recording` (mientras esa app graba). En su zona, la app recibe las acciones aunque no tenga la página. Así «Record» y «Click» del Launchpad siguen siendo de Stalker mientras graba con SoundBender al frente.
- **Debe:** si dos zonas activas quieren el mismo control, gana la de la app que graba; si ninguna graba, la que se conectó primero. El conflicto se muestra en el configurador de las dos apps y en nüün.
- **Conflictos que se muestran** (`SurfaceMaps` de `Nu9veSurface`), por perfil de la persona: dos apps con el mismo control en sus zonas para el mismo aparato (`zone`); un mapeo en el control de página, que el agente ignora (`reserved`); un control o una dirección repetidos en un mapa (`repeated`); y un mapa que no se lee (`unreadable`). Dos apps con el mismo control fuera de zonas no chocan: decide la página.
- **Modos:** `normal` y `recording`. El agente pasa a `recording` cuando la señal de grabación dice `capturing` ([grabación §4](nu9ve-recording-v1.md#4-cómo-lee-un-consumidor)), sin sondear: vigila la carpeta. En `recording` se usa la sección `modes.recording` del mapa, si la hay. Sirve para quitar las acciones que no deben dispararse sin querer mientras se graba.

## 6. Mapas por aparato, app, modo y perfil

- **Dónde:** `~/Library/Application Support/nu9ve/controllers/<perfil>/<app>.json`, con `<perfil>` el `profile_id` de [`nu9ve-profile-v1`](nu9ve-profile-v1.md), o `_` sin perfil. `NU9VE_CONTROLLERS_DIR` la reemplaza; en QA, cuelga de la raíz de QA, como la [señal de grabación §1](nu9ve-recording-v1.md#1-dónde). El agente usa las carpetas hermanas de la suya (`…/nu9ve/controllers` y `…/nu9ve/recording`), así que un agente con `NU9VE_AGENT_DIR` de QA lee las de su raíz de QA.
- **Qué:** un [mapa v1](nu9ve-controller-map-v1.md#3-mapa) (por aparato, con `devices[].match`) con dos campos opcionales por aparato:
  - `zones`: `[{"when":"recording","controls":["left-8","left-2"]}]`;
  - `modes`: `{"recording":{"mappings":[…]}}`, que reemplaza los mapeos de esos controles en ese modo.
- **Debe:** cada app escribe **sólo** su archivo, con su configurador. El agente los lee y los vuelve a leer cuando cambian (eventos del sistema de archivos, no sondeo). nüün los lee todos.
- **`suite.json`** en la raíz de `controllers/`: lo que no es de ninguna app. Por ahora, el control de cambiar de página por aparato: `{"schemaVersion":1,"devices":[{"match":"Launchpad Pro","pageControl":"bottom-8"}]}`, o `null` para ninguno. Sin él, vale el del perfil.
- Con agente, el archivo propio de una app (Stalker: `controllers.json`) pasa a esta carpeta. Sin agente, la app sigue leyendo el mismo archivo en modo directo, así que no hay dos verdades.

## 7. Si el agente se cae o se actualiza

- **Al salir en orden o antes de actualizarse** ([agente §4](nu9ve-agent-v1.md#4-instalar-actualizar-quitar)): el agente apaga todas las luces, manda `released` a cada app y cierra el aparato y las conexiones.
- **Si se cae:** las conexiones se cierran solas. La app espera **2 s** a que vuelva (una actualización lo arranca de nuevo), reintentando la conexión. Si no vuelve, pasa a **modo directo**: abre el aparato y lo limpia entero, porque las luces del agente quedaron pintadas.
- **Si el agente vuelve** con la app en modo directo, la app suelta el aparato como en la §1.
- **Debe:** la app nunca queda sin poder accionarse por un agente caído. Lo peor es un hueco de 2 s.

## 8. Reposo y eficiencia

- **Debe:** el agente abre el sistema MIDI **sólo** si hay al menos una app conectada con un mapa encendido. Sin aparato, sólo espera las notificaciones del sistema.
- **Debe:** nada corre por tiempo, salvo el parpadeo por software de un perfil `onOff` mientras algo parpadea.
- **Debe:** una app sin mapa para ningún aparato no se conecta.
- **Conviene:** el agente traduce cada mensaje MIDI a una acción sin reservar memoria nueva: la tabla control → (app, acción) se arma al cambiar un mapa, una página o un modo, no con cada mensaje.

## 9. Cómo llegan las apps

| Parte | Qué | Dónde |
| --- | --- | --- |
| C0 | `Nu9veMIDI` en `swift/` | core, **hecho el 2026-11-27** |
| C1 | Perfil v2 y [`nu9ve-controller-map-v1`](nu9ve-controller-map-v1.md) | core, **hecho el 2026-11-27** |
| C2 | Este contrato | core: propuesta, con el transporte decidido |
| C3 | El agente como dueño del aparato (`surface_v1`, MIDI en Rust con el perfil v2), con Stalker como primer cliente y su modo directo | core: **C3a, C3b y C3c hechos el 2026-11-27** (`nu9ve-midi` con luces y CoreMIDI; el agente 0.3.0 con el socket). Falta C3d, Stalker |
| C4 | Una segunda app (SoundBender o curado), las piezas comunes del configurador en `swift/` (la cara del aparato, la fila con «Aprender», la insignia de estado; el borrador es la página «Controles» de Stalker) y la vista de conjunto en nüün | core y cada app |

## Adopción

Quién cumple qué está en [adopción por app](../docs/adoption.md#nu9ve-surface-v1).

## 10. Decisiones abiertas

1. ~~**Transporte (§2).**~~ Decidido el 2026-11-27: socket local con marcos.
2. ~~**El MIDI del agente.**~~ Decidido por el usuario el 2026-11-27: el agente hace el MIDI en Rust, dentro de `nu9ve-midi`, sin un ayudante en Swift (que sería un proceso más). El crate gana el perfil v2 y las luces (C3a) y el puerto con CoreMIDI por FFI directo, sin crates (C3b, [README](../rust/nu9ve-midi/README.md#abrir-el-aparato)).
3. ~~**El control de cambiar de página** en el Launchpad Pro.~~ Decidido el 2026-11-27, con el pedido del usuario de tener uno por omisión: «Session» (`top-5`), como `pageControl` del perfil.
4. **Windows:** named pipe y la API MIDI de Windows, cuando haya una app con controlador allí.

## Cambios

- **2026-11-27, con C3c:** `focus` en lugar de `NSWorkspace` (decisión del usuario); `device` en `page`; `port` en `devices`; `color` en el catálogo y el orden de las luces; la ruta del socket en `engine_info`; `suite.json` en la raíz; una conexión por app. Sigue siendo `nu9ve-surface/1`: nadie la usaba.

Un tipo de mensaje o un campo opcional nuevo es una versión menor, con su línea en el [CHANGELOG](../CHANGELOG.md). Cambiar el significado de un mensaje o las reglas de dueño de la §1 es `nu9ve-surface/2`.
