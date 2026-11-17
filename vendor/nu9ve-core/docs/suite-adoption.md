# Qué se extrae a `nu9ve/core` y cómo lo adopta cada app

Plan vivo desde el 2026-11-16. Ordena en olas las piezas que se repiten entre apps, dice qué toma cada app y guarda el inventario de lo que cada una repite. Qué contrato cumple cada app está en [adopción por app](adoption.md); el mapa de apps, en [la suite](suite.md); la mudanza de nüün archivo por archivo, en [qué se muda](suite-split.md). Las reglas de siempre: cada pieza entra en un cambio propio aquí, con su prueba y su línea en el CHANGELOG, y la app de origen deja su primera copia fijada en el mismo paso ([AGENTS](../AGENTS.md)).

## Hecho

| Pieza | Fecha | Quién la usa |
| --- | --- | --- |
| Contratos, datos y documentos de la suite | 2026-11-16 | Todas las apps con copia fijada |
| `sync-core.sh` con `--check`, `--strict` y `--from` | 2026-11-16 | nüün, Fractos, nove, kyu, SoundBender, curado, Clipper |
| `NuMacShell` en `swift/` (macOS 15, [por qué](../swift/README.md#macos-mínimo-15)) | 2026-11-16 | Fractos. Lo siguen nüün, tras su semana de uso diario, y las demás apps de Mac |
| [`nu9ve-json-v1`](../contracts/nu9ve-json-v1.md), sus vectores y `rust/nu9ve-json` | 2026-11-16 | nüün (motor de Rust) y Clipper (sobre, `sha256_hex`, `uuid4`) |
| [`nu9ve-fs-v1`](../contracts/nu9ve-fs-v1.md) y `rust/nu9ve-fs` | 2026-11-16 | nüün (`nu9ve-volumes`) y Clipper (`fsio.rs` y `store.rs`); curado después |
| [`nu9ve-engine-v1`](../contracts/nu9ve-engine-v1.md) (propuesta), sus [ejemplos](../data/engine-examples-v1.json) y `scripts/engine-examples.py` | 2026-11-16 | Clipper y nüün pasan los 20 casos en sus dos motores. nove, pasos 1 a 3 en Mac; falta probarlo en Windows ([§12](../contracts/nu9ve-engine-v1.md#12-diferencias-por-app-y-camino-a-v1)) |
| [`nu9ve-engine`](../rust/nu9ve-engine/): servidor de líneas y marcos con lectura acotada, ids exactos y `engine_info` | 2026-11-16 | Clipper (`clipper-core serve`), app de origen, y nüün (`nuun-engine rpc-serve`) |
| `scripts/check-boundary.sh`: nada en `rust/`, `swift/` o `windows/` nombra una app | 2026-11-16 | CI de aquí. Falta compararlo con `check_suite_boundary.sh` de nüün |
| Vectores de tema probados en Swift (15 roles), C# (nüün) y TypeScript (curado, `accent` y `onAccent`) | 2026-11-16 | `Nu9veTheme`, nüün y Kyū |
| [`Nu9veTheme` en `swift/`](../swift/README.md): roles, `readable`, presets y skins | 2026-11-16 | Nüün (app de origen) y Kyū, ambos con mínimo macOS 15 |
| [`Nu9ve.Core` y `Nu9ve.WinUI` en `windows/`](../windows/README.md), sin la identidad de nüün y con sus pruebas | 2026-11-16 | nüün en Windows (app de origen). nove, después |

## Decisiones tomadas para lo que sigue

- **`nu9ve-fs`** (el usuario, el 2026-11-16):
  - La sincronización por omisión es `F_FULLFSYNC` en macOS (`File::sync_all`), porque nüün respalda media y ahí lo que importa es que los datos lleguen al disco.
  - El `fsync(2)` simple, el de Python, existe como variante explícita para los motores que comparan con Python (Clipper y las pruebas doradas de nüün).
  - Un fallo al sincronizar la carpeta **se propaga**, como en Python. Ignorarlo es justo lo que hoy hace diferir la cola de Rust de Clipper de la de Python.

- **`nu9ve-engine-v1`** (el usuario, el 2026-11-16, [§13](../contracts/nu9ve-engine-v1.md#13-decisiones-abiertas)):
  - Un imprevisto del motor es `internal_error`, no `operation_failed`.
  - En la sombra, `1` y `1.0` son respuestas distintas.
  - El registro de la sombra guarda rutas, códigos y tiempos, nunca valores.
  - En Windows, un motor se cancela primero de forma cooperativa (método o archivo, con plazo de gracia) y al final con `TerminateJobObject`. Sin `GenerateConsoleCtrlEvent`.
  - No hay progreso ni eventos dentro del canal. Un flujo de eventos es de `nu9ve-jobs`.
  - nove retira su sobre viejo en la primera versión en que todos sus clientes envían `id`.
  - El crate `nu9ve-engine` nace desde Clipper; nüün lo adopta después de cerrar sus brechas.

## Lo que sigue, por olas

### Ola 1 · ahora

| Pieza | Destino | De dónde | Primeros usuarios |
| --- | --- | --- | --- |
| Adoptar [`nu9ve-engine-v1`](../contracts/nu9ve-engine-v1.md): Clipper valida `contract` en su cliente; nüün corrige `id`, códigos y `reason` en Python y Rust a la vez; nove hace que sus clientes lean los dos sobres ([§12](../contracts/nu9ve-engine-v1.md#12-diferencias-por-app-y-camino-a-v1)) | Cada app, en su repo | El contrato y sus ejemplos, ya aquí | Clipper ya sirve los campos de v1; nüün sigue y nove lo adopta en cuatro pasos. Las decisiones 1 a 3 ya están tomadas: nüün cambia códigos y su sombra en el mismo paso |

### Ola 2 · con dos usuarios a la vista

| Pieza | Destino | De dónde | Condición |
| --- | --- | --- | --- |
| `nu9ve-loudness` (LUFS, limitador) y vectores de audio | `rust/` y `data/audio/` | curado `audio-engine/loudness.rs`; SoundBender `MasterLimiter` | SoundBender decide si su motor portable es Rust ([audio en la suite](platforms/audio.md)) |
| `nu9ve-volumes` | `rust/` | nüün; curado `identity.rs`/`relink.rs` como segundo | Decidir antes la identidad portable de un disco ([A3](audits/fable-audit-2026-11-10.md#a3--la-identidad-de-un-disco-no-viaja-entre-equipos-ni-entre-sistemas)), porque cambia su API |
| Guía de prácticas comunes | `docs/practices.md` | Lo que ya repiten cinco apps: QA aislada por variable de entorno, instalar en la carpeta de apps del usuario con respaldo del anterior, gate con `sync-core.sh --check`, CHANGELOG por pieza | Escribirla con lo que ya hacen nüün, nove, curado, kyu y SoundBender, sin inventar reglas nuevas |
| `nu9ve-py`: semántica de CPython (`repr`, `round`, `json.dumps` con indentación, cadenas, `pathlib`, `datetime`) | `rust/` | Clipper `py.rs`, `pyval.rs`, `paths.rs`; nüün `nu9ve-time` | Mientras nüün y Clipper porten desde Python. Vectores generados por CPython |

### Ola 3 · con su condición

| Pieza | Condición |
| --- | --- |
| `data/sound/` y el motor de sonido de Swift (`FractosSoundV2`) | Cerrar la pregunta de niveles del [contrato de sonido](../contracts/nu9ve-sound-v1.md) en la Mac con el repo de la web; después, cuando una segunda app de Mac lo adopte |
| `nu9ve-ipc`, `nu9ve-mcp` | Que nüün porte su MCP a Rust: entonces hay dos copias (nove `core/src/pipe.rs`, `mcp.rs`) |
| `sync-core.ps1` | Decidir si hace falta: el script de bash corre en Git Bash, que viene con Git para Windows |
| `ShortcutCatalog` y ayuda contextual (SoundBender) | Una segunda app que implemente el [contrato de interacción](../contracts/nu9ve-interaction-v1.md) |
| Paleta y dither en TypeScript (curado, web de Fractos) | Decidir si `nu9ve/core` tendrá una carpeta de TypeScript; hoy no |
| `nu9ve-jobs`: supervisar un proceso (grupo propio, NDJSON, cancelar, timeout, publicar) | Que un segundo motor de Rust lo necesite: curado (`stems.rs`) o nüün. Antes, separar en Clipper lo genérico de su cola ([inventario](#clipper)) |

<a id="3-cómo-empieza-cada-app"></a>
## Cada app

Cada app sigue lo suyo en **sesiones de su propio repo**: adoptar una pieza, sincronizar su copia, arreglar su gate. La sesión del core hace lo común: contratos, piezas compartidas, este plan y la adopción ([cómo se trabaja](../README.md#cómo-se-trabaja)). La última columna dice dónde retoma una sesión de esa app.

| App | Copia fijada | Siguiente paso de la suite en su repo | Qué aporta a la suite | Dónde retoma |
| --- | --- | --- | --- | --- |
| **nüün** (`n-creator-utils`) | `contracts`, `data`, `docs`, `rust/nu9ve-json`, `rust/nu9ve-fs`, `rust/nu9ve-engine`, `swift`, `windows`; `--check` en su gate | `NuMacShell` en lugar de su `ShellWindow`, `SkinChrome` y `AboutView`, después de la semana de uso diario. En Windows, su fase A | `nu9ve-json`, `nu9ve-volumes`, `nu9ve-time`, `Nu9ve.Core`/`Nu9ve.WinUI` | [STATUS](../../../nu9ve/n-creator-utils/docs/STATUS.md) y su PLAN |
| **Fractos** (`ios-fractos`) | `swift` y los contratos de Mac, tema y sonido | Arreglar las pruebas de su contrato de la web (`sync-contracts.sh` con `nu9ve.xyz`). Más adelante, que su motor de sonido se mude al core | `NuMacShell`, el motor de sonido | [MONOREPO](../../../nu9ve/ios-fractos/docs/MONOREPO.md) y [SUITE-DESKTOP](../../../nu9ve/ios-fractos/docs/SUITE-DESKTOP.md) |
| **Clipper** | Contratos de JSON, MCP, archivos y motor; vectores y ejemplos; `rust/nu9ve-json`, `nu9ve-fs`, `nu9ve-engine`; este plan; `--check` en su CI | Separar lo genérico de su supervisor de procesos, como base de `nu9ve-jobs` | Servidor de líneas, supervisor de procesos, semántica de Python | [Roadmap del motor](../../clipper/docs/2026-11-13-clipper-core-roadmap.md) |
| **curado** | Contratos de Mac, Windows, tema e interacción, y vectores de tema; `bun run core:check` | `nu9ve-fs` en su exportación; corregir el contraste de superficies elevadas y estados que encontró | Capas de audio y supervisión de stems | [STATUS](../../curado/docs/STATUS.md) |
| **nove** | Contratos de Mac, Windows, tema, interacción, MCP, avisos y motor; vectores y ejemplos | En Windows: C#, named pipe, la 0.12.0.53 y retirar el sobre viejo | IPC enmarcado con ACL y MCP con niveles | [Seguir en Windows](../../nove/docs/windows-next.md) |
| **SoundBender** (`soundshop`) | Contratos de Mac, tema, interacción y sonido, vectores de tema y `swift`; `--check` en `validate.sh` | Ventana cuadrada de época para los comunes; después, medidas y tipografía de cada skin | `ShortcutCatalog`; segundo usuario de loudness | [Ventana y skins de la suite](../../soundshop/docs/plans/suite-shell.md) |
| **kyu** | Contratos de Mac, tema e interacción, vectores de tema y `swift`; `--check` en `validate.sh` | `NuMacShell` en lugar de su ventana y cromo | Segunda verificación de los vectores en Swift; la regla de `slug()` | [STATUS](../../kyu/docs/STATUS.md) |
| **neuf** | Sin copia todavía | Primera copia fijada (`macos-app-v1`, `nu9ve-theme-v1`); después `NuMacShell` | — | [STATUS](../../neuf/docs/STATUS.md) |
| **Stalker** | Sin copia todavía | Primera copia fijada (`macos-app-v1`); `NuMacShell` al tocar su interfaz | — | [README](../../stalker/README.md) |

El primer paso de una app sin copia no mueve código: declarar en `vendor/nu9ve-core.paths` lo que ya sigue, fijarlo, poner `--check` en su gate y apuntar sus enlaces a la copia.

## Inventario por app

Lo que cada app repite de la suite o de otra app. Los paralelos no implican la misma API ni una extracción aprobada. El código sigue en su repo hasta que su pieza entre en una ola.

<a id="inventario-de-solapamientos-en-nüün"></a>
### nüün

Revisado el 2026-11-16. Los tamaños son líneas del archivo de nüün, salvo los intervalos indicados. Los paralelos no implican la misma API ni una extracción aprobada.

| Pieza | Archivo en nüün y tamaño | Quién más la tiene |
| --- | --- | --- |
| Roles, contraste, presets y preferencias | `MediaMenuCore/ThemeColor.swift` pasó a `swift/Sources/Nu9veTheme`; `MediaMenu/Appearance.swift` conserva el estado y las vistas. El port de C# pasó a `windows/Nu9ve.Core`; nüün declara su identidad en `Nuun.Core/Theme/NuunTheme.cs`. | Kyū comparte el cálculo de los roles y conserva sus superficies y colores de dominio. SoundBender: `Sources/SoundShop/Appearance.swift`; curado: `src/lib/theme.ts`. |
| Ventana, agarres de borde y controles de skin | `native/MediaMenu/Sources/MediaMenu/ShellWindow.swift` (243), `native/MediaMenu/Sources/MediaMenu/SkinChrome.swift` (411) | core: `swift/Sources/NuMacShell/{ShellWindow,WindowChrome}.swift`, ya consumidos por Fractos; kyu conserva `apps/macos/Sources/Kyu/SkinChrome.swift`. La ventana se migra después de la semana de uso diario. |
| Acerca de e identidad de build | `native/MediaMenu/Sources/MediaMenu/AboutView.swift` (50), `native/MediaMenu/Sources/MediaMenuCore/AppIdentity.swift` (74) | core: `swift/Sources/NuMacShell/{AboutView,AppIdentity}.swift`, usados por Fractos; SoundBender tiene su propio `Sources/SoundShop/AboutWindow.swift`. |
| Catálogo de atajos | `windows/Nu9ve.Core/Shell/Shortcuts.cs` (54, ya en el core) y `native/windows/Nuun.Core/Shell/ShortcutCatalog.cs` (36) | SoundBender: `Sources/SoundShop/AppCommands.swift` y `ShortcutsPanel.swift`. Se puede comparar la validación de acciones; cada catálogo conserva los atajos de su app. |
| JSON canónico, digest e id aleatorio | `media/common.py:52–61` (10) | core: `rust/nu9ve-json`; Clipper conserva su serialización Python en `core/src/rpc_jobs/pyval.rs`. La copia Python de nüün sigue como oráculo; el motor Rust ya usa el crate fijado. |
| Escritura atómica y sincronización de archivos | `engine/crates/nuun-media/src/tree.rs:251–286` (36), `engine/crates/nu9ve-volumes/src/fs.rs` (626; incluye otras operaciones) | Clipper: `core/src/rpc_writes/fsio.rs`. Las primitivas comunes viven en `nu9ve-fs`. El motor conserva las comprobaciones de rutas y usa `Full`. |
| Supervisión de subprocesos | `engine/crates/nuun-media/src/integrations.rs:390–480` (91), `native/MediaMenu/Sources/MediaMenuCore/Protocol.swift` (378; incluye el protocolo) | Clipper: `core/src/rpc_jobs/supervise.rs` y submódulos. Se repiten el manejo de tuberías, salida y terminación; el lector persistente y los trabajos de Clipper tienen ciclos de vida distintos. |
| Separación de stems | `manager/file.py:885–889` (5, `split_file`) | curado: `src-tauri/native/separate.py`; SoundBender: `Sources/SoundShop/Resources/separate.py`. Lo común es lanzar Demucs y manejar su resultado; nüün no implementa el DSP. |

<a id="4-clipper-qué-de-su-rust-sirve-a-la-suite"></a><a id="inventario-de-solapamientos-en-clipper"></a>
### Clipper

Leído el 2026-11-16 en `clipper` (`main`, `0dd5d81`). `clipper-core` es un solo crate de unas 39 000 líneas, `edition = "2024"`, `rust-version = "1.89"`, cuya única dependencia es `serde_json` (con `arbitrary_precision` y `preserve_order`). Igual que nüün, porta a Rust un motor de Python método por método, con la respuesta byte a byte como criterio y `not_ported` para lo que falta. Ese camino común es la razón de casi todo el solapamiento.

| Pieza de Clipper | Qué hace | Ya existe en la suite | Qué haría falta |
| --- | --- | --- | --- |
| Sobre de `main.rs` (`{id, ok, data}` / `{id, ok: false, error: {code, message}}`) | Respuestas del protocolo `clipper-engine/1` | Es el mismo sobre de [`nu9ve-json-v1` §9](../contracts/nu9ve-json-v1.md) (`ok_envelope`, `error_envelope`) | **Hecho el 2026-11-16** (`6312f727`), sin cambios en el protocolo |
| `py::sha256_hex` (SHA-256 escrito a mano), `fsio::random_bytes`, `fsio::uuid4` | Digest de esquemas y reglas, ids | `nu9ve-json`: `digest`, `hex`, `random_bytes`, `uid` (sobre `sha2`) | **Hecho el 2026-11-16** (`6312f727`): `sha256_hex` y `uuid4` salen del crate; la copia a mano se borró |
| `py::float_repr`, `pyval::dumps` (los formatos de `json.dumps`, con indentación incluida), `round_to`, `casefold`, `strip`; `paths.rs` (`pathlib` de POSIX) | Reproducir CPython para que las respuestas coincidan con Python | `nu9ve-json::python_float_repr` y el JSON canónico; `nu9ve-time` en nüün para `datetime` | **Candidato nuevo, `nu9ve-py`**: «semántica de Python» para los dos motores que portan desde Python. El JSON canónico se queda en `nu9ve-json`. Lo de `json.dumps(indent=2)`, `round`, cadenas y `pathlib` va al crate nuevo, con vectores generados por CPython como `data/json-vectors-v1.json` |
| `rpc_writes/fsio.rs` (`atomic_write` con `mkstemp`, fsync y rename, más fsync del directorio; `locked_path` con `flock`) y otra `atomic_write` en `rpc_jobs/store.rs` | Escribir y bloquear igual que `storage.py` | `nu9ve-volumes/src/fs.rs` en nüün: `full_sync`, `sync_dir`, `create_new`, leer sin caché | **Candidato nuevo, `nu9ve-fs`**: separar de `nu9ve-volumes` la parte de archivos (lo que no es identidad de disco) y sumar escritura atómica y bloqueo. Clipper sería el segundo usuario y curado el tercero. Ojo: Clipper usa `fsync(2)` simple a propósito, para igualar a Python, mientras `File::sync_all` en macOS hace `F_FULLFSYNC`. Hay que decidirlo en el contrato |
| `rpc_jobs/supervise.rs` (≈2 500 líneas con `publication` y `sequence`) | Supervisa un proceso: grupo propio, eventos NDJSON, cancelar por señal, archivo o cola, timeout y bloqueo con SIGTERM y luego SIGKILL, y publicación atómica de lo que produjo | Nada todavía. Lo mismo, en otra forma, en los stems de curado (Demucs aislado), en `separate.py` de SoundBender y en las transferencias de nüün | **Candidato de la ola 3, `nu9ve-jobs`**: el supervisor genérico sin el manifiesto `clipper-job/1`. Entra cuando un segundo motor de Rust (curado o nüün) lo necesite. Antes hay que separar en Clipper lo genérico de lo que conoce su cola |
| `rpc_jobs/progress.rs` (progreso de FFmpeg `-progress`, archivo `fracción\tetiqueta`, ETA) | Progreso de los workers | — | Se queda en Clipper por ahora. Es candidato a acompañar a `nu9ve-jobs` si nüün o curado leen el progreso de FFmpeg |
| `ClipperScore` (DSP en Swift, según `docs/2026-11-12-native-core-migration.md`) | Análisis de audio | Las capas de audio en propuesta (`nu9ve-loudness`, `nu9ve-dsp`) | Si Clipper lo porta a Rust, que use o alimente `nu9ve-loudness` y no un tercer limitador o LUFS |
| Timeline, cortes, subtítulos, framing, multicam, overlays, miniaturas, `program_clock`, cola de exportación | El dominio editorial | — | Se queda en Clipper |

**Para Clipper, después de `nu9ve-json`:** `nu9ve-fs` primero, porque ya tiene dos `atomic_write` que no se comportan igual. `nu9ve-py` se diseña con nüün. Su sobre ya es el de `nu9ve-json`, así que `nu9ve-engine-v1` describe lo que Clipper ya habla.

Revisado el 2026-11-16 sobre `4d3ad12`. Los tamaños son líneas del archivo de Clipper, salvo los intervalos indicados. Los paralelos no implican la misma API ni una extracción aprobada.

| Pieza | Archivo en Clipper y tamaño | Quién más la tiene |
| --- | --- | --- |
| Ventana, menús y Acerca de | `apps/ClipperStudio/ClipperStudio/ClipperStudioApp.swift` (38): `WindowGroup`, `Settings` y un `CommandMenu`, con el Acerca de del sistema | core: `swift/Sources/NuMacShell`, ya en Fractos; nüün `ShellWindow.swift` y `AboutView.swift`; SoundBender `AboutWindow.swift`. ClipperStudio no tiene cromo propio que borrar: adoptar `NuMacShell` es sumar, no reemplazar |
| Atajos y ayuda contextual | TUI: `src/clipper_tui/command_help.py` (500; paleta, hoja `?` y teclas fuera de los bindings) y `SHORTCUT_SPECS` en `app.py`; Mac: tres `keyboardShortcut` en `ClipperStudioApp.swift` | SoundBender `ShortcutsPanel.swift` y `AppCommands.swift`; curado `src/lib/keyboard.ts`; nüün `Shortcuts.cs`. El de Clipper es un catálogo único para paleta, ayuda y pruebas, como pide el [contrato de interacción](../contracts/nu9ve-interaction-v1.md), pero en Textual |
| Tema | TUI: `CSS_TEMPLATE` en `src/clipper_tui/app.py`; `theme_kits.py`, `theme_components.py`, `motion_theme.py` y `ThumbnailArtDirection.swift` (1 984) son estilos del video, no de la interfaz | Ningún paralelo directo: los kits de tema de Clipper son del dominio editorial. `Nu9veTheme` sólo tocaría a ClipperStudio si adopta skins |
| JSON canónico y digest | Rust: `core/src/rpc_jobs/pyval.rs` y `core/src/py.rs` (formatos de `json.dumps`); Python: 41 llamadas `sha256`/`sha1(json.dumps(...))` en 33 módulos de `src/clipper_tui/` con separadores, `ensure_ascii` y `default=str` distintos; sólo 7 usan la forma compacta con `ensure_ascii=False` | core: `rust/nu9ve-json` (Clipper ya toma de ahí el sobre, `sha256_hex` y `uuid4`); nüün `media/common.py:52–61`. Esos digests son claves de caché y firmas internas: cambiar su forma invalida cachés, así que no conviene unificarlos sin un motivo |
| Escritura atómica y bloqueo | Python: `src/clipper_tui/storage.py:63–123` (61, `locked_path`, `atomic_write_text`, `atomic_write_json`), `media_access.py` (75) y otros `fcntl.flock`/`os.replace` sueltos (`export_queue.py`, `engine_jobs_queue.py`, `creative_profiles.py`, `media_handoff.py`, `overlay_scenes.py`, `motion_render.py`); Rust: [la tabla de arriba](#nu9ve-fs-en-clipper-qué-es-genérico-en-fsiors-y-storers) | nüün `nu9ve-volumes/src/fs.rs` (626) y `nuun-media/src/tree.rs:251–286`; curado `rekordbox_xml.rs` (`write_atomic`, 11); SoundBender `ProjectStorage.swift` (`.atomic`) |
| Supervisión de procesos | Rust: `core/src/rpc_jobs/supervise.rs` (1 258), `supervise/publication.rs` (724), `supervise/sequence.rs` (525), `rpc_jobs/progress.rs` (472); Python: `src/clipper_tui/processes.py` (185; grupo propio, `killpg`, SIGTERM y luego SIGKILL, apagado de la app) | curado `src-tauri/src/stems.rs` (1 284) y `tracking.rs` (439); nüün `nuun-media/src/integrations.rs:390–480`; SoundBender `separate.py`. Candidato `nu9ve-jobs` de la ola 3 |
| Audio: loudness y limitador | `src/clipper_tui/master_audio.py` (400): −14 LUFS y −1 dBTP con FFmpeg (mide con `ebur128`, limita con `alimiter`), con caché de medidas; `audio_features.py` decodifica con FFmpeg; `core/src/rpc_score.rs` (705) | curado `audio-engine/src/loudness.rs` (92) y `limiter.rs` (266); SoundBender `MasterLimiter.swift`. Para `nu9ve-loudness`, Clipper es consumidor de las medidas, no implementación: mide con FFmpeg |
| Audio: síntesis | `native/clipper-macos-render/Sources/ClipperScore/` (2 491 en siete archivos: DSP, batería, efectos, voces, motor), `AudioCrossfade.swift`, `GeneratedIntroAudio.swift` | Fractos `FractosSoundV2` (sonido de interfaz, otro propósito); SoundBender `SoundCore`. La decisión de pasar el compositor a Rust está tomada en Clipper; cuando ocurra, comparar con `nu9ve-dsp` antes de escribir un tercer DSP |

<a id="nu9ve-fs-en-clipper-qué-es-genérico-en-fsiors-y-storers"></a>
#### `nu9ve-fs` en Clipper: qué es genérico en `fsio.rs` y `store.rs`

Leído el 2026-11-16 en `clipper` (`main`, `4d3ad12`). Inventario previo a la extracción. Los tamaños son líneas de la función o del intervalo, en `core/src/`. La [adopción](adoption.md#nu9ve-fs-v1) recoge las llamadas que ya migraron.

| Función | Dónde y tamaño | Genérica o de Clipper | Qué la ata a Clipper o a Python |
| --- | --- | --- | --- |
| `fsync_file` | `rpc_writes/fsio.rs:16–32` (17) | Genérica | `fsync(2)` simple por `extern "C"`, reintentando EINTR como CPython. Sirve también para directorios. Es la alternativa a `full_sync` (`F_FULLFSYNC`) de `nu9ve-volumes/fs.rs`: el contrato debe ofrecer las dos y decir cuál usa cada app |
| `atomic_write` | `rpc_writes/fsio.rs:138–189` (52) | Genérica | Temporal hermano `.<nombre>.XXXXXXXX.tmp` con el alfabeto de `tempfile`, modo 0600, `create_new`, hasta 100 intentos; fsync, rename y fsync del directorio, ignorando sólo un directorio que no abre. Los ocho caracteres salen de `nu9ve_json::random_bytes`. Igual a `storage.atomic_write_text` de Python |
| `lock_exclusive` y `Held` | `rpc_writes/fsio.rs:34–79` (46) | Genérica | `.<nombre>.lock` al lado, abierto como `"a+"` de Python, `flock` exclusivo con sondeo cada 10 ms hasta un plazo. El plazo y el nombre del candado serían parámetros |
| `media_shared` | `rpc_writes/fsio.rs:81–126` (46) | Mitad y mitad | El `flock` compartido es genérico. El token del dueño en JSON, las variables de entorno y el `NotPorted` son el protocolo de `media_access.py` de Clipper |
| `random_bytes`, `uuid4` | `rpc_writes/fsio.rs:128–137` (10) | Ya en la suite | Envoltorios de `nu9ve-json` |
| `atomic_write` y `temporary_token` | `rpc_jobs/store.rs:262–326` (65) | Genérica, segunda copia | **No hace lo mismo que la de `fsio.rs`**: usa `File::sync_all` (`F_FULLFSYNC` en macOS), saca el nombre de un xorshift sobre el reloj, el pid y un contador, ignora un fsync del directorio que falla y acepta un nombre sin carpeta (`.`). Python sí propaga ese fallo. La usan también `supervise.rs:627` y `supervise/sequence.rs:305`. Al adoptar `nu9ve-fs`, las dos se vuelven una |
| `lock_path` y `Guard` | `rpc_jobs/store.rs:82–118` (37) | Genérica, segunda copia | El mismo bucle que `lock_exclusive`. El plazo sale de constantes de Clipper (`queue.lock_wait_seconds`, su máximo y su variable de entorno) y los errores son `queue_busy` y `queue_unavailable` |
| `probe_runner`, `runner_held`, `is_running` | `rpc_jobs/store.rs:120–165` (46) | Genérica en la forma | «¿Alguien tiene este `flock`?», con y sin crear el archivo. El nombre `runner.lock` y el `Runner::Absent` son de la cola. `queue.rs:55–66` repite `is_running` |
| `Queue`, `Snapshot`, `read`, `write`, `check_shape`, `view`, `ledger`, `record` | `rpc_jobs/store.rs` (el resto, unas 230) | De Clipper | El formato de `queue.json`, su revisión por SHA-256 y el libro de la cola |

Otras primitivas del mismo tipo, escritas en línea, que convendría pasar por `nu9ve-fs` cuando exista: `place`, `exclusive_move` y `stage_copy` en `rpc_jobs/supervise.rs:913–990` (78; publicar sin reemplazar con `hard_link`, con copia si cruza de volumen y rename si el sistema no tiene enlaces, como exFAT), que equivalen a `rename_new` de `nu9ve-volumes/fs.rs`; `Journal::save` en `supervise/publication.rs:347–366` (temporal de nombre fijo bajo el candado del plan); el candado del plan en `supervise/sequence.rs:236–251` (`flock` sin espera, modo 0600); y `media_gate` en `rpc_jobs/queue_ops.rs:585–603`, otro `flock` compartido sobre el candado de medios sin el token.

<a id="5-curado-audio-workers-y-otras-coincidencias"></a>
### curado

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

## Orden

Actualizado el 2026-11-16, con lo de arriba hecho y un PC con Windows otra vez disponible.

1. **Windows:**
   - nove, según su [guía para seguir en Windows](../../nove/docs/windows-next.md): C#, named pipe e instalar la 0.12.0.53. Con eso, el paso 4 de su transición: retirar el sobre viejo.
   - ~~Mudar `Nu9ve.Core` y `Nu9ve.WinUI` de nüün a `windows/`~~: hecho el 2026-11-16. Sigue que nove adopte el tema con «Puerto» como original.
   - La fase A de nüün en Windows ([qué se muda §5](suite-split.md#5-c-nativewindows)).
2. **Ver en verde el CI de GitHub** (pestaña Actions) de core, nüün, Clipper y nove, sobre todo los jobs de Linux y Windows: ninguna sesión los ha visto correr.
3. **`NuMacShell` y `Nu9veTheme` en las demás apps de Mac:**
   - nüün, después de su semana de uso diario;
   - SoundBender, que sube a macOS 15;
   - primera copia fijada en neuf y Stalker.
4. Guía de prácticas comunes (`docs/practices.md`).
5. Lo que espera su condición:
   - `nu9ve-jobs`, desde Clipper y curado;
   - `nu9ve-loudness`, cuando SoundBender decida Rust;
   - `nu9ve-volumes`, cuando se decida la identidad portable;
   - `nu9ve-py`;
   - el sonido de Fractos.
6. Comparar `scripts/check-boundary.sh` con `check_suite_boundary.sh` de nüün y dejar uno.
