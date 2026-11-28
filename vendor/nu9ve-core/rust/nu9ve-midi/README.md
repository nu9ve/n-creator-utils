# nu9ve-midi

Lee mensajes de un controlador, detecta gestos y reproduce capturas de forma
determinista. El adaptador de cada app abre los puertos y decide qué hacer
con el valor de cada control.

## API

- `Message::parse(&[u8])` lee un mensaje de canal completo de tres bytes.
  Normaliza las dos formas de soltar una nota a velocidad cero. Las claves
  `Key` en memoria usan canales 0–15. El adaptador entrega cada mensaje por
  separado.
- `Explorer::new(quiet_ms)`, `start_baseline(now_ms, length_ms)`, `feed` y
  `poll` permiten aprender en vivo. `classify` trabaja sobre un gesto ya
  delimitado. `Detected` devuelve la clave, el modo, el LSB, la resolución,
  el rango observado y las CC que repiten la señal.
- `Encoding::delta` devuelve pasos con signo. `combine_cc14` une MSB y LSB.
  El adaptador conserva el último MSB por control y decide cómo tratar un
  LSB que llega primero. `normalize` da 0–1 con el detente exactamente en
  0,5 para 7 y 14 bits. `Gate::pass` usa milisegundos y una banda muerta
  como fracción de ese recorrido. Los mensajes a 40 ms o menos del anterior
  pasan durante un movimiento.
- `parse_log` lee líneas de milisegundos desde la conexión, tabulador y
  bytes en hex. `replay(&events, baseline_ms, quiet_ms)` usa un reloj
  simulado que empieza en cero. La marca de un gesto es la de la consulta
  que lo cerró, o la del último evento si se cierra al agotar la captura.
- `Profile::from_json(profile::PIONEER_DDJ_SB_V1)` carga y valida el perfil
  incluido. `matches` compara alias de puerto sin distinguir mayúsculas.
  `control` busca por ID estable; `control_by_key` lo hace por dirección
  principal. `Control::key` convierte el canal del perfil a 0–15.

## Perfil físico v1

[`pioneer-ddj-sb-v1.json`](../../data/midi/pioneer-ddj-sb-v1.json) declara
`schemaVersion: 1`, el ID del modelo, el fabricante, alias de puerto,
procedencia y controles. Cada control tiene un ID estable y una etiqueta
legible. Los canales del JSON empiezan en 1 y `number` y `lsb` admiten
enteros decimales o cadenas hexadecimales. En el caso de CC de 14 bits,
`number` es la dirección principal y `lsb` la baja. Las copias de los
niveles FX conservan sus direcciones y apuntan al ID principal con
`mirrorOf`. Los modos y codificaciones mantienen la forma de serialización
del lector original.

`verified` indica que se observó la dirección en hardware. `identified:
false` indica que sigue pendiente identificar el gesto físico. Browse
pulsado es la nota 0x41 del canal 7, observada con CoreMIDI. La nota 0x65
del mismo canal conserva la observación de WinMM con gesto sin identificar.

El único ajuste de calibración inicial es `deadband: 0.008` para ambos
faders de tempo, 0,8 % de recorrido normalizado. Para preservar la
precedencia al migrar, el ajuste explícito de la app o del usuario gana,
seguido del perfil y luego del valor por omisión de la app. En el lector
original ese último valor es 0,002 para 14 bits y 0 para 7 bits. La escala
del jog en segundos por paso, la dirección aplicada al valor, el rango de
tempo, los slots, las acciones y la inicialización del mixer siguen en
la app.

## Perfil físico v2 y luces

