# Adopción por app

Qué parte de cada contrato cumple cada app. Se mudó aquí desde los contratos el 2026-11-16: un contrato cambia cuando cambia la regla, y esta página cuando una app avanza. Así fijar un contrato no deja a una app atrás porque otra registró su adopción.

Cada app actualiza su fila desde su propio repo, con un cambio aquí y su línea en el [CHANGELOG](../CHANGELOG.md). Estas tablas resumen; el estado real está en el repo de cada app y, si difieren, manda el de la app. Las apps no necesitan fijar esta página.

## macos-app-v1

[Apps de macOS de nu9ve v1](../contracts/macos-app-v1.md)

Revisar el código al actualizar; esta tabla no sustituye el estado de cada app.

| Regla | SoundShop 0.27 | nüün 0.2 | Fractos Mac 0.1 | Stalker 0.5 |
| --- | --- | --- | --- | --- |
| 1 Barra de título | Sí | Sí | Sí: cada material salvo cristal dibuja su barra y sus controles; arrastre con ratón real sin probar | No: Configuración es un `NSWindow` con título y sin toolbar; el panel de la barra de menús no tiene barra. Falta decidir si se declara utilidad sin barra propia |
| 2 Ventana | Sí | Parcial: la pantalla completa recorre clases privadas | Sí, con desvío declarado: `ShellWindow` en vez de `Window`, por las esquinas cuadradas | Sí en código (build 9): mínimo 560×520 y marco recordado con `setFrameAutosaveName`; sin probar tras relanzar |
| 3 Temas | Sí | Sí | Sí; contraste probado sólo en los temas por defecto | No aplica: sigue al sistema (ver tema) |
| 4 Menús | Sí | Parcial: menú manual sin Servicios, Ocultar otras, Mostrar todo ni Zoom | Parcial: la Ayuda abre la web; interfaz con español base e inglés parcial; menús nativos por revisar; ⇧⌘Q cierra la sesión de la app y es un atajo del sistema | Parcial (build 9): menú principal oculto (`LSUIElement`) con Stalker, Edición y Ventana, visto en la app instalada; ⌘V dentro del campo de proyecto sin probar a mano. `CFBundleDevelopmentRegion` y `CFBundleLocalizations` (`es`). Sin menú Ayuda |
| 5 Acerca de | Sí | Sí (2026-11-07): ventana propia con tema y skin, versión copiable, enlaces y `NSHumanReadableCopyright` | Sí, con sello de build (`Nu9veBuildID`) | Sí (build 9): `AboutView`/`AppIdentity` de `NuMacShell` con colores del sistema, sello `Nu9veBuildID`, abierta en la app instalada. Sin prueba que compare la versión del código con el `Info.plist`: la lee del bundle |
| 6 Documentos | Sí | No aplica | No aplica | No aplica |
| 7 Ajustes | Sí | Sí | Sí: tema, paleta, material, logo, sonido y háptica; sin sincronizar con la cuenta | Sí (build 9): ⌘, en el menú principal y en el del clic derecho. Un solo formulario: «recordar la sección» no aplica |
| 8 Sistema | Sí, sin pasada completa de VoiceOver | Sí, sin pasada completa de VoiceOver | Parcial: sonido y háptica sin escuchar ni probar con trackpad; sin pasada de VoiceOver | Parcial: permisos al usarlos y aviso al guardar con clic a la carpeta exacta; Reducir movimiento y VoiceOver sin revisar |
| 9 Bundle y distribución | Parcial: scripts listos, nunca notarizada, sin actualizador | Parcial: sin notarización ni actualizador | Parcial: firma ad hoc, sin Developer ID, notarización ni actualizador; icono de iOS | Parcial: hardened runtime y firma de dentro hacia fuera sin `--deep`, identidad por `STALKER_SIGNING_IDENTITY`; `LSApplicationCategoryType` y `NSHumanReadableCopyright` (build 9); faltan notarización y actualizador (se actualiza con `install.sh`) |
| 10 Validación | Sí | Sí | Sí: modo QA con capturas y prueba de ventana; falta registro de pruebas físicas | Sí (build 9): `STALKER_QA_ROOT` aísla presets, `library.json`, retitle y biblioteca, y no toca el inicio de sesión (con pruebas); registro de lo no comprobado en `docs/validation.md`. Sin modo de capturas de ventana |

La columna de Fractos resume su propia tabla (`ios-fractos`: `docs/MONOREPO.md`, «Adopción del contrato de Mac»), leída el 2026-11-10 junto con su código; nadie vio la app corriendo desde esta sesión. Si las dos tablas difieren, manda la de su repo.

**NuMacShell (2026-11-16):** Kyū toma de ahí la ventana cuadrada, los bordes de arrastre, `TitleBarDragArea` y el Acerca de con `AppIdentity`, y borra su `SkinChrome.swift` (`kyu` `77a3449`). SoundBender crea su ventana principal con `ShellWindowHost` (`soundshop` `1cf31e3`). Desde el paso 2 (`29b0e90`) la hace cuadrada con su barra de época en Macintosh, Windows 9x y Terminal, usa `TitleBarDragArea` de aquí y conserva un puente propio, `MainWindowChrome` ([plan](../../soundshop/docs/plans/suite-shell.md)). El de aquí no le basta por cuatro cosas: el documento representado y el estado editado; que cerrar la ventana termine la app; conservar el marco al cambiar de barra; y mostrar el toolbar al **empezar** a salir de pantalla completa.

**Barra de época en `NuMacShell` (2026-11-18):** las tres propuestas de ese paso están hechas ([`PeriodWindowBar.swift`](../swift/Sources/NuMacShell/PeriodWindowBar.swift), [CHANGELOG](../CHANGELOG.md)):
- `WindowChromeBridge` muestra el toolbar en cuanto empieza `willExitFullScreen`, lo quita antes de la barra de título y lo devuelve después, y conserva el marco. `ShellWindow.usesSquareCorners` también conserva el marco por su cuenta;
- la barra de los tres comunes: `PeriodEra`, `PeriodBarColors`, `PeriodWindowBar`, `PeriodCaption`, `PeriodWindowControls` con `PeriodWindowControl`, `PeriodBarBackground`, `PeriodRowBackground`, `PeriodGrowBox`, `Win9xBevel`, `ClassicPinstripes`, `WindowHandle` y `WindowHandleReader`;
- `ShellWindowHost.present` y `WindowChromeBridge` avisan de que cambiar la barra o entrar en pantalla completa relanza los `.task` y `.onAppear` de la raíz.

Se partió de la copia de SoundBender, la más factorizada: los controles son un tipo con nombre, ayuda, signo y acción, y la ventana se lee en cada clic. Se le sumaron el fondo de fila y la caja de agarre de Kyū y nüün, que eran iguales entre sí. Las tres copias coincidían en medidas, orden, glifos, fuentes, biseles, rayas y degradado de Windows 9x. Diferían en esto, y el core elige:
- **+ en una ventana fija:** nüün lo quita y SoundBender lo atenúa. El core lo atenúa, porque el contrato quiere × − + siempre visibles (tema §8).
- **Título de Macintosh:** sólo SoundBender lo corta a una línea y a 420 pt. El core también.
- **Ayuda:** el core usa la de SoundBender, con el atajo (`Cerrar ventana · ⌘W`). Nombre y ayuda se buscan como claves en las cadenas de la app, así que las traducciones de la app siguen sirviendo.
- **Terminal sin fila (Ajustes, Acerca de):** nüün ocultaba también los controles. El core deja la fila con × − + y el título, como SoundBender.
- **Colores:** cada copia los sacaba de su paleta. El core los recibe en `PeriodBarColors`, a mano o desde `ThemeRoles` (`canvas`, `surface`, `ink`, `accentText` y el degradado desde `seed`).

Quedan en cada app sus skins propios (Neón de Kyū, Gótico de nüün), `Scanlines`, `NeonGrid`, `SkinAtmosphere` y los diálogos dentro de la ventana. `NuMacShell` depende ahora de `Nu9veTheme`, que las tres apps ya tienen. Los tipos de la app con el mismo nombre (`Win9xBevel`, `WindowHandle`…) tapan a los del core, así que **nada rompe hasta que la app adopte**. Se comprobó compilando un módulo con esos nombres contra el paquete.

Qué cambia cada app para adoptarla (sin hacer; cada una en su sesión, tras `sync-core.sh`):

