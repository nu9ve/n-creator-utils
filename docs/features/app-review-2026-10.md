# Revisión de la app y propuestas · 2026-10-29

Revisión de la app nativa (`native/MediaMenu`), su puente con el motor
(`media/service.py`) y sus flujos de ventana, navegación, formularios, detección de SD,
estilo, idiomas y plataformas. Es un **documento de propuestas**: no cambia
comportamiento ni sustituye [STATUS](../STATUS.md) o [PLAN](../../PLAN.md). La experiencia
objetivo por flujo ya está en [media-experience](../media-experience.md) y la ventana actual
en [nuun-window](nuun-window.md); aquí se agrega cómo llegar a ella con una base más fluida.

Base revisada: `master` en `f3d1651`. Al escribir la revisión no se abrió la app ni se usaron volúmenes o media
personales; los hallazgos vienen de leer código. El bug de fullscreen **no se reprodujo**
en esta sesión.

## Implementado el 2026-10-29

Evidencia y build en [STATUS](../releases/status-log-2026-10-25_2026-11-08.md#implementación-de-la-revisión--2026-10-29); flujo vigente en [nuun-window](nuun-window.md#ingreso-por-pasos-2026-10-29).

| Propuesta | Estado |
| --- | --- |
| §1 Fullscreen | `NSToolbar` compacto + `willUseFullScreenPresentationOptions` sin `.autoHideToolbar`; QA programática confirmó la barra visible en fullscreen. Matriz de pantallas/Spaces pendiente. |
| §2 Navegación | `Route`, cinco pestañas, Ajustes como ventana (⌘,), atajos, hojas en lugar de `runModal`, confirmación antes de ejecutar, banner de error descartable, marco por modo. |
| §3 Ingreso por pasos | Cinco pasos, modo rápido/guiado configurable, bloqueo mínimo, filtros, selección múltiple, panel de asignación y Quick Look. Agrupar por día/cámara queda pendiente. |
| §4 Vistas | Biblioteca con `Table`, búsqueda y filtro; Actividad con progreso medido y detalles; Inicio con tarjetas, discos y pendientes. Espacio conserva su flujo, sin pasos. |
| §5 Reactividad | `snapshot`, lector persistente de sólo lectura, `@Observable`, bloqueo por ámbito, miniaturas decodificadas en caché. Progreso: consulta de 1 s por el lector, sin eventos empujados todavía. Stores separados y miniaturas de cámara pendientes. |
| §6 SD | Huella superficial + recomendación + último proyecto + espacio previo. DiskArbitration pendiente: los eventos de NSWorkspace ya disparan una sola lectura. |
| §7 Estilo | Botones secundarios ligeros, títulos serif (New York), toolbar compacta. |
| §8 Idiomas | `en.lproj` con 418 claves, `scripts/check_localization.py`, Ajustes → Idioma. Errores del motor siguen en su idioma. |
| §9–10 | Sin cambios de plataforma. QA visual offscreen sólo en builds debug (`--qa-snapshots`, `--qa-fullscreen`, `--qa-ingest`). |

## Resumen

Lo que ya está bien y conviene conservar: un solo modelo compartido entre los tres modos,
el plan revisable como frontera de seguridad, el motor común CLI/app, cero sondeo en
reposo, cancelación por generación en discovery y miniaturas con concurrencia acotada.

Lo que más afecta la sensación de fluidez **no es el lenguaje de la UI**, sino:

1. **Un proceso Python por cada llamada.** `BackendClient.call` lanza
   `python3 nu9ve.py media rpc` por petición
   ([Protocol.swift:103](../../native/MediaMenu/Sources/MediaMenuCore/Protocol.swift)).
   `refresh()` hace nueve llamadas en serie
   ([Application.swift:165](../../native/MediaMenu/Sources/MediaMenu/Application.swift)), y se
   ejecuta al terminar cada operación y en cada montaje. El progreso del job abre otro
   proceso cada 2 s (Application.swift:361). Cada `suggest` y `source_cards` además
   ejecuta `diskutil` ([volumes.py:30](../../media/volumes.py)).
2. **Un `ObservableObject` con ~70 propiedades `@Published`.** Cualquier cambio (incluido
   el mensaje de progreso cada 2 s) invalida `WindowRoot` completo, la grilla y sus
   `onChange(of: model.units)`. Con tarjetas grandes esto se nota.
3. **Un `busy` global.** `.disabled(model.busy)` desactiva Ingreso, Biblioteca y Ajustes
   durante una copia que puede durar horas; no se puede ni navegar la biblioteca.
4. **Ingreso como una sola columna larga.** Fuente, ubicación, destinos, proyecto,
   tipo de fuente, política de copia, limpieza, inspección, resumen, grilla, asignación
   avanzada y plan en la misma vista (Application.swift:583). Es la causa principal de que
   se sienta abrumador.
5. **Dos barras superiores** (accesorio de titlebar + barra SwiftUI en fullscreen) con
   un truco de quitar/reinsertar el accesorio tras cada resize. Es frágil y es la zona del
   bug de fullscreen.

## 1. Bug de fullscreen: las pestañas desaparecen

**Síntoma reportado:** en pantalla completa, con la barra de menús en ocultación automática,
las pestañas no se ven y sólo aparecen al revelar la barra superior; en ventana funcionan.

**Mecanismo actual.** La navegación vive en un `NSTitlebarAccessoryViewController`
`.right` (Application.swift:690). Al entrar en fullscreen, `windowWillEnterFullScreen`
oculta el accesorio y activa `windowIsFullscreen`, y `WindowRoot` inserta otra
`WindowTopBar` con `.safeAreaInset(edge: .top)`
([Presentation.swift:13](../../native/MediaMenu/Sources/MediaMenu/Presentation.swift)).

**Causa probable (sin reproducir).** Esto depende de tres comportamientos de AppKit que
cambian en fullscreen: (a) el titlebar —con sus accesorios— se mueve a una ventana de
overlay que sólo aparece al revelar la barra; (b) la ventana no usa
`.fullSizeContentView`, de modo que el área segura del `NSHostingView` se recalcula cuando
el overlay aparece o desaparece; (c) el ocultamiento de un accesorio `.right` y su
`fullScreenMinHeight` no están garantizados durante la transición. Si la barra SwiftUI
queda dentro del área reservada del titlebar oculto o el accesorio vuelve a mostrarse
sólo en el overlay, se observa exactamente lo reportado. Antes de corregir, conviene
confirmarlo registrando `contentLayoutRect`, `safeAreaInsets` y el frame de la barra
al entrar, revelar y ocultar.

**Corrección propuesta (estructural, no un parche más):**

- Reemplazar accesorio + barra duplicada por **un `NSToolbar` real** con estilo
  `.unifiedCompact`: logo a la izquierda, pestañas como ítem centrado
  (`centeredItemIdentifiers`) y modo/cancelar a la derecha, en el mismo renglón que los
  semáforos.
- Implementar `window(_:willUseFullScreenPresentationOptions:)` y devolver
  `[.fullScreen, .autoHideMenuBar]` **sin** `.autoHideToolbar`: el menú se oculta y la
  toolbar queda visible, como en Finder con «Mostrar siempre la barra en pantalla
  completa».
- Eliminar `layoutTitlebar()`, `windowDidResize` del accesorio, `windowIsFullscreen` y la
  segunda `WindowTopBar`. Un solo camino de código en ventana y en fullscreen.
- En compacto/selección la toolbar sólo muestra título + modo; mismo objeto, otros ítems.

**Aceptación:** ventana, fullscreen con menú siempre visible y con ocultación automática,
dos pantallas/Spaces, cambio de modo desde fullscreen (hoy sale de fullscreen con
`pendingMode`), Reduce Motion, tema claro/oscuro.

## 2. Ventanas, navegación y layouts

### Pestañas

Hoy son seis pestañas identificadas con enteros mágicos (`selectedTab = 4`…), botones
de igual ancho en el titlebar y el pie cambia según pestaña.

- **Usar un `enum Route`** (`home`, `ingest`, `library`, `activity`, `space`) en vez de
  enteros; habilita deep links desde notificaciones/menubar y atajos.
- **Reducir a cuatro o cinco pestañas:** Inicio (hoy Bandeja), Ingreso, Biblioteca,
  Actividad y Espacio. **Ajustes sale de las pestañas** a la escena `Settings` estándar
  de macOS (⌘,), con sus propias secciones (Perfiles, Discos, Capturas, Apariencia,
  Idioma, Diagnóstico). Así el titlebar queda más corto y ordenado.
- **Atajos:** ⌘1…⌘5 para pestañas, ⌘R revisar plan, ⌘↩ ejecutar (siempre con la hoja de
  confirmación de §3), ⌘E expulsar, ⌘F buscar en Biblioteca, Espacio para Quick Look.
- Pestañas como control segmentado nativo compacto (icono + texto corto); en anchos
  pequeños, sólo icono con `help`.

### Tamaños y modos

Los tres modos (440×700, 780×820, 1180×900) son razonables, pero son tamaños fijos.

- Guardar el **último frame por modo** (`setFrameAutosaveName` por modo) en vez de volver
  siempre al tamaño de fábrica.
- Completo debería poder ser más pequeño (mínimo actual 920×720) si la navegación usa
  iconos y la grilla adapta columnas.
- Considerar que el compacto viva como **popover/panel del menubar** (`NSPanel` flotante)
  en vez de reutilizar la ventana principal: la ventana completa queda abierta mientras se
  atiende una SD rápida. La regla de un solo modelo se mantiene.

### Diálogos y hojas

- `NSOpenPanel.runModal()` bloquea la app (Application.swift:217, 237, 498, 517). Usar
  `beginSheetModal(for:)` o `.fileImporter` adjunto a la ventana.
- La confirmación «Sólo respaldo» es un `alert` de texto. Mejor una **hoja** con la
  misma estructura que el resumen del plan: qué se verifica, qué queda pendiente y qué se
  limpia.
- **Falta una confirmación final de acciones destructivas.** «Ejecutar plan revisado»
  ejecuta en seguida aunque incluya limpieza de SD o liberación Stalker. Proponer una hoja
  breve: *N archivos · MEDIA502 + ESSD · después se limpia la SD · se libera el original
  Stalker*, con el botón destructivo a la derecha y ⌘↩.
- Errores: hoy hay un único `error` arriba en rojo y un `message` en el pie. Proponer
  avisos por contexto (banner en la vista que falló), con acción de recuperación
  («Reintentar», «Abrir actividad», «Conectar ESSD») y botón para descartar.

## 3. Ingreso por pasos, a pantalla completa

La mejor forma de que no abrume es que **cada paso ocupe la vista y muestre sólo sus
decisiones**, con una barra de pasos arriba y Atrás/Siguiente en el pie fijo.

```text
① Fuente  →  ② Contenido  →  ③ Destinos  →  ④ Revisar  →  ⑤ Copiar  →  ⑥ Listo
```

| Paso | Contiene | Sale de la vista actual |
| --- | --- | --- |
| Fuente | SDs conectadas como tarjetas grandes (nombre, capacidad, cámara detectada, flujo recomendado), Stalker, «Otra…». Expulsar. | Menú de fuente + badges de tipo |
| Contenido | Grilla con selección estándar, filtros por tipo, agrupación por día/cámara, inspector lateral para asignar proyecto/perfil/cámara a la selección | Grilla + asignación avanzada + chips |
| Destinos | Archivo, SSD, copias para editar (ninguno/todo/elegidas), limpieza, trabajo diferido; espacio libre frente a tamaño del lote | Pickers + toggles |
| Revisar | Resumen del plan por proyecto y destino, advertencias, rutas en disclosure | `PlanReview` |
| Copiar | Progreso por fase y destino con barra real, ETA, cancelar en punto recuperable | Mensaje del pie |
| Listo | Qué quedó verificado, SD libre / formatear en cámara, expulsar, siguiente SD | Mensaje del pie |

- El paso actual vive en el modelo (sigue sobreviviendo al cambio de modo). Compacto usa
  los mismos pasos con menos columnas: es el mismo flujo, no otro formulario.
- **Atajo «exprés»** cuando el flujo recomendado no tiene ambigüedad (una cámara, un
  proyecto, destinos en línea): Fuente → Revisar directamente, con «Editar contenido»
  disponible. Mantiene el paso de revisión que exige [CONTEXT](../CONTEXT.md).
- **Inspector en vez de «Asignación avanzada».** La disclosure actual
  (Application.swift:590) repite rutas como texto y campos por unidad. Con `.inspector`
  (macOS 14) y selección múltiple en la grilla se asigna proyecto/perfil/cámara a varias
  unidades a la vez, que es justo el caso de tarjetas con varios proyectos.
- **Grilla:** clic selecciona, ⇧/⌘ extienden, ⌘A, Espacio abre Quick Look
  (`QLPreviewPanel`), y las cifras del resumen (Fotos, Videos, RAW…) funcionan como
  filtros. Marcar «copia para editar» pasa a ser una acción sobre la selección, no el
  significado del clic.

## 4. Biblioteca, Actividad, Inicio y Espacio

- **Biblioteca:** cambiar `List` por `Table` con columnas ordenables (proyecto, perfil,
  tipo, fecha, tamaño, estado de trabajo, ubicaciones), `.searchable` en la toolbar,
  filtros por estado y agrupación por proyecto. Acciones sobre la selección en el pie
  (como hoy) y en menú contextual.
- **Actividad:** cada job con barra de progreso real, fases como línea de tiempo,
  destinos por separado, duración, «Mostrar en Finder», «Copiar comando de reintento» y
  el detalle de motivos retenidos. Hoy es una fila con texto y un botón.
- **Inicio (Bandeja):** convertirlo en el centro de acciones de
  [dashboard-launcher](dashboard-launcher.md): tarjetas para «SD conectada», «Stalker
  pendientes», «Copias pendientes», «Requiere atención» y «Discos» (en línea, libre,
  reservado). Reducir los párrafos explicativos a una línea o a `help`.
- **Espacio:** mantenerlo como pestaña propia, pero con el mismo patrón paso a paso
  (auditar → revisar plan → reenlazar → liberar), porque también es destructivo.

## 5. Reactividad y rendimiento

### Puente con el motor

| Propuesta | Efecto | Riesgo |
| --- | --- | --- |
| **RPC `snapshot`** que devuelva volúmenes, biblioteca, jobs, perfiles, proyectos, ajustes y planes en una llamada | `refresh()` pasa de 9 procesos a 1 | Bajo; aditivo |
| **Proceso RPC persistente** hijo de la app (JSON por líneas con `id`, stdin/stdout), uno para lecturas y uno para mutaciones | Elimina el arranque por llamada; latencia de ms | Medio; requiere reinicio limpio si muere y no es un daemon del sistema |
| **Eventos de progreso empujados** por el mismo canal (`{"event":"progress",...}`) | Sustituye el sondeo de 2 s, progreso fluido | Medio |
| Cachear `diskutil info` por volumen montado hasta el siguiente evento de montaje | Menos llamadas lentas al conectar | Bajo |

El contrato de mensajes debe documentarse junto a
[media-library-v1](../contracts/media-library-v1.md); cancelar sigue interrumpiendo en un
punto recuperable.

### Estado SwiftUI

- Migrar `Model` a **`@Observable`** (Observation, disponible en macOS 14): cada vista se
  invalida sólo por lo que lee.
- Dividirlo por dominio: `SourcesStore` (SDs, Stalker, montajes), `IngestDraft` (fuente,
  unidades, asignaciones, plan, paso), `LibraryStore`, `ActivityStore` (jobs, progreso),
  `SettingsStore`. `Application.swift` (794 líneas) queda con el `AppDelegate` y la ventana.
- **`busy` por dominio**: una transferencia bloquea mutaciones de Ingreso/Espacio, no la
  navegación ni la lectura de la biblioteca. El motor ya protege con su lock entre
  procesos.
- Tipos en vez de cadenas: `SourceKind`, `CopyChoice`, `JobState`, etiquetas en un solo
  sitio. Quitar los `client!` y `NSScreen.main!` forzados.

### Miniaturas

- `ThumbnailLoader` codifica cada miniatura a PNG y la decodifica de nuevo
  ([ImportViews.swift:62](../../native/MediaMenu/Sources/MediaMenu/ImportViews.swift)).
  Guardar `CGImage` en la caché evita ese trabajo.
- Usar primero la miniatura que ya trae la cámara (Sony `THMBNL/*.JPG`, GoPro `.THM`,
  DJI `.LRF`) como imagen de vista previa; abrir el video sólo para el hover. Lectura más
  barata en SD lentas. No cambia qué se respalda ni la identidad de la unidad.
- Medir antes de ampliar concurrencia, como ya pide el plan.

## 6. Detección de SD y flujos recomendados

**Hoy:** `NSWorkspace.didMount` → espera 800 ms → `refresh()` (9 procesos) → por cada
volumen externo, `suggest` (otro proceso + `diskutil info`)
(Application.swift:533). Correcto en sus gates, pero lento y sin contexto.

**Propuesta:**

1. **DiskArbitration en Swift** (`DARegisterDiskAppearedCallback`,
   `DiskDescriptionChanged`) para saber al instante si es removible, expulsable, el
   protocolo (SD/USB), el UUID y el tamaño, sin `diskutil`. El motor sigue siendo la
   autoridad: valida identidad y destinos antes de ofrecer o expulsar.
2. **Huella superficial de la tarjeta**, sólo si hay destinos registrados en línea (respeta
   la regla de no escanear sin destinos): leer únicamente el primer y segundo nivel
   (`PRIVATE/M4ROOT`, `AVF_INFO`, `DCIM/100GOPRO`, carpetas `DJI_*`, `ZOOM*`/`FOLDER*`,
   `.insv`, bundles `.stalker`). Nunca clasificar todo DJI como dron: el resultado es
   «DJI detectado · elige dron/360/action».
3. **Flujo recomendado por tipo**, mostrado en la tarjeta de la fuente y en la
   notificación:

   | Huella | Recomendación |
   | --- | --- |
   | Sony XAVC | Respaldo + copia de trabajo del perfil; termina con formato en cámara |
   | Zoom H6 | Requiere SSD1 o confirmar excepción; audio por proyecto |
   | DJI | Pedir categoría por unidad antes de planear |
   | GoPro / Insta360 | Respaldo + copia; pares `.LRF`/`.THM` juntos |
   | Stalker | Lote por biblioteca con proyecto por sesión |

4. **Tarjeta conocida:** si el UUID ya se vio, sugerir el último perfil/proyecto usado con
   ella (sugerencia, nunca asignación silenciosa). Se conecta con el registro de
   dispositivos/modelos de P1.
5. **Espacio antes de planear:** mostrar tamaño de la tarjeta frente al libre en MEDIA y
   ESSD desde el paso Fuente, no hasta que el plan falle.

## 7. Estilo: más compacto y ordenado

- `CreatorButtonStyle` se aplica a **todos** los botones desde `CreatorAppearance`
  ([Appearance.swift:120](../../native/MediaMenu/Sources/MediaMenu/Appearance.swift)): fondo
  de tarjeta, borde y relleno de 12×7 en cada botón pequeño hacen la interfaz pesada.
  Proponer jerarquía: un botón primario relleno por vista, secundarios `.bordered` con
  `controlSize(.small)`, acciones en línea `.borderless`.
- **Tokens:** espaciado en múltiplos de 4/8, tres tamaños de texto en contenido
  (título, cuerpo, detalle) y colores semánticos (éxito, aviso, error, en curso) derivados
  de la misma paleta HSV, con contraste comprobado en claro y oscuro.
- **Estilo «New York»:** si se refiere a la tipografía serif de Apple, usarla sólo en
  títulos de vista y tarjetas de Inicio (`.fontDesign(.serif)`) y SF compacto para
  controles; si se refiere a un estilo editorial sobrio, la clave es la misma: menos cajas
  y bordes, más alineación a una retícula, una sola acción destacada.
- Barra de título densa: 28–32 pt de alto con toolbar compacta, sin la fila de logo
  repetida.

## 8. Idiomas: español e inglés

Hoy hay unas 140 cadenas literales en español en Swift, las etiquetas de estado se
mapean en `stateLabel`/`progressLabel`, y algunos errores del motor llegan en inglés
(`'Choose the root of a mounted removable SD/camera card'`, volumes.py) mientras la UI está
en español.

- **String Catalog** (`Localizable.xcstrings`) en el target `MediaMenu`:
  `defaultLocalization: "es"` en `Package.swift`, `resources: [.process("Resources")]`, y
  que `scripts/build_media_app.py` copie los `.lproj`/catálogo compilado y declare
  `CFBundleLocalizations` (`es`, `en`) en el `Info.plist`.
- Usar `LocalizedStringKey`/`String(localized:)`; `Text(variable)` no se traduce. Plurales
  («1 archivo / 3 archivos») en el catálogo, fechas/duraciones/bytes con `FormatStyle`.
- **El motor devuelve `code` + parámetros**; la UI traduce el código. El texto del motor
  queda en inglés para logs y CLI. Ya existe `BackendFailure.code`; falta catalogarlos.
- Idioma: seguir al sistema y permitir la preferencia por app de macOS (Ajustes del
  Sistema → Idioma y región → Apps); opcional un selector en Ajustes → Idioma.
- Prueba: pseudolocalización larga para detectar recortes en compacto, y una corrida en
  inglés de cada pantalla.

## 9. ¿Rust, Tauri o seguir nativo?

| Opción | Gana | Pierde | Cuándo |
| --- | --- | --- | --- |
| **A. SwiftUI + motor Python (actual), con RPC persistente** | Menor riesgo; aprovecha la validación existente | Sólo macOS; runtime Python a empaquetar | Ahora |
| **B. Tauri (Rust + web) como UI en todos los OS, motor como sidecar** | Una UI igual en mac/Windows/Linux | Sensación menos nativa (titlebar, Quick Look, AVFoundation, menubar requieren plugins); tres webviews distintos; sigue el sidecar Python | Si Windows/Linux se vuelven requisito real |
| **C. Núcleo en Rust (planner, copia, hash, catálogo) vía UniFFI para Swift y Tauri** | Sin runtime Python (hoy gate de release), binario único, rendimiento | Reescribir el código que protege contra pérdida de datos y repetir toda su validación | Sólo con el motor actual aceptado en hardware y con mediciones que lo pidan |
| **D. Rust sólo en rutas calientes** (hash/copia) como extensión Python | Rendimiento medido sin reescribir | Dependencia y build nativos | Si los benchmarks de copia/hash lo justifican |

**Recomendación:** A ahora, sin reescrituras. Lo que hace portable al proyecto es la
**frontera de mensajes**, no el framework de UI: documentar el protocolo RPC (y sus
eventos) como contrato. Con eso, una UI Tauri para Windows/Linux podría hablar con el
mismo motor manteniendo SwiftUI en macOS, y una UI web también serviría para un
dashboard. El trabajo por OS que no se evita con ninguna opción es identidad de volumen,
eventos de montaje, expulsión, E/S segura y empaquetado, tal como dice P3 del
[plan](../../PLAN.md). La UI idéntica en todos los OS cuesta la sensación nativa que hoy
es un valor del producto en Mac.

Para que la lógica de pantallas no se duplique si llega una segunda UI: el motor puede
describir el estado del flujo (paso actual, decisiones pendientes, advertencias) y cada UI
sólo lo presenta.

## 10. Robustez para producción

- Pruebas de la UI: hoy hay 7 pruebas Swift, todas del núcleo. Agregar pruebas del
  modelo/stores (pasos, invalidación del plan, `busy` por dominio) y capturas de los tres
  modos en ambos idiomas. Las pruebas de ventana (fullscreen, Spaces) requieren un proyecto
  Xcode/XCUITest o una matriz manual registrada en [release](../release.md).
- Evaluar mover la app de SwiftPM + bundler propio a un **proyecto Xcode** (o generado),
  que facilita String Catalog, asset catalog, entitlements, XCUITest y notarización.
- Accesibilidad: etiquetas en botones de solo icono, orden de foco, navegación completa por
  teclado en la grilla y en los pasos, contraste del acento claro.
- Errores sin ruta de recuperación, `applicationShouldTerminate` con bucle de espera y
  fuerza bruta en `cancel()` de todos los procesos: revisar al migrar al RPC persistente.

## Orden sugerido

| Fase | Contenido | Valida |
| --- | --- | --- |
| 1 · rápido | Toolbar nativa + opciones de fullscreen (bug), `Route` enum, Ajustes como escena, atajos, hojas en vez de `runModal`, confirmación destructiva | Matriz de ventana/fullscreen |
| 2 · fluidez | RPC `snapshot`, luego proceso persistente con eventos de progreso; `@Observable` y stores; `busy` por dominio; miniaturas sin PNG | Discovery y miniaturas medidos antes/después |
| 3 · flujo | Ingreso por pasos con inspector y selección múltiple; Actividad con progreso real; Biblioteca con `Table`/búsqueda | Aceptación con una SD pequeña autorizada |
| 4 · SD | DiskArbitration + huella superficial + flujo recomendado + espacio previo | Sony, H6, DJI, GoPro físicos, sin escanear sin destinos |
| 5 · idiomas | String Catalog es/en, códigos de error del motor, pseudolocalización | Capturas en ambos idiomas |
| 6 · plataformas | Protocolo RPC como contrato; decidir Tauri/Rust sólo con requisitos y mediciones | — |

Ninguna fase cambia las invariantes de [CONTEXT](../CONTEXT.md): plan revisado antes de
copiar, limpieza sólo tras verificar destinos requeridos y sin escaneo automático cuando no
hay destinos registrados en línea.
