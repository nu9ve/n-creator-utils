# Linux en la suite nu9ve: motor, interfaz, paquetes y distros

Estado: **propuesta** (2026-11-10), parte de la [auditoría Fable](../audits/fable-audit-2026-11-10.md). Hoy ninguna app de la suite corre en Linux; curado tiene notas de compilación (`curado`: `docs/PLATFORMS.md`) y el motor de nüün compila ramas `unix` que nadie ha ejecutado ahí. Esta página propone cómo llegar, en fases que se pueden detener en cualquier punto. Los nombres de API y de paquetes son la dirección; se confirman al implementar.

## 1. Respuesta corta

- **El motor es casi gratis; la interfaz no.** Un motor en Rust con SQLite embebido se compila para Linux hoy. Lo caro es una tercera interfaz nativa y la variedad de escritorios.
- **Empezar por el motor en CI** (fase L0). Sin interfaz, sin paquete: sólo `cargo test` en Ubuntu. Atrapa de inmediato lo que hoy se cuela por `cfg(not(windows))` ([auditoría B6](../audits/fable-audit-2026-11-10.md#b6--el-motor-decide-por-cfgwindows)).
- **El primer producto útil en Linux es sin ventana:** `nuun-engine` + CLI + MCP en un binario estático, también para arm64. Sirve en una Raspberry Pi como nodo de archivo, que conecta con la visión de dispositivos del [README](../../README.md).
- **La interfaz se decide con una prueba corta**, no por adelantado ([§3](#3-interfaz-la-decisión-pendiente)).
- **Compatibilidad entre distros** se resuelve con dos decisiones: contra qué libc se enlaza el motor y en qué formato viaja la interfaz ([§5](#5-paquetes-y-compatibilidad-entre-distros)).

## 2. Motor en Linux

### 2.1 Qué falta en el código

| Pieza | Hoy | En Linux |
| --- | --- | --- |
| JSON canónico, receta, planner, layout, EXIF | Portables | Nada |
| Catálogo SQLite | `rusqlite` con `bundled` | Nada |
| `nu9ve-time` | `localtime_r` en Mac, `chrono` en Windows | Usar la rama de `libc` de Mac |
| `fs`: sync, sin seguir enlaces | Rama `unix` | `fsync` + `syncfs` del destino al cerrar un job |
| `fs`: renombrar sin reemplazar | En Unix que no es Mac, comprueba y luego renombra bajo el lock | `renameat2(RENAME_NOREPLACE)`; si el sistema de archivos no lo admite, conservar la comprobación actual y registrarlo |
| `fs`: lectura sin caché | Sólo Mac | `O_DIRECT` alineado o `posix_fadvise(DONTNEED)` ([auditoría A1](../audits/fable-audit-2026-11-10.md#a1--la-relectura-de-verificación-no-evita-la-caché-fuera-de-macos)) |
| Backend de volúmenes | Windows real; fixture | **Nuevo:** UDisks2 ([§2.2](#22-volúmenes-con-udisks2)) |
| Expulsión | Sin portar | UDisks2 |
| No dormir | Sin capacidad | Inhibidor de `systemd-logind` |
| Liberar | Papelera de Mac | Cuarentena del motor ([auditoría A4](../audits/fable-audit-2026-11-10.md#a4--liberar-a-la-papelera-no-se-traduce-entre-sistemas)) |

### 2.2 Volúmenes con UDisks2

UDisks2 es el servicio D-Bus que usan GNOME, KDE y casi todos los escritorios para montar discos. Da lo que el motor necesita sin privilegios:

| Campo del motor | De dónde |
| --- | --- |
| `volume_uuid` | `Block.IdUUID` (UUID del sistema de archivos; en exFAT/FAT es el serial) |
| `disk_uuid` / serie física | `Drive.Serial`, `Drive.Id` (`/dev/disk/by-id`) |
| `physical_disks` | La relación `Block.Drive`; con LVM o RAID, todos los discos que lo forman (misma regla que en Windows: comparten disco, no son independientes) |
| `external`, `removable`, `bus` | `Drive.Removable`, `Drive.MediaRemovable`, `Drive.ConnectionBus` (`usb`, `sdio`) |
| `disk_image` | Dispositivos `loop` |
| `mount`, `label`, `filesystem`, `free` | `Filesystem.MountPoints`, `Block.IdLabel`, `Block.IdType`, `statvfs` |
| Llegada y salida | Señales `InterfacesAdded` / `InterfacesRemoved` |
| Expulsar | `Filesystem.Unmount`, luego `Drive.Eject` o `Drive.PowerOff` |

- **Sin UDisks2** (servidor mínimo, contenedor, algunas imágenes de Raspberry Pi): leer `/proc/self/mountinfo` y `/sys/block` para identidad básica; si falta la identidad física, **fallar cerrado** como en Windows. Un nodo sin escritorio registra sus destinos por ruta con `--allow-local` explícito y lo dice.
- **Montajes:** `/run/media/$USER/<etiqueta>` (lo usual), `/media/$USER/<etiqueta>` (Debian/Ubuntu) o `/mnt/...` a mano. Nunca asumir una ruta.
- **Una dependencia nueva:** un cliente D-Bus. Se anota con su razón en `Cargo.toml`, como pide la regla del repo. Alternativa sin dependencia: invocar `udisksctl` y `lsblk --json`; más frágil, aceptable para una primera fase.
- **Pruebas:** el backend de fixture ya existe. El real se prueba con un archivo de imagen montado por loop en CI (sin privilegios donde el runner lo permita) y con una memoria USB en una corrida física pedida.

### 2.3 Sistemas de archivos que el usuario ya tiene

| Disco | Formato | En Linux |
| --- | --- | --- |
| MEDIA502 | exFAT | Lectura y escritura con el controlador del núcleo (5.4+). Es el disco de intercambio natural |
| ESSD | HFS+ | Con diario, **sólo lectura** en Linux. No sirve como disco de trabajo ahí |
| SD de cámara | exFAT o FAT32 | Bien. Atención a las fechas ([auditoría A5](../audits/fable-audit-2026-11-10.md#a5--nombres-fechas-y-durabilidad-dependen-del-sistema-de-archivos)) |
| Destinos nuevos | ext4, btrfs | Distinguen mayúsculas y no los lee Mac ni Windows sin ayuda |

## 3. Interfaz: la decisión pendiente

La regla de la suite es interfaz nativa por sistema. En Linux «nativo» significa GTK (GNOME) o Qt (KDE); no hay un solo sistema.

| Opción | A favor | En contra | Para quién |
| --- | --- | --- | --- |
| **A. Sin interfaz: CLI + MCP + avisos** | Casi sin costo sobre el motor; sirve en servidores y Raspberry Pi; el agente es la interfaz | No es una app de escritorio | Primer paso para nüün y nove |
| **B. GTK 4 + libadwaita desde Rust** | Mismo lenguaje que el motor; nativo en GNOME; admite CSS propio para los skins; accesibilidad AT-SPI incluida | Una tercera interfaz que mantener; en KDE se ve ajena; libadwaita limita cuánto se puede cambiar la ventana | nüün y nove, si se quiere nativo de verdad |
| **C. Tauri** | curado ya lo es; una interfaz para tres sistemas; empaqueta deb, rpm y AppImage | WebKitGTK rinde menos que WebView2 y que WKWebView; depende de la versión que traiga cada distro | curado. Posible para una app nueva y ligera |
| **D. Qt/QML** | Mejor en KDE; maduro | Enlaces a Rust más pesados; licencia a revisar; otro lenguaje de interfaz | Sólo si KDE fuera el objetivo |

**Recomendación:** A para nüün y nove (fases L1–L2), C para curado (ya está), y decidir B con una prueba de dos pantallas (Inicio y Discos de nüün con un skin de época) antes de comprometerse. No empezar B hasta que la ventana de Windows de nüün haga `plan`/`apply` real: tres interfaces a medias es peor que dos completas.

Lo que cambia en los contratos si hay interfaz en Linux:

- **Botones de ventana:** el lado lo elige el usuario. Los skins de época con controles propios (hoy sólo en Mac) no aplican; como en Windows, la ventana es del sistema y el skin viste el contenido.
- **Bandeja:** no garantizada en GNOME. «Sigue trabajando con la ventana cerrada» se expresa con un aviso persistente y con que la ventana se pueda reabrir desde el lanzador. La semántica cerrar / pausar / detener / desactivar de nove no cambia.
- **Wayland:** la app no fija su posición ni se pone «siempre encima». El modo compacto existe, pero no flota; se declara como capacidad ausente y Ajustes lo dice.
- **Atajos globales:** restringidos en Wayland. Los de la suite son de ventana enfocada; no hay cambio.
- **Tema «Sistema»:** portal `org.freedesktop.appearance` para claro/oscuro, contraste y, donde exista, color de acento.

## 4. Integración de escritorio mínima

Para cualquier app con ventana, sea B o C:

| Pieza | Qué |
| --- | --- |
| Entrada de escritorio | `app.nu9ve.<app>.desktop` con nombre, icono, categorías, `StartupWMClass` y acciones (el equivalente de la Jump List) |
| Metadatos | AppStream `metainfo.xml` (lo exigen Flathub y los centros de software) |
| Iconos | Tema `hicolor`: SVG y PNG de 48 a 512 px, generados del mismo vector que en Mac y Windows |
| Rutas | XDG: datos en `$XDG_DATA_HOME/<app>`, preferencias en `$XDG_CONFIG_HOME/<app>`, caché en `$XDG_CACHE_HOME/<app>`, estado en `$XDG_STATE_HOME/<app>`, sockets y locks en `$XDG_RUNTIME_DIR/<app>` |
| Enlaces | `x-scheme-handler/nu9ve` ([os-integration §4](os-integration.md#4-enlaces-nu9ve)) |
| Avisos | Portal de notificaciones o `org.freedesktop.Notifications`, con acciones |
| Secretos | Secret Service; sin él, la función que lo necesita se niega y lo explica |
| Arranque | Unidad de usuario de systemd para el host de fondo; opt-in desde Ajustes; desinstalar la quita |
| Diálogos de archivo | Portal FileChooser (obligatorio en Flatpak, correcto fuera) |
| Sonido de interfaz | Rol de evento, salida por defecto ([sonido v1](../contracts/nu9ve-sound-v1.md)) |

## 5. Paquetes y compatibilidad entre distros

### 5.1 El motor y el CLI

El problema clásico es glibc: un binario enlazado en una distro nueva no arranca en una vieja.

| Estrategia | Resultado | Nota |
| --- | --- | --- |
| **Enlazar contra musl, estático** (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`) | Un binario que corre en cualquier distro, sin dependencias | Recomendada para `nuun-engine` y `nove`. SQLite ya va embebido. Si se usa un cliente D-Bus en Rust puro, sigue siendo estático. Medir el asignador de musl en trabajos con muchos hilos |
| Enlazar contra una glibc antigua | Corre en esa versión y posteriores | Compilar en un contenedor de la distro más vieja soportada, o apuntar a una versión de glibc con un enlazador cruzado. Necesario si alguna dependencia exige glibc |
| Compilar por distro | Paquetes nativos exactos | Sólo si un repositorio oficial lo pide |

Entrega: un `.tar.gz` con el binario, su licencia y su suma SHA-256 firmada; y paquetes `deb`/`rpm` generados del mismo binario para quien prefiera su gestor.

### 5.2 Las apps con ventana

| Formato | A favor | En contra | Uso sugerido |
| --- | --- | --- | --- |
| **Flatpak (Flathub)** | Una compilación para todas las distros; actualizaciones; runtime estable | Caja de arena: nüün necesita hablar con UDisks2 y ver `/run/media` y `/media`, permisos amplios que Flathub revisa; un motor fuera de la caja complica el modelo | nove y curado (red y audio encajan con los portales). nüün después, con permisos documentados |
| **AppImage** | Un archivo, sin instalar, sin caja | Sin actualizador propio; integración de escritorio manual; depende de la glibc de base | nüün con ventana, primera entrega |
| **deb / rpm** con repositorio propio | Integración completa, unidades de systemd, actualización por el gestor | Hay que mantener y firmar un repositorio; una compilación por familia | Cuando haya usuarios fuera del colectivo |
| AUR, Nix | Comunidades que empaquetan solas | Mantenimiento ajeno | Publicar una receta de ejemplo, sin prometer soporte |

Tauri (curado) ya produce `deb`, `rpm` y AppImage. Su atadura es WebKitGTK 4.1: Ubuntu 22.04 en adelante y equivalentes.

### 5.3 Matriz de prueba propuesta

No se promete una distro hasta haberla probado. Empezar por tres y crecer.

| Distro | Escritorio | Por qué |
| --- | --- | --- |
| Ubuntu LTS vigente | GNOME, Wayland | La más común; `/media/$USER` |
| Fedora vigente | GNOME, Wayland, SELinux | Componentes nuevos; `/run/media/$USER` |
| Una con KDE Plasma | KDE | Bandeja presente, decoración del servidor, otro selector de archivos |
| Debian estable | Cualquiera | glibc más vieja de la matriz (sólo importa si no se usa musl) |
| Raspberry Pi OS arm64 | Sin escritorio | El caso de nodo de archivo |
| Arch | Cualquiera | Rodante: detecta roturas antes |

Lo que cada entrada debe registrar: versión, escritorio, sesión (Wayland o X11), formato instalado, y qué se probó a mano (montar, expulsar, suspender, aviso, lector de pantalla).

### 5.4 Firma y confianza

Linux no tiene una firma de código central. Se firma el **canal**: clave GPG del repositorio `deb`/`rpm`, sumas firmadas junto a los `.tar.gz` y AppImage, y la verificación de Flathub. La clave se guarda fuera de los repos.

## 6. Fases

| Fase | Trabajo | Criterio de salida |
| --- | --- | --- |
| **L0 · Compila y prueba** | Trabajo de CI en Ubuntu: `fmt`, `clippy -D warnings`, `cargo test` de `engine/`, y lo mismo en nove y curado | Primera corrida verde registrada en el STATUS de cada repo. Las pruebas que dependen del backend de Windows se omiten diciendo por qué |
| **L1 · Volúmenes** | Backend UDisks2 detrás del rasgo existente; `volumes`, `source_cards`, `register_volume`; renombrado y lectura sin caché de Linux | Pruebas con fixture y con una imagen por loop; `capabilities` dice lo que falta. Sin corrida física no se declara |
| **L2 · Motor entregable** | Binarios musl x86_64 y arm64; `plan`/`apply` en Linux; MCP de lectura; tarball y `deb`/`rpm` | Instalado en una máquina limpia de dos distros; una corrida física pedida con memoria USB y disco externo |
| **L3 · Decisión de interfaz** | Prueba de GTK 4 con dos pantallas y un skin; comparar con Tauri | Decisión del usuario escrita, con capturas y costo estimado |
| **L4 · Primera app con ventana** | La que se decida (curado es la candidata más cercana) | Matriz §5.3 con tres distros probadas a mano; `linux-app-v1` escrito a partir de lo aprendido |

L0 se puede hacer ya y no compromete nada. L1 y L2 dependen de cerrar antes lo de Windows (primera corrida física y verificación sin caché), para no abrir un tercer frente sobre una base sin medir.

## 7. Por app

| App | Camino más corto | Bloqueos propios |
| --- | --- | --- |
| **curado** | Ya es Tauri: compilar, empaquetar y escuchar | ALSA o PipeWire y latencia; MIDI por ALSA; visuales de Metal sólo en Mac; bandeja en GNOME; stems con PyTorch en Linux (el caso mejor soportado por PyTorch) |
| **nove** | Motor con socket Unix (ya existe para Mac) + MCP; ventana después | El lector desde Chrome necesita el host de mensajería nativa registrado en `~/.config/google-chrome/NativeMessagingHosts`; avisos por portal; Telegram no cambia |
| **nüün** | Motor + CLI + MCP (L1–L2); ventana después de L3 | Volúmenes, expulsión, cuarentena, y que Stalker y Clipper no existen en Linux (el ingreso de cámara y dron no los necesita) |
| **kyu** | Su etapa 3 ya prevé wgpu | Fuera de esta máquina; sin revisar |
| **Fractos** | La web ya funciona en Linux | Ver `ios-fractos`: `docs/SUITE-DESKTOP.md` |

## 8. Qué no hacer

- No anunciar soporte de Linux por tener un trabajo de CI verde.
- No elegir la interfaz antes de la prueba de L3.
- No depender de la bandeja, de la posición de la ventana ni de «siempre encima».
- No pedir `sudo` para leer discos: si la identidad física no está disponible sin privilegios, fallar cerrado y explicarlo.
- No guardar secretos en un archivo cuando no hay Secret Service.
- No abrir Linux como tercer frente antes de la primera corrida física de nüün en Windows.
