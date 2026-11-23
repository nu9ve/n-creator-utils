# Mapa de la suite nu9ve

Las menciones a «SoundShop» en los contratos se refieren a **SoundBender**, su nombre desde el 2026-11-16; las rutas de código siguen siendo las de hoy.

`nu9ve/core` coordina la suite: aquí viven los contratos que comparten las apps, su código común y este mapa de quién es quién. Cada app conserva sus datos, su estado y su plan en su repo; este mapa **enlaza** y no reemplaza esos estados. Leído en los repos el 2026-11-07; las filas de SoundBender, nove, Atlas, kyu y neuf, el 2026-11-20. Antes de decidir algo, revisa el STATUS de la app.

## Las apps

Cada nombre es «nueve» en un idioma, y de ahí sale su identidad ([tema §7](../contracts/nu9ve-theme-v1.md#7-identidad-de-cada-app)).

| App | Para qué | Identidad | Plataformas | UI | Motor y transporte | Repo | Estado según su repo |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **nüün** | Ingresar, respaldar y preparar media; biblioteca, discos y liberación de espacio | Suiza: la navaja multiherramienta | macOS · Windows (maqueta) | SwiftUI/AppKit, barra de menús · C#/WinUI 3 (maqueta sin motor) | Python (`media/service.py`), proceso hijo por stdin/stdout con JSON; **migrando a Rust** (`engine/`, adoptado 2026-11-09) | `nu9ve/n-creator-utils` | En desarrollo; instalada localmente ([STATUS](https://github.com/nu9ve/n-creator-utils/blob/master/docs/STATUS.md)) |
| **Stalker** | Grabar pantalla, cámara y audio desde la barra de menús como fuente para editar | Utilidad: sigue al sistema | macOS | SwiftUI/AppKit | Swift (`StalkerCore`/`StalkerCapture`) | `nu9ve/stalker` | 0.5.0; validación larga pendiente |
| **Clipper** | Motor editorial: proyectos, transcripción, timeline, colas, export | Motor | macOS (CLI/TUI) | Textual TUI + CLI; `ClipperStudio` (app nativa previa) | Python; `app-service` como autoridad editorial | `nu9ve/clipper` | CLI/TUI con contratos cerrados; aceptación física pendiente |
| **neuf** | App nativa de edición sobre el motor de Clipper | Francia: el cine | macOS | SwiftUI (Xcode) | Clipper vendorizado por `AppServiceClient` (proceso) | `nu9ve/neuf` | Privada, sin instalar; sustitución de Studio en curso. Core fijado (`a70b717`) |
| **kyu** | Cribado, revelado y entrega de fotos | Japón: las cámaras profesionales | macOS (Windows/Linux en etapa 3) | AppKit + SwiftUI + Metal | Rust (`kyu-core`) por FFI/XCFramework | `nu9ve/kyu` | v0.1 con fixtures; usa `NuMacShell` y `Nu9veTheme` |
| **curado** | Biblioteca de música y consola de dos decks | México/Sonora: curar, «curado» | macOS, Windows (Linux por validar) | Tauri + React, ventana Metal nativa | Rust (`audio-engine`) + comandos Tauri | `nu9ve/curado` | 0.1.0; Windows compilado y probado automáticamente |
| **SoundBender** (antes SoundShop) | Editor espectral: el sonido como lienzo | Sin país: el lienzo espectral | macOS; plugin VST3/AU para Ableton Live y después Windows (plan) | SwiftUI + Metal | Swift (`SoundCore`) y motor portable en Rust (`engine/`, su decisión 036), que la app ya usa para la escena espacial por `soundbender-ffi`; stems con Python opcional | `nu9ve/soundbender` (desde 2026-11-16) | v0.31.0 publicada; sin publicar: escena espacial 3D, racks y pincel de patrón, secciones. El código aún dice SoundShop |
| **nove** | Recolectar datos, seguir números de páginas, condiciones y avisos; desde el 2026-11-19, también el hub de datos de la suite (proyectos, fuentes, geo, Atlas, conversaciones de agentes) | Portugal: exploradores, el puerto | macOS, **Windows (activa)** | SwiftUI/AppKit · C#/WinUI 3 · extensión de Chrome | Rust; socket Unix / named pipe con el mismo protocolo; MCP local; se registra en la suite y sirve `data_catalog` y `mcp_tools` | `nu9ve/nove` | `main` en datos 33, sin instalar; Mac instalada 0.11.0 (17), Windows 0.12.0.68 (datos 27) |
| **Atlas** | Análisis de fotos nadir de dron: DSM propio por bloques, baches, tramos y su calibración | Datos | macOS (Python) | Visor web local | Python | `~/code/data/atlas`, en su propia sesión | Exploración activa; nove lo lee en sólo lectura (pestaña Atlas) y su DSM es el paquete de ejemplo de artefactos. Sin copia del core. **Desde el 2026-11-22 (el usuario):** sólo lo urgente (raíces por volumen) y laboratorio de algoritmos; sus flujos pasan a nove, que en el corto plazo debería hacer todo eso nativo y en red con nüün ([varias máquinas §10](suite-devices.md#10-fases)) |
| **Fractos** | Comunidad: mapa, trabajo, grupos, perfiles; la referencia expresiva de la suite | Fractos | iOS, macOS, web | SwiftUI (`FractosKit`) · Next.js | API de Fractos (`/api/v1`) + Supabase solo para auth | `nu9ve/ios-fractos`, `nu9ve-machine/nu9ve.xyz` | Leído el 2026-11-10 (`f1a7bcb`): iOS `implemented-local`; Mac 0.1.0 (`Apps/macOS`, macOS 26) con login, Trabajo, Calendario, Mapa, Textos, Grupos y Perfil, probada en QA y sin cuenta real; web activa en nu9ve.xyz. Detalle en su `docs/MONOREPO.md` y `docs/SUITE-DESKTOP.md` |

Lo compartido privado (cuenta, nube, dispositivos, licencias) vive en [`nu9ve/inner-core`](https://github.com/nu9ve/inner-core), creado el 2026-11-22; cómo lo usa una app sin dejar de compilar sola: [apps con y sin la parte privada](suite-modules.md).

Otros proyectos del colectivo, fuera de este mapa por ahora: docufin (documentos → conocimiento), arre.click (scout), forja (GUI para agentes) y video (Remotion, con herramientas nativas de Vision y un compositor; ver la tabla de abajo).

En propuesta: un navegador diario sobre un Chromium ajeno (Helium o Brave) con una capa nu9ve de perfiles, extensión con módulos, host nativo y runtime de fondo para nove y Clipper ([navegador, versión 2](suite-browser-v2.md); inventario y contratos en la [versión 1](suite-browser.md)). Lo único con código es la extensión de nove para su broker. También en propuesta: identidad de discos y máquinas, una carpeta de intercambio entre tus máquinas y, después, la cuenta de Fractos como nube de la suite ([varias máquinas, discos y nube](suite-devices.md)).

## Flujos que cruzan apps

| Flujo | Recorrido | Contrato |
| --- | --- | --- |
| Grabación → edición | Stalker graba → nüün archiva el bundle → Clipper/neuf edita → Final Cut a mano | [stalker-retitle](https://github.com/nu9ve/n-creator-utils/blob/master/docs/contracts/stalker-retitle-v1.md), [handoff](https://github.com/nu9ve/n-creator-utils/blob/master/docs/contracts/media-library-v1.md), [cierre de proyectos](https://github.com/nu9ve/n-creator-utils/blob/master/docs/contracts/project-closeout-v1.md) |
| Cámara → edición | SD → nüün (respaldo verificado + copia de trabajo) → Clipper/neuf | [ingreso](https://github.com/nu9ve/n-creator-utils/blob/master/docs/contracts/ingest-flow-v1.md), [naming](https://github.com/nu9ve/n-creator-utils/blob/master/docs/contracts/media-naming-v1.md), [biblioteca](https://github.com/nu9ve/n-creator-utils/blob/master/docs/contracts/media-library-v1.md) |
| Fotos | SD → nüün → kyu reconecta por huella, solo lectura (`nu9ve media projects/sources`) | [README de contratos](https://github.com/nu9ve/n-creator-utils/blob/master/docs/contracts/README.md), [fotos y Kyū](https://github.com/nu9ve/n-creator-utils/blob/master/docs/features/product-flows.md#fotos-y-kyū) |
| Dron | SD + exports de subaereo → nüün → Atlas | [atlas-drone](https://github.com/nu9ve/n-creator-utils/blob/master/docs/contracts/atlas-drone-v1.md) |
| Audio | Descargas/stems en nüün (CLI legado); curado y SoundBender con sus propias bibliotecas; Clipper compone música para el video | Sin contrato común todavía; capas compartidas en propuesta ([audio en la suite](platforms/audio.md)) |
| Datos | nove reúne proyectos, fuentes y datasets de la suite (incluidos Atlas, Clipper y las conversaciones de agentes, en sólo lectura) y los publica con `data_catalog`; su motor se anuncia en el registro y `nu9ve mcp` lo ve | [registro](../contracts/nu9ve-registry-v1.md), [catálogo](../contracts/nu9ve-data-v1.md), [MCP v1](../contracts/nu9ve-mcp-v1.md); los suyos en `nove/docs/contracts` |
| Gráfica → video | nove exporta una gráfica o tabla como artefacto → Clipper la importa como overlay | [artefactos](../contracts/nu9ve-artifact-v1.md) |

## Contratos de la suite

| Contrato | Trata | Estado |
| --- | --- | --- |
| [macos-app-v1](../contracts/macos-app-v1.md) | Barra de título, ventana, menús, Acerca de, documentos, distribución en Mac | En adopción (SoundBender, Kyū, Stalker, Fractos; nüün tras su semana de uso) |
| [windows-app-v1](../contracts/windows-app-v1.md) | Lo mismo en Windows, con motor compartido y equivalencias con Mac | Propuesta; referencia nove |
| [nu9ve-theme-v1](../contracts/nu9ve-theme-v1.md) | Roles de color, contraste, Apariencia, skins (original, contraste, Macintosh, Windows 9x, Terminal), identidad | Propuesta; nüün completo, kyu en curso |
| [nu9ve-interaction-v1](../contracts/nu9ve-interaction-v1.md) | Todo por teclado, catálogo de atajos, badges con capas, ayuda contextual, panel ⌘/ | Propuesta |
| [nu9ve-mcp-v1](../contracts/nu9ve-mcp-v1.md) | Servidor MCP por app: stdio local, cliente del motor, niveles leer/mostrar/editar/actuar, permiso por sesión, datos `_untrusted` | Propuesta; nove completo en Windows, nüün sólo lectura |
| [nu9ve-sound-v1](../contracts/nu9ve-sound-v1.md) | Sonidos de interfaz por skin desde el motor v2 de Fractos: señales, gestos, voces, entrega, silencio | Propuesta (2026-11-10); sólo Fractos suena hoy |
| [nu9ve-notifications-v1](../contracts/nu9ve-notifications-v1.md) | Avisos del sistema: cola en el motor, clic al objeto exacto, valores por defecto, límites, urgencia, sonido | Propuesta (2026-11-10); nove completo en Windows, nüün y nove parciales en Mac |
| [nu9ve-json-v1](../contracts/nu9ve-json-v1.md) | JSON canónico, digest, ids y sobre de respuestas | nüün, Clipper y SoundBender, con `rust/nu9ve-json` |
| [nu9ve-fs-v1](../contracts/nu9ve-fs-v1.md) | Escritura atómica, sincronización y candados | nüün, Clipper y curado, con `rust/nu9ve-fs` |
| [nu9ve-engine-v1](../contracts/nu9ve-engine-v1.md) | Protocolo de un motor: sobre, ids, errores, cancelación | Propuesta; Clipper y nüün pasan los ejemplos; nove en transición |
| [nu9ve-registry-v1](../contracts/nu9ve-registry-v1.md) | Un archivo por motor vivo, para que la suite lo encuentre | Propuesta (2026-11-19); nove se registra en Mac |
| [nu9ve-data-v1](../contracts/nu9ve-data-v1.md) | Catálogo de proyectos, raíces y datasets, con clase de privacidad | Propuesta (2026-11-19); nove lo sirve |
| [nu9ve-artifact-v1](../contracts/nu9ve-artifact-v1.md) | Paquete de gráfica o tabla con datos exactos, render, receta y linaje | Propuesta (2026-11-19); nove produce, Clipper consume |
| [nu9ve-bench-v1](../contracts/nu9ve-bench-v1.md) | Registro de benchmarks: una corrida por línea, comparable entre motores, commits y máquinas | Propuesta (2026-11-21); la usa primero la M0 del cierre de Clipper |
| [nu9ve-device-v1](../contracts/nu9ve-device-v1.md) | Identidad local de cada máquina, sin serial ni nombre del equipo | Propuesta (2026-11-22); F0 de [varias máquinas](suite-devices.md) |
| [nu9ve-agent-v1](../contracts/nu9ve-agent-v1.md) | Un solo agente `nu9ve` por máquina: instalación, versiones, qué ve una app | Propuesta (2026-11-22); F1 |
| [nu9ve-exchange-v1](../contracts/nu9ve-exchange-v1.md) | Carpeta de intercambio entre las máquinas de la persona, con registros firmados | Propuesta (2026-11-22); F1 |
| [nu9ve-volume-v1](../contracts/nu9ve-volume-v1.md) | Marcador de disco con rol y segundo factor, estados al conectar, carpeta e índice de cada app | Propuesta (2026-11-22); cierra A3 cuando nüün la implemente |
| [Contratos de media](https://github.com/nu9ve/n-creator-utils/blob/master/docs/contracts/README.md) | Ingreso, naming, biblioteca/handoff, cierre, Stalker, Atlas | Vigentes, con adopción por app en su índice |

Guías por plataforma (propuestas del 2026-11-10): [integración con el sistema operativo](platforms/os-integration.md) y [Linux](platforms/linux.md). Hallazgos entre repos: [auditoría Fable](audits/fable-audit-2026-11-10.md). Dónde debería vivir todo esto y cómo lo consumiría cada app: [cómo se comparte entre repos](suite-sharing.md) (decidido: `nu9ve/core` con copia fijada).

Clipper mantiene además los suyos (`clipper/apps/APP_CONTRACT.md`, `APP_SERVICES_CONTRACT.md`, `CROSS_APP_EDITORIAL_GUIDE.md`), y nove los de su motor (`nove/docs/contracts`).

## Plataformas

- **macOS** es la plataforma principal de todas las apps. Las reglas: [macos-app-v1](../contracts/macos-app-v1.md).
- **Windows** está activo en **nove** (WinUI 3 nativo) y **curado** (Tauri), y **nüün** tiene una maqueta WinUI 3 sin motor (2026-11-09). Las reglas: [windows-app-v1](../contracts/windows-app-v1.md); cómo hacerlo y lo aprendido: [Windows en la suite](platforms/windows.md).
- **Candidatas a Windows** son las apps cuyo motor ya es portable:
  - **kyu:** motor Rust, en su etapa 3 con wgpu.
  - **nüün:** motor Python, pero la parte de discos y volúmenes es específica de Mac.
  - **Clipper:** Python, y su TUI ya corre en cualquier terminal.

  Las que dependen de frameworks de Apple (Stalker con ScreenCaptureKit, neuf) necesitarían otro motor o capa de captura. **SoundBender** planea un motor portable: primero un plugin VST3/AU para Ableton Live y después Windows ([audio en la suite](platforms/audio.md)).
- **iOS y web:** Fractos (y subaereo para la captura de dron).

## Lo que ya se repite y podría compartirse

La regla vigente es **replicar primero y extraer después** ([tema §10](../contracts/nu9ve-theme-v1.md#10-implementación-compartida); nove llegó a lo mismo: compartir especificación y fixtures antes que una librería). Una pieza se vuelve paquete cuando está igual en tres apps. Candidatas por plataforma:

| Plataforma | Pieza | Dónde está hoy | Copias |
| --- | --- | --- | --- |
| Swift (Mac) | Roles de color, `readable`, presets y skins (`ThemeRoles`, `ThemeSkin`) | nüün `MediaMenuCore/ThemeColor.swift`, SoundShop `Appearance.swift`, kyu (en curso) | 2–3 |
| Swift (Mac) | Ventana cuadrada y barra propia (`ShellWindow`, `TitleBarDragArea`, controles × − +) | Fractos `NuMacShell`, nüün, SoundShop, neuf (doble clic) | 3–4 |
| Swift (Mac) | `AppIdentity` / Acerca de | Fractos `NuMacShell`, SoundShop, neuf, nove | 4 |
| Swift (Mac) | Catálogo de atajos, badge del footer, ayuda contextual | SoundShop (`ShortcutCatalog`, `WorkspaceHelpStore`) | 1 |
| Swift (Apple) | Motor de sonido v2, señales de componente, puente a sonido y tacto, háptica | Fractos `FractosSoundV2`, `FractosCue`, `SoundCues`, `NuMacShell` `HapticPlayer` ([sonido v1](../contracts/nu9ve-sound-v1.md)) | 1 |
| Todas | Contrato ajeno como copia fijada con commit de origen y hashes | Fractos `Contracts/`, `scripts/sync-contracts.sh` | 1 |
| Todas | Sello de build `Nu9veBuildID` / `Nu9veBuildDate` en Acerca de | Fractos `NuMacShell` `AppIdentity`, nüün `AppIdentity` | 2 |
| C# (Windows) | Cliente del pipe, host de fondo, diccionarios de tema, settings cards | nove `Nove.Core` | 1 |
| C# (Windows) | Tema puro (`ThemeRoles` portado de Swift), superficies por skin, reglas de atajos, ventana e instalador (`Nu9ve.Core`); servicio de tema, controles con skin y cromo de ventana (`Nu9ve.WinUI`) | [`windows/`](../windows/README.md) en el core, desde nüün; nove la tomaría como segunda | 1 |
| Todas | Vectores de tema: hex esperado por rol, skin, modo y base | [`data/theme-vectors-v1.json`](../data/theme-vectors-v1.json); verificado en C#, pendiente en Swift y TypeScript | 1 |
| Rust | IPC enmarcado sobre socket/pipe con ACL de usuario | nove `core` | 1 |
| Rust | Servidor MCP con niveles y permiso por sesión | nove `core/src/mcp.rs` (nüün lo tiene en Python, sólo lectura) | 1 |
| Rust | Identidad de volumen por serial, letras y espacio libre en Windows | curado `identity.rs`/`relink.rs` (nüün lo necesitará: [motor en Rust](https://github.com/nu9ve/n-creator-utils/blob/master/docs/platforms/rust-engine.md#6-qué-compartir-en-rust-entre-las-apps)) | 1 |
| Rust / Python | Regla de `slug()` para nombres | nüün `media`, kyu `naming.rs` (copia declarada) | 2 |
| TypeScript | Paleta pura, dither y pruebas de contraste | curado `theme.ts`, `dither.ts`; Fractos web `packages/styles` | 2 |
| Rust / Swift / Python | Separación de stems con Demucs: proceso aislado y publicación de resultados | curado `stems.rs`, SoundBender `separate.py`, nüün `manager/` | 3 |
| Rust / Swift | Loudness (LUFS), limitador, BPM y clave | curado `audio-engine`, SoundBender `SoundCore`; Clipper en Python | 2–3 ([audio en la suite](platforms/audio.md)) |
| Todas | QA aislada por variable de entorno; instalar en la carpeta de apps del usuario con respaldo del anterior | nüün, nove, curado, kyu, SoundShop | 5. Escritas, con sus diferencias, en [prácticas comunes](practices.md) (2026-11-22) |
| Swift (Mac) | Apple Vision: caras, cabezas por pose, personas por recuadro y máscara de segmentación, con caché rápida (previews) y precisa (masters) | video `native/fractos-vision`; Clipper `ClipperMacOSRender/SubjectDetection.swift` (sujeto y encuadre) | 2 (leído el 2026-11-22). **Extraído** a [`Nu9veVision`](../swift/README.md) el mismo día; falta que las dos apps lo adopten |
| Swift (Mac) | Compositor procedural (DSP, baterías, voces, efectos, manifiesto) | Clipper `ClipperMacOSRender/Sources/ClipperScore`; video `native/fractos-score`, copia declarada en su `provenance.json` (Clipper `6fa115b`, 2026-10-11) que ya divergió | 2. **Extraído** a [`Nu9veScore`](../swift/README.md) el 2026-11-22, con una revisión por motor; falta que las dos apps lo adopten |
| Rust | BPM, grid de beats y clave | curado `audio-engine`; SoundBender `SoundCore` (`Tempo.swift`, `ChromaAnalysis.swift`) | 2. **Extraído** a [`nu9ve-music`](../rust/nu9ve-music/) el 2026-11-22, desde curado; falta que las dos apps lo adopten |

**Primer candidato maduro:** `NuMacShell` + roles de tema en Swift (Acerca de, ventana, barra y paleta), con nüün, Fractos, SoundShop y kyu como consumidores. Vive en `nu9ve/core/swift/`, con copia fijada en cada app (decidido el 2026-11-16).

## Mantener este mapa

Actualízalo cuando nazca una app, cambie su plataforma o motor, se adopte un contrato o una pieza llegue a tres copias. Los detalles y la evidencia van en el repo de cada app y en [adopción por app](adoption.md) de cada contrato; aquí solo el resumen y los enlaces.
