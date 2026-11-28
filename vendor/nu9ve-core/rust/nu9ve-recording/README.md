# nu9ve-recording

El lector de la [señal de grabación](../../contracts/nu9ve-recording-v1.md): si esta máquina está grabando, qué sesión y desde qué instante cuenta su reloj. Lo usa el agente (`nu9ve-mcp`, `surface_v1`) para el modo de grabación y las zonas `recording`. Tiene las mismas reglas que `Nu9veRecording` en Swift.

- `problem(file_name, bytes)` dice por qué un archivo de estado no vale, con los códigos de los ejemplos (`too_large`, `bom`, `missing_seq`, `bad_epoch`…), o `None`. `parse` da el `Record`.
- `status` suma la vida del proceso ([registro §4](../../contracts/nu9ve-registry-v1.md#4-vida-pid-con-hora-de-inicio)): `Live`, `Stale` o `Invalid`. Los dos últimos cuentan como `idle`.
- `derive` es la tabla de la §4 (`capturing`, `quiet`, `sessions`), y `read_dir` la lee de una carpeta real con tope por archivo. `Derived::recording_apps` da las apps que graban.
- `is_newer` dice si se actúa sobre una notificación (§4), y `session_time` da `t` en µs desde el epoch (§5).
- `directory_for` y `directory` resuelven la carpeta de la §1, con `NU9VE_RECORDING_DIR` y la raíz de QA, en macOS y en Windows.

Producir la señal queda en cada productor; en Swift, `RecordingSignalPublisher` de `Nu9veRecording`.

## Fijar y verificar

```text
rust/nu9ve-recording
rust/nu9ve-ipc
rust/nu9ve-json
```

Desde `rust/`: `cargo test --locked -p nu9ve-recording`, que pasa `data/recording-examples-v1.json` caso por caso, y `cargo clippy --locked -p nu9ve-recording --all-targets -- -D warnings`.
