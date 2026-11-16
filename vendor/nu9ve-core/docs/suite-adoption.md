# Qué se extrae a `nu9ve/core` y cómo lo adopta cada app

Estado: **plan del 2026-11-16**. Hecho el mismo día: `NuMacShell` vive en `swift/` y Fractos lo usa por su copia fijada (ola 1, primera fila). Sale de leer los repos ese día y aplica las decisiones del mismo día: `NuMacShell` y el motor de sonido salen de Fractos, y los documentos siguen en español ([cómo se comparte §8](suite-sharing.md#8-lo-que-decide-el-usuario)). El inventario de nüün archivo por archivo sigue en [qué se muda](suite-split.md); el mapa de lo que se repite, en [la suite](suite.md#lo-que-ya-se-repite-y-podría-compartirse). Este documento ordena las piezas en olas y dice qué hace cada app para empezar a compartirlas.

## 1. Qué cambió para el orden

- **nüün ya usa Rust en la Mac.** La instalación local `e70e525ae792` usa el motor de Rust para el ingreso y las demás operaciones portadas. El ensayo físico con una Sony pasó el 2026-11-13. Falta la semana de uso diario ([PLAN de nüün](https://github.com/nu9ve/n-creator-utils/blob/master/PLAN.md#motor-en-rust--ruta)). Por decisión del usuario, `nu9ve-json` ya se mudó y se consume desde la copia fijada. La semana sigue siendo la condición para migrar la ventana a `NuMacShell` en un cambio aparte.
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
| `Nu9veTheme`: roles, `readable`, presets, skins | `swift/` | nüün `ThemeColor.swift`; kyu `ThemeRoles` y `SkinChrome`; SoundBender `Appearance.swift` | nüün ya comprueba los 110 vectores en Swift; falta comparar kyu con `data/theme-vectors-v1.json` |
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
| **nüün** (`n-creator-utils`) | Hecha: `contracts`, `data`, `docs`, `rust/nu9ve-json` | Ya usa `nu9ve-json` fijado; `NuMacShell` después de la semana de uso diario; luego `Nu9veTheme` | `nu9ve-json`, `nu9ve-volumes`, `Nu9ve.Core`/`Nu9ve.WinUI`, sus vectores de tema | Origen de casi todo. Su gate corre `sync-core.sh --check` desde el 2026-11-16 |
| **Fractos** (`ios-fractos`) | Hecha: `swift` y los contratos de Mac, tema y sonido | Ya consume `NuMacShell` por su copia y borró la suya | `NuMacShell`, `Nu9veBuildID`, el motor de sonido | `sync-contracts.sh` sigue para el contrato de la web; `sync-core.sh` para la suite |
| **nove** | `contracts/{macos-app,windows-app,nu9ve-theme,nu9ve-interaction,nu9ve-mcp,nu9ve-notifications}-v1.md`, `data/theme-vectors-v1.json` | Mac: `NuMacShell`. Windows: `Nu9ve.Core`/`Nu9ve.WinUI` cuando haya Windows | IPC enmarcado con ACL y MCP con niveles (ola 3) | Sus `docs/native-platforms.md` y `suite-alignment-windows.md` ya enlazan aquí |
| **curado** | Hecha: contratos de tema, macOS, Windows e interacción, y vectores de tema | `nu9ve-volumes` cuando se decida la identidad portable | Capas de audio y supervisión de procesos ([§5](#5-curado-audio-workers-y-otras-coincidencias)) | TypeScript verifica `accent` y `onAccent` en los 110 casos y usa sus bases en la matriz propia |
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

## 5. curado: audio, workers y otras coincidencias

Inventario del 2026-11-16 sobre `63f0ddd`. Los tamaños cuentan líneas del archivo completo, incluidas sus pruebas, salvo donde se indica la función. El código sigue en su repo. [Audio en la suite](platforms/audio.md) describe las capas propuestas.

| Pieza | Archivos y tamaño en curado | Quién más lo tiene y qué conviene comparar |
| --- | --- | --- |
| LUFS y limitador | `crates/audio-engine/src/loudness.rs` (92), `limiter.rs` (266) | SoundBender `Sources/SoundCore/MasterLimiter.swift` mide BS.1770 y limita; Clipper `src/clipper_tui/master_audio.py` mide con FFmpeg. Para `nu9ve-loudness`, comparar umbrales, picos entre muestras y latencia con las mismas señales |
| Entrada y remuestreo | `crates/audio-engine/src/decode.rs` (680), `resample.rs` (191) | SoundBender `Sources/SoundShop/AudioIO.swift` usa AVFoundation; Clipper decodifica con FFmpeg en `src/clipper_tui/audio_features.py`. Para `nu9ve-audio-io`, separar PCM, metadatos y remuestreo de los límites de memoria, cancelación, forma de onda y construcción del clip. Las posiciones existentes conservan el retardo del codificador |
| Pitch y estiramiento | `crates/audio-engine/src/pitch.rs` (426) | SoundBender `Sources/SoundCore/SampleRecipe.swift` aplica `RegionTransform` fuera de tiempo real. En curado, separar el procesamiento Signalsmith del transporte y de las voces; comprobar asignaciones, anticipación y saltos antes de compartir |
| Análisis musical y sonda | `crates/audio-engine/src/music.rs` (337), `crates/audio-engine/examples/music_probe.rs` (49) | SoundBender `Tempo.swift` y `ChromaAnalysis.swift` en `Sources/SoundCore`; Clipper tiene análisis offline de intros. La sonda genera un acorde y pulsos de 120 BPM y mide análisis y render de dos decks con key lock. Sirve para diseñar casos de `nu9ve-music`, con tiempos de diagnóstico específicos de la máquina |
| Demucs y supervisión | `src-tauri/src/stems.rs` (1 284), `src-tauri/native/separate.py` (190); otro supervisor de cámara en `src-tauri/src/tracking.rs` (439) | Clipper `core/src/rpc_jobs/supervise.rs` y sus módulos ([§4](#4-clipper-qué-de-su-rust-sirve-a-la-suite)); SoundBender `Sources/SoundShop/Resources/separate.py`; la guía de audio registra también nüün. Para `nu9ve-jobs`, comparar spawn, eventos NDJSON, cancelación, espera y vigilancia del padre (stdin en Unix, handle de proceso en Windows). El modelo, manifiesto, cuotas y registro en SQLite pertenecen al adaptador de stems o a la app |
| Identidad y volúmenes | `src-tauri/src/identity.rs` (282), `src-tauri/src/relink.rs` (491) | nüün `nu9ve-volumes`. La búsqueda, los fingerprints y la identidad portable del volumen tienen límites distintos. Candidato de ola 2 |
| Escritura atómica | `src-tauri/src/rekordbox_xml.rs`, `write_atomic` (11) | Clipper `core/src/rpc_writes/fsio.rs`; SoundBender `Sources/SoundCore/ProjectStorage.swift` usa `.atomic`. La exportación XML usa un temporal con nombre fijo y rename; el bloqueo y la sincronización explícita son diferencias para `nu9ve-fs` |
| Tema y dither | `src/lib/theme.ts` (328), `src/lib/dither.ts` (37) | Fractos web `packages/styles` según [el mapa de la suite](suite.md#lo-que-ya-se-repite-y-podría-compartirse); kyu `KyuKit/ThemePalette.swift` y `Kyu/SkinChrome.swift`. Los vectores fijados dan una referencia para los roles de acento |
| Cromo de ventana | `src-tauri/tauri.macos.conf.json` (21), configuración de overlay y semáforo nativo | `swift/Sources/NuMacShell/WindowChrome.swift` y `ShellWindow.swift` en core; SoundBender `Sources/SoundShop/WindowChrome.swift`. En Tauri se comparte el contrato de comportamiento y la configuración equivalente |
| Atajos | `src/lib/keyboard.ts` (400), `src/lib/mixer-keys.ts` (206) | SoundBender `Sources/SoundShop/ShortcutsPanel.swift` tiene catálogo y badges. Comparar detección de conflictos, etiquetas por plataforma y ayuda contextual antes de extraer; los controles de mezcla quedan en la app |

La revisión no encontró una implementación propia de Acerca de ni un serializador JSON canónico. El manifiesto de stems usa `serde_json::to_vec_pretty`; su SHA-256 de archivos lee por bloques en `identity.rs`, mientras `nu9ve-json::sha256_hex` opera sobre bytes en memoria.

## 6. Orden sugerido

1. ~~Mudar `NuMacShell` a `swift/`, con Fractos como primera copia fijada.~~ Hecho el 2026-11-16.
2. ~~Escribir `nu9ve-json-v1` con sus vectores, mudar `nu9ve-json` y crear el workspace de `rust/`.~~ Hecho el 2026-11-16.
3. ~~Hacer la primera copia fijada en nove, kyu, SoundBender y curado.~~ Hecho el 2026-11-16, en `e0e7683`. Además de lo de la tabla, kyu y curado fijan `macos-app-v1`, y SoundBender los vectores de tema. kyu no tiene remoto.
4. ~~Clipper adopta `nu9ve-json`~~ Hecho el 2026-11-16 ([§4](#4-clipper-qué-de-su-rust-sirve-a-la-suite)): primera copia fijada, y el sobre, el SHA-256 y los ids salen del crate. Sus 186 equivalencias doradas con Python siguen iguales.
5. Pruebas de tema en Swift contra los vectores: nüün hecho, kyu pendiente; después `Nu9veTheme`.
6. Diseñar `nu9ve-fs` (de `nu9ve-volumes/fs.rs` y `fsio.rs` de Clipper) y `nu9ve-py` (de `py.rs`, `pyval.rs`, `paths.rs` y `nu9ve-time`), cada uno con su contrato y sus vectores.
7. Escribir la guía de prácticas comunes.

## Inventario de solapamientos en nüün

Revisado el 2026-11-16. Los tamaños son líneas del archivo de nüün, salvo los intervalos indicados. Los paralelos no implican la misma API ni una extracción aprobada.

| Pieza | Archivo en nüün y tamaño | Quién más la tiene |
| --- | --- | --- |
| Roles, contraste, presets y preferencias | `native/MediaMenu/Sources/MediaMenuCore/ThemeColor.swift` (284), `native/MediaMenu/Sources/MediaMenu/Appearance.swift` (351); el port de roles está en `native/windows/Nu9ve.Core/Theme/ThemeRoles.cs` (155) | kyu: `apps/macos/Sources/KyuKit/ThemePalette.swift`; SoundBender: `Sources/SoundShop/Appearance.swift`; curado: `src/lib/theme.ts`. Los primeros dos son los paralelos en Swift de `Nu9veTheme`. |
| Ventana, agarres de borde y controles de skin | `native/MediaMenu/Sources/MediaMenu/ShellWindow.swift` (243), `native/MediaMenu/Sources/MediaMenu/SkinChrome.swift` (411) | core: `swift/Sources/NuMacShell/{ShellWindow,WindowChrome}.swift`, ya consumidos por Fractos; kyu conserva `apps/macos/Sources/Kyu/SkinChrome.swift`. La ventana se migra después de la semana de uso diario. |
| Acerca de e identidad de build | `native/MediaMenu/Sources/MediaMenu/AboutView.swift` (50), `native/MediaMenu/Sources/MediaMenuCore/AppIdentity.swift` (74) | core: `swift/Sources/NuMacShell/{AboutView,AppIdentity}.swift`, usados por Fractos; SoundBender tiene su propio `Sources/SoundShop/AboutWindow.swift`. |
| Catálogo de atajos | `native/windows/Nu9ve.Core/Shell/Shortcuts.cs` (54) y `native/windows/Nuun.Core/Shell/ShortcutCatalog.cs` (36) | SoundBender: `Sources/SoundShop/AppCommands.swift` y `ShortcutsPanel.swift`. Se puede comparar la validación de acciones; cada catálogo conserva los atajos de su app. |
| JSON canónico, digest e id aleatorio | `media/common.py:52–61` (10) | core: `rust/nu9ve-json`; Clipper conserva su serialización Python en `core/src/rpc_jobs/pyval.rs`. La copia Python de nüün sigue como oráculo; el motor Rust ya usa el crate fijado. |
| Escritura atómica y sincronización de archivos | `engine/crates/nuun-media/src/tree.rs:251–286` (36), `engine/crates/nu9ve-volumes/src/fs.rs` (626; incluye otras operaciones) | Clipper: `core/src/rpc_writes/fsio.rs`. Comparten el patrón temporal, sincronización y publicación; las garantías de no reemplazar y el tipo de sincronización deben decidirse al diseñar `nu9ve-fs`. |
| Supervisión de subprocesos | `engine/crates/nuun-media/src/integrations.rs:390–480` (91), `native/MediaMenu/Sources/MediaMenuCore/Protocol.swift` (378; incluye el protocolo) | Clipper: `core/src/rpc_jobs/supervise.rs` y submódulos. Se repiten el manejo de tuberías, salida y terminación; el lector persistente y los trabajos de Clipper tienen ciclos de vida distintos. |
| Separación de stems | `manager/file.py:885–889` (5, `split_file`) | curado: `src-tauri/native/separate.py`; SoundBender: `Sources/SoundShop/Resources/separate.py`. Lo común es lanzar Demucs y manejar su resultado; nüün no implementa el DSP. |
