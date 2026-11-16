# Audio en la suite nu9ve

Estado: **propuesta** del 2026-11-16, pedida por el usuario al revisar SoundBender. Nada de esto está construido. Leído en el código de cada repo el 2026-11-16. Diseño del lado de SoundBender: `soundbender/docs/plans/portable-engine.md` y su decisión 034. [Mapa de la suite](../suite.md) · [Cómo se comparte](../suite-sharing.md) · [Motor en Rust](rust-engine.md) · [Sonido de tema v1](../contracts/nu9ve-sound-v1.md).

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
| `nu9ve-audio-io` | WAV de 16, 24 y 32 bits con dither, metadatos BPM y clave, decodificación, resample | curado, SoundBender, Clipper | Al existir el repo común |
| `nu9ve-dsp` | FFT y STFT por bloques con halos, ventanas, filtros, interpolación | SoundBender, curado | Cuando lo use una tercera app |
| `nu9ve-loudness` | LUFS integrado y de corto plazo, true peak, limitador | curado, SoundBender (Clipper normaliza con FFmpeg) | Al existir el repo común; los números deben coincidir |
| `nu9ve-music` | Beats y downbeats, BPM, clave, escalas, afinaciones, acordes por grados | curado, SoundBender, Clipper (Python) | Al existir el repo común |
| `nu9ve-rt` | Colas sin bloqueo, rampas, comprobación de no-asignación, salida a dispositivo | curado; el plugin de SoundBender | Con el plugin |
| `nu9ve-stems` | Adaptador de Demucs: proceso aislado y cancelable, manifiesto, publicación atómica | curado, SoundBender, nüün | Ya hay tres copias |
| `nu9ve-sound` | Motor de sonidos de interfaz ([S17](../audits/session-briefs-2026-11-10.md#s17--motor-de-sonido-en-rust)) | Fractos (Swift) | Según S17; no depende de las capas anteriores |

Lo que **no** se comparte todavía:

- **Síntesis musical.** Los patches de SoundBender, las voces de ClipperScore y las cues de Fractos sirven a propósitos distintos. Conviene esperar a ver si el motor de patches de SoundBender termina sirviendo a Clipper.
- **Motores de dominio:** el editor espectral y sus recetas, los decks de curado y el compositor de Clipper.
- **Sonidos de interfaz** como motor. Se comparten como datos, con el paquete y la referencia medida de [sonido v1 §4](../contracts/nu9ve-sound-v1.md#4-cómo-llegan-los-sonidos-a-cada-app).

## 4. Reglas para las capas

- Sin dependencias de UI ni de procesos. Los stems son la excepción, y solo como adaptador de procesos externos.
- Funciones deterministas con vectores de prueba en `data/`, como los de tema: un archivo de entrada y su LUFS, BPM o clave esperados, comprobado por cada app que lo use.
- Licencias aptas para un binario cerrado, porque SoundBender puede venderse: `cargo-deny` en el gate. symphonia es MPL-2.0 y es compatible.
- Las mismas prácticas de Rust que [motor en Rust §6](rust-engine.md#6-qué-compartir-en-rust-entre-las-apps): `edition = "2024"`, `clippy -D warnings`, dependencias fijadas y anotadas.

## 5. Preguntas para el usuario

1. El nombre del repo común ([suite-sharing §8](../suite-sharing.md#8-lo-que-decide-el-usuario)).
2. Si curado acepta pasar su `audio-engine` a consumir estas capas cuando se muden, o si las capas nacen copiando de curado y curado sincroniza después.
3. Si Clipper quiere su normalización y su análisis de intros en Rust compartido o se quedan en Python.
