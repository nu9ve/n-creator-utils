# Datos

Archivos que varias apps leen y prueban igual, para que el mismo cálculo dé el mismo número en Swift, C# y Rust.

| Archivo | Qué es | Quién lo prueba |
| --- | --- | --- |
| [theme-vectors-v1.json](theme-vectors-v1.json) | Hex esperado de cada rol de tema para cada color base ([tema v1 §10](../contracts/nu9ve-theme-v1.md#10-implementación-compartida)) | C# (`ThemeVectorsTests` de nüün); Swift pendiente. Se regenera aquí, nunca en una copia fijada |
| [engine-examples-v1.json](engine-examples-v1.json) | Sobres válidos e inválidos, casos de petición y respuesta para un motor, ids, marcos, los dos sobres de nove y comparaciones de sombra ([motor v1 §11](../contracts/nu9ve-engine-v1.md#11-ejemplos)) | Ningún motor todavía; los casos de motor se corrieron a mano contra nüün y Clipper el 2026-11-16. Se regenera aquí con `python3 -I scripts/engine-examples.py` |
| [json-vectors-v1.json](json-vectors-v1.json) | Bytes canónicos y digest de textos JSON, entradas rechazadas, límites conocidos e ids UUID 5 ([JSON canónico v1 §10](../contracts/nu9ve-json-v1.md#10-vectores)) | Rust (`nu9ve-json/tests/vectors.rs` de nüün). Se regenera aquí con `python3 -I scripts/json-vectors.py` |

| [artifact-examples-v1.json](artifact-examples-v1.json) y [artifact-example-v1/](artifact-example-v1/) | Manifiestos válidos e inválidos, casos de consumo y un paquete real con datos, SVG, PNG y 24 cuadros ([artefactos v1 §10](../contracts/nu9ve-artifact-v1.md#10-ejemplos-y-validador)) | Rust (`nu9ve-artifact/tests/examples.rs`). Se regenera aquí con `python3 -I scripts/artifact-examples.py`; `--check` compara los PNG por sus píxeles |
| [bench-examples-v1.json](bench-examples-v1.json) y [bench-fixtures/atlas/](bench-fixtures/atlas/) | Corridas válidas e inválidas, distribuciones, registros con líneas rotas, comparaciones y la traducción de un fixture de Atlas, copia recortada de sus archivos reales ([benchmarks v1 §12](../contracts/nu9ve-bench-v1.md#12-ejemplos)). `python3 -I scripts/bench-examples.py --check` |
| [device-examples-v1.json](device-examples-v1.json) | Dónde está `device.json`, archivos válidos e inválidos, `host_check` y los estados de un lector ([dispositivo v1 §7](../contracts/nu9ve-device-v1.md#7-ejemplos)) | Ninguna app todavía. Se regenera aquí con `python3 -I scripts/device-examples.py` |
| [volume-examples-v1.json](volume-examples-v1.json) | Marcadores válidos e inválidos, quién registra con qué rol, estados al conectar un disco, carpetas de app e índices ([volumen v1 §10](../contracts/nu9ve-volume-v1.md#10-ejemplos)) | Ninguna app todavía. Se regenera aquí con `python3 -I scripts/volume-examples.py`, que valida los índices con la referencia de `data-examples.py` |
| [audio/loudness-vectors-v1.json](audio/loudness-vectors-v1.json) y [audio/loudness-goldens/](audio/loudness-goldens/) | LUFS integrado de los 23 dorados de SoundBender (`Tests/Goldens`, PCM float32 copiado con su sha256: no se regeneran fuera de su motor), y las señales generables de EBU Tech 3341 (casos 1–6, 9, 10, 12, 13, 15–23) y 3342 (1–4) como recetas, con lo esperado y la tolerancia de la norma | Rust (`nu9ve-loudness/tests/vectors.rs`), a 44,1, 48 y 96 kHz. Se regenera aquí con `python3 -I scripts/loudness-vectors.py`; los dorados se vuelven a importar con `--goldens <Tests/Goldens>` |

Lo que entra después:

| Archivo | De dónde | Cuándo |
| --- | --- | --- |
| `sound/`: referencia medida y paquete de sonidos de interfaz | Fractos ([sonido v1](../contracts/nu9ve-sound-v1.md)) | Cuando el usuario decida si el motor de sonido sale de Fractos |
| `audio/`: BPM o clave esperados | curado y SoundBender ([audio en la suite](../docs/platforms/audio.md)) | Con `nu9ve-music` |
