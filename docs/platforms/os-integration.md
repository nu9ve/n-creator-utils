# Integración con el sistema operativo: macOS, Windows y Linux

Estado: **propuesta** (2026-11-10), parte de la [auditoría Fable](../audits/fable-audit-2026-11-10.md). Las reglas adoptadas están en [macos-app-v1](../contracts/macos-app-v1.md) y [windows-app-v1](../contracts/windows-app-v1.md); esta página **amplía** su tabla de equivalencias, añade Linux y lista lo que el sistema ofrece y la suite todavía no usa. No declara nada implementado.

Cómo leerla: cada fila es una necesidad del producto, no una API. La columna «Hoy» dice qué app la tiene según sus documentos; «sin ver» significa que no lo encontré en docs ni en una búsqueda del código, no que no exista. Los nombres de API son el punto de partida: se confirman con la documentación del sistema al implementar.

## 1. Principios para traducir entre sistemas

1. **Se traduce la intención, no el control.** «Algo sigue trabajando con la ventana cerrada» es un icono en la barra de menús en Mac, un icono en el área de notificación en Windows y, en GNOME, un aviso persistente más un servicio de usuario, porque ahí la bandeja no está garantizada.
2. **La capacidad es del motor; la presentación, del cliente.** Mantener despierto el equipo, expulsar un disco o vigilar la llegada de una tarjeta se piden al motor por el protocolo. El cliente no llama a `diskutil`, `SetThreadExecutionState` ni `udisksctl` por su cuenta. Así Mac hereda lo que Windows necesitó y al revés ([windows-app-v1 §0](../contracts/windows-app-v1.md#0-principio-dos-apps-nativas-un-producto)).
3. **Cada capacidad se declara.** El motor responde `capabilities` por plataforma y por volumen (¿hay papelera?, ¿se puede expulsar?, ¿hay lectura sin caché?). La interfaz oculta o explica lo que no hay; nunca lo simula.
4. **Lo que el sistema ya hace, no se redibuja:** botones de ventana, Snap Layouts, KeyTips de Alt, Mission Control, diálogos de archivo, menús nativos.
5. **Opt-in para todo lo que persiste fuera de la app:** arranque con la sesión, manejadores de dispositivo, asociaciones, esquemas de enlace. Desinstalar los retira.

## 2. Tabla de equivalencias

### Ventana, apariencia y entrada

| Necesidad | macOS | Windows 11 | Linux | Hoy |
| --- | --- | --- | --- | --- |
| Barra propia junto a los botones del sistema | `NSToolbar` unificado | `TitleBar` de Windows App SDK | `AdwHeaderBar` (GNOME, decoración del cliente); en KDE la decoración puede ser del servidor | nüün, nove, SoundShop |
| Lado de los botones de ventana | Izquierda, fijo | Derecha, fijo | **Configurable por el usuario** (`gtk-decoration-layout`): un skin nunca asume el lado | — |
| Recordar el marco | `setFrameAutosaveName` | `AppWindow`, dentro de monitores disponibles | En Wayland la app **no** puede fijar su posición: sólo tamaño y estado | nove parcial; nüün Windows no |
| Compacto «siempre encima» | `NSWindow.level` | `OverlappedPresenter.IsAlwaysOnTop` | X11 sí; Wayland/GNOME no lo permite a una ventana normal. Declararlo como capacidad | nüün Mac y Windows |
| Claro / oscuro / sistema | `effectiveAppearance` | `UISettings`, `ActualTheme` | Portal `org.freedesktop.appearance` (`color-scheme`) | nüün, nove, curado |
| Color de acento del sistema como opción «Sistema» | `NSColor.controlAccentColor` | `UISettings.GetColorValue(Accent)` | Portal `accent-color` en escritorios recientes | Sin ver: el tema usa el color del usuario, que es lo correcto; ofrecer «tomar el del sistema» como preset |
| Alto contraste | «Aumentar contraste» → Contraste: Alto | Diccionario `HighContrast`, colores del sistema, `forced-colors` en WebView2 | Portal `contrast`; temas de alto contraste | En código en nüün y nove; **nunca observado**; curado sin `forced-colors` |
| Reducir movimiento | `accessibilityDisplayShouldReduceMotion` | `UISettings.AnimationsEnabled` | `gtk-enable-animations` | nüün, curado |
| Reducir transparencia | «Reducir transparencia» | «Efectos de transparencia» → sin Mica | No hay equivalente general: sólido por defecto | nüün |
| Tamaño de texto del sistema | Tamaño de texto por app (macOS reciente) | **Tamaño de texto** de Accesibilidad (`UISettings.TextScaleFactor`), distinto de la escala de pantalla | Factor de texto de GNOME/KDE | Sin ver en ninguna app: sólo se habla de 125/150 % de pantalla |
| Capa de teclas | Menús con equivalentes; badges propios | `AccessKey` (KeyTips de Alt) + aceleradores | Mnemónicos de GTK con Alt | nüün Windows, curado |
| Lector de pantalla | VoiceOver | Narrador (UI Automation) | Orca (AT-SPI) | QA por UI Automation en nove y nüün; nadie lo ha **escuchado** |
| Háptica | Trackpad Force Touch (`NSHapticFeedbackManager`) | Paneles táctiles hápticos y lápices en equipos recientes (evaluar; pocos equipos) | No | curado y Fractos en Apple. `HapticPlayer` y `PressureMonitor` de Fractos viven en `NuMacShell`, sin tipos de Fractos: reutilizables |
| Sonido de interfaz | Respetar «Reproducir efectos de sonido de la interfaz» | Respetar el esquema de sonidos del sistema | Rol de evento de PipeWire/libcanberra | Fractos Mac suena (motor v2) pero no lee esa preferencia; el resto no suena. [Sonido v1](../contracts/nu9ve-sound-v1.md) |

### Segundo plano, avisos y energía

| Necesidad | macOS | Windows 11 | Linux | Hoy |
| --- | --- | --- | --- | --- |
| Presencia con la ventana cerrada | Elemento en la barra de menús | Icono del área de notificación desde un host sin XAML | `StatusNotifierItem` (KDE sí; GNOME sólo con extensión). No depender de él | nüün Mac, nove Windows, curado (bandeja de Tauri) |
| Arranque con la sesión | `SMAppService` | `StartupTask` del MSIX, deshabilitado hasta Ajustes | Unidad de usuario de systemd o `~/.config/autostart`; en Flatpak, portal Background | nove Windows |
| Supervisión del motor | Helper por usuario | Host padre del motor (`--parent-pid`), Job Object | `systemd --user` con `Restart=on-failure`, respetando la intención de detener | nove |
| Aviso al terminar | `UNUserNotificationCenter` | `AppNotificationManager` con activación | `org.freedesktop.Notifications` o portal | nüün Mac, nove Windows |
| **Aviso con acciones** («Expulsar tarjeta», «Ver resultado») | Categorías y acciones de `UNNotification` | Botones del aviso con argumentos de activación | Acciones de la especificación de notificaciones | Sin ver en nüün |
| **Aviso con progreso** | — (usar el Dock) | Barra de progreso dentro del aviso, actualizable | Sugerencia `value` en algunos escritorios | Sin ver |
| **Progreso fuera de la ventana** | Insignia y progreso en el Dock; `NSProgress` publicado para que Finder lo muestre sobre el archivo | `ITaskbarList3::SetProgressValue` y estado (normal, pausa, error); insignia en el icono | API de lanzador de Unity/KDE (`com.canonical.Unity.LauncherEntry`) | [interacción §7](../contracts/nu9ve-interaction-v1.md#acciones-largas) lo pide en Windows; sin implementar. En Mac hay código de progreso en `Application.swift`/`Model.swift`, sin revisar a fondo |
| **No dormir durante una copia** | `ProcessInfo.beginActivity` / aserción de IOKit | `PowerCreateRequest` (o `SetThreadExecutionState`) | Inhibidor de `systemd-logind` o portal Inhibit | nove (A30, en su `os.rs`). **nüün no**: una copia de 64 GB puede cruzarse con la suspensión |
| Avisar de batería baja antes de un trabajo largo | `IOPSCopyPowerSourcesInfo` | `GetSystemPowerStatus` | UPower | Sin ver. Preguntar, no bloquear |
| Trabajo de mantenimiento en reposo (rehash periódico) | `NSBackgroundActivityScheduler` | El host de fondo con detección de inactividad; no Task Scheduler (deja tareas huérfanas, nove A15) | Temporizador de systemd de usuario | PLAN lo lista como pendiente |
| Una sola instancia de escritura | Lock de archivo | Lock + mutex `Local\…` | Lock de archivo; socket en `$XDG_RUNTIME_DIR` | nüün, nove, curado |

### Dispositivos, volúmenes y archivos

| Necesidad | macOS | Windows 11 | Linux | Hoy |
| --- | --- | --- | --- | --- |
| Llegada y salida de una tarjeta | DiskArbitration / notificaciones de montaje | `RegisterDeviceNotification` + `WM_DEVICECHANGE` (interfaz de volumen) | Señales `InterfacesAdded/Removed` de UDisks2 | nüün Mac. En Windows el motor lista tarjetas al pedirlo; sin evento |
| **Ofrecerse al insertar** | No hay registro por app; lo hace la app residente | **AutoPlay**: declarar un manejador en el manifiesto MSIX para que «Respaldar con nüün» aparezca al insertar una tarjeta con `DCIM`. Opt-in | Manejadores de medios del escritorio (`x-content/image-dcf`) en el `.desktop` | El «popup al insertar» del PLAN (P0 3) es esta fila. En Windows, AutoPlay es la forma nativa y no exige proceso residente |
| Identidad de volumen | `VolumeUUID`, `DiskUUID`, almacenes físicos | Ruta GUID, serial, `PhysicalDriveN`, bus | UUID y serial de UDisks2 / `/dev/disk/by-id` | nüün (Mac en Python, Windows en Rust), curado. No es portable: [auditoría A3](../audits/fable-audit-2026-11-10.md#a3--la-identidad-de-un-disco-no-viaja-entre-equipos-ni-entre-sistemas) |
| Expulsión segura | `DADiskUnmount` + `DADiskEject` (hoy `diskutil eject`) | Bloquear y desmontar el volumen, luego `CM_Request_Device_Eject` | `Filesystem.Unmount` + `Drive.Eject`/`PowerOff` de UDisks2 | nüün Mac. `eject_card` sin portar a Rust |
| Cámaras y teléfonos **sin** letra de unidad | ImageCaptureCore (PTP/MTP) | Windows Portable Devices (MTP/PTP) | gvfs/libmtp, gphoto2 | Ninguna. Hoy nüün sólo ve almacenamiento masivo: una cámara conectada por cable USB en modo MTP no aparece. Decidir si es alcance |
| Salud del disco | Estado SMART en `diskutil info` | Contadores de fiabilidad de almacenamiento (`Get-StorageReliabilityCounter` / WMI) | `Drive.Ata` de UDisks2, `smartctl` | Ninguna. Alto valor para un archivo de respaldo |
| Lectura sin caché para verificar | `F_NOCACHE` | `FILE_FLAG_NO_BUFFERING` | `O_DIRECT` / `posix_fadvise` | Sólo Mac: [auditoría A1](../audits/fable-audit-2026-11-10.md#a1--la-relectura-de-verificación-no-evita-la-caché-fuera-de-macos) |
| Escritura durable | `F_FULLFSYNC` | `FlushFileBuffers`; política «extracción rápida» o «mejor rendimiento» del dispositivo | `fsync` + `syncfs` | nüün |
| Renombrar sin reemplazar | `renamex_np(RENAME_EXCL)` | `MoveFileExW` sin reemplazo | `renameat2(RENAME_NOREPLACE)`; hoy el código comprueba y luego renombra bajo el lock | nüün; mejorar la rama de Linux |
| Papelera | `.Trashes` por volumen | `$RECYCLE.BIN` sólo en discos fijos | Especificación de FreeDesktop | nüün Mac: [auditoría A4](../audits/fable-audit-2026-11-10.md#a4--liberar-a-la-papelera-no-se-traduce-entre-sistemas) |
| Residuos a ignorar | `.DS_Store`, `._*`, `.Spotlight-V100`, `.Trashes`, `.fseventsd` | `System Volume Information`, `$RECYCLE.BIN`, `Thumbs.db`, `desktop.ini` | `.Trash-*`, `lost+found` | El escáner de Rust ya ignora los de Mac y `System Volume Information`. Comprobar `$RECYCLE.BIN`, `Thumbs.db`, `desktop.ini`, `.Trash-*`, `lost+found` |
| Elegir carpeta | `NSOpenPanel` (y marcadores con sandbox) | Selectores de Windows App SDK | Portal FileChooser | nüün, curado (`rfd`) |
| Recientes y accesos rápidos | Menú «Abrir reciente», menú del Dock | **Jump List**: proyectos recientes, «Preparar respaldo», «Abrir Discos» | Acciones del `.desktop` | Sin ver. Barato en Windows |
| Asociaciones de tipo | `CFBundleDocumentTypes` | Manifiesto MSIX | MIME en `.desktop` | SoundShop. No aplica a nüün hoy |
| Vista previa | Quick Look | Manejadores de vista previa; la app usa su propio visor | — | nüün Mac |
| Arrastrar una carpeta a la ventana para usarla como fuente | `onDrop` | `DragDrop` de WinUI (no funciona si la app corre elevada) | GTK DnD | Sin ver |
| Menú contextual del explorador («Respaldar con nüün») | Extensión de Finder (costosa) o Servicios | Comando del explorador declarado en el paquete | Acciones de Nautilus/Dolphin | No. Baja prioridad; AutoPlay y la Jump List cubren el caso común |

### Identidad, datos, instalación y diagnóstico

| Necesidad | macOS | Windows 11 | Linux | Hoy |
| --- | --- | --- | --- | --- |
| Datos del usuario | `~/Library/Application Support/<app>` | `%LOCALAPPDATA%\<app>` sin virtualizar | `$XDG_DATA_HOME/<app>` | Ver [§3](#3-convención-propuesta-para-nombres-y-rutas) |
| Preferencias | `UserDefaults` o archivo junto a los datos | Archivo en `%LOCALAPPDATA%\<app>\Preferencias` | `$XDG_CONFIG_HOME/<app>` | Cada app a su modo |
| Caché regenerable | `~/Library/Caches/<app>` | `%LOCALAPPDATA%\<app>\Cache` | `$XDG_CACHE_HOME/<app>` | curado, nüün Mac |
| Estado de ejecución (sockets, locks) | Carpeta de datos, permisos `0700` | Named pipe con ACL | `$XDG_RUNTIME_DIR/<app>` | nove |
| Secretos | Llavero | Credential Manager | Secret Service (libsecret); si no hay, **rechazar**, no guardar en claro | nove Windows. Fractos Mac: llavero de data protection con firma de equipo, y archivo `0600` en su contenedor sólo si la firma es ad hoc (`ProtectedSessionStorage`) |
| Enlaces profundos | `CFBundleURLTypes` | Protocolo declarado en el manifiesto | `x-scheme-handler/…` en `.desktop` | Sin ver. Propuesta `nu9ve://` en [§4](#4-enlaces-nu9ve) |
| Automatización del sistema | **App Intents** (Atajos, Spotlight) | CLI con alias de ejecución + MCP | CLI + D-Bus | MCP en nove y nüün. App Intents sin ver |
| Búsqueda del sistema | Core Spotlight | Indexador de Windows | Tracker/Baloo | No. Baja prioridad |
| Widgets | WidgetKit | Tablero de widgets | — | No. Idea: «pendiente de respaldar» y espacio libre |
| Instalación | `.app` en `~/Applications` o DMG | MSIX + Setup con identidad; NSIS en Tauri | deb/rpm, AppImage, Flatpak | Desarrollo en todas |
| Actualización | Sparkle o propio | `.appinstaller` o Store | Repositorio o Flathub | **Ninguna**: [auditoría B1](../audits/fable-audit-2026-11-10.md#b1--ninguna-app-tiene-firma-de-distribución-ni-actualización) |
| Firma | Developer ID + notarización | Authenticode (Store, firma en la nube u OV/EV) | GPG del repositorio | Desarrollo en todas |
| Desinstalar | Arrastrar a la Papelera; los datos quedan | Configuración; el MSIX retira `StartupTask` y alias | Gestor de paquetes | nove comprobado |
| Diagnóstico | `os_log` + archivo | Archivo (`last-ui-error.txt`); ETW opcional | journald + archivo | nove y nüün Windows con archivos |
| Reporte de fallos | Sin telemetría: «Copiar diagnóstico» | Igual | Igual | nove parcial (G10) |

## 3. Convención propuesta para nombres y rutas

Aplica a lo **nuevo** (una app o plataforma que aún no tiene datos de usuarios). Lo existente no se renombra sin migración, respaldo y decisión del usuario: conservar datos pesa más que la simetría.

| Cosa | Convención | Ejemplo |
| --- | --- | --- |
| Identificador de app | `app.nu9ve.<app>` | `app.nu9ve.nuun`, `app.nu9ve.nove` |
| Carpeta de datos | El nombre visible en minúsculas ASCII | `nuun`, `nove`, `curado`, `fractos` |
| Subcarpetas | `Biblioteca` (datos del motor), `Preferencias` (ventana y tema), `Cache`, `Desarrollo` (builds Debug) | `%LOCALAPPDATA%\nuun\Biblioteca` |
| Compartido de la suite | Carpeta `nu9ve` hermana | `~/Library/Application Support/nu9ve/theme.json` |
| Raíz de QA | `<APP>_QA_ROOT` | `NUUN_QA_ROOT` |
| Otras variables | `<APP>_<COSA>`; `NU9VE_<COSA>` sólo para lo compartido | `NUUN_ENGINE_BIN`, `NU9VE_WRITE_THEME_VECTORS` |
| Servidor MCP | `serverInfo.name` = nombre de la app | `nuun` (hoy `ncreator-media`) |
| Esquema de enlace | `nu9ve://<app>/…` | [§4](#4-enlaces-nu9ve) |

Diferencias de hoy contra esta tabla: [auditoría C2](../audits/fable-audit-2026-11-10.md#c2--identificadores-carpetas-y-variables-sin-convención).

## 4. Enlaces `nu9ve://`

Un solo mecanismo para tres cosas que hoy no existen o son propias de cada app: el nivel **Mostrar** del MCP (`show_in_app`), el clic en un aviso que abre el objeto exacto, y los saltos entre apps.

```text
nu9ve://nuun/job/<id>          abrir el resultado de un trabajo
nu9ve://nuun/plan/<id>         abrir un plan revisado (sólo verlo)
nu9ve://nuun/project/<id>
nu9ve://nove/pair/<id>
nu9ve://curado/track/<id>
```

Reglas:

- Un enlace **muestra**. Nunca copia, limpia, guarda ni confirma. Es el mismo límite del nivel Mostrar de [nu9ve-mcp-v1 §2](../contracts/nu9ve-mcp-v1.md#2-cuatro-niveles-de-herramientas).
- Los identificadores son opacos, del motor. Un enlace con un id desconocido abre la app y dice «no encontrado».
- El texto de un enlace es dato no confiable: se valida la forma y se acota la longitud antes de tocar el motor.
- Registro: `CFBundleURLTypes` en Mac, protocolo en el manifiesto MSIX en Windows, `x-scheme-handler/nu9ve` en Linux. Con un esquema para toda la suite, un pequeño despachador decide qué app abre; con un esquema por app (`nuun://`) no hace falta. **Decidir cuál** antes de implementar.

## 5. Las diez con mejor relación valor / costo

Para la ventana de Windows de nüün salvo que se indique. El orden es mi recomendación.

| # | Función | Por qué | Costo |
| --- | --- | --- | --- |
| 1 | No dormir durante `apply` | Una suspensión a mitad de copia es el fallo más probable en una laptop. nove ya lo resolvió | Bajo: capacidad del motor, copiar de nove |
| 2 | Progreso en la barra de tareas y en el Dock | El contrato de interacción ya lo pide; la copia se sigue sin la ventana | Bajo |
| 3 | Aviso al terminar con «Expulsar tarjeta» | Cierra el flujo SD → respaldo → tarjeta libre en un clic | Medio: necesita `eject_card` en Rust |
| 4 | Llegada de tarjetas por evento | Quita el sondeo y habilita lo demás | Medio |
| 5 | AutoPlay «Respaldar con nüün» (opt-in) | Es el «popup al insertar» nativo de Windows, sin proceso residente | Medio: manifiesto y activación |
| 6 | Enlaces `nu9ve://` | MCP Mostrar, avisos y saltos entre apps con un mecanismo | Medio |
| 7 | Jump List con proyectos recientes y «Preparar respaldo» | Acceso directo desde la barra de tareas | Bajo |
| 8 | Salud SMART del archivo en Discos | Avisa antes de confiar el único respaldo a un disco que falla | Medio, por plataforma |
| 9 | Tamaño de texto del sistema | Accesibilidad real; hoy sólo se piensa en la escala de pantalla | Bajo–medio |
| 10 | App Intents en Mac («Estado del respaldo», «Preparar respaldo de…») | Atajos y Spotlight sin abrir la app; sólo lectura y Mostrar | Medio |

Fuera de la lista a propósito: widgets, búsqueda del sistema, menú contextual del explorador y MTP. Tienen sentido, pero después de que el flujo principal funcione físicamente en Windows.

## 6. Qué cambiaría en los contratos

Si el usuario adopta lo anterior, estas son las ediciones concretas (no hechas):

- [windows-app-v1 §8](../contracts/windows-app-v1.md#8-sistema-y-segundo-plano): filas nuevas para energía, progreso en la barra de tareas, llegada de dispositivos, AutoPlay y aviso con acciones.
- [macos-app-v1 §8](../contracts/macos-app-v1.md#8-sistema): progreso en el Dock, actividad que evita la suspensión, App Intents como «conviene».
- Un par `linux-app-v1` cuando exista la primera app en Linux; mientras tanto, [linux.md](linux.md).
- [nu9ve-mcp-v1 §2](../contracts/nu9ve-mcp-v1.md#2-cuatro-niveles-de-herramientas): que `show_in_app` y los enlaces `nu9ve://` sean el mismo camino.
- Un contrato corto de capacidades del motor por plataforma y por volumen (papelera, expulsión, lectura sin caché, identidad portable).
