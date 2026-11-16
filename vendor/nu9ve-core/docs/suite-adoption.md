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
| **Clipper** | `contracts/nu9ve-mcp-v1.md` y, con el contrato, `nu9ve-json-v1` | `nu9ve-json` para firmas y sobres, después de comprobar los vectores byte a byte | — | Mayormente Python; su roadmap ya enlaza aquí |

## 4. Orden sugerido

1. ~~Mudar `NuMacShell` a `swift/`, con Fractos como primera copia fijada.~~ Hecho el 2026-11-16.
2. ~~Escribir `nu9ve-json-v1` con sus vectores, mudar `nu9ve-json` y crear el workspace de `rust/`.~~ Hecho el 2026-11-16.
3. Hacer la primera copia fijada en nove, kyu, SoundBender y curado. Cada app, un cambio propio en su repo.
4. Pruebas de tema en Swift contra los vectores (nüün y kyu), y después `Nu9veTheme`.
5. Escribir la guía de prácticas comunes.
