# Qué se extrae a `nu9ve/core` y cómo lo adopta cada app

Estado: **plan del 2026-11-16**. Hecho el mismo día: `NuMacShell` vive en `swift/` y Fractos lo usa por su copia fijada (ola 1, primera fila). Sale de leer los repos ese día y aplica las decisiones del mismo día: `NuMacShell` y el motor de sonido salen de Fractos, y los documentos siguen en español ([cómo se comparte §8](suite-sharing.md#8-lo-que-decide-el-usuario)). El inventario de nüün archivo por archivo sigue en [qué se muda](suite-split.md); el mapa de lo que se repite, en [la suite](suite.md#lo-que-ya-se-repite-y-podría-compartirse). Este documento ordena las piezas en olas y dice qué hace cada app para empezar a compartirlas.

## 1. Qué cambió para el orden

- **nüün ya usa Rust en la Mac.** La instalación local `e70e525ae792` usa el motor de Rust para el ingreso y las demás operaciones portadas. El ensayo físico con una Sony pasó el 2026-11-13. Falta la semana de uso diario ([PLAN de nüün](https://github.com/nu9ve/n-creator-utils/blob/master/PLAN.md#motor-en-rust--ruta)). Con eso se cumple la condición para mudar `nu9ve-json`. Conviene hacerlo al cerrar esa semana, para no mezclar un arreglo del motor con un cambio de ruta.
- **`NuMacShell` ya salió** (2026-11-16). Son 965 líneas en cinco archivos (`ShellWindow`, `WindowChrome`, `AboutView`, `AppIdentity`, `HapticPlayer`), con pruebas propias. Su regla de frontera es ahora `scripts/check-boundary.sh` de aquí. Pide macOS 15 ([por qué](../swift/README.md#macos-mínimo-15)).
- **El repo tiene remoto** (`nu9ve/core`, privado), así que la copia fijada ya se puede comparar en cualquier máquina con un clon.

## 2. Olas

Cada fila es un cambio propio en `nu9ve/core`, con su prueba y su línea en el CHANGELOG. La app de origen deja su primera copia fijada en el mismo paso ([AGENTS](../AGENTS.md)).

### Ola 1 · ahora, en la Mac

| Pieza | Destino | De dónde | Antes de mudarla | Primeros consumidores |
| --- | --- | --- | --- | --- |
| ~~`NuMacShell` (ventana, cromo, Acerca de, `AppIdentity` con `Nu9veBuildID`, háptica)~~ **Hecho el 2026-11-16** | `swift/` (`Package.swift` propio, macOS 15) | `ios-fractos/Packages/FractosKit/Sources/NuMacShell` y sus pruebas | Hecho: la regla de `verify-packages.sh` es `scripts/check-boundary.sh`, en el CI con `swift test`. macOS 15 por los cursores de borde; nüün (macOS 14) sube a 15 o el paquete baja con un `#available` | Fractos (copia fijada, ya sin la suya), después nüün |
| ~~Contrato del JSON canónico `nu9ve-json-v1`~~ **Hecho el 2026-11-16** | `contracts/` | Lo que hoy hace `nu9ve-json`, compatible byte a byte con Python | Vectores en `data/json-vectors-v1.json`: decimales, Unicode, claves, valores no finitos | nüün, Clipper (lo evalúa en su roadmap), nove (sobre común) |
| ~~`nu9ve-json` (sobre de errores, JSON canónico, digest, ids)~~ **Hecho el 2026-11-16**, a pedido del usuario antes de cerrar la semana de uso diario | `rust/` (nace el workspace con `Cargo.toml`, `cargo-deny` y clippy en el CI) | `n-creator-utils/engine/crates/nu9ve-json` (461 líneas; `serde`, `serde_json`, `sha2`) | El contrato de arriba, y la semana de uso diario de nüün cerrada | nüün |
| Gate de frontera | `scripts/` | `check_suite_boundary.sh` de nüün y `verify-packages.sh` de Fractos | Empezado: `scripts/check-boundary.sh` ya corre en el CI con la regla de Fractos y la lista de apps; falta comparar con la de nüün | Corre aquí, en el CI: nada en `rust/`, `swift/` o `windows/` nombra una app |

### Ola 2 · con dos consumidores a la vista

| Pieza | Destino | De dónde | Condición |
| --- | --- | --- | --- |
| `Nu9veTheme`: roles, `readable`, presets, skins | `swift/` | nüün `ThemeColor.swift`; kyu `ThemeRoles` y `SkinChrome`; SoundBender `Appearance.swift` | Primero, que las pruebas de nüün y kyu lean `data/theme-vectors-v1.json` y den lo mismo |
| `nu9ve-loudness` (LUFS, limitador) y vectores de audio | `rust/` y `data/audio/` | curado `audio-engine/loudness.rs`; SoundBender `MasterLimiter` | SoundBender decide si su motor portable es Rust ([audio en la suite](platforms/audio.md)) |
| `nu9ve-volumes` | `rust/` | nüün; curado `identity.rs`/`relink.rs` como segundo | Decidir antes la identidad portable de un disco ([A3](audits/fable-audit-2026-11-10.md#a3--la-identidad-de-un-disco-no-viaja-entre-equipos-ni-entre-sistemas)), porque cambia su API |
| Guía de prácticas comunes | `docs/practices.md` | Lo que ya repiten cinco apps: QA aislada por variable de entorno, instalar en la carpeta de apps del usuario con respaldo del anterior, gate con `sync-core.sh --check`, CHANGELOG por pieza | Escribirla con lo que ya hacen nüün, nove, curado, kyu y SoundBender, sin inventar reglas nuevas |

### Ola 3 · con su condición

| Pieza | Condición |
| --- | --- |
| `data/sound/` y el motor de sonido de Swift (`FractosSoundV2`) | Cerrar la pregunta de niveles del [contrato de sonido](../contracts/nu9ve-sound-v1.md) en la Mac con el repo de la web; después, cuando una segunda app de Mac lo adopte |
| `nu9ve-ipc`, `nu9ve-mcp` | Que nüün porte su MCP a Rust: entonces hay dos copias (nove `core/src/pipe.rs`, `mcp.rs`) |
| `Nu9ve.Core`, `Nu9ve.WinUI` y `sync-core.ps1` | Que vuelva a haber una máquina con Windows |
| `ShortcutCatalog` y ayuda contextual (SoundBender) | Una segunda app que implemente el [contrato de interacción](../contracts/nu9ve-interaction-v1.md) |
| Paleta y dither en TypeScript (curado, web de Fractos) | Decidir si `nu9ve/core` tendrá una carpeta de TypeScript; hoy no |

## 3. Cómo empieza cada app

El primer paso es igual en todas y no mueve código: declarar en `vendor/nu9ve-core.paths` los contratos y datos que la app ya sigue, fijarlos, correr `vendor/nu9ve-core/scripts/sync-core.sh --check` en su gate y apuntar sus enlaces a la copia. Así cada sesión en ese repo lee los contratos sin salir de él. El código llega después, pieza por pieza, en cambios propios.

| App | Primera copia fijada | Qué adopta después | Qué aporta a la suite | Nota |
| --- | --- | --- | --- | --- |
| **nüün** (`n-creator-utils`) | Hecha: `contracts`, `data`, `docs` | `rust/nu9ve-json` por su copia (ola 1); `NuMacShell` en lugar de su `ShellWindow`, `SkinChrome` y `AboutView`; `Nu9veTheme` | `nu9ve-json`, `nu9ve-volumes`, `Nu9ve.Core`/`Nu9ve.WinUI`, sus vectores de tema | Origen de casi todo. Su gate corre `sync-core.sh --check` desde el 2026-11-16 |
| **Fractos** (`ios-fractos`) | Hecha: `swift` y los contratos de Mac, tema y sonido | Ya consume `NuMacShell` por su copia y borró la suya | `NuMacShell`, `Nu9veBuildID`, el motor de sonido | `sync-contracts.sh` sigue para el contrato de la web; `sync-core.sh` para la suite |
| **nove** | `contracts/{macos-app,windows-app,nu9ve-theme,nu9ve-interaction,nu9ve-mcp,nu9ve-notifications}-v1.md`, `data/theme-vectors-v1.json` | Mac: `NuMacShell`. Windows: `Nu9ve.Core`/`Nu9ve.WinUI` cuando haya Windows | IPC enmarcado con ACL y MCP con niveles (ola 3) | Sus `docs/native-platforms.md` y `suite-alignment-windows.md` ya enlazan aquí |
| **curado** | `contracts/{nu9ve-theme,windows-app}-v1.md`, `data/theme-vectors-v1.json` | `nu9ve-volumes` en lugar de su `identity.rs` | Capas de audio (`nu9ve-loudness`, `nu9ve-audio-io`, `nu9ve-music`, `nu9ve-stems`) | Documenta en inglés; los contratos que fija siguen en español |
| **SoundBender** (`soundshop`) | `contracts/{macos-app,nu9ve-theme,nu9ve-interaction,nu9ve-sound}-v1.md` | `NuMacShell`, `Nu9veTheme` | `ShortcutCatalog`; segundo usuario de loudness y limitador | El código aún dice SoundShop |
| **kyu** | `contracts/{nu9ve-theme,nu9ve-interaction}-v1.md`, `data/theme-vectors-v1.json` | `NuMacShell`, `Nu9veTheme` | Sus pruebas de roles, como segunda verificación de los vectores en Swift; la regla de `slug()` | Sin remoto todavía |
| **neuf** | `contracts/{macos-app,nu9ve-theme}-v1.md` | `NuMacShell` (ventana y doble clic) | — | Remoto `nu9ve/neuf` |
| **Stalker** | `contracts/macos-app-v1.md` | `NuMacShell` al tocar su interfaz | — | Último cambio el 2026-10-29; baja prioridad |
| **Clipper** | `contracts/nu9ve-json-v1.md`, `contracts/nu9ve-mcp-v1.md`, `data/json-vectors-v1.json`, `rust/nu9ve-json` | `nu9ve-json` en lugar de su sobre, su SHA-256 a mano y sus ids ([§4](#4-clipper-qué-de-su-rust-sirve-a-la-suite)) | Semántica de Python (`nu9ve-py`), archivos atómicos y bloqueo (`nu9ve-fs`), supervisor de procesos (`nu9ve-jobs`) | Motor de Rust de unas 39 000 líneas que porta desde Python, como nüün |

## 4. Clipper: qué de su Rust sirve a la suite

Leído el 2026-11-16 en `clipper` (`main`, `0dd5d81`). `clipper-core` es un solo crate de unas 39 000 líneas, `edition = "2024"`, `rust-version = "1.89"`, cuya única dependencia es `serde_json` (con `arbitrary_precision` y `preserve_order`). Igual que nüün, porta a Rust un motor de Python método por método, con la respuesta byte a byte como criterio y `not_ported` para lo que falta. Ese camino común es la razón de casi todo el solapamiento.

| Pieza de Clipper | Qué hace | Ya existe en la suite | Qué haría falta |
| --- | --- | --- | --- |
| Sobre de `main.rs` (`{id, ok, data}` / `{id, ok: false, error: {code, message}}`) | Respuestas del protocolo `clipper-engine/1` | Es el mismo sobre de [`nu9ve-json-v1` §9](../contracts/nu9ve-json-v1.md) (`ok_envelope`, `error_envelope`) | Adoptar `nu9ve-json` para el sobre. Ningún cambio en el protocolo |
| `py::sha256_hex` (SHA-256 escrito a mano), `fsio::random_bytes`, `fsio::uuid4` | Digest de esquemas y reglas, ids | `nu9ve-json`: `digest`, `hex`, `random_bytes`, `uid` (sobre `sha2`) | Adoptar `nu9ve-json` y borrar las copias. Antes, aceptar en Clipper la dependencia `sha2` y comprobar los vectores |
| `py::float_repr`, `pyval::dumps` (los formatos de `json.dumps`, con indentación incluida), `round_to`, `casefold`, `strip`; `paths.rs` (`pathlib` de POSIX) | Reproducir CPython para que las respuestas coincidan con Python | `nu9ve-json::python_float_repr` y el JSON canónico; `nu9ve-time` en nüün para `datetime` | **Candidato nuevo, `nu9ve-py`**: «semántica de Python» para los dos motores que portan desde Python. El JSON canónico se queda en `nu9ve-json`. Lo de `json.dumps(indent=2)`, `round`, cadenas y `pathlib` va al crate nuevo, con vectores generados por CPython como `data/json-vectors-v1.json` |
| `rpc_writes/fsio.rs` (`atomic_write` con `mkstemp`, fsync y rename, más fsync del directorio; `locked_path` con `flock`) y otra `atomic_write` en `rpc_jobs/store.rs` | Escribir y bloquear igual que `storage.py` | `nu9ve-volumes/src/fs.rs` en nüün: `full_sync`, `sync_dir`, `create_new`, leer sin caché | **Candidato nuevo, `nu9ve-fs`**: separar de `nu9ve-volumes` la parte de archivos (lo que no es identidad de disco) y sumar escritura atómica y bloqueo. Clipper sería el segundo usuario y curado el tercero. Ojo: Clipper usa `fsync(2)` simple a propósito, para igualar a Python, mientras `File::sync_all` en macOS hace `F_FULLFSYNC`. Hay que decidirlo en el contrato |
| `rpc_jobs/supervise.rs` (≈2 500 líneas con `publication` y `sequence`) | Supervisa un proceso: grupo propio, eventos NDJSON, cancelar por señal, archivo o cola, timeout y bloqueo con SIGTERM y luego SIGKILL, y publicación atómica de lo que produjo | Nada todavía. Lo mismo, en otra forma, en los stems de curado (Demucs aislado), en `separate.py` de SoundBender y en las transferencias de nüün | **Candidato de la ola 3, `nu9ve-jobs`**: el supervisor genérico sin el manifiesto `clipper-job/1`. Entra cuando un segundo motor de Rust (curado o nüün) lo necesite. Antes hay que separar en Clipper lo genérico de lo que conoce su cola |
| `rpc_jobs/progress.rs` (progreso de FFmpeg `-progress`, archivo `fracción\tetiqueta`, ETA) | Progreso de los workers | — | Se queda en Clipper por ahora. Es candidato a acompañar a `nu9ve-jobs` si nüün o curado leen el progreso de FFmpeg |
| `ClipperScore` (DSP en Swift, según `docs/2026-11-12-native-core-migration.md`) | Análisis de audio | Las capas de audio en propuesta (`nu9ve-loudness`, `nu9ve-dsp`) | Si Clipper lo porta a Rust, que use o alimente `nu9ve-loudness` y no un tercer limitador o LUFS |
| Timeline, cortes, subtítulos, framing, multicam, overlays, miniaturas, `program_clock`, cola de exportación | El dominio editorial | — | Se queda en Clipper |

**Regla para Clipper:** la primera adopción es `nu9ve-json`, porque ya duplica su sobre, su digest y sus ids. Como `nu9ve-json` exige `rust-version = "1.89"`, igual que Clipper, la adopción se reduce a fijar `rust/nu9ve-json` y `data/json-vectors-v1.json`, comprobar los vectores en su gate y borrar las copias. `nu9ve-py` y `nu9ve-fs` se diseñan con nüün y Clipper juntos, cada uno en un cambio propio aquí.

## 5. Orden sugerido

1. ~~Mudar `NuMacShell` a `swift/`, con Fractos como primera copia fijada.~~ Hecho el 2026-11-16.
2. ~~Escribir `nu9ve-json-v1` con sus vectores, mudar `nu9ve-json` y crear el workspace de `rust/`.~~ Hecho el 2026-11-16.
3. ~~Hacer la primera copia fijada en nove, kyu, SoundBender y curado.~~ Hecho el 2026-11-16, en `e0e7683`. Además de lo de la tabla, kyu y curado fijan `macos-app-v1`, y SoundBender los vectores de tema. kyu no tiene remoto.
4. ~~Clipper adopta `nu9ve-json`~~ Hecho el 2026-11-16 ([§4](#4-clipper-qué-de-su-rust-sirve-a-la-suite)): primera copia fijada, y el sobre, el SHA-256 y los ids salen del crate. Sus 186 equivalencias doradas con Python siguen iguales.
5. Pruebas de tema en Swift contra los vectores (nüün y kyu), y después `Nu9veTheme`.
6. Diseñar `nu9ve-fs` (de `nu9ve-volumes/fs.rs` y `fsio.rs` de Clipper) y `nu9ve-py` (de `py.rs`, `pyval.rs`, `paths.rs` y `nu9ve-time`), cada uno con su contrato y sus vectores.
7. Escribir la guía de prácticas comunes.
