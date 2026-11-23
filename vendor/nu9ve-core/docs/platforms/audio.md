# Audio en la suite nu9ve

Estado: **propuesta** del 2026-11-16, pedida por el usuario al revisar SoundBender. Construido: `nu9ve-loudness` (2026-11-17, [rust/](../../rust/README.md#loudness)); lo demás no. Leído en el código de cada repo el 2026-11-16. Diseño del lado de SoundBender: `soundbender/docs/plans/portable-engine.md` y su decisión 034. [Mapa de la suite](../suite.md) · [Cómo se comparte](../suite-sharing.md) · [Motor en Rust](https://github.com/nu9ve/n-creator-utils/blob/master/docs/platforms/rust-engine.md) · [Sonido de tema v1](../../contracts/nu9ve-sound-v1.md).

Responde a esta pregunta: ¿conviene un solo motor de audio para todas las apps, o utilidades compartidas con lo propio de cada app?

## 1. Respuesta corta

- **No a un solo motor.** Las apps ejecutan el audio de formas incompatibles: decks en tiempo real, render offline al soltar, lotes de video y disparos cortos de interfaz. Un motor único obligaría a cada una a cargar con las restricciones de las demás y ataría sus versiones.
- **Sí a capas compartidas** de primitivas y mediciones en Rust, porque los números tienen que coincidir entre apps: un −14 LUFS debe ser el mismo en Clipper, curado y SoundBender.
- **Cada app conserva su motor de dominio**, construido sobre esas capas.
- Se aplica la regla vigente: replicar primero y extraer a la tercera copia, con copia fijada ([suite-sharing](../suite-sharing.md)).

## 2. Inventario

| App | Qué audio tiene | Lenguaje | Modelo de ejecución |
| --- | --- | --- | --- |
| curado | `crates/audio-engine`, unas 10 200 líneas: salida (cpal), decodificación (symphonia), stretch (`signalsmith-stretch`), beat, pitch, loudness, limitador, resample, pruebas de no-asignación; stems en `src-tauri/src/stems.rs` | Rust | Tiempo real, dos decks |
| Clipper | `clipper-core/src/rpc_score.rs` planea la hoja de cues. `native/clipper-macos-render/Sources/ClipperScore` sintetiza voces, batería, FX, buses, ducking y master (unas 2 900 líneas). El análisis de intros es numpy + FFmpeg | Rust + Swift + Python | Offline, por lotes |
| Fractos | `FractosSoundV2`, unas 4 300 líneas con pruebas: sonidos de interfaz | Swift + Accelerate | Disparos cortos de baja latencia |
| SoundBender (antes SoundShop) | `SoundCore`, 6 900 líneas: espectral, síntesis de pinceles, master, LUFS, BPM y clave; stems con `separate.py` | Swift + Accelerate; motor portable en plan | Offline al soltar, más mezcla en vivo |
| nüün | Descargas y stems por CLI (`manager/`, `demucs`) | Python | Procesos externos |

Repeticiones de hoy:

- La **separación con Demucs** está en tres sitios.
- **Loudness y limitador** en curado y en SoundBender.
- **BPM y clave** en curado, SoundBender y Clipper (análisis de intros).
- **Tres sintetizadores**, cada uno con un propósito distinto.

## 3. Capas propuestas

| Crate | Contenido | Copias hoy | Cuándo mudarlo |
| --- | --- | --- | --- |
| `nu9ve-audio-io` | WAV de 16, 24 y 32 bits con dither, metadatos BPM y clave, decodificación, resample | curado, SoundBender, Clipper | Al existir `nu9ve/core` |
| `nu9ve-dsp` | FFT y STFT por bloques con halos, ventanas, filtros, interpolación | SoundBender, curado | Cuando lo use una tercera app |
| `nu9ve-loudness` | LUFS integrado y de corto plazo, true peak. Sin limitador: cada app conserva el suyo (§5) | curado, SoundBender (Clipper normaliza con FFmpeg) | Al existir `nu9ve/core`; los números deben coincidir |
| `nu9ve-music` | Beats y downbeats, BPM, clave, escalas, afinaciones, acordes por grados | curado, SoundBender, Clipper (Python) | Al existir `nu9ve/core` |
| `nu9ve-rt` | Colas sin bloqueo, rampas, comprobación de no-asignación, salida a dispositivo | curado; el plugin de SoundBender | Con el plugin |
| `nu9ve-stems` | Adaptador de Demucs: proceso aislado y cancelable, manifiesto, publicación atómica | curado, SoundBender, nüün | Ya hay tres copias |
| `nu9ve-sound` | Motor de sonidos de interfaz ([S17](../audits/session-briefs-2026-11-10.md#s17--motor-de-sonido-en-rust)) | Fractos (Swift) | Según S17; no depende de las capas anteriores |

Lo que **no** se comparte todavía:

- **Síntesis musical.** Los patches de SoundBender, las voces de ClipperScore y las cues de Fractos sirven a propósitos distintos. Conviene esperar a ver si el motor de patches de SoundBender termina sirviendo a Clipper.
- **Motores de dominio:** el editor espectral y sus recetas, los decks de curado y el compositor de Clipper.
- **Sonidos de interfaz** como motor. Se comparten como datos, con el paquete y la referencia medida de [sonido v1 §4](../../contracts/nu9ve-sound-v1.md#4-cómo-llegan-los-sonidos-a-cada-app).

## 4. Reglas para las capas

- Sin dependencias de UI ni de procesos. Los stems son la excepción, y solo como adaptador de procesos externos.
- Funciones deterministas con vectores de prueba en `data/`, como los de tema: un archivo de entrada y su LUFS, BPM o clave esperados, comprobado por cada app que lo use.
- Licencias aptas para un binario cerrado, porque SoundBender puede venderse: `cargo-deny` en el gate. symphonia es MPL-2.0, que el `deny.toml` de `rust/` no admite: está por decidir (§5).
- Las mismas prácticas de Rust que [motor en Rust §6](https://github.com/nu9ve/n-creator-utils/blob/master/docs/platforms/rust-engine.md#6-qué-compartir-en-rust-entre-las-apps): `edition = "2024"`, `clippy -D warnings`, dependencias fijadas y anotadas.

## 5. curado y SoundBender, pieza por pieza

Medido el 2026-11-16, con el código de curado (`crates/audio-engine`, `2fcc0c0`) y el motor en Rust de SoundBender (`engine/soundbender-core`, fase 1 de su [decisión 036](../../../soundshop/docs/decisions/036-rust-engine-benchmark.md)). Es lo único que hoy existe dos veces en Rust.

| Pieza | curado | SoundBender | ¿Mismo algoritmo? | Qué va al core |
| --- | --- | --- | --- | --- |
| LUFS integrado | `loudness.rs`: ponderación K diseñada con biquads RBJ, cualquier frecuencia de muestreo, sin corto plazo | `loudness.rs`: coeficientes de BS.1770 a 48 kHz, integrado y máximo de corto plazo | **No, y no coinciden** (ver abajo) | `nu9ve-loudness::lufs`, partiendo del de SoundBender |
| True peak | Detector 4× con sinc de 12 taps en `limiter.rs` | No tiene (está en su hoja de ruta) | — | `nu9ve-loudness::true_peak`, partiendo del de curado |
| Limitador | En tiempo real, enlazado, true peak, 1,5 ms de anticipación, sin asignaciones | Dos fuera de línea: el del master (pico de muestra, 5 ms) y la protección de salida (64 muestras) | No: sirven a cosas distintas | Ninguno por ahora. El de curado es candidato para el plugin de SoundBender; los de SoundBender quedan fijados por sus dorados |
| Filtros | `Biquad`, `BiquadCoef` (RBJ) y `Svf` en `dsp.rs` | Filtros de los efectos de pista, en Swift | Sin comparar | `nu9ve-dsp`, cuando SoundBender porte sus efectos |
| Sin asignaciones en tiempo real | `allocation_tests.rs`: asignador que cuenta y `malloc_logger` en macOS, que también ve C++ | Lo necesita su plugin (fase 3) | — | `nu9ve-rt`, con el plugin |
| Decodificar | `decode.rs` con symphonia | AVFoundation, en Swift | — | `nu9ve-audio-io`. Antes hay que resolver la licencia (ver abajo) |
| Remuestrear y estirar | `resample.rs` (lectura fraccionaria para reproducir) y `pitch.rs` (signalsmith-stretch, MIT) | `TempoStretch` propio, en Swift | No | Nada todavía |
| BPM y clave | `beat.rs`, `music.rs` | `Tempo.swift`, `ChromaAnalysis.swift` | Sin comparar | `nu9ve-music`, comparando las dos con los mismos vectores antes de elegir |
| Salida a dispositivo | `output.rs` con cpal | AVAudioEngine | — | `nu9ve-rt` si la Windows de SoundBender la necesita; el plugin no |

### LUFS: curado lee bajo

Con el mismo audio, los 23 dorados de SoundBender (`Tests/Goldens`), curado da **entre 0,04 y 0,29 LU menos** que SoundBender. La tolerancia entre motores es de 0,01. La causa está en el filtro de estante: curado lo diseña con la fórmula RBJ a partir de 1681,97 Hz, 3,9998 dB y Q 0,7072, y eso no reproduce el filtro de la norma.

| Frecuencia | Respuesta de BS.1770 a 48 kHz | La de curado | Diferencia |
| --- | --- | --- | --- |
| 110 Hz | −0,940 dB | −0,983 dB | −0,04 |
| 1 kHz | +0,698 dB | +0,438 dB | **−0,26** |
| 1,76 kHz | +2,650 dB | +2,175 dB | **−0,48** |
| 10 kHz | +4,042 dB | +3,998 dB | −0,04 |

- **Consecuencia:** la señal de prueba 1 de EBU Tech 3341 (1 kHz a −23 dBFS en estéreo) debería leer −23,0 LUFS. Con SoundBender da −23,0; con curado, unos −23,26. Queda fuera de la tolerancia de ±0,1 de la norma.
- **Por qué no se notaba:** su prueba acepta ±0,3 LU.
- **Qué cambia en curado:** usa LUFS para igualar el nivel entre pistas, así que el error se cancela en parte. Pero depende del espectro, y dos temas con distinto brillo quedan desiguales en hasta 0,4 dB.

**Propuesta para `nu9ve-loudness`:**
1. Partir de `loudness.rs` de SoundBender.
2. Para otras frecuencias de muestreo, calcular los coeficientes con la derivación de libebur128. A 48 kHz debe reproducir los de la norma.
3. Vectores en `data/audio/`:
   - los 23 dorados de SoundBender con su LUFS integrado y de corto plazo;
   - los casos de EBU Tech 3341 y 3342 que se puedan generar.
4. Sumar el true peak de curado.
5. Cuando exista el crate, curado lo adopta: sus niveles se mueven entre 0,04 y 0,3 LU, y conviene decirlo en su CHANGELOG.

**Hecho el 2026-11-17, pasos 1 a 4:** [`rust/nu9ve-loudness`](../../rust/nu9ve-loudness/). El integrado da los mismos bits que el de SoundBender en los 23 dorados. Pasan los casos generables de Tech 3341 y 3342 a 44,1, 48 y 96 kHz; el true peak tiene el margen más justo, −0,19 dB frente a −0,4. Lo que se sumó al plan: el rango de 3342 (lo piden sus señales) y el máximo momentáneo. Los máximos se miden cada 10 ms. Falta el paso 5, que es de curado ([adopción](../adoption.md#nu9ve-loudness)).

### Licencia de la decodificación

- La §4 dice que symphonia (MPL-2.0) es compatible, pero el `deny.toml` de `rust/` no admite MPL.
- SoundBender puede venderse cerrado y curado depende de symphonia.
- Hay que decidir una de dos cosas:
  - admitir MPL-2.0 solo para decodificar, porque es copyleft por archivo y permite un binario cerrado si no se modifican esos archivos;
  - o dejar la decodificación fuera del core.
- La política general está en [licencias §2](../licensing.md#2-qué-pide-cada-licencia-de-terceros): MPL-2.0 solo como excepción por crate.

## 6. Preguntas para el usuario

1. ~~El nombre del repo común~~: **`nu9ve/core`** (2026-11-16, [suite-sharing §8](../suite-sharing.md#8-lo-que-decide-el-usuario)). Las capas van en su `rust/` y cada app las copia en `vendor/nu9ve-core/`.
2. Si curado acepta pasar su `audio-engine` a consumir estas capas cuando se muden, o si las capas nacen copiando de curado y curado sincroniza después.
3. Si Clipper quiere su normalización y su análisis de intros en Rust compartido o se quedan en Python.
4. Si `nu9ve-loudness` nace del LUFS de SoundBender y curado acepta que sus niveles cambien (§5).
5. Si se admite MPL-2.0 para la decodificación (§5).
6. Si la entrega de SoundBender se mide en dBTP (por ejemplo, techo de −1 dBTP con el true peak del crate) y qué pasa con su protección actual de −0,3 dBFS de muestra. Es decisión de producto.
7. Si el `deny.toml` de `rust/` lleva `allow-wildcard-paths = true` para los crates que las apps usan por ruta. Sin eso, cada app repite `version = "=x.y.z"` a mano: SoundBender lo hizo con `nu9ve-loudness`.
8. Si una app que fija `nu9ve-loudness` debe fijar también `data/audio` para correr los vectores, o le basta con sus propios dorados. SoundBender hoy no los fija.
