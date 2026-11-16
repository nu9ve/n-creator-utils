# Datos

Archivos que varias apps leen y prueban igual, para que el mismo cálculo dé el mismo número en Swift, C# y Rust.

| Archivo | Qué es | Quién lo prueba |
| --- | --- | --- |
| [theme-vectors-v1.json](theme-vectors-v1.json) | Hex esperado de cada rol de tema para cada color base ([tema v1 §10](../contracts/nu9ve-theme-v1.md#10-implementación-compartida)) | C# (`ThemeVectorsTests` de nüün); Swift pendiente. Se regenera aquí, nunca en una copia fijada |
| [json-vectors-v1.json](json-vectors-v1.json) | Bytes canónicos y digest de textos JSON, entradas rechazadas, límites conocidos e ids UUID 5 ([JSON canónico v1 §10](../contracts/nu9ve-json-v1.md#10-vectores)) | Rust (`nu9ve-json/tests/vectors.rs` de nüün). Se regenera aquí con `python3 -I scripts/json-vectors.py` |

Lo que entra después:

| Archivo | De dónde | Cuándo |
| --- | --- | --- |
| `sound/`: referencia medida y paquete de sonidos de interfaz | Fractos ([sonido v1](../contracts/nu9ve-sound-v1.md)) | Cuando el usuario decida si el motor de sonido sale de Fractos |
| `audio/`: un archivo de entrada y su LUFS, BPM o clave esperados | curado y SoundBender ([audio en la suite](../docs/platforms/audio.md)) | Con la primera capa de audio |