Desde el 2026-11-27 el crate lee también el **perfil v2** de [`nu9ve-controller-map-v1` §1](../../contracts/nu9ve-controller-map-v1.md#1-perfil-físico-v2): `feedback`, `grid`, `layout` y `lit`. Un perfil v1 que trae `feedback` es inválido.

- `Profile::is_lit` dice si un control tiene luz (las copias `mirrorOf` nunca); `Profile::primary` da el principal de una copia. `profile::NOVATION_LAUNCHPAD_PRO_V2` es el Launchpad Pro original en modo programmer.
- `Light` es la luz que pide una app: `Color` (los nueve del contrato), tenue y `Motion` (`Solid`, `Flash`, `Pulse`). `Light::for_state` da la luz por omisión de cada `ActionState` ([`nu9ve-surface-v1` §3](../../contracts/nu9ve-surface-v1.md#3-estados-y-luces)).
- `Feedback::messages` da los bytes de una luz en un control; `blinks_in_software`, si hay que parpadear con un temporizador.
- `LightBoard` es lo que el aparato muestra: `update` manda sólo lo que cambió, `forget` manda `init`, todo apagado y lo que se quiere (al conectar), `blink` es un paso del parpadeo por software, `needs_blink` dice si hace falta el temporizador y `darken` apaga lo encendido al soltar el aparato.
- `Reader` es la lectura en vivo (§4 del contrato): disparo al bajar y una vez, 14 bits con el último MSB por control (un LSB sin MSB se ignora), banda muerta con la precedencia de abajo, relativos en pasos y copias leídas como su principal.

[`data/midi/controller-vectors-v1.json`](../../data/midi/controller-vectors-v1.json) son los casos comunes con `Nu9veMIDI`: se escriben con Swift como referencia (`NU9VE_WRITE_MIDI_VECTORS=1 swift test --filter ControllerVectors`) y los pasan los dos (`tests/controller_vectors.rs`).

## Abrir el aparato

- `MidiSystem` es el trait con que el dueño de un aparato lo abre, por nombre de puerto: `sources`, `open`, `close` y `send` (mensajes cortos y SysEx, en orden, al destino del mismo nombre). Las pruebas del dueño usan uno falso; `NoMidi` sirve donde no hay MIDI.
- Todo llega por eventos a un `Sink` (`PortEvent::Connected`, `Disconnected` y `Message` con `host_ns`, el reloj de host en nanosegundos), nunca por sondeo.
- `split_channel_messages` separa los bytes de un paquete nativo en mensajes de canal: expande el running status y descarta SysEx, tiempo real y lo de dos bytes.
- `coremidi::CoreMidi` (sólo macOS) es el adaptador, por **FFI directo** a CoreMIDI y CoreFoundation, sin crates: no suma dependencias ni licencias (`cargo deny` sigue igual). Un hilo propio crea el cliente y corre un `CFRunLoop`, donde CoreMIDI deja sus avisos; duerme en el kernel hasta que llega uno. Reintenta durante unos 3 s si el servidor MIDI está arrancando (`kMIDIServerStartErr`, -10839, y -304). `coremidi::VirtualPort` es un aparato virtual (fuente y destino con el mismo nombre) para las pruebas.
- Prueba de punta a punta: `tests/coremidi.rs`, con un aparato virtual que aparece, toca dos notas en un paquete, recibe luces con un SysEx y se desconecta.

## Fijar y verificar

En `vendor/nu9ve-core.paths`, incluir estas rutas:

```text
rust/nu9ve-midi
data/midi/pioneer-ddj-sb-v1.json
data/midi/novation-launchpad-pro-v2.json
```

Desde el perfil v2, el crate embebe también el Launchpad Pro: una app que vuelve a fijar suma esa ruta.

Referenciar `nu9ve-midi` por ruta desde el manifiesto consumidor. El crate
incluye sus pruebas, la captura original y la salida esperada del replay.
La [correspondencia de los 125 controles](ddj-sb-control-ids.md) permite
migrar las 95 asignaciones del preset de curado en su propio cambio.

Desde `rust/`, ejecutar `cargo test --locked -p nu9ve-midi` y
`cargo clippy --locked -p nu9ve-midi --all-targets -- -D warnings`.

La [captura CoreMIDI](tests/fixtures/ddj-sb-2026-11-22.txt) procede del
árbol local de curado sobre `bb6f780`, con 1165 mensajes de la sesión del
2026-11-22. Su SHA-256 es
`d13ed8b92ab7ec56f68fa5fa99e3f370951e6b616dade2508ce296d41f627651`.
El primer mensaje llegó a los 21 356 ms. La
[salida esperada](tests/fixtures/ddj-sb-2026-11-22-replay.json) se generó con
el lector de ese árbol, con 10 000 ms de línea base desde la conexión y
400 ms de silencio entre gestos. El replay conserva los 18 gestos.
La prueba del fader de tempo derecho conserva 206 de sus 208 valores.
Quedan pendientes la identificación física de las direcciones marcadas y
las comprobaciones de escucha y reconexión en la app.