| App | Borra | Pasa a usar | Cambia a la vista | Comprueba |
| --- | --- | --- | --- | --- |
| **SoundBender** (`soundshop` `8a79f82`, `Sources/SoundShop/PeriodWindowBar.swift`) | `WindowControl`, `PeriodCaption`, `PeriodWindowControls`, `PeriodPromptRule`, `Win9xBevel`, `ClassicPinstripes`; `PeriodWindowMetrics` se vuelve `PeriodEra(skin.shared)` | `PeriodWindowBar(title: session.documentTitle, era:, colors:, drawsRowBackground: false, rowSpacing: 4, window: { shell.window }) { WindowTopBar(…).disabled(modal) }` en la principal. En Acerca de, Ayuda y Ajustes, `showsRow: false` y `zoomEnabled`. Colores a mano: `surface: Theme.panel`, `ink: Theme.text`, `accent: Theme.accent` y el degradado con `win9xCaption(seed:)`. `MainWindowChrome` y `startOnce` se quedan; su `setFrame` tras cambiar la barra ya es redundante | Nada, si los colores coinciden. En Terminal sin fila, el título va a 6 pt de los controles en vez de 10: pasar `rowSpacing: 10` | Las 19 capturas de su QA de shell contra HEAD, byte a byte, y las `aux-*` y `sheet-*` |
| **Kyū** (`kyu` `cd6d1c8`, `apps/macos/Sources/Kyu/SkinChrome.swift`) | `Win9xBevel`, `ClassicPinstripes`, `WindowHandle`, `WindowHandleReader`, `WindowGrowBox` (por `PeriodGrowBox`), y en `SkinWindowBar` `caption`, `barRow`, `windowControls` y los glifos de los comunes | `PeriodWindowBar(title:, era:, colors:) { KyuBarControls() }`, con su margen de 8 pt a cada lado dentro de la fila. `SkinBarBackground` dibuja con `PeriodBarBackground` los comunes. `SkinWindowBar` queda sólo para Neón | Ayuda con atajo; título de Macintosh cortado a una línea | Que `PeriodBarColors(p.roles)` dé sus colores. Usaba `roles.base` para el degradado y `p.surface`, `p.ink` y `p.accentText`; si no coinciden, el init por campos. Capturas de los tres comunes antes y después |
| **nüün** (`n-creator-utils` `f18d6bd`, `native/MediaMenu/Sources/MediaMenu/SkinChrome.swift`), tras su semana de uso diario, junto con el resto de `NuMacShell` | Lo mismo que Kyū, y `SkinWindowBar` para los comunes | `PeriodWindowBar(…, zoomEnabled: resizable, showsRow: barRow(skin)) { bar }` en `SkinnedWindow`; `PeriodGrowBox` en la esquina. Gótico se queda con su barra | Acerca de muestra + atenuado. En Terminal, Ajustes vuelve a tener × − + y título. Ayuda nueva: sumar las claves `Cerrar ventana · ⌘W`, `Minimizar ventana · ⌘M` y `Ampliar ventana · con Opción: pantalla completa` a sus cadenas en inglés | `PeriodBarColors(palette.roles)` contra su paleta; capturas de skins y VoiceOver de los tres controles |
| **Fractos** | Nada | Recibe el arreglo del puente al sincronizar | El toolbar vuelve al empezar a salir de pantalla completa | Su prueba de ventana |

