# Qué se muda a la suite y qué se queda en nüün

Estado: **inventario, sin mover nada**. Escrito el 2026-11-11 en el PC de Windows, leyendo el árbol de este repo. Aplica la decisión del usuario del mismo día: repo `nu9ve/suite` con copia fijada en cada app ([cómo se comparte](suite-sharing.md)). Aquí está, archivo por archivo, qué sale de `n-creator-utils`, cuándo y con qué condición, y qué se queda porque es de nüün.

Contexto que cambia el orden: **las siguientes sesiones de este repo serán en la Mac**, sin un PC con Windows por un tiempo. Lo que sólo se puede probar en Windows (WinUI, MSIX, la ventana, la verificación sin caché de Windows) espera.

## 1. Regla general: documentos ya, código después

| Qué | Cuándo se muda | Por qué |
| --- | --- | --- |
| Contratos de suite, mapa, guías de plataforma, datos comunes | **Ya** ([S5](audits/session-briefs-2026-11-10.md#s5--decidir-y-crear-el-repo-de-la-suite), pasos 1–2) | Son estables, los leen varias apps y moverlos no rompe ninguna compilación |
| Código de Rust `nu9ve-*` | **Después** de que el motor funcione en la Mac (backend de macOS y app de Mac usando Rust) | Hoy cambia cada semana. Con copia fijada, cada arreglo sería «editar en la suite → sincronizar → probar en nüün». Mientras sólo nüün lo usa, ese ciclo es puro costo |
| Código de Swift | Cuando el usuario decida si `NuMacShell` sale de `ios-fractos` ([pregunta 3](suite-sharing.md#8-lo-que-decide-el-usuario)) y la app de Mac de nüün ya use Rust | nüün tiene su propio shell; converger con `NuMacShell` es trabajo de interfaz y no conviene mezclarlo con el cambio de motor |
| Código de C# | Cuando haya otra vez una máquina con Windows | WinUI sólo compila y se prueba en Windows; moverlo sin poder compilarlo sería mover a ciegas |

Mientras el código siga aquí, la frontera se cuida igual: **nada `nu9ve-*` importa algo `nuun-*`**. Hoy se cumple: `nu9ve-volumes` sólo depende de `nu9ve-json`, y `Nu9ve.Core`/`Nu9ve.WinUI` reciben el nombre de la app como parámetro (`Storage.Configure("nuun", "NUUN_QA_ROOT", …)`). Conviene volverlo un script, como `verify-packages.sh` de Fractos: que falle si un crate `nu9ve-*` o un proyecto `Nu9ve.*` nombra algo de nüün. Así el día de la mudanza es copiar carpetas, no desenredar.

## 2. Documentos y datos

| Archivo | Destino | Nota |
| --- | --- | --- |
| `docs/contracts/macos-app-v1.md`, `windows-app-v1.md`, `nu9ve-theme-v1.md`, `nu9ve-interaction-v1.md`, `nu9ve-mcp-v1.md`, `nu9ve-sound-v1.md`, `nu9ve-notifications-v1.md` | `suite/contracts/` | Son de toda la suite. Las tablas de adopción tienen una columna por app |
| `docs/contracts/examples/theme-vectors-v1.json` | `suite/data/` | Hoy lo prueba C#; Swift pendiente |
| `docs/suite.md`, `docs/suite-sharing.md`, este documento | `suite/docs/` | Este documento se archiva cuando la mudanza termine |
| `docs/platforms/windows.md`, `os-integration.md`, `linux.md` | `suite/docs/platforms/` | `windows.md` mezcla lecciones de la suite con el estado de nüün en su §1: el estado se queda en STATUS |
| `docs/platforms/rust-engine.md` | **Se parte** | La §6 («qué compartir en Rust») va a la suite. El resto es la migración de nüün y se queda |
| `docs/audits/fable-audit-2026-11-10.md`, `session-briefs-2026-11-10.md` | `suite/docs/audits/` | Cubren cuatro repos. nüün deja un enlace. Los briefs nuevos se escriben en el repo donde va el trabajo |
| `docs/contracts/README.md` | **Se parte** | Hoy es el índice de los contratos de media (Stalker, Clipper) con un apartado de la suite. El apartado de la suite va a `suite/contracts/README.md`; lo demás se queda |

**Se queda en nüün:** `atlas-drone-v1`, `clipper-transcripts-v1`, `ingest-flow-v1`, `media-handoff.schema.json`, `media-library-v1`, `media-naming-v1`, `project-closeout-v1`, `stalker-retitle-v1` (y su esquema), `next-sessions.md`, `examples/handoff.json`, y todo `docs/features/`, `operations.md`, `release.md`, `CONTEXT.md` y `STATUS.md`. Son contratos entre nüün y otra app, no reglas de toda la suite.

## 3. Rust (`engine/`)

| Crate | Destino | Condición | Nota |
| --- | --- | --- | --- |
| `nu9ve-json` | `suite/rust/` | Tras el cambio de motor en la Mac | Sobre de errores, JSON canónico, digest e ids: lo que la suite necesita para el [sobre común](audits/fable-audit-2026-11-10.md#c1--dos-sobres-y-dos-transportes). Al retirar Python, «compatible con Python byte a byte» pasa de ser una restricción a ser la **especificación** del JSON canónico de la suite. Escribirla como contrato corto antes de mudarlo |
| `nu9ve-volumes` | `suite/rust/` | Tras la verificación sin caché ([S1](audits/session-briefs-2026-11-10.md#s1--verificación-sin-caché-en-windows-y-linux)) y el backend de macOS ([S16](audits/session-briefs-2026-11-10.md#s16--backend-de-volúmenes-de-macos-en-rust)) | Es la pieza que más cambia ahora. `fs.rs` (sincronizar, renombrar sin reemplazar, leer sin caché) le sirve también a curado. La identidad portable ([A3](audits/fable-audit-2026-11-10.md#a3--la-identidad-de-un-disco-no-viaja-entre-equipos-ni-entre-sistemas)) se decide antes de mudarlo, porque cambia su API |
| `nu9ve-time` | **Se queda**, aunque se llame `nu9ve-` | Hasta que otra app lo necesite | Reproduce el `datetime` de CPython para que los nombres y digests coincidan con Python. Sólo nüün lo usa. Cuando se retire Python, sigue haciendo falta para que los nombres de destino no cambien |
| `nuun-media`, `nuun-recipe`, `nuun-engine` | Se quedan | — | Dominio de nüün: catálogo, receta, planner, transferencia, binario |

Hoy no hay más candidatos. `nu9ve-ipc` (el pipe de nove), `nu9ve-mcp` y `nu9ve-os` nacen en la suite cuando haya dos copias ([rust-engine §6](platforms/rust-engine.md#6-qué-compartir-en-rust-entre-las-apps)).

## 4. Swift (`native/MediaMenu`)

El paquete tiene `MediaMenuCore` (sin interfaz) y la app. Nada se muda tal cual; lo que hay es **convergencia** con piezas de la suite:

| Pieza de nüün | Pieza de la suite | Qué pasaría |
| --- | --- | --- |
| `ShellWindow.swift`, `SkinChrome.swift`, `AboutView.swift`, parte de `Appearance.swift` | `NuMacShell` (hoy en `ios-fractos`) | nüün adopta `NuMacShell` y borra lo suyo, en un cambio de interfaz propio |
| `MediaMenuCore/AppIdentity.swift` | `Nu9veBuildID` (Fractos) | Un solo sello de build en la suite |
| `MediaMenuCore/ThemeColor.swift` (roles de tema) | Futuro `NuTheme` en `suite/swift/`, probado con `theme-vectors-v1.json` | Es el gemelo de `Nu9ve.Core/Theme` en C#. Primero, que su prueba lea los vectores compartidos (PLAN, Windows paso 2) |
| `MediaMenuCore/Protocol.swift` (`BackendConfiguration`, transporte) | Se queda | Cuando exista el sobre común, su forma vendrá de un contrato, pero el cliente es de nüün |

**Se queda:** todo lo demás (`Model.swift`, `IngestFlow`, `DisksView`, `ProjectClosureView`, `StorageView`, `InboxView`, `Missions`, `PlanTable`, `QASnapshots`…) y `Localization/` (858 claves). El catálogo único de textos ([S15](audits/session-briefs-2026-11-10.md#s15--un-solo-catálogo-de-textos)) es un mecanismo de suite, pero los textos son de cada app.

## 5. C# (`native/windows`)

| Proyecto | Destino | Condición |
| --- | --- | --- |
| `Nu9ve.Core` (tema, shell, atajos, ventana, instalador) | `suite/windows/` | Con una máquina con Windows, y de preferencia cuando nove lo adopte |
| `Nu9ve.WinUI` (pinceles, controles con skin, cromo DWM) | `suite/windows/` | Igual |
| `Nuun`, `Nuun.Core`, `Nuun.Core.Tests`, `Nuun.Setup`, `Package` | Se quedan | La app de nüün, su cliente del motor y su instalador. `Nuun.Setup` es la base propuesta para los Setup de nove y curado: se generaliza cuando el segundo lo adopte |

## 6. Scripts

| Script | Destino | Nota |
| --- | --- | --- |
| `scripts/windows/capture-window.ps1`, `ui-invoke.ps1`, `make-icons.ps1` | `suite/scripts/windows/` | Genéricos, con la app como parámetro. Revisar antes que no tengan rutas de nüün |
| `scripts/windows/qa-skins.ps1`, `package-msix.ps1` | Se generalizan primero | Hoy conocen la app y su paquete |
| `scripts/check_localization.py` | Más adelante, con S15 | Pasa a ser el comprobador del catálogo de textos de toda la suite |
| `build_media_app.py`, `install_media_app.py`, `media_icon.swift`, `engine_golden.py`, `test_stalker_retitle_bridge.py` | Se quedan | Son de nüün |
| `sync-suite` (sh y ps1) | Nace en la suite | Calcado de `sync-contracts.sh` de Fractos ([suite-sharing §4](suite-sharing.md#4-cómo-lo-consume-una-app)) |

## 7. Qué queda en `n-creator-utils` después

`media/` (Python, hasta retirarlo), `engine/` sin los crates mudados, `native/MediaMenu`, `native/windows` sin las bases, los contratos de media, `docs/features`, operación, release, CONTEXT, STATUS y PLAN, y `vendor/suite/` con la copia fijada de lo que nüün usa. Las utilidades viejas (`manager/`, descargas, stems, MIDI en `hardware/`) también se quedan: no son de la suite, son el pasado y el futuro de este repo ([README](../README.md#más-utilidades-y-hacia-dónde-puede-crecer)).

## 8. Orden mientras se trabaja en la Mac

Dos pistas que no se pisan: el **motor** (código de nüün, en este repo) y la **suite** (sólo documentos y datos hasta que el motor funcione en la Mac). El orden del motor está en [PLAN · Motor en Rust](../PLAN.md#motor-en-rust--ruta).

| Paso | Pista | Trabajo |
| --- | --- | --- |
| 1 | Ambas | Higiene sin diseño: `.gitattributes`, `rust-toolchain.toml` ([S3](audits/session-briefs-2026-11-10.md#s3--higiene-de-repos)). Antes de guardar fixtures doradas que se comparan byte a byte |
| 2 | Suite | Crear `nu9ve/suite` con los documentos y datos de la §2 y el script `sync-suite`; nüün deja enlaces y su primera copia fijada de documentos ([S5](audits/session-briefs-2026-11-10.md#s5--decidir-y-crear-el-repo-de-la-suite), pasos 1–2). Responder las preguntas 3 y 4 de [suite-sharing §8](suite-sharing.md#8-lo-que-decide-el-usuario) |
| 3 | Suite | El script de frontera de la §1 (nada `nu9ve-*` nombra algo `nuun-*`) |
| 4 | Motor | Pasos de la Mac del PLAN: compilar y probar el motor en macOS, dorado, backend de volúmenes de macOS, la app de Mac usando Rust por método |
| 5 | Suite | Con el motor ya usado en la Mac: escribir el contrato del JSON canónico y mudar `nu9ve-json` y `nu9ve-volumes` ([suite-sharing §7](suite-sharing.md#7-migración-por-pasos), paso 3) |
| 6 | Suite | Swift: `NuMacShell` y `NuTheme`, si el usuario decide que salen de Fractos |
| — | Suite | C#: `Nu9ve.Core`/`Nu9ve.WinUI` cuando vuelva a haber Windows |
