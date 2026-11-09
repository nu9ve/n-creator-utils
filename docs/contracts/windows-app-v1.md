# Apps de Windows de nu9ve v1

Estado: **propuesta** (2026-11-07). Es el par de [apps de macOS v1](macos-app-v1.md): qué debe cumplir una app nativa de nu9ve en Windows 11 para sentirse de Windows, compartir motor y flujos con su versión de Mac y adoptar el [tema](nu9ve-theme-v1.md) y la [interacción](nu9ve-interaction-v1.md) de la suite. La referencia es **nove**, que ya tiene un cliente WinUI 3 sobre el mismo motor Rust que su app de Mac ([`apps/windows`](../../../../ai/nove/apps/windows/README.md), decisiones [A14–A16](../../../../ai/nove/docs/decisions.md#a14--windows-como-plataforma-activa-pocket-option-telegram-y-mcp-2026-11-05), [matriz de plataformas](../../../../ai/nove/docs/native-platforms.md)). curado es el caso Tauri ([WINDOWS.md](../../../../ai/curado/docs/WINDOWS.md)). Cumplirlo no declara una app lista para producción.

**Debe** es obligatorio salvo que la app documente por qué no aplica. **Conviene** es lo recomendado.

## 0. Principio: dos apps nativas, un producto

La similitud está en las capacidades, los flujos y la calidad de interacción; **el chrome sigue al sistema** (nove, `native-platforms.md`). No se copian los traffic lights de Mac en Windows ni el caption de Windows en Mac.

- **Debe:** la lógica vive en un motor común y los clientes solo presentan estado y piden acciones. El cliente no recalcula jobs, planes, precios ni reglas.

  | App | Motor | Mac | Windows |
  | --- | --- | --- | --- |
  | nove | Rust | SwiftUI/AppKit por socket Unix | C#/WinUI 3 (.NET 10, Windows App SDK 1.8) por named pipe |
  | curado | Rust (`audio-engine`) + Tauri | Tauri con WebView | El mismo Tauri con WebView2 |
  | nüün | Python (`media/service.py`) por stdin/stdout | SwiftUI/AppKit | C#/WinUI 3, **maqueta** sin motor (2026-11-09): [`native/windows`](../../native/windows/README.md) |
  | kyu | Rust (`kyu-core` por FFI) | Swift + Metal | Etapa 3: wgpu/CPU |

- **Debe:** un solo protocolo, independiente del transporte. En nove es JSON enmarcado, igual por socket Unix (`0700`) o por named pipe (ACL del SID del usuario más SYSTEM y Administradores, `FILE_FLAG_FIRST_PIPE_INSTANCE`, clientes remotos rechazados, PID del servidor verificado).
- **Debe:** los DTOs de cada cliente se prueban contra el **motor real** en raíces temporales (pruebas de contrato), no contra mocks. Escribirlos a mano o generarlos desde JSON Schema es decisión de cada app; nove los escribe a mano en Swift y en C#.
- **Conviene:** este orden: motor → app de Mac para refinar los flujos → Windows con el mismo contrato. Windows no se anuncia hasta validarlo en una máquina física.
- **No debe** existir una lógica de negocio por plataforma. Si Windows necesita algo, se agrega al motor y Mac lo hereda (como `activity_summary` y `engine_shutdown` en nove A15).

## 1. Barra de título

| Mac (macos-app-v1 §1) | Windows |
| --- | --- |
| `NSToolbar` `.unifiedCompact`, una fila junto a los traffic lights | **Control `TitleBar` de Windows App SDK 1.8**: atrás y panel con nombres para Narrador, regiones de paso para lo interactivo (búsqueda) |
| Identidad a la izquierda, navegación al centro, acciones a la derecha | Identidad e icono a la izquierda, búsqueda o navegación al centro, acciones antes de los botones del sistema |
| Espacio vacío arrastra; doble clic según `AppleActionOnDoubleClick` | Arrastre, doble clic, **Snap Layouts** y botones de ventana son del sistema: no se dibujan |
| Cede por pasos al estrechar, con regla pura | Igual, con regla pura y probada |

- **Debe:** usar `TitleBar` y no una barra propia. nove la cambió en A15 porque la barra propia exigía calcular a mano las regiones de paso.
- **Debe:** fijar los colores de los botones de ventana en `AppWindow.TitleBar` según el tema, y liberarlos en alto contraste.
- **Debe:** un encabezado de página en el shell (título + `CommandBar` con las acciones de la página), en la misma capa que el cuerpo. Las páginas no dibujan su propio título.

## 2. Ventana y tamaños

- **Debe:** mínimos reales por DPI y recordar el marco con `AppWindow`, restaurándolo solo dentro de monitores disponibles.
- **Debe:** compacto y completo con el mismo estado, como en Mac. Cambiar de modo nunca pierde una configuración en edición.
- **Conviene:** columnas alineadas a la izquierda con el encabezado (patrón de Configuración de Windows), canaleta 16/24/32 según ancho, y tokens de espacio (`Space4…Space32`) en vez de números sueltos.

## 3. Tema

[nu9ve-theme-v1](nu9ve-theme-v1.md) aplica igual. Cambia cómo llega al sistema:

- **Debe:** los roles comunes (§2 del tema) son recursos de tema con diccionarios `Light`, `Dark` y **`HighContrast`**. En alto contraste mandan los colores del sistema, no la paleta del usuario: es un requisito de accesibilidad de Windows.
- **Debe:** Claro, Oscuro y Sistema, siguiendo `UISettings`/`ActualTheme`. La derivación de color y la matriz de contraste son las mismas funciones puras que en Mac, idealmente en el motor o en datos compartidos (vectores de prueba, §10 del tema).
- **Conviene:** el equivalente de Cristal es **Mica** de fondo y **Acrylic** en superficies flotantes, con respaldo sólido si «Efectos de transparencia» está apagado. No verificado aún en ninguna app de la suite.
- **Skins** (§5 del tema): el juego es el mismo (original, contraste, Macintosh, Windows 9x, Terminal) y cambia **el contenido**: superficies, botones, campos, tipografía y texturas. **La ventana sigue siendo del sistema**, con sus botones a la derecha, porque es la posición original para un usuario de Windows. Un skin de época **puede** pedir esquinas cuadradas con `DwmSetWindowAttribute(DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND)`, pero no reemplaza el caption ni los botones.
- **Debe:** cada ventana y diálogo propio (Ajustes, Acerca de, broker, avisos, el instalador de la §9) usa el mismo tema, skin y colores de botones de ventana. `ContentDialog` y las ventanas secundarias heredan los diccionarios; ninguna queda con la barra por defecto.
- **Debe (2026-11-09, de nüün):** el skin de época se dibuja **detrás** de la `TitleBar` (franja con el degradado de Windows 9x, rayas de Macintosh, dintel gótico, regla de Terminal) y los botones del sistema se tiñen para leerse encima. Si la franja lleva título de época (Macintosh, Windows 9x), la barra de la app baja a una segunda fila, como `SkinWindowBar` en Mac. Mica sólo en el original de vidrio y con «Efectos de transparencia» activos.
- **Debe:** los colores llegan a los controles nativos por **sus claves** (`SliderTrackValueFill`, `TextControlBorderBrushFocused`, `ToggleSwitchFillOn`…). Sobrescribir sólo el acento base no alcanza a las plantillas de `XamlControlsResources`.
- **Debe:** releer alto contraste en `UISettings.ColorValuesChanged`. `AccessibilitySettings.HighContrastChanged` falla en apps de escritorio (0x80070490).
- **Debe:** una prueba de contraste por superficie con skin (panel, botón, presionado, píldora, tarjeta) × skin × modo × color, además de la de roles. En nüün encontró cuatro estados presionados ilegibles antes de verlos en pantalla.
- La identidad de cada app (§7 del tema) se mantiene. El tema «Puerto» de nove en Windows es su original.

## 4. Menús, comandos y teclado

[nu9ve-interaction-v1](nu9ve-interaction-v1.md) aplica igual, con estas traducciones:

| Mac | Windows |
| --- | --- |
| ⌘ | Ctrl |
| ⌥ | Alt (pero Alt solo abre las access keys) |
| ⌘, Ajustes | Ctrl+, |
| ⌘/ Panel de atajos | Ctrl+/ |
| ⌘? Ayuda | F1 |
| ⌘1…⌘9 vistas | Ctrl+1…Ctrl+9 |
| ⌃⌘F pantalla completa | F11 |
| ⇧⌘Z rehacer | Ctrl+Y (y Ctrl+Shift+Z) |
| ⌘Q salir | Alt+F4 es del sistema; «Salir» va en el menú de la app o de la bandeja |
| Menú de la barra de menús | `MenuBar` o `CommandBar` + menú «…»; todo con `KeyboardAccelerator` |

- **Debe:** todo comando con acelerador, estado (`IsEnabled`) y nombre accesible. El catálogo único de la §1 de interacción genera los `KeyboardAccelerator` y las `ToolTip`.
- **Debe:** usar las **access keys** de Windows (`AccessKey`). Al pulsar Alt aparecen los KeyTips del sistema. Es la versión nativa de los «badges al mantener un modificador»: en Windows, la capa de Alt la dibuja el sistema y la app solo añade las capas de Ctrl y Shift con sus propios badges.
- **Debe:** Shift+F10 y la tecla de menú abren el menú contextual. Los menús de la bandeja se pueden recorrer con teclado (probado en nove con UI Automation).
- **Debe:** un campo de texto activo se queda con sus teclas, como en Mac.

## 5. Acerca de

Los mismos hechos que en Mac (§5 de macos-app-v1): logo, frase, `Versión 1.2.3 (45)` leída del paquete, sitio, código, copyright y atribuciones. Clic o «Copiar» deja versión, build, edición de Windows y arquitectura para reportes. Es una página o diálogo del tema y forma parte del menú de la app y de la bandeja.

## 6. Documentos

Solo para apps con archivos propios: asociaciones de tipo en el manifiesto MSIX, abrir desde el Explorador, Ctrl+S/Ctrl+Shift+S/Ctrl+O, recientes en la Jump List y escrituras atómicas. nove todavía no es una app de documentos y no los declara.

## 7. Ajustes

- **Debe:** se abre con Ctrl+, y desde el menú, y recuerda la sección. Se arma con *settings cards*: nove tiene un patrón propio sin depender del Community Toolkit.
- Cada opción cambia algo que ya funciona. Segundo plano, inicio con Windows y canales de avisos van aquí, con su efecto escrito en palabras.

## 8. Sistema y segundo plano

La semántica es la misma en ambas plataformas (nove A15); cambian las piezas:

| Situación | Mac | Windows |
| --- | --- | --- |
| Ventana cerrada con trabajo | Icono en la barra de menús (nüün: `.accessory`) | **Icono en el área de notificación** desde un host por usuario (`Shell_NotifyIcon` v4, menú Win32), sin XAML ni WebView2 |
| Arranque con la sesión | `SMAppService` (helper/login item) | **`StartupTask` del MSIX**, registrado deshabilitado y activado solo desde Ajustes; Windows lo retira al desinstalar (Task Scheduler dejaría tareas huérfanas) |
| Supervisión | Helper por usuario | El host es padre del motor (`--parent-pid`): si el host muere, el motor se detiene. Nunca un motor invisible |
| Avisos | `UNUserNotificationCenter` | `AppNotificationManager` con activación COM declarada; el clic abre el elemento exacto |
| Secretos | Llavero | Credential Manager (genérica, local, no itinerante); nunca en la base, el IPC ni los logs |

- **Debe:** cerrar ventana, **pausar todo**, **detener** y **desactivar el segundo plano** son cuatro cosas distintas, con texto visible. Pausar es durable. Detener no se relanza solo. La X de Windows no detiene en silencio una vigilancia activa, ni ⌘Q en Mac.
- **Debe:** pedir permisos al habilitar lo que los necesita, no al abrir.
- **Debe:** respetar «Efectos de animación» (`UISettings.AnimationsEnabled`) como «Reducir movimiento», «Efectos de transparencia» como «Reducir transparencia» y el escalado de texto.
- **Debe:** una sola instancia de escritura por biblioteca (lock exclusivo + mutex `Local\…`), como en Mac.

**Un ejecutable, varios roles.** nove usa un solo `Nove.exe` con modos por argumento:
- `--background-host`: icono del área de notificación.
- `--web-worker`: navegador sin UI.
- `--deliver-toast`: avisos.
- `--broker-session`: ventana con un perfil propio.
- Solo en Debug, imitaciones locales de los servicios externos para QA.

Para lo que lanzan herramientas externas usa **alias MSIX con un solo rol permitido**: `nove-mcp.exe` para el MCP de agentes y `nove-broker-host.exe` para la mensajería nativa de Chrome. **Conviene** el mismo patrón en Mac (un bundle, roles por argumento), porque reduce binarios, firma y rutas que validar.

**Integraciones con el navegador y agentes** (nove A16–A17, QA 2026-11-06):
- Una extensión MV3 con ID fijo habla con un **host de mensajería nativa** registrado por usuario (HKCU de Chrome y Brave), que acepta solo el origen de esa extensión y entrega datos ya clasificados al motor.
- El servidor MCP local es otro cliente del pipe: lectura por defecto, y acciones solo con capability y confirmación en un diálogo nativo que el modelo no puede contestar.
- En ambos casos el texto remoto es dato, nunca instrucción, y nada lee contraseñas, cookies ni tokens de sesión.

## 9. Paquete y distribución

- **Debe:** MSIX con versión monotónica por build, para poder instalar encima. Mientras no haya firma real: certificado de desarrollo con un script para confiar en él (una vez, como administrador). Nunca un `.exe` sin firma.
- **Debe:** decidir y documentar dónde viven los datos. Lo que debe sobrevivir a desinstalar (la biblioteca) va en `%LOCALAPPDATA%\<App>` **sin virtualizar** (`unvirtualizedResources`). Lo que no debe heredarse (concesiones, tokens) va en `LocalState` del paquete, que Windows borra.
- **Debe:** desinstalar quita tareas, helpers y StartupTask. Borrar la biblioteca es una acción explícita.
- Tauri (curado) usa los instaladores de Tauri (NSIS/MSI). Esto también se documenta con firma, actualización y desinstalación.

### Instalador con identidad

El instalador nativo de MSIX (App Installer) no se puede personalizar. Para que instalar se sienta como la app, **conviene** un **Setup propio** opcional, junto al MSIX y no en su lugar.
- **Aspecto:** sigue el original de la app o uno de sus skins. Puede tener forma propia (ventana por capas con alfa por pixel, sin el rectángulo de siempre), estética demoscene, keygen o skin de Winamp, animación y música.
- **Música y sonido:** siguen el §12 del tema. Arrancan en silencio salvo que el usuario ya los haya encendido, siempre con un botón de silencio visible.
- **Debe seguir siendo un instalador honesto:**
  - Firmado.
  - Instala con la API de paquetes (`PackageManager`), igual que `Add-AppxPackage`.
  - Muestra qué instala, dónde, la versión y las atribuciones.
  - Progreso real, Cancelar y errores en palabras.
  - Funciona con teclado y Narrador.
  - Tiene modo silencioso para scripts (`--quiet`).
  - Pide administrador solo para el paso que lo necesita (confiar el certificado de desarrollo) y lo dice.
  - Desinstalar sigue en Configuración de Windows.
- **No debe** imitar marcas ni grupos reales de cracks, pedir que se desactive el antivirus, ofuscar ni empaquetar con compresores que disparen heurísticas, ni instalar nada que no se haya mostrado. La estética es un guiño; el comportamiento es el de un instalador de confianza.
- Las imitaciones de sitios o servicios para QA (como en nove) no van nunca en el Setup de Release.
- **Debe:** sin paquete, el Setup es una **demostración rotulada** que no escribe nada en el sistema, y `--quiet` devuelve códigos estables (0 instalado, 1 falló, 2 argumentos, 3 cancelado).
- **Referencia (2026-11-09):** `NuunSetup.exe` con la forma de la SD del icono de nüün (`SetWindowRgn`), arrastre por `InputNonClientPointerSource`, − × a la derecha, los skins de la app y `PackageManager`. Probado en demostración y en `--quiet` con argumentos inválidos; **no** con un paquete firmado. [Guía](../platforms/windows.md#9-paquete-e-instalador-con-identidad).
- **En Mac** el equivalente es un DMG con fondo y disposición del tema (o una app de instalación con la misma regla). Hoy nüün y SoundShop instalan con scripts de desarrollo, y eso se mantiene para desarrollo.

## 10. Validación

- **Debe:** un modo de QA aislado por variable de entorno (nove: `NOVE_QA_ROOT`; preferencias junto a la raíz). La biblioteca personal nunca se usa en pruebas. Igual que en Mac: correr con raíces temporales y revisar las capturas antes de compartirlas.
- **Debe:** QA de UI por **UI Automation**, que es el mismo árbol que usa Narrador (nove: `ui-invoke.ps1`, `capture-window.ps1`). Es el equivalente de `--qa-snapshots`/`--qa-skins` en Mac, y además prueba el teclado de verdad.
- **Debe:** registrar lo no comprobado físicamente: escala 150 %, alto contraste, Narrador escuchado, inicio de sesión, suspensión, firma real.
- **Conviene:** probar el paquete instalado lanzando su ejecutable por ruta con la variable de QA (conserva la identidad del paquete). `Invoke-CommandInDesktopPackage` no hereda el entorno.
- **Conviene:** capturar cada skin × página con una raíz de QA temporal, que es el par de `--qa-skins` de Mac (nüün: `scripts/windows/qa-skins.ps1`).

## 11. Base compartida

`Nu9ve.Core` (tema puro, superficies por skin, reglas de atajos, ventana e instalador) y `Nu9ve.WinUI` (servicio de tema, controles con skin, cromo de ventana, `Storage` por app) viven hoy en `n-creator-utils/native/windows` como primera copia. Una app nueva los referencia o los copia con comentario de origen. Se extraen a un paquete cuando haya tres consumidores y se decida repo y versión. Detalle y aprendizajes: [Windows en la suite](../platforms/windows.md).

## Adopción

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

Las demás apps no tienen cliente Windows. El [mapa de la suite](../suite.md#plataformas) dice cuál podría tenerlo y con qué motor.

## Cambios

Regla nueva o más estricta: añadirla aquí y, si tiene par en Mac, en [macos-app-v1](macos-app-v1.md), con la tabla de equivalencias al día.