**Una sola fila (2026-11-26, [tema §8](../contracts/nu9ve-theme-v1.md#8-barras-del-sistema-menús-y-controles)):** `PeriodWindowBar(singleRow: true)` con `periodClearing()` y `periodPlate()`. Nace de nüün, que ya adoptó `NuMacShell` y la época con color (su F3, `62a1c1a`) y usa una sola barra en su ventana principal con su propia copia (`89606dc`, `SkinnedWindow(singleBar:)`, `PeriodClearing` y `PeriodPlate`). Le toca cambiar esa copia por la del core al volver a fijar, y quitar el nombre de su fila: el core lo escribe una vez en las dos épocas. Las demás apps con navegación en su ventana principal (SoundBender, Kyū) la adoptan cuando quieran; Ajustes, Acerca de y Ayuda siguen con dos filas. `AboutView(systemButtonStyle: false)` deja los botones al estilo del skin.

Abiertas, de SoundBender (paso 3, `8a79f82`): las alertas del sistema con un común siguen redondeadas, y el contrato las deja así (tema §8, paneles del sistema). `ShellWindowHost.present` no admite `contentViewController` ni cambiar el estilo antes de mostrar la ventana. `PeriodSheetPanel` y `windowSheet` (diálogos dentro de la ventana, como `SkinSheet` de nüün) son candidatos a `NuMacShell`, ya que dos apps los tienen. Ninguna de esas tres entra en este cambio. Kyū y SoundBender todavía no tienen columna en la tabla: la suma su sesión con lo que le falta. **SoundBender la adoptó el 2026-11-18** (`soundshop` `2ab211d`, core fijado en `91531f4`), según su fila de la tabla. 17 de sus 19 capturas salen idénticas byte a byte; las otras dos difieren por el foco, el icono del bundle de prueba o el estado de carga, nunca dentro de una barra de época. Conserva su puente propio, `MainWindowChrome`, porque a `WindowChromeBridge` le faltan tres cosas: el documento representado y editado (`representedURL`, `isDocumentEdited`), una opción para que cerrar la ventana termine la app, y poder leer la ventana (`WindowShellState.window` es interno). Conserva también su `setFrame`, que cubre la vuelta del toolbar al quitar `usesSquareCorners`. Con `drawsRowBackground: false`, la barra no dibuja la raya de acento de Terminal y SoundBender la superpone: puede valer un parámetro aparte. Las cuatro son propuestas, sin hacer.

La columna de Stalker se leyó en su código el 2026-11-18 (`stalker` `9777275`, core fijado en `bcd8d43`) y se actualizó con la build 9 (`d5c48d7`, core fijado en `2f67cc8`), comprobada en la app instalada por System Events; el detalle y el orden de los arreglos están en su [revisión de la suite](../../stalker/docs/2026-11-18-suite-y-prioridades.md#2-contra-macos-app-v1-qué-falta). Clipper y otras apps: sin revisar. Una app adopta el contrato añadiéndose aquí con lo que le falta.

## nu9ve-interaction-v1

[Teclado e interacción de las apps de nu9ve v1](../contracts/nu9ve-interaction-v1.md)

Leído en el código el 2026-11-07, sin probar a mano. Revisar al actualizar. La columna de Fractos se releyó el 2026-11-10 en la app de Mac de `ios-fractos` (`f1a7bcb`).

| Regla | curado | SoundShop | Clipper TUI | nüün | kyu | Fractos | nove · neuf | Stalker |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 Catálogo | Sí (`keyboard.ts`) | Sí, con prueba | Sí (`SHORTCUT_SPECS`), con prueba | No: atajos solo en el menú | Sin revisar | No (Mac): atajos en `AppCommands.swift` y sueltos en pantallas | Sin revisar | No: un atajo global (⌘⇧9, Carbon) y los del menú del clic derecho, sin catálogo |
| 2 Todo por teclado | Sí en la cabina | Parcial | Sí | Parcial: menús, ⌘1…⌘6, pasos de Ingreso | Parcial: cribado con 0–5, P/X/U, flechas | Parcial (Mac): menús, flechas en Trabajo y Calendario; las secciones van en ⌥⌘1… y ⌘1/⌘2 son filtros de Trabajo, no las vistas de la §8 | Sin revisar | Parcial: iniciar y detener con ⌘⇧9; el panel y Configuración sin revisar con teclado |
| 3 Badges y capas | Sí, con capa de Option | Sí en el footer; en los controles sin revisar | Barra de contexto (terminal) | No | Sin revisar | No | Sin revisar | No |
| 4 Ayuda contextual | Sí (footer) | Sí (barra de estado) | Barra de contexto | No | Sin revisar | No | Sin revisar | No |
| 5 Panel de atajos | Pestaña teclado | Sí (⌘/) | Sin revisar | No | Sin revisar | No | Sin revisar | No |
| 6 Remapeo | Sí | Sin revisar | Sin revisar | No | Sin revisar | No | Sin revisar | No: el atajo global es fijo (configurable está en su fase D) |

Una app adopta el contrato añadiéndose aquí con lo que le falta. Primeros pasos sugeridos:
- **nüün:** catálogo a partir de su menú, badges con capa de modificador, línea de ayuda en el footer del Ingreso y ⌘/. En Windows (maqueta, 2026-11-09) ya hay catálogo con pruebas de duplicados y de teclas del sistema, aceleradores y tooltips generados desde él, `AccessKey` en la navegación y panel Ctrl+/; es la referencia para portar el catálogo a la app de Mac.
- **kyu y neuf:** el recorrido completo por teclado de la §10, porque son las apps de edición más densas.

## nu9ve-controls-v1

[Controles por tipo de dato v1](../contracts/nu9ve-controls-v1.md)

Leído en el código el 2026-11-22, sin probar a mano, con los commits de [controles comunes §1](suite-ui.md#1-qué-se-leyó); el detalle por archivo está en su [§2](suite-ui.md#2-inventario-por-tipo-de-interacción). «—» es que la app no tiene ese tipo de dato. Ninguna app usa todavía controles del core: no existen.

| Regla | nüün | nove | curado | SoundBender | kyu | neuf | Stalker | Fractos (Mac) | Atlas |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2 Tema y skins | Sí | Sí | Sí | Sí | Sí: foto sobre gris neutro | No: estilos del sistema | — (sigue al sistema) | Sí | Parcial: colores propios en CSS |
| 3 Números | — | Parcial: valores exactos y `∅` sí; `NumberBox` y `Slider` sin las reglas de la §3.2 | Parcial: el fino va con ⇧ y pasa a ⌥ | Parcial: perilla sin flechas, Inicio ni rueda; deshacer sin revisar | Parcial: `ParamSpec` del motor y una entrada de deshacer sí; sin fino ni flechas propias | Básico | — | — | — |
| 4 Fechas y tiempo | Parcial: fecha en la zona de quien mira sí; rangos — | Sí: zona, bordes, horario del motor y huecos | Sin revisar | Parcial: regla de tiempo sí; código de tiempo sin revisar | Sin revisar | Parcial: código de tiempo y dos relojes sí; la barra de posición es la del sistema | Básico | Parcial: mes por teclado sí; rango no | Sin revisar |
| 5 Textos largos | — | Sin revisar | — | — | — | Parcial: sin búsqueda | — | Parcial: editor sí; sin ⌘F | — |
| 6 Listas y selección | Sí: dos vacíos | Sí: `DataTable` | Sí: problemas como filtros | Sin revisar | Sí | Sin revisar | — | Sin revisar | Parcial |
| 7 Mapas, imágenes y medios | Sí: Quick Look | Sí: lienzo sin red | Sí: forma de onda | Sí | Parcial: lienzo sí; teclas de zoom sin revisar | Sin revisar | — | Parcial: sin red sin revisar | No: Leaflet de un CDN |
| 8 Voz, seguimiento y gestos | — | — | Sí: seguimiento | Parcial: háptica | Parcial: pellizco | — | — | — | — |
| 9 Estados | Sí: vacíos y desconectado | Parcial: avisos en Windows; sin conexión sin revisar | Sí: faltan y sin conexión | Sin revisar | Sí: fuera de línea | Sin revisar | Parcial: «no disponible» | Parcial: vacío, aviso y cargando; sin conexión no | Sin revisar |
| 10 Accesibilidad | Parcial | Parcial | Parcial: la más anotada | Parcial: acción ajustable | No: 9 anotaciones | Parcial | No: 1 anotación | Parcial | No |

Clipper es una TUI: cumple la §5 (buscar en la transcripción) a su manera, y el resto no aplica. video no tiene interfaz interactiva.

## nu9ve-engine-v1

[Protocolo de un motor detrás de una app v1](../contracts/nu9ve-engine-v1.md)

Leído en el código el 2026-11-16: nüün `4f6b07c`, Clipper `41084ccb`, nove `9935227`. Los casos de motor de [`data/engine-examples-v1.json`](../data/engine-examples-v1.json) se corrieron ese día contra los binarios de nüün y Clipper; nove no, porque su petición todavía no es v1. Qué falta en detalle y en qué orden está en la [§12 del contrato](../contracts/nu9ve-engine-v1.md#12-diferencias-por-app-y-camino-a-v1).

| Regla | nüün (Python y `nuun-engine`) | Clipper (Python y `clipper-core`) | nove |
| --- | --- | --- | --- |
| 1 Transporte | Sí: líneas (`rpc-serve`) y una vez (`rpc`). Los dos motores descartan líneas de más con lectura acotada a 1 MiB. Rust usa el servidor de `nu9ve-engine` | Sí: líneas (`serve`), con `nu9ve-engine` en Rust | Sí: marcos por stdio, socket y pipe; una vez (`call`) |
| 2 Petición e id | Sí: el `id` se lee antes que `method` y sólo vale entero, cadena o `null`. Los dos motores devuelven ids de 350 dígitos exactos en ambos modos (Rust con `raw_value`, sin `arbitrary_precision`). `params` se omite o es un objeto | Sí | No: `request_id`, `protocol` |
| 3 Sobre | Sí en los dos modos. Python conserva `command` en los éxitos de `rpc` | Sí | Paso 1 implementado: los clientes leen el sobre viejo y v1. El motor emite el sobre viejo. |
| 4 `engine_info` | Sí en los dos: `contract`, `protocol` (`nuun-media/1`), `engine`, `version`, `capabilities`, `ported` y `native`, más sus `writes` | Los dos motores incluyen `contract`, `version`, `capabilities` y `queue_views`. Al cliente le falta validar `contract` | No: `hello` |
| 5 `not_ported` | Sí: Rust incluye `reason`, entre ellos `method_unavailable` y `platform_unavailable` | Sí | No aplica |
| 6 Grandes, progreso, escrituras | Progreso por sondeo; escrituras sólo en una vez, sin clave. Cancelar: SIGINT en Mac; **en Windows, un evento con nombre** (`NUUN_CANCEL_EVENT`, `n-creator-utils` `5fa3bf8`), que la §6.2 no nombra, y todavía sin plazo de gracia ni Job Object ([decisión abierta](windows-shared-next.md#2-decisión-abierta-cómo-se-cancela-un-trabajador-en-windows)) | Sí: `data` ya serializado, `request_id`, `write_status` | Parcial: claves e `idempotency_key` sí; una respuesta de más de 1 MiB se pierde |
| 7 Códigos | Sí: `invalid_request`, `invalid_params`, `unknown_method` antes que `read_only` e `internal_error` (antes `operation_failed`), en Python y Rust a la vez | Sí | Parcial: `invalid_json`, `frame_size` |
| 8 Ciclo de vida | Sí | Sí | Sí: `--parent-pid`, `engine_shutdown` |
| 9 Seguridad | Sí (stdio) | Sí (stdio) | Sí: socket `0600` con uid del par; pipe con DACL y PID |
| 10 Sombra | Sí: compara tipos y el registro guarda sólo rutas, códigos, `reason` y tiempos. Quita `not_ported` de `data` antes de comparar (respuesta parcial) | Sí | No aplica: sin oráculo |
| Casos de motor (§11) | 20 de 20 en ambos motores (`7e2f561` en `n-creator-utils`). Gate de macOS con Rust 1.99.1, 23 diferenciales y sombra sobre el catálogo del dorado sin diferencias nuevas. Windows queda en CI | 20 de 20 en los dos, en su CI | Sin medir |

**nove (2026-11-16).** Contrato y ejemplos fijados desde `nu9ve/core@3bab5d6` en `8a7efd6`; lectores de Swift, C#, host del broker y `pipe-call` en `83fd776`. Las pruebas de ambos sobres pasan en Rust y Swift en Mac. Desde `42719f6`, Clippy 1.89 pasa y `.github/workflows/engine.yml` corre el gate de Mac; **Pasos 2 y 3 hechos** (`6ceb214` a `8fb870f`): el motor responde v1 a quien manda `id` y el sobre viejo a quien manda `request_id`, y Swift, el cliente del broker y MCP, `pipe-call` y C# envían `id`. El motor pasa los 20 casos de los ejemplos en modo de marcos. `8fb870f` arregla en un commit aparte las tres pruebas Swift previas (esquema 27, `calendarDefaultZone` opcional) y el formato. El gate de Mac pasa entero con Rust 1.89: fmt, clippy, pruebas, build de release, replay offline, 29 pruebas Swift, `swift format lint --strict` y `sync-core.sh --check`. **En Windows (`39d8c77`, Rust 1.89 MSVC, sin instalar):** pasos 1 a 4 de su guía. Pasan `sync-core.sh --check` (tras fijar LF en la copia), Rust 229/229, Clippy, release, replay offline, los 20 casos del core y C# 72/72; corrigió el sondeo de `pipe::call` que rompía `broker_host`. Faltan la app por named pipe, `pipe-call`, MCP, el lector de Chrome, el segundo plano y la MSIX 0.12.0.53. El paso 4, retirar el sobre viejo, llega con la primera versión de nove que publique estos clientes con C# probado en Windows ([§12.3](../contracts/nu9ve-engine-v1.md#123-nove-transición-con-los-dos-sobres)).

**Cliente Swift `Nu9veEngine` (2026-11-22).** En el core, no en una app: [`swift/`](../swift/README.md#nu9veengine-cliente-de-nu9ve-engine-v1), con lo que cumple en la [§12.4](../contracts/nu9ve-engine-v1.md#124-cliente-de-swift-nu9veengine). Pasa los 20 casos de motor contra su motor de prueba y, con `NU9VE_ENGINE_LIVE_PROBE`, contra `clipper-core serve` de Clipper `910a7291` (release del 21 nov): 20 de 20, más `capabilities` y `project_summary` sobre un proyecto temporal. Las leases de la §6.4 se probaron sólo contra el motor de prueba; contra `clipper-core` piden un proyecto con su mapa publicado. Consumidores: ninguno todavía. Neuf, el primero previsto, lo fija con su `sync-core.sh` cuando Clipper cierre su migración.

## nu9ve-fs-v1

[Escritura duradera y acceso a archivos v1](../contracts/nu9ve-fs-v1.md)

| App | Implementación | Alcance |
| --- | --- | --- |
| nüün | `nu9ve-volumes::fs` usa `nu9ve-fs` desde la copia fijada | `Full`, creación exclusiva, reemplazo, rename exclusivo bajo el candado del escritor y lectura de verificación. El motor conserva sus errores y sus comprobaciones de archivos regulares. |
| Clipper | `rpc_writes/fsio.rs` y `rpc_jobs/store.rs` usan `nu9ve-fs` desde la copia fijada | Las escrituras que siguen a Python usan `Plain`. Usa candados exclusivos y compartidos, y sondea el candado del runner. La cola distingue el fallo anterior al rename del fallo de sincronización posterior. |
| Stalker | Swift, sin el crate: `SessionFiles` en `StalkerCore/MediaNaming.swift` (leído el 2026-11-18, `9777275`) | Temporal hermano `.<nombre>.<UUID>.tmp` creado sin sobrescribir, `F_FULLFSYNC` con caída a `fsync`, rename y fsync de la carpeta con su error propagado; rename exclusivo con `renamex_np(RENAME_EXCL)` y conflicto si existe. Difiere: el temporal no nace en modo `0600` y el error posterior al rename no se distingue del anterior. Los presets usan `Data.write(.atomic)` sin sincronizar. |
| curado | La exportación XML de rekordbox (`src-tauri/src/rekordbox_xml.rs`) usa `nu9ve_fs::atomic_write` desde la copia fijada (`2fcc0c0`, ADR-035) | `Full`. Mensajes distintos para el fallo anterior y el posterior al rename. `vendor` queda fuera de su workspace de Cargo. |

## nu9ve-json-v1

[JSON canónico, digest, ids y sobre de errores v1](../contracts/nu9ve-json-v1.md)

| App | Implementación | Prueba los vectores | Nota |
| --- | --- | --- | --- |
| nüün | Python `media/common.py` (oráculo) y Rust `nu9ve-json` | Rust: `engine/nuun-engine/tests/json_vectors.rs`, con el crate y los vectores de su copia fijada | El gate compara además el motor con el dorado de Python (`scripts/engine_golden.py --check` y 23 diferenciales en macOS) |
| Clipper | Rust `nu9ve-json` desde su copia fijada, para el sobre de `clipper-engine/1`, `sha256_hex` y `uuid4` (desde el 2026-11-16); su JSON propio sigue en `py.rs`/`pyval.rs` | No todavía: el canónico no lo usa | Compila `serde_json` con `arbitrary_precision` y `preserve_order` (§8) |
| nove | Sobre `{code, message}` propio | — | Ola 3 |

## nu9ve-loudness

[`rust/nu9ve-loudness`](../rust/nu9ve-loudness/), con sus vectores en [data/audio/](../data/audio/loudness-vectors-v1.json). Es una pieza sin contrato: la regla es BS.1770-4 y EBU Tech 3341 y 3342, y los vectores dicen qué número debe salir ([audio §5](platforms/audio.md#5-curado-y-soundbender-pieza-por-pieza)).

| App | Implementación | Prueba los vectores | Nota |
| --- | --- | --- | --- |
| curado | Propia: `crates/audio-engine/src/loudness.rs` (sólo integrado, ponderación K con biquads RBJ) y el detector de true peak de `limiter.rs`, que es el origen del del crate | No | **Adoptarla mueve sus niveles entre 0,04 y 0,3 LU** (hoy lee bajo; con los 23 dorados, entre 0,04 y 0,29 LU menos). Su CHANGELOG debe decirlo al adoptarla. El true peak del crate es el suyo, con la misma aritmética |
| SoundBender | **Fija el crate** (`soundshop` `9eb4ceb` y `3df4dfb`, core `d3fed8e`); su `loudness.rs`, origen del LUFS del crate, se borró. En Swift sigue `LoudnessMeter`, cada 100 ms y solo a 48 kHz | Con sus 23 dorados: integrado a 5e-5 LU o menos del manifiesto. No fija `data/audio` | Ningún dorado dura 3 s, así que el máximo de corto plazo (aquí cada 10 ms) no está comparado con Swift. El true peak aún no lo usa: lo que falta para el medidor y el limitador está en su `portable-engine.md`. **Más lento que Swift en `d3fed8e`:** 95 ms frente a 64 de `LoudnessMeter` en 3 min estéreo a 48 kHz (su cuarta medición, 2026-11-18). Medido por partes, no era la rejilla de 10 ms sino los filtros, un canal tras otro. El crate de hoy tarda la mitad con los mismos bits ([CHANGELOG](../CHANGELOG.md), 2026-11-18, `f332b48`). SoundBender lo tomó al fijar `bcd8d43` y repitió su medición: 35 ms (`41ab675`, `16ac1c8`, su quinta medición) |
| Clipper | **Fija el crate** desde el 2026-11-22 (`clipper` `0bdd1f74`, core `08744e3`): su motor mide y decide el mastering del master, y la canción de la intro, con `nu9ve-loudness`. `master_audio.py` (FFmpeg `ebur128`) queda en el Python que no se retira | No | Consumidor de medidas, no implementación ([plan](suite-adoption.md)) |

## nu9ve-midi

| App | Implementación | Verificación |
| --- | --- | --- |
| curado | **Fija el crate** (`curado` `8fb7844`, core `1114e02`): `crates/midi` depende de `nu9ve-midi` por la copia fijada, que trae también el perfil físico `data/midi/pioneer-ddj-sb-v1.json`; el preset de `crates/midi/presets/pioneer-ddj-sb.json` pasa a sus IDs. | La extracción reproduce los 18 gestos de la captura CoreMIDI. Falta que el usuario pruebe la build con el DDJ-SB (faders, EQ, jogs, pads, desconectar y reconectar); después, escala del jog, LEDs y soft takeover. |

## nu9ve-jobs

[`rust/nu9ve-jobs`](../rust/nu9ve-jobs/) y `Engine/OneShotWorker` de [`Nu9ve.Core`](../windows/README.md), desde el 2026-11-26. Sin contrato propio: siguen [motor §6.2](../contracts/nu9ve-engine-v1.md#62-progreso-y-cancelación). Nadie los usa todavía.

| App | Qué usaría | Estado |
| --- | --- | --- |
| Clipper | Todo `nu9ve-jobs`: es la app de origen (`core/src/rpc_jobs/jobkit/`) | Sin empezar: fijar el crate y borrar `jobkit`. Sus tres ganchos del export (escala del progreso de R7e, relevo de R7h y `sink` de R7j) se rehacen sobre `Transform` y `Sink`; su `supervise` no cambia de forma |
| nüün | `OneShotWorker` en su cliente de Windows, con `NUUN_CANCEL_FILE` | Sin empezar. Antes de su paso 4 de Windows: cambiar `RunAsync`/`CallCancellableAsync` de `RpcConnection.cs` por `OneShotWorker` y probarlo en el PC |
| curado | `nu9ve-jobs` para Demucs (`stems.rs`) | Sin empezar |

## nu9ve-py y nu9ve-time

[`rust/nu9ve-py`](../rust/nu9ve-py/) y [`rust/nu9ve-time`](../rust/nu9ve-time/), desde el 2026-11-26, con los vectores de CPython 3.14 de [`data/py-vectors-v1.json`](../data/py-vectors-v1.json). Nadie los usa todavía.

Los vectores encontraron cinco diferencias con Python en el código que se mudó, ya corregidas aquí. **Las copias de origen las tienen todavía:**

- `str.title()`: la sigma final (`"ΣΑΣ"` → `"Σας"`, el código daba `"Σασ"`), las letras en titlecase como `ǅ`, que Python cuenta como letras con caso, y las ligaduras (`ﬁ` → `Fi`), con una tabla de titlecase sacada de Python junto a las de Unicode 15.0.
- `repr(str)` de un carácter no imprimible fuera de ASCII (`\xa0`, `\u200b`): Python lo escapa y el código lo copiaba; ahora es `None`.
- `PurePosixPath('') / 'x'` es `x` (daba `/x`), `PurePosixPath('/') / '..'` es `/..` (daba `//..`) y `PurePosixPath('.').name` es `""` (daba `.`).

| App | Qué toma | Estado |
| --- | --- | --- |
| Clipper | `nu9ve-py` en lugar de `py.rs`, `pynum.rs`, `pyval.rs` (salvo sus constantes), `paths.rs` y la parte de Unicode 15.0 de `unicode.rs`. Sus dos tipos de número (`P` y `Num`) son uno, `Num`, con operaciones `checked_*` que devuelven `None` fuera de ±2**53 | Sin empezar. Se quedan en Clipper: `even` del layout, `round3_repr` y las tablas de Unicode 16.0 de sus transcripts, `numpy_rng`, `tzif` y `civil_datetime`. Al adoptar, sus dorados dicen si alguna de las cinco correcciones cambia una respuesta |
| nüün | `nu9ve-time` en lugar de `engine/crates/nu9ve-time` | Sin empezar. Cambia la API: `wall` es ahora `jiff::civil::DateTime` (alias `Naive`) y ya no se reexporta `chrono`. Los sitios que tocan `.wall` (`nuun-media/src/layout.rs` y `missions.rs`) usan `micros()` y `whole_seconds()` en lugar de `timestamp_subsec_micros` y `with_nanosecond(0)`. `now_utc` (69 llamadas) no cambia. Su dorado contra Python debe seguir sin diferencias |

## nu9ve-mcp-v1

[Servidores MCP de las apps de nu9ve v1](../contracts/nu9ve-mcp-v1.md)

nove y nüün leídos en su código el 2026-11-09. Las demás apps no están clonadas en esta máquina: sin revisar.

| Regla | nove 0.12 (Windows) | nüün (`media mcp`) | curado | Clipper · kyu · Stalker · neuf · SoundShop · Atlas |
| --- | --- | --- | --- | --- |
| 0 Local stdio, sin HTTP | Sí | Sí | No tiene servidor (revisado: sin `mcp` en el código) | Sin revisar |
| 0 Cliente del motor en ejecución | Sí: named pipe, usuario y PID verificados | **No:** abre el catálogo con `Service` en su proceso | — | Sin revisar |
| 1 Protocolo y errores | Sí; versiones hasta 2025-06-18 | Sí; versiones hasta 2025-11-25; `serverInfo` `ncreator-media` 0.2.0, no el nombre y la versión de la app | — | Sin revisar |
| 2 Niveles | Leer, Mostrar, Editar, Actuar | Sólo Leer (7 herramientas: biblioteca, recurso, jobs, job, volúmenes, almacenamiento) | — | Sin revisar |
| 3 Permiso por sesión y registro | Sí (datos 22, 60 min, deshacer) | No aplica todavía | — | Sin revisar |
| 4 `_untrusted` e instrucciones | Sí | Parcial: instrucciones de una línea; nombres de archivo y proyecto sin marcar como datos | — | Sin revisar |
| 5 Registro e instalación | Alias MSIX `nove-mcp.exe`; Mac responde «sólo Windows» | `nu9ve media mcp` desde el checkout; sin alias ni ruta en el bundle | — | Sin revisar |
| 6 Validación | Motor real + MCP en `tests/mcp.rs`; **cliente real comprobado el 2026-11-09**: esta sesión de Claude Code llamó `nove_status` y respondió (motor en segundo plano, esquema 24) | `test_mcp_is_read_only_and_does_not_invoke_mutations` (en proceso); sin cliente real registrado | — | Sin revisar |

Primeros pasos sugeridos:

- **nüün:** que el MCP pida al lector persistente (`snapshot`) en lugar de abrir el catálogo, `serverInfo` `nuun` con la versión de la app, campos `_untrusted` para nombres de archivo y proyecto, y `show_in_app` para abrir un proyecto o un plan revisado en la ventana. Escribir `docs/mcp.md`.
- **nove:** ~~servidor también en Mac~~: desde el 2026-11-18 `nove mcp` habla con el motor por el socket Unix y confirma con un diálogo del sistema (`nove` `daea92b`), y desde el 2026-11-20 el motor publica las mismas herramientas a `nu9ve mcp` (`c7d223e`, su A55). Falta probarlo con un cliente real en la Mac. La tabla de arriba sigue siendo la lectura de Windows del 2026-11-09.
- **curado, kyu, Clipper:** empezar por Leer (biblioteca, proyectos, estado) y Mostrar; Editar sólo con registro deshacible.

## nu9ve-registry-v1

[Registro de motores v1](../contracts/nu9ve-registry-v1.md)

nove es el primero que se registra (2026-11-20). Las demás necesitan antes:

| App | Transporte que podría registrar | Falta |
| --- | --- | --- |
| nove | Socket Unix `runtime/ipc` (Mac) y pipe (Windows), ya con las comprobaciones de [motor §9](../contracts/nu9ve-engine-v1.md#9-seguridad-sólo-local-sólo-el-usuario) | **Versión menor del 2026-11-25:** su motor escribe `profile_id` cuando arranca con `--profile` (A98, build 34). **En Mac, hecho** (`nove` `d430c04`, su A53): escribe y borra `nove.json` con `nu9ve-ipc` de su copia fijada, sólo con `--suite-registry`, que la app pasa desde su propio bundle y nunca en QA, demos ni pruebas. Anuncia `data_catalog_v1` y `mcp_tools_v1`. Sin probar con la app instalada. En Windows falta, como en `nu9ve-ipc` |
| nüün | Socket Unix en marcos, mientras corre `rpc-serve`, con las reglas de [motor §9](../contracts/nu9ve-engine-v1.md#9-seguridad-sólo-local-sólo-el-usuario) | **En Mac, hecho** (`n-creator-utils` `0c21510`, 2026-11-23): se registra como `nuun` (`nuun-suite/1`) y sirve `data_catalog` y `data_export`; bajo QA sólo con `NU9VE_ENGINES_DIR`, y el registro se borra al cerrar. Desde `f088343` (core `f267022`) el registro lleva `library` y `profile_id`. Aceptado en ventana en un sandbox (`45b47fd`). En Windows falta el named pipe |
| Clipper | Ninguno: `serve` es stdio | Igual que nüün |

## nu9ve-data-v1

[Catálogo de datos v1](../contracts/nu9ve-data-v1.md)

nove lo sirve desde el 2026-11-20 y nüün desde el 2026-11-23.

| App | Qué serviría | Estado |
| --- | --- | --- |
| nove | Proyectos, fuentes y datasets del [modelo de datos v1](../../nove/docs/contracts/data-model-v1.md) | **Lo sirve** (`d430c04` y `036315d`, sus A53 y A56): el registro de datasets (incluidos Atlas, Clipper y los rásters en sitio), las series de módulos (pares OTC, mercados, seguimientos web) con ids estables y las conversaciones de agentes como `personal`, sólo con su forma. Probado con Codex CLI y registro aislado en la M1 ([QA](../../nove/docs/mcp.md#nu9ve-mcp-con-un-cliente-real-2026-11-23-la-m1)). Los datos reales se leyeron en una biblioteca aislada |
| nüün | Proyectos de media y colecciones (`~/.claude`, `~/.codex`) como modalidad `files`, `personal` | **Lo sirve** por su socket registrable (`0c21510`): el catálogo local, el último índice publicado de cada disco con raíces por `volume_id`, y las colecciones vivas. Pasa `data-examples.py --server` 11/11. `nu9ve agent publish` escribió `catalog/nuun.json` en QA. Faltan claves de proyecto legibles (hoy un UUID) y la colección Agentes |
| Clipper | Proyectos y transcripts | Sin empezar |

Versión menor del 2026-11-22 ([data §9](../contracts/nu9ve-data-v1.md#9-decisiones), decisión 3): raíces con `volume_id` y `volume_path`, y la clave conocida `conversaciones`. Los validadores de referencia (`scripts/data-examples.py`) y de `nu9ve-mcp` ya los aceptan; ninguna app los usa todavía.

Uso de agentes por cuenta: [decidido el 2026-11-26](agent-usage-next.md#decisión-2026-11-26). Vive en nove hasta que haya un segundo lector. La cuenta sale de los campos de cada sesión (Claude Code `ownerAccountUuid`, Codex `creator_account_id`) como clave opaca, sin declarar nada por proyecto. Le toca a nove: leerlos, el turno nativo con `turn_basis` y los tokens de Codex por diferencia.

## nu9ve-music

[`rust/nu9ve-music`](../rust/nu9ve-music/), desde el 2026-11-22. Nadie lo usa todavía.

| App | Qué usaría | Estado |
| --- | --- | --- |
| curado | Todo: es la app de origen (`audio-engine/src/beat.rs` y `music.rs`) | Sin empezar: cambiar sus dos archivos por el crate, en un cambio propio, con su `ANALYSIS_VERSION` igual (3) para no volver a analizar la biblioteca |
| SoundBender | BPM, primer beat y clave, en su motor en Rust, en lugar de `TempoAnalysis` y `ChromaAnalysis` de Swift | Sin empezar. Su análisis da otros números ([comparación](../rust/nu9ve-music/README.md#de-dónde-sale)): conviene volver a analizar al adoptarlo |
| Clipper | El análisis de intros, hoy en Python | Sin empezar |

## nu9ve-agent-v1

[Agente de la suite v1](../contracts/nu9ve-agent-v1.md), propuesta del 2026-11-22 (F1). El agente está completo en el core desde U7c; nüün lo trae en su bundle desde el 2026-11-23.

| App | Papel | Estado |
| --- | --- | --- |
| core | El agente: `nu9ve agent` en el binario `nu9ve` (U7b y U7c) | **Hecho en QA** (2026-11-22, [órdenes](../rust/nu9ve-mcp/README.md#nu9ve-agent)): identidad y clave en el llavero, carpeta, publicar, leer y emparejar (U7b); `serve` con un solo agente por máquina, kqueue y cierre ordenado; `install`, `uninstall` y `state` con la tabla de §3 y el LaunchAgent `xyz.nu9ve.agent`; el motor `suite` con sus seis herramientas (U7c). Probado con dos máquinas de QA y con `nu9ve mcp`. **Instalado en la M1** (2026-11-22, por decisión del usuario): `nu9ve agent install` desde `61e8c49`, con el LaunchAgent `xyz.nu9ve.agent` cargado, la clave en el llavero y el `device.json` «M1» que ya había creado nüün (mismo `device_id`, huella `87e6 7b6a 0478 68e2 …`). Comprobado: `state` da `running`, el motor `suite` está en `engines/` y `nu9ve mcp` lista sus cuatro herramientas de lectura; launchd lo reinicia tras un `kill -9` y no tras un `SIGTERM` (salida 0, registro borrado). Carpeta de intercambio `~/nu9ve`. **Simulado con una M4 de QA** contra ese agente real: todos los flujos de F1, y después se deshizo todo (CHANGELOG, 0.2.1). **Cada actualización pide permiso del llavero** mientras el binario no esté firmado. **Sin verificar:** Windows entero (la tarea programada, el llavero, el pipe del motor y el cierre sin `SIGTERM`), y la etiqueta `xyz.nu9ve.agent` contra el identificador de firma (§8.3) |
| nüün | Primera app que lo trae y lo ofrece en Ajustes (decisión del usuario) | **Hecho en Mac** (`n-creator-utils` `9930fd5` y `680554f`, 2026-11-23, core `1d11d30`): el bundle lleva `Resources/backend/nu9ve`; al arrancar y al abrir Ajustes llama `nu9ve agent state`, y Ajustes → Suite ofrece «Instalar el agente», «Actualizar» (`install --by nuun`, sólo al pulsarlo) o «Arrancar»; nunca `serve`. Con el agente instalado ya no crea `device.json`. Aceptado en ventana en un sandbox con un agente de QA (`45b47fd`), sin tocar el agente de la M1. Falta Windows |
| nove, Clipper y las demás | Comprobar al arrancar y ofrecerlo; usar sus capacidades | Sin empezar. Con `nu9ve agent state` (o `nu9ve_mcp::agent::install::state_of` en Rust) y los métodos del motor `suite` (`devices`, `where`…) |

## nu9ve-exchange-v1

[Carpeta de intercambio v1](../contracts/nu9ve-exchange-v1.md), propuesta del 2026-11-22 (F1), con la versión menor de U7c (§7.1 y `not_found`). La escribe y la lee sólo el agente; las apps la ven a través de él.

| App | Qué aporta o usa | Estado |
| --- | --- | --- |
| core | El agente que escribe y lee (U7b y U7c) | **Completo en QA** (2026-11-22): los siete `kind`, cada uno con su firma y su estado; lo personal va con su forma salvo lo compartido; pedidos de hasta 64 MiB con `too_large`, `not_found` y `not_shared` (versión menor del 2026-11-22, con [exportar](#nu9ve-export-v1)); dueñas entregadas sin relojes (§7.1); `apps/<app>/` con `.json` y `.ndjson` línea a línea. Probado con dos máquinas de QA en una carpeta temporal. **Sin verificar:** con Syncthing entre la M1 y la M4 reales; el canal propio entre agentes no existe todavía |
| nüün | Su catálogo (necesita un transporte registrable, [registro](#nu9ve-registry-v1)), su diario de decisiones en `apps/nuun/` con `app_append`, y «Trabajar aquí» con `lease_take` | **Catálogo y «Trabajar aquí» hechos en QA** (`0c21510`, `918a53f`): el agente publica su catálogo, y en Proyectos el menú ofrece «Trabajar aquí» o «Dejar de trabajar aquí» con `lease_take` y `lease_release` (`tests/suite_lease.rs` con `nu9ve agent serve` real). Colecciones nuevas por dispositivo, `<colección>/<device_id>/…` (`8ae733a`). Falta el diario de decisiones con `app_append`, la lease en sólo lectura al abrir un proyecto y el intercambio real entre la M1 y la M4 |
| nove | «Otras máquinas» (`devices`, `where`), «disponible en todas mis máquinas» (`share` y `offer`) y el proyecto `conversaciones` | «Otras máquinas» y «Mis conversaciones» en desarrollo (A74–A75), verificados con un agente ficticio. Pendientes el intercambio real M1/M4 y compartir desde la app. [QA](../../nove/docs/STATUS.md#suite-varias-máquinas-2026-11-23) |

## nu9ve-export-v1

[Exportar un dataset v1](../contracts/nu9ve-export-v1.md), propuesta del 2026-11-22 (F1). Lo llama sólo el agente, para responder el pedido de un dataset de otra máquina.

| App | Qué aporta o usa | Estado |
| --- | --- | --- |
| core | El agente que lo pide al motor dueño (§3), `not_shared` para lo personal sin compartir, y `shared`, `share` y `unshare` en el motor `suite` (§5) | **Hecho en QA** (2026-11-22, `nu9ve` 0.2.2): probado con un motor falso que sirve `data_catalog` y `data_export` y dos máquinas de QA, también por `nu9ve mcp` (`tests/agent.rs`, `tests/suite_engine.rs`). **Sin verificar:** con un motor real de una app y entre la M1 y la M4 reales |
| nüün | `data_export` de lo pequeño que procesa (resúmenes de una tanda), y `too_large` con el disco para el raw | **Sirve el método** (`0c21510`): por ahora sólo `too_large`, con disco y ruta, o `not_found`, nunca bytes. Pasa `export-examples.py --server` 8/8. Falta entregar lo pequeño y probar un pedido entre Macs reales |
| nove | `data_export` de sus tablas y capas (GeoJSON de baches, series), y «disponible en todas mis máquinas» con `share`, `unshare` y `shared`, ofreciendo sola lo compartido (§5) | `data_export` implementado (A75), 8/8 ejemplos contra el motor aislado. Pendiente compartir desde la app y comprobar un pedido entre Macs reales. [QA](../../nove/docs/STATUS.md#suite-varias-máquinas-2026-11-23) |
| Clipper | `data_export` de sus transcripts | Sin empezar |

## nu9ve-action-v1

[Acciones largas y su progreso v1](../contracts/nu9ve-action-v1.md), propuesta del 2026-11-26, desde la [propuesta de nüün](actions-next.md). Nadie lo cumple todavía; los dos productores de hoy escriben casi la misma forma.

| App | Qué aporta o usa | Estado |
| --- | --- | --- |
| core | La vista común: `Nu9veAction` y `Nu9veActionProgress` en `swift/` (2026-11-26) | Hecho: el modelo pasa los ejemplos del contrato; las vistas, en los cinco skins, sin probar a mano en una app |
| nüün | Productor (`ActionMeter` en Python, `Meter`/`ActionProgress` en Rust) y la vista de origen (`ActionProgress.swift`), que cambia por `Nu9veActionProgress` | Sin empezar. Le falta, según la [§7](../contracts/nu9ve-action-v1.md#7-cómo-llegan-las-apps): `contract`, `progress_id` dado por quien lanza, `action`, `state`, `unit`, `cancellable`, los tres instantes con el latido, etapas comunes o `nuun.*`, **conservar el registro al terminar** (hoy `Drop` lo borra) y el cierre. Después, el respaldo de conversaciones y la revisión de «Copiar al otro disco» con el mismo registro, y la vista en tres tamaños |
| Clipper | Productor: `ProgressPlan` con pesos (`unit` `work`), Stage y Step (`phase` y `step`) | Sin empezar: `action_progress` en `clipper-core serve` leyendo el archivo de progreso de cada trabajo. Sus reglas de ETA ya son las del contrato |
| nove | Productor para sus trabajos largos | Sin empezar. `events` sigue para su historial |


[Perfiles en esta máquina v1](../contracts/nu9ve-profile-v1.md), propuesta del 2026-11-25 (la pieza local de los perfiles de [suite-devices §5](suite-devices.md#5-modelo)).

| App | Papel | Estado |
| --- | --- | --- |
| nove | Primer escritor: registra su biblioteca personal como `e64` y cada perfil nuevo como una biblioteca nueva; su motor lee el archivo (`profile_info`, `profiles_overview`) y dice su perfil en el registro de motores | Integrado en la build 34 (A98): fija `nu9ve-profiles`. Falta el recorrido de perfiles en la app instalada |
| nüün | Lee el archivo para saber qué perfiles hay en la máquina y en qué apps; agrega su entrada (`nuun`) a sus perfiles | **Hecho en QA** (`n-creator-utils` `25a356b`, core `f267022`): su motor fija el crate (`suite_profiles`, `suite_profiles_register`) y Ajustes → Perfiles muestra «Perfiles en esta Mac». Probado con raíces aisladas. Falta registrar los perfiles reales, cuando el usuario lo pida, y aceptar la vista en ventana |
| Atlas, Clipper | Leen; agregan su entrada si trabajan por perfil | Sin empezar |

## nu9ve-device-v1

[Identidad de dispositivo v1](../contracts/nu9ve-device-v1.md), propuesta del 2026-11-22 (F0 de [varias máquinas](suite-devices.md)). nüün lo crea desde el 2026-11-22.

| App | Papel | Estado |
| --- | --- | --- |
| nüün | Lo crea al registrar el primer disco, hasta que exista el agente `nu9ve` | **Creado y nombrado en la M1** (2026-11-22, `n-creator-utils` `c85b881` y `0212b02`): `device.json` con `0600` y `host_check`; la persona lo llamó «M1» (antes «Mac 5577», mismo `device_id`). Ajustes → Discos muestra y cambia el nombre, y lo lee sin crear el archivo (`suite_device` con `peek`). Pasa la referencia del core. **Desde `680554f`** (2026-11-23), con el agente instalado sólo lee y renombra el archivo del agente: no lo crea ni reemplaza uno copiado. Falta crearlo en la M4 |
| nove, Atlas, Clipper | Lo leen para decir en qué máquina vieron un disco | Sin empezar |

## nu9ve-volume-v1

[Identidad de volumen v1](../contracts/nu9ve-volume-v1.md), propuesta del 2026-11-22 (F0). Cierra A3 cuando nüün la implemente.

| App | Papel | Estado |
| --- | --- | --- |
| nüün | Registra discos (único con rol `archive`), observa, escribe su carpeta con recibos, manifiestos, snapshot e índice, y adopta un disco en otra Mac. Muda `nu9ve-volumes` al core con este contrato | **F0 en la M1** (2026-11-22; `n-creator-utils` `c85b881`, `0212b02` y `7d7eb7b`, core fijado en `9711564`; su `docs/contracts/suite-volume-f0.md`). MEDIA502 registrado como `archive` (exFAT) y ESSD como `work` (HFS+), cada uno con marcador e índice en `.nu9ve/apps/nuun/` (7 666 y 1 960 datasets). **Aceptado en ventana:** «Adoptar este disco…» con los dos discos en un entorno con catálogo vacío; snapshot de MEDIA502 verificado; hashes y fechas iguales antes y después. **`nu9ve-volumes` mudado** al core (U2): nüün borró su copia y toma del módulo `marker` las reglas del marcador y de los estados. **Tope del índice, decidido por el usuario y hecho** (`01b4c20`): un dataset por proyectos × carpeta de primer nivel, con hash de manifiesto; MEDIA502 baja de 5,1 MiB a 20 KB (7 666 archivos en 27 grupos) y ESSD a 2,7 KB. Pasan `index_problem` de la referencia. No cambia el contrato (§13.4 se puede cerrar). **Falta:** prueba en la M4 y cotejo de MEDIA502 en Windows, con pasos en su STATUS |
| Atlas | **Sólo lo urgente** (decisión del usuario, 2026-11-22): raíces por volumen en `config/atlas.json`, para que deje de crecer atado a una ruta; su carpeta en los discos de trabajo (ESSD, XSSD). Sus flujos pasan a nove | Sin empezar. Hoy usa `/Volumes/ESSD/…` |
| nove | Raíces por volumen; «disco no conectado»; su carpeta en los discos de trabajo (excepción a su modelo §9, que decide su sesión). **F0 (A70 y A76)** | Lectura de marcadores reales en ESSD y MEDIA502 sin escritura. Escribir índice y adoptar comprobados con dos bibliotecas y un disco de QA. [Estado](../../nove/docs/STATUS.md#suite-varias-máquinas-2026-11-23) |
| Clipper | Su carpeta en los discos de trabajo | Sin empezar |

## nu9ve-artifact-v1

[Artefactos de datos v1](../contracts/nu9ve-artifact-v1.md)

nove los produce desde el 2026-11-20 y Clipper los consume.

| App | Papel | Estado |
| --- | --- | --- |
| nove | Productor: gráficas, mapas y comparaciones de «Datos» y de los benchmarks (S10) | **`artifact_export`** (`2f5468d`, core fijado en `39c1b9b`, su A57): una revisión de un dataset tabular o las velas de un par OTC. El motor dibuja el PNG sin dependencias nuevas, guarda todos los puntos en `drawn.csv` y valida con `nu9ve_artifact::open_package` antes de publicar. Lo `personal` pide `release` en un diálogo de la Mac y nunca sale por MCP. Sin mapas ni animación. Falta un recorrido nove → Clipper con un paquete real. **Salidas (A95, en integración):** exporta lienzos con PDF y `documents` (Markdown, DOCX) de la versión menor del 2026-11-24 |
| Clipper | Consumidor: overlay en la timeline (póster como imagen, cuadros como video) | `clipper overlay import-artifact` (`clipper` `0e3056f2`, core fijado en `699ccaf`): comprueba en Rust con `nu9ve-artifact` (`artifact_inspect`), póster en `data.chart`, cuadros a ProRes 4444 con `--animated`, procedencia en la escena, `personal` sólo con `--allow-personal`. Visto en un proyecto de QA con `scene-frame`; sin export completo ni TUI. Las animaciones declaradas se rechazan |
| `ai/video` | Consumidor: cuadros o animación declarada a video | Sin empezar |
| neuf | Consumidor en su proyecto | Sin empezar |

## nu9ve-bench-v1

[Registro de benchmarks v1](../contracts/nu9ve-bench-v1.md), con `rust/nu9ve-bench`, `scripts/nu9ve_bench.py` y `scripts/bench-from-atlas.py`.

Clipper escribe en el registro desde el 2026-11-21.

**Versión menor del 2026-11-25 (`resources` y `soak`, §5.1 y §5.2):** cada app suma a su gate un soak de su operación más repetida (peticiones al motor, lecturas de proyecto) con un límite de pendiente de memoria, para detectar fugas o cambios raros; nove las grafica. Lo que tiene cada app hoy y lo que le toca:

| App | Qué mide hoy | Qué le toca |
| --- | --- | --- |
| Clipper | **Adoptado** (2026-11-21): fija contrato, crate, ayudante y ejemplos en `6284877` (`ad80ef2a`). `scripts/core_bench.py` (`cdb46ebe`) mide la suite dorada con los dos motores con el ayudante de Python: suite `clipper-engine`, casos `golden/<dominio>` y `compose/score`, entradas por SHA-256 de `reads.json` y del módulo de casos. Línea base de M0 en `docs/benchmarks/2026-11-21-m0-engine-baseline.{md,ndjson}` (`835f4fc5`). Benchmark de M9 en `docs/benchmarks/2026-11-23-m9-engine.{md,ndjson}` (`4caa4786`) | Flujos del TUI (`tui_flow_bench.py`) al registro, petición en caliente, caché fría y exports por rango; repetir el benchmark después de R7, que movió el export al motor |
| nüün | `scripts/engine_bench.py` (`25a356b`) escribe `bench/nuun.ndjson` con el ayudante del core: seis lecturas del dorado en Python y Rust con `response_equal`, el recorrido de un árbol de fixture y un soak de 2 000 lecturas del catálogo a un `rpc-serve` vivo (`--check-soak` para el gate) | Llevar el soak al gate y sumar las copias en GiB/s |
| Atlas | `atlas_vision/engines/bench.py`: corridas en `derived/bench/` y líneas base en `config/bench.json` | Traducido en sólo lectura por `scripts/bench-from-atlas.py` y, desde el 2026-11-24, por `nu9ve_bench::atlas` en Rust, con los mismos `run_id`; los hitos de `analysis/history/evolucion.json` entran como corridas de sólo calidad (versión menor) y `derived/bench/*.ndjson` se lee como registro. Le toca escribir el formato nuevo en su sesión |
| nove | Mediciones del worker tabular, Parquet y el `measure` de `nu9ve-algo` | S10 (A95, en integración): fuentes de sólo lectura (registros y repos de Atlas con `nu9ve_bench::atlas`), tablas `bench:runs` y `bench:quality`, comparación y unión con sesiones de agentes. **A96 (en integración):** `scripts/gate-bench.py` escribe `nove-gate` (duraciones del gate, pruebas, tamaños) y `nove-engine` (arranque, memoria en reposo y un soak de 2 000 peticiones con su pendiente) en `bench/nove.ndjson` |
| Clipper | Ver arriba | Un soak del motor (`measure_soak` con el `pid` del motor) en su gate, con `resources` |
| curado, SoundBender | Dorados de paridad y de LUFS | Traductores por escribir; su calidad es `bytes_equal` o un máximo absoluto con tolerancia |

## nu9ve-notifications-v1

[Avisos del sistema de las apps de nu9ve v1](../contracts/nu9ve-notifications-v1.md)

Leído en el código el 2026-11-10; nadie probó un aviso desde esta sesión.

| Regla | nove (Windows) | nove (Mac) | nüün (Mac) | nüün (Windows) | curado | Fractos Mac | Stalker |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 Cola en el motor | Sí (`deliveries.rs`) | No: publica la app | No: publica la app | No | No | No | No: publica la app |
| 2 Catálogo de tipos | Implícito en el motor (web, OTC, prueba) | Implícito | Un tipo: tarjeta detectada. Falta fin de trabajo | No | No | No | Un tipo: sesión guardada (o guardada con problemas) |
| 3 Por defecto y control | Sí: «todo salvo excepciones» (A36), prueba de canal | Opt-in por vigilancia | Sin ajustes de avisos | No | No | No | Sin ajustes de avisos; permiso pedido al primer guardado |
| 4 Contenido | Sí: tres líneas, botón «Ver en el Buzón», argumentos validados | Parcial | Parcial: título y recomendación; sin botones | No | No | No | Parcial: título y nombre de la carpeta con detalle; sin botones |
| 5 Identidad y límites | Sí: etiqueta y grupo, 30/h, 12 h, reintentos | Identificador por aviso | Identificador por tarjeta; sin límite | No | No | No | Identificador por aviso; sin límite (uno por sesión) |
| 6 Urgencia | Sí: recordatorio para señales de estrategia | No | No | No | No | No | No |
| 7 Sonido | Del sistema | Del sistema (pide `.sound`) | Silencioso (sólo pide `.alert`) | No | No | No | Silencioso (sólo pide `.alert`) |
| 8 Adaptador | Sí: rol `--deliver-toast`, activación COM | En la app | En la app; el clic abre Ingreso | No | No | No | En la app; el clic abre la carpeta de la sesión en Finder; protegido sin bundle real |
| 9 Validación | Automática sí; clic físico pendiente | Con ayudante ficticio; aceptación del sistema pendiente | Sin pruebas vistas | — | — | — | Sin pruebas |

Primeros pasos sugeridos:

- **nüün (Mac):** avisos de trabajo terminado y fallido con la app en segundo plano, con «Ver resultado» y «Expulsar tarjeta»; sección Avisos en Ajustes; sonido.
- **nüün (Windows):** el adaptador copiando el rol de nove con comentario de origen, cuando `plan`/`apply` estén en la ventana ([brief S9](audits/session-briefs-2026-11-10.md#s9--integración-de-sistema-en-la-ventana-de-windows)).
- **nove (Mac):** llevar el canal del sistema a la cola del motor al alcanzar el esquema actual, para que Mac y Windows compartan límites, alcance y caducidad.
- **nove (Windows):** registrar el clic físico; decidir si el canal nace encendido.
- **curado y Fractos Mac:** sólo cuando tengan algo que avisar en segundo plano.

Brief: [S18](audits/session-briefs-2026-11-10.md#s18--avisos-del-sistema-en-nüün-para-mac).

## nu9ve-sound-v1

[Sonido de tema de las apps de nu9ve v1](../contracts/nu9ve-sound-v1.md)

Leído en el código el 2026-11-10; nada escuchado.

| Regla | Fractos Mac | Fractos iOS | nüün | nove | curado |
| --- | --- | --- | --- | --- | --- |
| 1 Motor v2 | Sí, receta 3 | No: motor v1 | No | No | No |
| 2 Señales y gestos | Sí: es el origen de la tabla | Vocabulario propio de 11 acciones | No | No | No |
| 3 Voz por skin | Sí, por tema | Por tema | No | No | No |
| 4 Entrega | Motor Swift | Motor v1 | — | — | — |
| 5 Reproducción | Sí: precarga, límites, fatiga, reposo a los 20 s | `AVAudioEngine` con grupos y caché | No | No | No (tiene respuesta visual y háptica de teclado, no sonido de interfaz) |
| 6 Silencio | No: sólo en QA; no lee la preferencia del sistema | Coordinador de sesión de audio | No | No | No |
| 7 Por defecto y ajustes | Encendido por defecto (cumple la decisión); dos interruptores en Ajustes, sin volumen, sin «Silenciar» a un paso ni aviso de primera vez | Efectos, voz, volúmenes y háptica; sincroniza con su API | Parcial: Windows guarda `app.interfaceSounds`; nada suena | No | No |
| 8 Háptica | Sí, sin probar con trackpad | Sí | No | No | Háptica propia en Mac |

Primeros pasos sugeridos:

1. **Mac, con el repo de la web:** confirmar este contrato contra el motor de la web, resolver la pregunta de niveles y exportar el paquete (S19).
2. **Usuario:** confirmar las voces de los skins que Fractos no tiene (§3).
3. **Fractos:** decir en sus documentos que el v2 es el vigente; en la app de Mac, «Silenciar» a un paso, volumen y reglas de silencio.
4. **nüün Windows** como primera app consumidora (paquete): ya tiene los cinco skins y un `ThemeService` central.
5. **nüün Mac** con el motor Swift, cuando se decida dónde vive lo compartido.

## nu9ve-theme-v1

[Tema visual de las apps de nu9ve v1](../contracts/nu9ve-theme-v1.md)

**Época** ([§5](../contracts/nu9ve-theme-v1.md#época-de-los-skins-de-época), 2026-11-22): `Nu9veTheme` (Swift) y `Nu9ve.Core` (C#) la traen desde el core, y `PeriodWindowBar` de `NuMacShell` toma papel y tinta de los roles. Ninguna app la ofrece todavía en Apariencia: cada una la suma al volver a fijar el core. C# sin compilar ni probar todavía (hace falta el PC con Windows).

**Por omisión con tu color (2026-11-26, [§5](../contracts/nu9ve-theme-v1.md#época-de-los-skins-de-época)):** sin `era` guardado, cada app usa `tinted`; Macintosh empieza en claro (`defaultDark(era:)`), y sus rayas, reglas y bordes llevan el color (`PeriodBarColors.line`, que `PeriodBarColors(roles)` ya trae; `PeriodGrowBox(era:colors:)` para la caja de agarre). Le toca a cada app con Macintosh o Windows 9x al volver a fijar: su valor por omisión de la época, el modo inicial y el marco de la ventana con `line`. nüün ya ofrece la época (su F3) y su marco usa hoy `ink`. Las pruebas de C# de `Nu9ve.Core` ya corren en la Mac y pasan con los vectores nuevos.

Leído en el código el 2026-11-06, sin revisión visual. Revisar al actualizar. La columna de Fractos se releyó el 2026-11-10 en `ios-fractos` (`f1a7bcb`, app de Mac y `FractosDesign`), también sin verla.

| Regla | Fractos | curado | SoundShop | nüün | kyu | nove | neuf | Stalker |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2 Roles | Parcial: roles propios | Parcial: roles propios | Parcial: roles propios | Sí, 15 roles desde `Nu9veTheme`; falta `secondary` | Sí, 15 roles desde `Nu9veTheme`, más `secondary` y el entorno fotográfico neutro | Sí en Windows: 15 roles desde `Nu9ve.Core`; Mac sigue con colores fijos | No: colores del sistema | No aplica: sistema |
| 3 Contraste | Parcial: `mutedInk` y pruebas en Claro, Oscuro y Matrix; sin matriz | Sí, con matriz y los 110 vectores para `accent` y `onAccent` (TypeScript, 2026-11-16) | Sí, sin matriz | Sí, matriz y [110 vectores fijados](../data/theme-vectors-v1.json) con `Nu9veTheme` | Sí, matriz y los 110 vectores con `Nu9veTheme` (Gótico se prueba en la biblioteca) | Sí en Windows: matriz de sus cinco skins con sus presets y 108 colores | Parcial: texto sobre acento | No aplica |
| 4 Apariencia | Parcial: tema, paleta, material, logo, sonido y háptica; claves propias, no el formato común | Sí, el más completo | Parcial: preset, superficie, brillo | Parcial: presets, HSV y modo | Sí: skin, modo, color, contraste, textura y tipografía con migración | Parcial en Windows: skin, color (presets y hex) y modo en el formato común, con migración del modo | Parcial: modo | No aplica |
| 5 Skins | Sí: nueve materiales (`glass`, `classicMac`, `win9x`, `terminal`, `neon`, `gothic`, `parchment`, `ledger`, `chrome`) | Parcial: cinco estilos propios, incluidos Grano (`curado`), Pixel, Gótico, Cromo y Mínimo. Macintosh, Windows 9x y Terminal pendientes | No: uno | Sí: `glass`, `classicMac`, `win9x`, `terminal`, `gothic` | Sí: washi, Macintosh, Windows 9x, Terminal, Neón, Cristal, Grano y Mínimo | Sí en Windows: Puerto, Carta náutica, Macintosh, Windows 9x y Terminal | No | No aplica |
| 7 Identidad | Sí | Parcial: falta lo de barrio | Sí | Parcial: presets suizos | Sí | Sí en Windows: Puerto y sus tres direcciones | No | No aplica |
| 8 Todas las ventanas | Sí en código (Mac): `ThemedWindowContent` da barra y atmósfera a Ajustes y Acerca de | Sin revisar | Sin revisar | Sí (2026-11-07): principal, Ajustes y Acerca de en los 5 skins | Sin revisar | Sin revisar | Sin revisar | No aplica |
| 9 Idioma | No en Mac: sólo español, sin catálogo de textos; iOS con catálogo incompleto | Parcial: selector persistente en el WebView, español base e inglés para parte de React. Faltan mensajes y menús nativos, textos dinámicos y recorrido bilingüe | No | Sí | No | Sin revisar | Sin revisar | Sin revisar |

Una app adopta el contrato añadiéndose aquí con lo que le falta. Primeros pasos sugeridos:

- **nüün:** roles, contraste y presets suizos hechos (2026-11-07). Skins Grano, Macintosh, Windows 9x y Gótico (2026-11-07). Terminal hecho; Grano retirado. Faltan la opción de Contraste y la tipografía suiza del wordmark. **Windows (2026-11-09, maqueta):** mismos roles y cinco skins portados a C# con su matriz, más una prueba de contraste por superficie con skin (`SurfaceChromeTests`). En Windows la ventana y sus botones siguen siendo del sistema ([windows-app-v1 §3](../contracts/windows-app-v1.md#3-tema)).
- **SoundBender (2026-11-16, `e1635e8`):** juego de skins completo en colores: Original (el suyo, sin cambios y por omisión, con capturas idénticas byte a byte), Cristal claro como contraste (decisión del usuario) y los tres comunes, con `Nu9veTheme` desde su copia fijada en macOS 15. El lienzo y el pincel conservan los colores del usuario. Faltan la ventana cuadrada de época, los radios y la tipografía de cada skin.
- **Identidad propia (2026-11-16, core `4b07443`):** `Nu9veTheme` ya no trae colores de ninguna app. nüün declara su ámbar y sus presets suizos en `MediaMenuCore/ThemeIdentity.swift` (`1cd1690`), con los mismos valores. kyu ya tenía los suyos (Kyū·washi, neón) en `KyuSkin` y no cambió (`2de4e0f`). Una app que adopte el paquete conserva su original y su contraste, y suma los comunes de `ThemeSkin.commons`.
- **nüün en Windows (2026-11-16):** la base de C# se mudó a [`windows/`](../windows/README.md). Igual que en Swift, sin colores de ninguna app: nüün declara su ámbar, sus presets suizos, Cristal como original y Gótico como contraste en `Nuun.Core/Theme/NuunTheme.cs` (`cb31a06`). Usa la base desde su copia fijada, con la misma apariencia.
- **Kyū (2026-11-16):** usa `Nu9veTheme` desde su copia fijada en macOS 15. Cristal, Macintosh, Windows 9x y Terminal toman los roles comunes en contraste neutro. El fondo de la foto y de la cuadrícula se comprueba en gris neutro.
- **nove en Windows (2026-11-16, `cee0684`, [A40](../../nove/docs/decisions.md)):** adopta `Nu9ve.Core` y `Nu9ve.WinUI` desde su copia fijada (`d82eb2a`). Puerto es el original (el agua de Harbor, Mica y `#1D7A85`), Carta náutica el contraste (papel y tinta, ventana sólida, siempre claro) y Macintosh, Windows 9x y Terminal los comunes. Las tres direcciones son presets (Puerto, Atlántico, Anochecer). Las claves Harbor son los pinceles de los roles y los estados siguen las reglas de la suite (decisión del usuario). 87/87 en C# y capturas de los seis casos en QA. Falta la app de Mac, la ventana del broker vista y el alto contraste a mano.
- **neuf:** elegir su dirección de cine.
- **SoundShop:** añadir i18n.

- **curado (revisado el 2026-11-22, core `2d45333`):** `bun test src/lib/theme.test.ts` comprueba `accent` y `onAccent` en los 110 casos y añade sus 11 bases a la matriz de los cinco estilos propios. La ruta propuesta es portar los roles a TypeScript con los mismos vectores, mantener la barra nativa de Tauri y aplicar la apariencia a la ventana y el acento al auxiliar Metal. Faltan los demás roles, los skins comunes y su verificación por sistema. La preferencia sigue en Theme v2 de curado. [Detalle](../../curado/docs/SUITE.md)

## windows-app-v1

[Apps de Windows de nu9ve v1](../contracts/windows-app-v1.md)

nove y curado leídos en sus repos el 2026-11-07; nüün probado en Windows 11 el 2026-11-09 (compilación, pruebas, capturas y recorrido por UI Automation).

| Regla | nove 0.12 Windows | curado 0.1 (Tauri) | nüün 0.1 Windows (maqueta) |
| --- | --- | --- | --- |
| 0 Motor común | Sí: Rust, named pipe, pruebas de contrato | Sí: Rust + comandos Tauri | Parcial: protocolo de `media rpc-serve`, transporte probado en memoria; motor no portado (volúmenes de Mac) |
| 1 Barra de título | Sí: `TitleBar` | Barra decorada del sistema en Windows (THEMES.md) | Sí: `TitleBar`, botones del sistema teñidos, dos filas en skins de época |
| 2 Ventana | Parcial | Sin revisar | Parcial: tres modos con tamaños de Mac por DPI, compacto encima; no recuerda el marco |
| 3 Tema | Parcial: «Puerto» claro/oscuro/alto contraste en código, sin captura de alto contraste | Sí en la WebView; alto contraste sin revisar | Sí en código y capturas: cinco skins, roles y superficies probados, vectores compartidos; alto contraste sin revisar a mano |
| 4 Teclado | Parcial: menús de bandeja con teclado; catálogo y access keys sin revisar | Sí en la WebView (remapeo); access keys no aplican | Parcial: catálogo con pruebas, aceleradores, `AccessKey`, panel Ctrl+/; sin badges de Ctrl/Shift |
| 5 Acerca de | Sí | Sin revisar | Sí (maqueta) |
| 7 Ajustes | Sí | Sí | Parcial: Apariencia y Teclado |
| 8 Sistema | Sí: host, bandeja, StartupTask, avisos, Credential Manager; falta aceptación física de sesión/suspensión | Sin segundo plano | No aplica todavía: sin segundo plano |
| 9 Paquete | Parcial: MSIX con certificado de desarrollo | Parcial: instaladores generados, uso en Windows pendiente | Parcial: MSIX empaquetado sin firmar; Setup con identidad en demostración |
| 10 Validación | Sí: `NOVE_QA_ROOT`, UI Automation | Pruebas automáticas en Windows 11; GUI pendiente | Sí: `NUUN_QA_ROOT`, `qa-skins.ps1`, UI Automation; 55 pruebas |

Las demás apps no tienen cliente Windows. El [mapa de la suite](suite.md#plataformas) dice cuál podría tenerlo y con qué motor.
