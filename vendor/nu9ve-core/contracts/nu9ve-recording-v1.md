# Señal de grabación v1

Estado: **propuesta** (2026-11-27). Dice a cualquier app de la suite **si se está grabando en esta máquina**, qué sesión es y desde qué instante cuenta su reloj. Así una app calla sus sonidos y sus avisos mientras se graba, y alinea su propia línea de tiempo con la de la grabación. Lo pidió Stalker: la decisión del usuario del 2026-11-27 es que sea un contrato propio del core y no una revisión de `stalker-clipper`. Los que la usan casi no tienen que ver con Clipper (Brave, el escenario, nove, la cámara virtual), y la señal es un estado en vivo, no un archivo entregado, así que cambia a otro ritmo ([Stalker en la suite](../../stalker/docs/2026-11-18-suite-y-prioridades.md), [navegador §7](../docs/suite-browser.md#7-contratos-nuevos)).

**Debe** es obligatorio. **Conviene** es lo recomendado. «Productor» es la app que graba: hoy Stalker, después el escenario o una cámara. «Consumidor» es cualquier app que lee la señal. «Sesión» es la del productor, con el mismo id que va en lo que entrega (`sessionID` del manifiesto de Stalker).

## 0. Para qué y para qué no

- **Para** contestar tres preguntas en esta máquina: ¿se está grabando?, ¿qué sesión?, ¿desde qué instante? Una app que arranca a media grabación lo sabe igual que una que estaba abierta.
- **Para** alinear: quien escribe otra línea de tiempo (`web.ndjson` del navegador, eventos de una escena) usa el mismo `t` que los eventos de la sesión (§5).
- **No** lleva contenido: ni rutas, ni nombre del proyecto, ni la pantalla o la ventana capturada, ni títulos. Sólo el estado (§6).
- **No** controla la grabación. En v1 nadie empieza ni detiene una grabación por aquí: sólo se lee.
- **No** calla el contenido. `quiet` (§4) calla los sonidos de **interfaz** ([sonido v1](nu9ve-sound-v1.md)) y retiene los avisos. Nunca calla el sonido del programa: un sampler, un pad o la mezcla que va a la grabación o a una llamada. Ese sonido es lo que se graba.
- **No** sustituye lo que el productor entrega al terminar (el bundle y su manifiesto de [`stalker-clipper`](../../stalker/docs/clipper-contract.md)). Una sesión que terminó se busca allí, no aquí.
- **Local.** La señal no sale de la máquina: el agente no la publica en la [carpeta de intercambio](nu9ve-exchange-v1.md).

## 1. Dónde

| Sistema | Carpeta |
| --- | --- |
| macOS | `~/Library/Application Support/nu9ve/recording/` |
| Windows | `%LOCALAPPDATA%\nu9ve\recording\` |

- **Debe:** la variable `NU9VE_RECORDING_DIR` (ruta absoluta) reemplaza la carpeta, para el productor y para el consumidor. Si la variable no es absoluta, no se escribe ni se lee nada.
- **Debe:** una app en su modo QA (`STALKER_QA_ROOT` o similar) **no** escribe ni lee la carpeta de la persona, porque callaría las apps de verdad. Sin `NU9VE_RECORDING_DIR`, la carpeta cuelga de su raíz de QA, en la misma ruta relativa que bajo la carpeta de usuario (`<raíz de QA>/Library/Application Support/nu9ve/recording/` en macOS). Así una app en QA nunca se cruza con la señal real, aunque falte la variable. Las pruebas la fijan igual (versión menor del 2026-11-27, pedido de Stalker).
- **Debe:** carpeta, permisos, temporales y lectura con tope como el [registro de motores §5](nu9ve-registry-v1.md#5-escribir-reemplazar-borrar): carpeta `0700` y archivo `0600` en macOS, sin seguir enlaces, y en Windows la DACL del perfil.
- Windows no tiene productor todavía. La carpeta queda escrita para cuando exista la captura allí (§9).

## 2. Archivo de estado

Un archivo por productor: `<app>.json`, con `<app>` como en el [registro §2](nu9ve-registry-v1.md#2-archivo) (`stalker`). El archivo es **la autoridad**: la notificación de la §3 sólo avisa que cambió.

```json
{
  "contract": "nu9ve-recording/1",
  "app": "stalker",
  "app_version": "0.6.0",
  "pid": 5120,
  "process_start_us": 1795600000123456,
  "seq": 7,
  "state": "recording",
  "session_id": "ded6fcd1-7019-42e1-835e-c25896147a7f",
  "epoch": {"clock": "coremedia-host-clock", "value": 123456789000, "timescale": 1000000000},
  "epoch_wall_clock": "2026-11-27T18:00:00.123Z",
  "captures": ["screen", "system_audio", "microphone"],
  "ended_at": null,
  "written_at": "2026-11-27T18:00:00.125Z"
}
```

| Campo | Debe | Qué es |
| --- | --- | --- |
| `contract` | Sí | `"nu9ve-recording/1"` |
| `app` | Sí | Igual al nombre del archivo sin `.json` |
| `app_version` | Sí | La de su «Acerca de» |
| `pid`, `process_start_us` | Sí | El proceso que graba y su hora de inicio, con las reglas y la vida del [registro §4](nu9ve-registry-v1.md#4-vida-pid-con-hora-de-inicio) |
| `seq` | Sí | Entero ≥ 1 que sube en cada escritura del mismo proceso. Sirve para ignorar una notificación vieja (§4) |
| `state` | Sí | `idle`, `preparing`, `recording` o `finalizing` (tabla de abajo) |
| `session_id` | En `preparing`, `recording` y `finalizing`; `null` en `idle` | UUID de la sesión, en minúsculas con guiones ([JSON §7](nu9ve-json-v1.md#7-ids)). Es el `sessionID` del manifiesto de Stalker, que lo escribe en mayúsculas: se compara sin distinguir mayúsculas |
| `epoch` | En `recording` y `finalizing`; `null` en los demás | El t = 0 de la sesión: `clock`, `value` (entero) y `timescale` (entero > 0). En v1, `clock` sólo puede ser `coremedia-host-clock` (§5) |
| `epoch_wall_clock` | Con `epoch`; `null` sin él | Hora civil muestreada junto al epoch, RFC 3339 en UTC con milisegundos. Sólo para personas y para agrupar: **nunca** para alinear (§5) |
| `captures` | Sí | Qué se graba: subconjunto sin repetir de `screen`, `system_audio`, `microphone` y `camera`. Vacío en `idle` |
| `ended_at` | En `finalizing`; `null` en los demás | Cuándo dejó de capturar, RFC 3339 en UTC |
| `written_at` | Sí | Cuándo se escribió, RFC 3339 en UTC. Sólo para personas: la vida la decide el [registro §4](nu9ve-registry-v1.md#4-vida-pid-con-hora-de-inicio) |

| `state` | Qué pasa | En Stalker |
| --- | --- | --- |
| `idle` | Nada se graba | Listo |
| `preparing` | Va a empezar: cuenta atrás, permisos, apertura de dispositivos. Todavía no hay epoch | Por empezar… y Preparando… |
| `recording` | Se captura. El epoch está fijo y no cambia en toda la sesión | Grabando |
| `finalizing` | Ya no se captura; el productor cierra archivos y derivados | Guardando… |

- **Debe:** UTF-8 sin BOM, un objeto JSON, como mucho **4 KiB**. Todos los enteros caben en 2^53 − 1.
- **Debe** al consumidor: ignorar campos que no conoce; tratar como inválido un archivo al que le falta un campo, lo trae de otro tipo o no respeta lo que la tabla pide para su `state`.
- **Debe:** un `contract` con otro mayor (`nu9ve-recording/2`) es inválido para un consumidor v1.

### 2.1 Escribir

El productor:

- **Debe:** escribir **sólo en cada cambio de estado**, nunca con un latido. Grabando no se toca el disco por esto. Escribe con [`nu9ve-fs`](nu9ve-fs-v1.md) en modo `Plain` (temporal hermano y rename, sin vaciar la caché del disco), como Stalker ya hace con sus presets.
- **Debe:** pasar a `recording` en cuanto fija el epoch y antes de aceptar la primera muestra. Pasa a `finalizing` cuando deja de capturar, antes de cerrar archivos, y a `idle` cuando la sesión quedó escrita, haya salido bien o no. El resultado (`completed`, `interrupted`, `failed`) lo dice su manifiesto, no esta señal.
- **Debe:** escribir el archivo **antes** de mandar la notificación de ese cambio (§3).
- **Debe:** no pisar el archivo **vivo** de otro proceso de la misma app. Si existe uno así, no publica la señal y lo dice en su registro local. Uno viejo o inválido lo reemplaza.
- **Debe:** al salir de forma ordenada, borrar su archivo sólo si sigue siendo suyo (mismo `pid` y `process_start_us`). Ausente es lo mismo que `idle`.
- Si el productor muere grabando, el archivo queda en `recording`, pero su proceso ya no está vivo, así que no cuenta (§4). Recuperar esa sesión es cosa del productor.

## 3. Notificación

- **macOS, Debe:** después de cada escritura, `DistributedNotificationCenter.default().postNotificationName("app.nu9ve.recording.changed", object: "<app>", userInfo: …, deliverImmediately: true)`. `object` es el nombre del productor. `deliverImmediately` llega también a un observador suspendido.
- **Conviene:** `userInfo` con `state`, `session_id` y `seq`, copiados del archivo, para un consumidor que sólo necesita saber si graba. Es una pista: **Debe** al consumidor releer el archivo antes de actuar sobre algo que no sea callar un sonido.
- **Windows:** sin notificación en v1 (§9). El consumidor vigila la carpeta (`ReadDirectoryChangesW`) o la relee como dice la §4.
- «Empezó» es el paso a `recording`; «terminó» es el paso de `recording` a `finalizing`. No hay nombres de notificación aparte: una sola notificación con el estado nuevo.

## 4. Cómo lee un consumidor

- **Debe:** al arrancar, listar los `*.json` de la carpeta con nombre válido, leer cada uno con tope y comprobar su vida con el [registro §4](nu9ve-registry-v1.md#4-vida-pid-con-hora-de-inicio). Un archivo **viejo** o inválido cuenta como `idle`.
- **Debe:** con cada notificación, releer el archivo de esa app. Si su `seq` es menor o igual al último que vio de ese mismo `pid` y `process_start_us`, la ignora.
- **Debe:** mientras crea que alguien graba, comprobar la vida de ese proceso **al menos cada 5 s**. Así un productor que murió sin avisar deja de contar. Es una llamada al sistema, no una lectura de disco.
- **Conviene:** un consumidor que no puede observar la notificación relee la carpeta cada 5 s como mucho, y sólo mientras le importa (una app con sonidos abierta, la extensión con una pestaña activa).
- Lo que se deduce, para toda la máquina, sumando los archivos vivos:

| Pregunta | Respuesta | Quién la usa |
| --- | --- | --- |
| `capturing` | Algún archivo vivo en `recording` | Línea de tiempo del navegador, cámara virtual, nüün (no tocar un bundle en curso) |
| `quiet` | Algún archivo vivo en `preparing` o `recording` [Sonido §6](nu9ve-sound-v1.md#6-cuándo-calla) y [avisos](nu9ve-notifications-v1.md): callar los sonidos de interfaz y retener los avisos, nunca el sonido del programa (§0). En `preparing` ya se calla, para que el sonido no caiga en el primer segundo |
| `sessions` | `session_id`, `app`, `captures` y `epoch` de cada archivo vivo en `preparing`, `recording` o `finalizing` | Quien escribe una línea de tiempo, o la cámara que se suma con el mismo epoch |

- **No debe:** el consumidor borra o escribe archivos de otra app en esta carpeta.

## 5. Reloj: alinear otra línea de tiempo

Es la regla de `stalker-clipper` §3, para quien no es Stalker: **`t` es el tiempo de la sesión en microsegundos desde el epoch, con el mismo reloj de la captura.** Un evento con `t = 12500000` cae en el segundo 12,5 de `screen.mov` y de `mic.wav`.

- **macOS, Debe:** `coremedia-host-clock` es el reloj de host de CoreMedia (`CMClockGetHostTimeClock()`), el mismo de `mach_absolute_time()`. El consumidor toma «ahora» con `CMClockGetTime(CMClockGetHostTimeClock())`, o con `mach_absolute_time()` pasado a nanosegundos con `mach_timebase_info` (Rust, C), y calcula:

  ```text
  epoch_ns = floor(epoch.value × 1 000 000 000 / epoch.timescale)
  t        = floor((ahora_ns − epoch_ns) / 1000)        // µs, entero
  ```

- **Debe:** no escribir eventos con `t < 0` (antes del epoch) ni después de que el productor pasó a `finalizing`.
- **No debe:** alinear con la hora civil (`epoch_wall_clock`, `Date()`, `SystemTime`). En una sesión real de Stalker difirió 219 s del offset acústico ([`stalker-clipper` §3](../../stalker/docs/clipper-contract.md#3-reloj-la-regla-única)).
- Ese reloj no avanza con la Mac dormida, igual que la captura: los dos siguen alineados.
- Lo que el consumidor escribe lleva el `session_id` y llega a la sesión por su propio contrato (por ejemplo `web.ndjson` dentro del bundle, [navegador §7](../docs/suite-browser.md#7-contratos-nuevos)). Cómo se entrega no es parte de este contrato.

## 6. Privacidad

- La señal dice **que** se graba, no **qué**. `captures` dice el tipo de fuente, nunca cuál pantalla, ventana, app o micrófono.
- **No debe:** rutas, nombres de proyecto o de carpeta, títulos ni nada de lo grabado. `session_id` es opaco: con él sólo se encuentra la sesión quien ya tiene acceso al bundle.
- La notificación distribuida la puede ver cualquier proceso del usuario. Por eso lleva lo mismo que el archivo, nada más.

## 7. Ejemplos

[`data/recording-examples-v1.json`](../data/recording-examples-v1.json): archivos válidos e inválidos con su motivo, la vida contra una tabla de procesos, lo que deduce un consumidor de varios archivos (`capturing`, `quiet`, `sessions`), el orden por `seq` y la cuenta de `t` desde el epoch. `python3 -I scripts/recording-examples.py --check` comprueba el archivo contra la referencia de §1 a §5 que lleva el script.

## 8. Cómo llegan las apps

| App | Papel | Primer paso |
| --- | --- | --- |
| Stalker | Productor | Escribir el archivo y mandar la notificación en sus cuatro estados, con `STALKER_QA_ROOT` apuntando a una carpeta de QA; probar que un cierre forzado grabando deja de contar |
| Las apps con sonidos | Consumidores de `quiet` | Callar como pide [sonido §6](nu9ve-sound-v1.md#6-cuándo-calla) también cuando graba **otra** app |
| nove, nüün | Consumidores de `quiet` | Retener sus avisos mientras se graba y entregarlos al terminar |
| Extensión de Brave, escenario | Consumidores de `sessions` | `web.ndjson` sólo mientras `capturing`, con el `t` de la §5 |
| Cámara virtual | Consumidor de `sessions` | Sumarse con el mismo epoch (Fase B de Stalker) |

## Adopción

Quién cumple qué está en [adopción por app](../docs/adoption.md#nu9ve-recording-v1).

## 9. Decisiones abiertas

1. **Windows.** Ni productor ni notificación todavía. Candidatos: un evento con nombre por productor, o sólo vigilar la carpeta. Se decide cuando haya captura en Windows. El reloj de allí (QPC) sería otro valor de `clock` en una versión menor.
2. **`userInfo` y el sandbox.** No se ha probado que un consumidor con App Sandbox reciba `userInfo` de una notificación distribuida. Por eso el archivo es la autoridad. Lo prueba la primera app con sandbox que consuma la señal.
3. **Pausa.** Stalker no pausa hoy. Si llega, sería un estado `paused` en una versión menor: el epoch no cambia y `t` sigue corriendo.
4. **Control.** Empezar o detener desde otra app (MCP, un atajo de la suite) queda fuera de v1. Si llega, irá por el motor del productor ([`nu9ve-engine-v1`](nu9ve-engine-v1.md)), no por esta carpeta.

## Cambios

Un campo, un estado o un valor de `clock` nuevo es una versión menor, con su línea en el [CHANGELOG](../CHANGELOG.md). Cambiar el significado de un campo o la regla de la §5 es `nu9ve-recording/2`.

- **2026-11-27, versión menor** (pedidos de Stalker tras implementarlo): en QA, la carpeta cuelga de la raíz de QA aunque falte `NU9VE_RECORDING_DIR` (§1); `quiet` sólo calla la interfaz, nunca el sonido del programa (§0); el ejemplo con BOM trae sus bytes en base64 (§7). Nadie cambia de comportamiento: Stalker ya lo hacía así.
