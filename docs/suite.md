# Mapa de la suite nu9ve

Este repo coordina la suite: aquí viven los contratos que comparten las apps y este mapa de quién es quién. Cada app conserva sus datos, su estado y su plan en su repo; este mapa **enlaza** y no reemplaza esos estados. Leído en los repos el 2026-11-07: antes de decidir algo, revisa el STATUS de la app.

## Las apps

Cada nombre es «nueve» en un idioma, y de ahí sale su identidad ([tema §7](contracts/nu9ve-theme-v1.md#7-identidad-de-cada-app)).

| App | Para qué | Identidad | Plataformas | UI | Motor y transporte | Repo | Estado según su repo |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **nüün** | Ingresar, respaldar y preparar media; biblioteca, discos y liberación de espacio | Suiza: la navaja multiherramienta | macOS | SwiftUI/AppKit, barra de menús | Python (`media/service.py`), proceso hijo por stdin/stdout con JSON | `nu9ve/n-creator-utils` | En desarrollo; instalada localmente ([STATUS](STATUS.md)) |
| **Stalker** | Grabar pantalla, cámara y audio desde la barra de menús como fuente para editar | Utilidad: sigue al sistema | macOS | SwiftUI/AppKit | Swift (`StalkerCore`/`StalkerCapture`) | `nu9ve/stalker` | 0.5.0; validación larga pendiente |
| **Clipper** | Motor editorial: proyectos, transcripción, timeline, colas, export | Motor | macOS (CLI/TUI) | Textual TUI + CLI; `ClipperStudio` (app nativa previa) | Python; `app-service` como autoridad editorial | `nu9ve/clipper` | CLI/TUI con contratos cerrados; aceptación física pendiente |
| **neuf** | App nativa de edición sobre el motor de Clipper | Francia: el cine | macOS | SwiftUI (Xcode) | Clipper vendorizado por `AppServiceClient` (proceso) | local, sin remoto | Privada, sin instalar; sustitución de Studio en curso |
| **kyu** | Cribado, revelado y entrega de fotos | Japón: las cámaras profesionales | macOS (Windows/Linux en etapa 3) | AppKit + SwiftUI + Metal | Rust (`kyu-core`) por FFI/XCFramework | local, sin remoto | v0.1 con fixtures; adoptando tema e interacción |
| **curado** | Biblioteca de música y consola de dos decks | México/Sonora: curar, «curado» | macOS, Windows (Linux por validar) | Tauri + React, ventana Metal nativa | Rust (`audio-engine`) + comandos Tauri | `nu9ve/curado` | 0.1.0; Windows compilado y probado automáticamente |
| **SoundShop** | Editor espectral: el sonido como lienzo | Sin país: el lienzo espectral | macOS | SwiftUI + Metal | Swift (`SoundCore`); stems con Python opcional | local, sin remoto | v0.29.0 |
| **nove** | Recolectar datos, seguir números de páginas, condiciones y avisos | Portugal: exploradores, el puerto | macOS, **Windows (activa)** | SwiftUI/AppKit · C#/WinUI 3 · extensión de Chrome | Rust; socket Unix / named pipe con el mismo protocolo; MCP local | `nu9ve/nove` | Mac 0.11.0, Windows 0.12.0.x por MSIX de desarrollo |
| **Atlas** | Análisis de fotos nadir de dron (daño en calles) | Datos | macOS (Python) | Visor web local | Python | `data/atlas`, local | Exploración; consume archivos de nüün |
| **Fractos** | Comunidad: mapa, trabajo, grupos, perfiles; la referencia expresiva de la suite | Fractos | iOS, macOS, web | SwiftUI (`FractosKit`) · Next.js | API de Fractos (`/api/v1`) + Supabase solo para auth | `nu9ve/ios-fractos`, `nu9ve-machine/nu9ve.xyz` | iOS y Mac en desarrollo; web activa en nu9ve.xyz |

Otros proyectos del colectivo, fuera de este mapa por ahora: docufin (documentos → conocimiento), arre.click (scout), forja (GUI para agentes) y video (Remotion).

## Flujos que cruzan apps

| Flujo | Recorrido | Contrato |
| --- | --- | --- |
| Grabación → edición | Stalker graba → nüün archiva el bundle → Clipper/neuf edita → Final Cut a mano | [stalker-retitle](contracts/stalker-retitle-v1.md), [handoff](contracts/media-library-v1.md), [cierre de proyectos](contracts/project-closeout-v1.md) |
| Cámara → edición | SD → nüün (respaldo verificado + copia de trabajo) → Clipper/neuf | [ingreso](contracts/ingest-flow-v1.md), [naming](contracts/media-naming-v1.md), [biblioteca](contracts/media-library-v1.md) |
| Fotos | SD → nüün → kyu reconecta por huella, solo lectura (`nu9ve media projects/sources`) | [README de contratos](contracts/README.md), [fotos y Kyū](features/product-flows.md#fotos-y-kyū) |
| Dron | SD + exports de subaereo → nüün → Atlas | [atlas-drone](contracts/atlas-drone-v1.md) |
| Audio | Descargas/stems en nüün (CLI legado); curado y SoundShop con sus propias bibliotecas | Sin contrato común todavía |
| Datos | nove por su cuenta; MCP local de solo lectura | Contratos propios en `nove/docs/contracts` |

## Contratos de la suite

| Contrato | Trata | Estado |
| --- | --- | --- |
| [macos-app-v1](contracts/macos-app-v1.md) | Barra de título, ventana, menús, Acerca de, documentos, distribución en Mac | En adopción (SoundShop, nüün) |
| [windows-app-v1](contracts/windows-app-v1.md) | Lo mismo en Windows, con motor compartido y equivalencias con Mac | Propuesta; referencia nove |
| [nu9ve-theme-v1](contracts/nu9ve-theme-v1.md) | Roles de color, contraste, Apariencia, skins (original, contraste, Macintosh, Windows 9x, Terminal), identidad | Propuesta; nüün completo, kyu en curso |
| [nu9ve-interaction-v1](contracts/nu9ve-interaction-v1.md) | Todo por teclado, catálogo de atajos, badges con capas, ayuda contextual, panel ⌘/ | Propuesta |
| [Contratos de media](contracts/README.md) | Ingreso, naming, biblioteca/handoff, cierre, Stalker, Atlas | Vigentes, con adopción por app en su índice |

Clipper mantiene además los suyos (`clipper/apps/APP_CONTRACT.md`, `APP_SERVICES_CONTRACT.md`, `CROSS_APP_EDITORIAL_GUIDE.md`), y nove los de su motor (`nove/docs/contracts`).

## Plataformas

- **macOS** es la plataforma principal de todas las apps. Las reglas: [macos-app-v1](contracts/macos-app-v1.md).
- **Windows** está activo en **nove** (WinUI 3 nativo) y **curado** (Tauri). Las reglas: [windows-app-v1](contracts/windows-app-v1.md).
- **Candidatas a Windows** son las apps cuyo motor ya es portable:
  - **kyu:** motor Rust, en su etapa 3 con wgpu.
  - **nüün:** motor Python, pero la parte de discos y volúmenes es específica de Mac.
  - **Clipper:** Python, y su TUI ya corre en cualquier terminal.

  Las que dependen de frameworks de Apple (Stalker con ScreenCaptureKit, SoundShop con Metal, neuf) necesitarían otro motor o capa de captura.
- **iOS y web:** Fractos (y subaereo para la captura de dron).

## Lo que ya se repite y podría compartirse

La regla vigente es **replicar primero y extraer después** ([tema §10](contracts/nu9ve-theme-v1.md#10-implementación-compartida); nove llegó a lo mismo: compartir especificación y fixtures antes que una librería). Una pieza se vuelve paquete cuando está igual en tres apps. Candidatas por plataforma:

| Plataforma | Pieza | Dónde está hoy | Copias |
| --- | --- | --- | --- |
| Swift (Mac) | Roles de color, `readable`, presets y skins (`ThemeRoles`, `ThemeSkin`) | nüün `MediaMenuCore/ThemeColor.swift`, SoundShop `Appearance.swift`, kyu (en curso) | 2–3 |
| Swift (Mac) | Ventana cuadrada y barra propia (`ShellWindow`, `TitleBarDragArea`, controles × − +) | Fractos `NuMacShell`, nüün, SoundShop, neuf (doble clic) | 3–4 |
| Swift (Mac) | `AppIdentity` / Acerca de | Fractos `NuMacShell`, SoundShop, neuf, nove | 4 |
| Swift (Mac) | Catálogo de atajos, badge del footer, ayuda contextual | SoundShop (`ShortcutCatalog`, `WorkspaceHelpStore`) | 1 |
| C# (Windows) | Cliente del pipe, host de fondo, diccionarios de tema, settings cards | nove `Nove.Core` | 1 |
| Rust | IPC enmarcado sobre socket/pipe con ACL de usuario | nove `core` | 1 |
| Rust / Python | Regla de `slug()` para nombres | nüün `media`, kyu `naming.rs` (copia declarada) | 2 |
| TypeScript | Paleta pura, dither y pruebas de contraste | curado `theme.ts`, `dither.ts`; Fractos web `packages/styles` | 2 |
| Todas | QA aislada por variable de entorno; instalar en la carpeta de apps del usuario con respaldo del anterior | nüün, nove, curado, kyu, SoundShop | 5 |

**Primer candidato maduro:** `NuMacShell` + roles de tema en Swift (Acerca de, ventana, barra y paleta), con nüün, Fractos, SoundShop y kyu como consumidores. Antes de crearlo hay que decidir en qué repo vive y cómo se versiona; queda pendiente y no se crea sin esa decisión.

## Mantener este mapa

Actualízalo cuando nazca una app, cambie su plataforma o motor, se adopte un contrato o una pieza llegue a tres copias. Los detalles y la evidencia van en el repo de cada app y en la tabla de adopción de cada contrato; aquí solo el resumen y los enlaces.
