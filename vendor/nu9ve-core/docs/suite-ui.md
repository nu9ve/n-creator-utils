# Controles comunes de la suite

Estado: **propuesta** (2026-11-22), sin código. Revisa cómo resuelve hoy cada app las fechas, los textos largos, los números, las listas, los mapas y medios, el teclado, la voz y el seguimiento, los estados y la accesibilidad, y propone una librería de controles por plataforma con un solo vocabulario. Los controles son nativos de cada sistema, leen los roles de [tema](../contracts/nu9ve-theme-v1.md) y de skin, y se entienden con el tipo de dato de cada app. Se apoya en el [contrato de interacción](../contracts/nu9ve-interaction-v1.md), en [macOS](../contracts/macos-app-v1.md) y en [Windows](../contracts/windows-app-v1.md), y no cambia ninguno: las reglas están en su contrato propio, [`nu9ve-controls-v1`](../contracts/nu9ve-controls-v1.md), también propuesta (decisión del usuario, 2026-11-22).

**Debe** y **Conviene** valen como en los contratos, pero aquí son propuesta.

## 1. Qué se leyó

Sólo lectura, el 2026-11-22, en el checkout de cada repo en la M1, sin `git fetch`. curado tiene 66 cambios sin commit: se leyó el árbol de trabajo.

| App | Commit | Interfaz | Dónde está la interfaz |
| --- | --- | --- | --- |
| nüün | `01b4c20` | SwiftUI/AppKit; WinUI 3 en C# | [`native/MediaMenu`][nuun-src], [`native/windows`][nuun-win] |
| nove | `a03a8e2` | SwiftUI/AppKit; WinUI 3 con XAML; gráficas en una página web común | [`apps/macos`][nove-src], [`apps/windows`][nove-win] |
| Clipper | `e27534e7` | TUI de Textual en Python (el foco); `ClipperStudio` comparte código con neuf | [`src/clipper_tui`][clip-src] |
| curado | `bb6f780` + árbol de trabajo | Tauri con React y TypeScript | [`src`][cur-src] |
| SoundBender (`soundshop`) | `add66ed` | SwiftUI | [`Sources/SoundShop`][sb-src] |
| kyu | `cd6d1c8` | AppKit con SwiftUI | [`apps/macos/Sources/Kyu`][kyu-src] |
| neuf | `b7214cc` | SwiftUI | [`apps/Neuf/Neuf`][neuf-src] |
| Stalker | `d5c48d7` | SwiftUI en la barra de menús | [`Sources/Stalker`][st-src] |
| video | `cf3f132` | Composiciones de Remotion; sin interfaz interactiva propia | [`src`][video-src] |
| Atlas | `d09d5b3` | Páginas web locales: visor con Leaflet y etiquetadores | [`viewer`][atlas-viewer], [`label`][atlas-label] |
| Fractos (Mac) | `048b41e` | SwiftUI con `FractosDesign` | [`Apps/macOS/FractosMac`][fr-mac], [`FractosDesign`][fr-design] |

Y en el core: `Nu9veTheme` y `NuMacShell` en [`swift/`](../swift/README.md); `Nu9ve.Core` (tema y [catálogo de atajos](../windows/Nu9ve.Core/Shell/Shortcuts.cs)) y `Nu9ve.WinUI` ([`UI.*`](../windows/Nu9ve.WinUI/Controls/UI.cs): `Button`, `Toggle`, `Keycap`, `Meter`, `Badge`, `Section`) en [`windows/`](../windows/README.md).

## 2. Inventario por tipo de interacción

«Qué tan resuelto» es un juicio leyendo el código, sin probarlo a mano: **bien** (reglas claras, teclado, accesibilidad), **parcial** o **básico** (el control del sistema tal cual).

### 2.1 Fechas, horas, rangos y líneas de tiempo

| App | Qué hace | Archivo | Qué tan resuelto |
| --- | --- | --- | --- |
| nove | Rango de una serie con dos `DatePicker` («Desde UTC», «Hasta UTC»), minutos sólo si el intervalo es de 60 s; el rango se aplica con un botón y el texto dice que el fin es exclusivo y que todo está en UTC | [`MarketHistory.swift`][nove-market] | Bien: dice la zona y los bordes |
| nove | Horario de una tarea: modo, zona, hora local y días de la semana, con una vista previa de las próximas ejecuciones que calcula el motor | [`Calendaring.swift`][nove-cal] | Bien: el cliente no calcula fechas |
| nove | Gráfica de velas con los huecos marcados (ámbar: el broker no mandó nada; gris: nove no leía) y los huecos largos comprimidos, con el lapso real en la leyenda | [`Chart/chart.js`][nove-chart] (Lightweight Charts en WebView, la misma página en Mac y Windows) | Bien: «el tiempo es honesto» |
| nüün | `Timestamps`: lee las fechas del motor (UTC con microsegundos) y de los manifiestos (minutos con desfase) y las muestra en la zona y el idioma de quien mira; lo ilegible se muestra como llegó | [`Timestamps.swift`][nuun-ts] | Bien, pero sólo en nüün |
| Fractos | Calendario de mes con agenda del día: flechas ±1 día y ±7, «Hoy», puntos por elemento, etiqueta accesible con la fecha completa y el número de elementos | [`CalendarScreen.swift`][fr-cal] | Bien |
| Fractos | `DatePicker` de fecha y hora para eventos; los intervalos se muestran «lun 3 nov, 10:00–11:30» | [`WorkView.swift`][fr-work] | Básico |
| neuf | `NeufTimecode` (`m:ss.cc` o `h:mm:ss.cc`, centésimas porque la cadencia cambia por reloj) y dos relojes, fuente y editado, elegibles en la transcripción | [`WorkspaceLayout.swift`][neuf-layout], [`ItemDetailViews.swift`][neuf-detail] | Bien en formato; la barra de posición es un `Slider` del sistema ([`TimelineViews.swift`][neuf-timeline]) |
| SoundBender | Una sola regla de tiempo para todas las pistas: arriba se arrastra el loop, abajo un clic mueve el cabezal y arrastrar selecciona tiempo; con snap a la rejilla y háptica en cada compás | [`TimelineViews.swift`][sb-timeline] | Bien |
| SoundBender | Tempo: BPM en negras con campo numérico, compás con `Stepper`, rejilla Libre/Guía/Snap, origen en segundos | [`TempoControls.swift`][sb-tempo] | Parcial: campos sin arrastre |
| Clipper | Línea de sincronización en la terminal (`SyncTimelineWidget`) | [`app.py`][clip-app] | Bien para la TUI |
| Stalker | Reloj de la grabación en dígitos monoespaciados y cuenta atrás | [`Views.swift`][st-views] | Básico |

### 2.2 Textos largos

| App | Qué hace | Archivo | Qué tan resuelto |
| --- | --- | --- | --- |
| Fractos | Editor de textos: título, resumen de 1 a 3 líneas, cuerpo en `TextEditor` con la fuente `body` del tema, ⌘S, el problema del borrador en línea y, si el texto trae formato de la web, lectura seleccionable con un aviso | [`TextLogsScreen.swift`][fr-text] | Bien |
| Clipper | Búsqueda en la transcripción: abrir, siguiente, anterior y marcar todas, con la consulta normalizada (`find_spans`, `normalize_query`) | [`app.py`][clip-app], `transcript_search.py` | Bien: el mejor buscador de la suite |
| neuf | Transcripción efectiva por segmentos, con tiempos en el reloj elegido y fichas de lo quitado y lo editado | [`ItemDetailViews.swift`][neuf-detail] | Parcial: sin búsqueda; textos en inglés |
| nove | Conversaciones de agentes leídas como tablas del catálogo | [`ConversationsPage.swift`][nove-conv] | Parcial |
| Atlas | `<textarea>` de notas en los etiquetadores | [`label`][atlas-label] | Básico |

Nadie más edita texto largo. Ninguna app de Mac tiene ⌘F dentro de un texto.

### 2.3 Números: sliders, perillas, campos con unidades, arrastrar

| App | Qué hace | Archivo | Qué tan resuelto |
| --- | --- | --- | --- |
| SoundBender | `ParameterScale` (lineal o logarítmica, 0…1 de ida y vuelta), `Knob` (arrastre vertical, ⌥ fino, doble clic restablece, acción ajustable de VoiceOver, ayuda contextual) y `XYPad` (dos parámetros en una superficie, con su valor escrito) | [`ParameterWidgets.swift`][sb-param] | Bien: 60 perillas en la app |
| curado | `Knob`, `Slider` vertical u horizontal y `Fader`: arrastre (⇧ fino, más fino con presión de Force Touch), rueda sin mover la página, flechas y Re Pág/Av Pág **sólo si se llegó con Tab**, Inicio al valor por omisión, Fin al máximo, doble clic restablece, `origin` (el centro de un EQ), muescas hápticas, `role="slider"` con `aria-valuetext` | [`Controls.tsx`][cur-controls] | Bien: el más completo |
| curado | `XY` con modos (color, filtro, beat, EQ…) y un eje Z | [`xy.ts`][cur-xy] | Bien |
| kyu | Parámetros de revelado que declara el motor (mínimo, máximo, paso, valor por omisión, etiqueta), escala logarítmica para Kelvin, snap al paso, la cifra en `ink` si cambió y en `muted` si no, doble clic en la etiqueta restablece y **una sola entrada de deshacer al soltar** (`interactive` mientras arrastra, `commit` al final) | [`InspectorView.swift`][kyu-inspector] | Bien: el mejor modelo de datos |
| nove (Windows) | `NumberBox` (8) y `Slider` (4) de WinUI | [`Pages`][nove-win-pages] | Básico |
| SoundBender | BPM y origen como `TextField` con formato numérico | [`TempoControls.swift`][sb-tempo] | Básico |
| Medidores | SoundBender: barras L/R con marcas de pico, siempre visibles ([`AudioMeter.swift`][sb-meter]); Stalker: pico de −60 a 0 dBFS con valor accesible en dBFS ([`Views.swift`][st-views]); curado: `Meter` y lámpara del limitador ([`Controls.tsx`][cur-controls]); `Nu9ve.WinUI`: `UI.Meter` | — | Tres copias del mismo control |

### 2.4 Listas y tablas grandes, filtros y selección múltiple

| App | Qué hace | Archivo | Qué tan resuelto |
| --- | --- | --- | --- |
| nove | `DataTable`: SwiftUI `Table` virtualizada, orden por cualquier columna (números como números, el resto como texto, nulos al final), filtro, esquema al lado, el texto exacto del motor sin redondear y `∅` para un nulo | [`DataTable.swift`][nove-table] | Bien: la referencia de tablas |
| nüün | `Table` con selección y orden en Discos y en el plan; filtro por estado en Proyectos con **dos vacíos distintos**: «Sin proyectos» y «Nada aquí» (el filtro lo oculta todo) | [`DisksView.swift`][nuun-disks], [`PlanTable.swift`][nuun-plan], [`ProjectsView.swift`][nuun-projects] | Bien |
| kyu | Mesa de luz y tira de película en un solo `NSCollectionView` con fuente de datos diffable; las flechas van a la rejilla y las letras al cribado; estrellas, elegida y rechazada dibujadas sin subvistas | [`GridController.swift`][kyu-grid] | Bien: la referencia de rejillas |
| kyu | Filtros compactos en la barra (calificación, marcas, color) y búsqueda de texto aparte | [`SidebarView.swift`][kyu-sidebar] | Bien |
| curado | Biblioteca con los problemas como filtros que se pulsan (faltan, sin conexión, duplicados) | [`Library.tsx`][cur-library] | Bien |
| nove (Windows) | `ItemsRepeater` (26), `ListView` (9), `ComboBox` (69) | [`Pages`][nove-win-pages] | Parcial |
| Clipper | Listas de la TUI con atajos por alcance | [`app.py`][clip-app] | Bien para la TUI |

### 2.5 Mapas, imágenes y medios

| App | Qué hace | Archivo | Qué tan resuelto |
| --- | --- | --- | --- |
| nove | Atlas en MapKit: capas que da el motor (`EngineTileOverlay`), polígonos y puntos con su referencia, y un **lienzo en blanco** que reemplaza el mapa de Apple cuando se apaga | [`AtlasPage.swift`][nove-atlas] | Bien |
| Fractos | `Map` de SwiftUI con anotaciones de lugares y eventos, cámara por región y vacíos propios («No está en el mapa público») | [`MapScreen.swift`][fr-map] | Bien |
| Atlas | Leaflet 1.9.4 **cargado de unpkg**, ortofotos en tiles y miniaturas en un canvas | [`viewer/index.html`][atlas-viewer] | Parcial: depende de la red |
| kyu | Lienzo de foto: ajustar o 100 %, rueda que desplaza a 100 %, pellizco para ampliar, foto sobre gris neutro | [`ImageCanvasView.swift`][kyu-canvas] | Bien |
| nüün | Vista rápida (Quick Look) de lo que llega de una SD | [`InboxView.swift`][nuun-inbox] | Bien |
| curado | Forma de onda general y ampliada, con `aria-valuenow` para buscar | [`Waveform.tsx`][cur-wave] | Bien |
| SoundBender | Espectrograma como lienzo | [`SpectrumView.swift`][sb-spectrum] | Bien, de dominio |
| neuf | Vista previa con su línea de tiempo | [`PreviewTimelineViews.swift`][neuf-preview] | Parcial |

### 2.6 Teclado, voz, seguimiento y gestos

| App | Qué hace | Archivo | Qué tan resuelto |
| --- | --- | --- | --- |
| SoundBender | `ShortcutCatalog`: una tabla que leen el panel ⌘/, la ayuda y la barra de estado, con prueba de duplicados y el nombre corto del keycap; `WorkspaceHelp` sigue el puntero | [`ShortcutsPanel.swift`][sb-shortcuts], [`WorkspaceHelp.swift`][sb-help] | Bien |
| curado | `keyboard.ts` (teclas por código físico, remapeo, conflictos, campos de texto respetados), `hints.ts` (ayuda por control) | [`keyboard.ts`][cur-keys], [`hints.ts`][cur-hints] | Bien: el único con remapeo |
| Clipper | `ShortcutSpec` con alcance y `visible_when` | [`app.py`][clip-app] | Bien |
| nüün (Windows) y `Nu9ve.Core` | Catálogo con alcance y prueba de teclas del sistema, en C# | [`Shortcuts.cs`](../windows/Nu9ve.Core/Shell/Shortcuts.cs) | Bien, ya compartido |
| kyu | Cribado de una tecla: 0–5, P/X/U | [`ContentController.swift`][kyu-content] | Bien |
| Atlas | Etiquetado con una tecla por clase, flechas, Retroceso, Esc, `b` para ocultar la metadata | [`cells.js`][atlas-cells] | Bien para su tarea |
| Fractos | `onKeyPress` en el calendario; señales de interacción (`FractosCue`) con sonido | [`CalendarScreen.swift`][fr-cal], [`FractosCue.swift`][fr-cue] | Parcial |
| **Voz** | Nadie usa la voz como entrada de la interfaz. Clipper y neuf transcriben audio fuera de línea (Whisper); Stalker graba el micrófono con procesado de voz y muestra «Entrada guardada · no disponible» si falta el dispositivo | [`transcribe.py`][clip-transcribe], [`Views.swift`][st-views] | — |
| **Seguimiento** | curado sigue manos y cara con la cámara (Apple Vision en un ayudante Swift): suavizado, pellizco con histéresis (0,45 para empezar, 0,7 para soltar), se suelta lo agarrado tras 300 ms sin ver la mano, el centro de la cámara es todo el rango y sólo corre con la ventana visible y enfocada | [`tracking.ts`][cur-tracking], [`CuradoTracker.swift`][cur-tracker] | Bien |
| | Stalker registra teclado y ratón para la sesión (no es una entrada de la interfaz) | [`InputEventRecorder.swift`][st-input] | — |
| **Gestos** | Presión de Force Touch para el ajuste fino (curado) y háptica en muescas (curado, SoundBender `GestureHaptics`); `HapticPlayer` y `PressureMonitor` ya están en `NuMacShell` | — | Parcial: sin regla común |

### 2.7 Carga, vacío, error, sin conexión y «disco no conectado»

| App | Qué hace | Archivo | Qué tan resuelto |
| --- | --- | --- | --- |
| Fractos | `FractosEmptyState`, `FractosCallout` (con tono), `LoadingMessage`, `FractosProgressBar`; vacíos que dicen qué hacer («Día libre: las tareas con fecha aparecen aquí») | [`FractosControls.swift`][fr-controls], [`ProfileScreen.swift`][fr-profile] | Bien: la única con piezas propias |
| nüün | `ContentUnavailableView` con texto propio («Sin discos: regístralos en Ajustes…»), `ProgressView` pequeño junto a lo que carga, la acción larga de `ActionProgress` (la referencia del contrato) | [`DisksView.swift`][nuun-disks], [`ActionProgress.swift`][nuun-progress] | Bien |
| nüün | **Disco no conectado:** «desconectado · 120 GB libres», proyectos de Clipper desconectados como aviso, entregas «No encontrada» | [`DisksView.swift`][nuun-disks], [`ProjectsView.swift`][nuun-projects] | Bien |
| kyu | **Disco no conectado:** la foto se atenúa al 50 % con una insignia, la barra dice «N fuera de línea» y la carpeta usa `externaldrive.badge.xmark` | [`GridController.swift`][kyu-grid], [`ContentController.swift`][kyu-content], [`SidebarView.swift`][kyu-sidebar] | Bien |
| curado | Pistas que faltan, sin conexión o duplicadas, como filtro | [`Library.tsx`][cur-library] | Bien |
| nove (Windows) | `InfoBar` (12), `AlertCard`, `StatusDot`, `SettingsCard` propios | [`Controls`][nove-win-controls] | Bien |
| **Sin conexión (red)** | Ninguna app tiene un estado común de red caída. Fractos muestra el error del servicio como `FractosCallout`; nove dice en sus fuentes si leía o no | — | Falta |

### 2.8 Accesibilidad y alto contraste

| App | Anotaciones (`accessibilityLabel`/`Value`, `AutomationProperties`, `aria-*`) | Reducir movimiento | Alto contraste |
| --- | --- | --- | --- |
| curado | 208 | Sí (`feedback.ts`) | Contraste Alto del tema |
| nove | 163 | — | Windows: diccionario `HighContrast` y colores del sistema ([`Look.cs`][nove-look]) |
| SoundBender | 125; acción ajustable en sus controles | Sí, en la escena | Del tema |
| nüün | 53 | Sí, en `ActionProgress` | Windows: sí ([`MainWindow.cs`][nuun-win-main]) |
| Fractos (Mac) | 26 en la app; los controles de `FractosDesign` | Sí, en los controles | Del tema |
| neuf | 25 | — | — |
| kyu | 9 | — | Sí: Aumentar contraste ([`Theme.swift`][kyu-theme]) |
| Atlas | 8 | — | — |
| Stalker | 1 | — | — |

## 3. Lo mejor de cada una

| Patrón | De | Por qué conviene a las demás |
| --- | --- | --- |
| **El parámetro lo declara el motor** (rango, paso, valor por omisión, etiqueta, escala) y el control sólo lo dibuja; vista previa mientras se arrastra y una sola entrada de deshacer al soltar | kyu | El rango no se repite en dos lugares, la app no inventa límites y deshacer no se llena de pasos intermedios. Es la [§0 de Windows](../contracts/windows-app-v1.md#0-principio-dos-apps-nativas-un-producto) aplicada a un slider |
| **La perilla completa**: rueda, flechas sólo con foco, Inicio al valor por omisión, `origin`, muescas hápticas, presión para el ajuste fino | curado | Es el único control de valor que se usa entero sin ratón y que se siente en el trackpad |
| `ParameterScale` y `XYPad` | SoundBender | La escala logarítmica es la misma para Hz, Kelvin y ganancia; el XY escribe su valor |
| **El tiempo es honesto**: zona y bordes escritos («UTC, fin excluido»), huecos marcados en vez de interpolados | nove | Evita leer mal un rango, que es el error más caro en datos de mercado o de sensores |
| **El motor calcula las fechas** (próximas ejecuciones de un horario) | nove | La app no repite zonas, horarios de verano ni días de la semana |
| Fechas del motor a la zona de quien mira, con respaldo legible | nüün `Timestamps` | Una sola función, probada, para todos los textos de fecha |
| Calendario por teclado (±1, ±7, Hoy) con etiqueta accesible completa | Fractos | Es el único selector de fecha de la suite que no es el del sistema y se maneja sin ratón |
| Dos relojes con nombre (fuente y editado) y un formato de tiempo fijo | neuf | El video y el audio editados siempre tienen dos tiempos; decir cuál evita errores de corte |
| Una sola regla de tiempo para todas las pistas | SoundBender | Clic, arrastre, loop y snap se comportan igual en cualquier carril |
| Tabla con texto exacto, `∅` para nulo y orden por tipo | nove | Un número nunca se redondea ni un nulo se vuelve cero |
| Dos vacíos: «aún no hay nada» y «el filtro lo oculta todo» | nüün | Cada uno lleva su salida (crear o quitar el filtro) |
| Rejilla diffable con flechas para moverse y letras para cribar | kyu | Escala a miles de fotos y respeta la regla de que una lista no roba las teclas |
| Los problemas como filtros | curado | Lo que falta se encuentra con un clic, sin una pantalla de errores |
| **Lo desconectado sigue a la vista**, atenuado, con insignia y cuenta | kyu y nüün | Un disco que no está no es un error ni hace desaparecer el trabajo |
| Buscar en un texto con siguiente, anterior y marcar todas | Clipper | Es lo que falta en todos los textos largos de Mac |
| Editor con el problema del borrador en línea y lectura seleccionable cuando no se puede editar | Fractos | El usuario sabe por qué no puede guardar sin un diálogo |
| Seguimiento con histéresis, soltar al perder la mano y correr sólo con la ventana visible | curado | Sin parpadeo ni notas colgadas, y sin usar la cámara a escondidas |
| Piezas de estado propias (`EmptyState`, `Callout`, `ProgressBar`, `Panel`, `Tabs`, `Disclosure`, `Badge`) y una galería de componentes | Fractos ([`ComponentGalleryScreen.swift`][fr-gallery]) | Es el punto de partida de la librería de Swift: ya sigue los skins |
| `SettingsCard`, `StatusDot`, `AlertCard` sin el Community Toolkit | nove (Windows) | nüün en Windows arma lo mismo con `UI.Section`: son dos usuarios |

**Hallazgos que no se arreglan aquí** (van a la sesión de cada app):
- neuf tiene la interfaz en inglés («Effective Transcript», «Clear Transcript»), y el [tema §9](../contracts/nu9ve-theme-v1.md#9-idioma) pide el español como base.
- Atlas carga Leaflet de unpkg: sin red, el visor no abre.
- Stalker tiene una sola anotación de accesibilidad; kyu, nueve.
- curado usa ⇧ para el ajuste fino, y el [contrato](../contracts/nu9ve-interaction-v1.md#2-todo-por-teclado) y SoundBender usan ⌥. El usuario decidió el 2026-11-22 que vale ⌥ ([pregunta 1](#9-preguntas-para-el-usuario)): curado pasa el fino a ⌥ y deja ⇧ para el paso grande. La presión de Force Touch sigue afinando.

## 4. Vocabulario común

Un control se llama igual en los documentos, en Swift, en WinUI y en TypeScript. En Swift lleva el prefijo `Nu9ve`; en C#, `Nu` dentro de `Nu9ve.WinUI`, como `NuButton`; en TypeScript, sin prefijo dentro de su módulo. Donde el sistema ya tiene el control, el nombre común es una **variante con reglas** del nativo, no un dibujo propio.

| Nombre | Swift | WinUI | TypeScript | Tipo de dato | Nativo de base |
| --- | --- | --- | --- | --- | --- |
| **Valor** (cifra con unidad, de sólo lectura) | `Nu9veValueText` | `NuValueText` | `ValueText` | `ParamSpec` + número | `Text` monoespaciado |
| **Perilla** | `Nu9veKnob` | `NuKnob` | `Knob` | Número acotado | Propio (no hay en el sistema) |
| **Deslizador** (horizontal o vertical; «fader» es el vertical de audio) | `Nu9veSlider` | `NuSlider` | `Slider` | Número acotado | `Slider` del sistema con las reglas de la §6.1 |
| **Plano XY** | `Nu9veXYPad` | `NuXYPad` | `XYPad` | Dos números acotados | Propio |
| **Campo numérico** (escribir, flechas, arrastrar la etiqueta) | `Nu9veNumberField` | `NuNumberField` | `NumberField` | Número con unidad | `TextField` con formato / `NumberBox` |
| **Medidor** | `Nu9veLevelMeter` | `NuLevelMeter` | `Meter` | Nivel en dBFS, o fracción | Propio; `UI.Meter` hoy |
| **Progreso de acción** | `Nu9veActionProgress` | `NuActionProgress` | — | `overall_done`/`overall_total` | [Interacción §7](../contracts/nu9ve-interaction-v1.md#acciones-largas) |
| **Código de tiempo** | `Nu9veTimecode` (formato) | `NuTimecode` | `timecode()` | Segundos | — |
| **Regla de tiempo** (con **cabezal**, loop y selección) | `Nu9veTimeRuler` | `NuTimeRuler` | `TimeRuler` | Ventana de segundos | Propio |
| **Barra de posición** | `Nu9veScrubber` | `NuScrubber` | `Scrubber` | Segundos y duración | `Slider` |
| **Fecha** (texto) | `Nu9veDateText` | `NuDateText` | `dateText()` | Instante ISO con zona | Formateadores del sistema |
| **Selector de fecha** | `DatePicker` con `Nu9veDateField` | `CalendarDatePicker` | `<input type="date">` | Día o instante | El del sistema |
| **Rango de fechas** | `Nu9veDateRangeField` | `NuDateRangeField` | `DateRangeField` | Inicio incluido, fin excluido, zona | Dos selectores del sistema |
| **Mes** (calendario con agenda) | `Nu9veMonthGrid` | `CalendarView` | — | Días con conteo | Propio en Mac |
| **Horario** | `Nu9veScheduleField` | `NuScheduleField` | — | Zona, hora local, días; vista previa del motor | Selectores del sistema |
| **Editor de texto** | `Nu9veTextEditor` | `RichEditBox`/`TextBox` | `textarea` | Texto plano | `TextEditor`/`NSTextView` |
| **Buscar en el texto** | `Nu9veFindBar` | `NuFindBar` | `FindBar` | Consulta y coincidencias | Barra de búsqueda de `NSTextView` donde alcance |
| **Tabla de datos** | `Nu9veDataTable` | `NuDataTable` | — | Filas con esquema | `Table`/`NSTableView`; `ItemsRepeater` |
| **Rejilla** (mesa de luz) | `Nu9veGrid` | `NuGrid` | — | Elementos con miniatura | `NSCollectionView`; `ItemsView` |
| **Filtro** (fichas o menú con cuenta) | `Nu9veFilterBar` | `NuFilterBar` | `FilterBar` | Conjunto de criterios | `Menu`/`Picker`; `ComboBox` |
| **Mapa** | `Nu9veMap` | — ([pregunta 5](#9-preguntas-para-el-usuario)) | — | Capas del motor | MapKit |
| **Lienzo de imagen** | `Nu9veImageCanvas` | `NuImageCanvas` | — | Imagen y zoom | `NSView` propio; `ScrollViewer` |
| **Catálogo de atajos**, **keycap**, **ayuda contextual**, **panel de atajos**, **HUD** | `Nu9veShortcutCatalog`, `Nu9veKeycap`, `Nu9veContextHelp`, `Nu9veShortcutsPanel`, `Nu9veKeyHUD` | `ShortcutCatalog` (ya en `Nu9ve.Core`), `UI.Keycap`, … | Los de curado | Acciones | [Interacción §1 a §7](../contracts/nu9ve-interaction-v1.md) |
| **Cargando** | `Nu9veLoadingState` | `NuLoadingState` | `LoadingState` | — | `ProgressView`/`ProgressRing` |
| **Vacío** | `Nu9veEmptyState` | `NuEmptyState` | `EmptyState` | Causa (nada aún, filtrado) | `ContentUnavailableView` |
| **Aviso** (información, advertencia, error) | `Nu9veCallout` | `InfoBar` con tema | `Callout` | Tono y texto | — |
| **Sin conexión** | `Nu9veOfflineState` | `NuOfflineState` | `OfflineState` | Servicio y última lectura | — |
| **Disco no conectado** | `Nu9veVolumeBadge` | `NuVolumeBadge` | `VolumeBadge` | Estado de [`nu9ve-volume-v1`](../contracts/nu9ve-volume-v1.md) | Insignia propia |
| **Punto de estado** | `Nu9veStatusDot` | `StatusDot` (nove) | `StatusDot` | Estado de un servicio | — |
| **Tarjeta de ajuste** | (`Form` del sistema) | `NuSettingsCard` | — | Opción con su efecto | `SettingsCard` de nove |
| **Panel**, **Pestañas**, **Divulgación**, **Insignia** | `Nu9vePanel`, `Nu9veTabs`, `Nu9veDisclosure`, `Nu9veBadge` | `NuSurface`, `TabView`, `Expander`, `UI.Badge` | — | — | De `FractosDesign` |

## 5. Dónde va cada librería

### 5.1 `swift/`: `Nu9veFormat` y `Nu9veControls`

- **`Nu9veFormat`**, sólo Foundation y sin interfaz: `ParamSpec` (rango, paso, valor por omisión, `origin`, escala, unidad, formato), `ValueScale` (lineal y logarítmica, el `ParameterScale` de SoundBender), el paso por teclado, el código de tiempo, las duraciones, las fechas (`Nu9veDateText`: leer ISO del motor y mostrar en la zona de quien mira), los rangos con borde excluido y los bytes. Se prueba con vectores en `data/` (§5.4), como el tema.
- **`Nu9veControls`**, SwiftUI con AppKit donde haga falta (`NSCollectionView`, `NSTextView`), depende de `Nu9veTheme` y `Nu9veFormat`: los controles de la §4. Nace con la base de `FractosDesign` (estados, panel, pestañas) y con las piezas de SoundBender, kyu y nüün. macOS 15, como el resto del paquete.
- **iOS, control por control** ([pregunta 3](#9-preguntas-para-el-usuario), decidida): `Nu9veFormat` compila en macOS e iOS, porque sólo usa Foundation. En `Nu9veControls` compila en los dos lo que es SwiftUI puro y sirve a una pantalla táctil (estados, valor, medidor, perilla, deslizador, XY, mes), con zonas de toque de 44 pt. Lo que depende de AppKit (rejilla en `NSCollectionView`, editor en `NSTextView`, lienzo de imagen) va dentro de `#if os(macOS)` hasta que haga falta su par táctil. Cada pieza dice en qué sistemas compila.
- `NuMacShell` no crece: queda para la ventana, el Acerca de y la háptica. `Nu9veControls` usa su `HapticPlayer` y su `PressureMonitor`.
- Ningún control nombra una app (`scripts/check-boundary.sh` ya lo revisa). Los textos que se ven llegan de la app o de un catálogo de cadenas con español e inglés.

### 5.2 `windows/`: `Nu9ve.Core` y `Nu9ve.WinUI`

- La lógica de `Nu9veFormat` va en `Nu9ve.Core/Format/` con los mismos vectores. El catálogo de atajos ya está en `Nu9ve.Core/Shell/Shortcuts.cs`.
- Los controles van en `Nu9ve.WinUI/Controls/` junto a `NuButton`, `NuSurface` y `UI.*`. Los que WinUI ya tiene (`NumberBox`, `Slider`, `CalendarDatePicker`, `InfoBar`, `ItemsRepeater`) se usan tal cual, con los pinceles de tema en sus claves ([Windows §3](../contracts/windows-app-v1.md#3-tema)) y un ayudante que les aplica las reglas de la §6 (paso fino, restablecer, formato). Lo que no existe (perilla, XY, regla de tiempo, medidor, insignia de disco) se dibuja con el tema.
- `SettingsCard`, `StatusDot` y `AlertCard` suben de nove, con nüün como segundo usuario ([§7](#7-orden-de-extracción)). Encaja con [lo que se repite en Windows](windows-shared-next.md).

### 5.3 TypeScript

curado (Tauri), el visor de Atlas, la gráfica común de nove (`chart.js`, una página web también en Mac) y la web de Fractos tienen interfaz web. Hoy el core no tiene carpeta de TypeScript (Ola 3, «paleta y dither en TypeScript»). **Decidido: todavía no** ([pregunta 2](#9-preguntas-para-el-usuario)). La web de Fractos ya tiene su librería de interfaz: primero la documenta, y se decide allá si va a un core de web. Mientras:
- el core da las reglas (este documento y su contrato) y los vectores de `data/`, que curado prueba con su copia, como ya hace con los de tema;
- la gráfica de nove sigue en su repo;
- si después se decide un core de web, su base es la librería de la web de Fractos, y curado es el segundo usuario (`Knob`, `Slider`, `Meter`, `FilterBar`, `timecode()`, `dateText()`). Podría usar `Nu9veFormat` o los crates de Rust por WebAssembly en vez de copiarlos.

### 5.4 Vectores comunes

`data/ui-format-vectors-v1.json` (nuevo, cuando se construya `Nu9veFormat`): por cada caso, la entrada y lo esperado. Escala lineal y logarítmica (ida y vuelta), paso normal, grande y fino, restablecer, `snap`, código de tiempo (negativos, horas, redondeo de centésimas, no finitos como `--:--.--`), fechas del motor (con y sin fracción, con desfase, ilegibles) y bytes. Lo generan Swift o C#, y las otras dos lo comprueban, igual que [`theme-vectors-v1.json`](../data/theme-vectors-v1.json). El texto de una fecha depende del idioma y la zona, así que los vectores fijan `es_MX`, `en_US` y dos zonas.

## 6. Reglas por tipo de dato

Desde el 2026-11-22 las reglas están en su contrato, [`nu9ve-controls-v1`](../contracts/nu9ve-controls-v1.md) (propuesta), y ya no se repiten aquí. Las referencias de este documento llevan a:

| Aquí | En el contrato |
| --- | --- |
| §6.0 Tema y skins | [§2](../contracts/nu9ve-controls-v1.md#2-tema-y-skins) |
| §6.1 Números | [§3](../contracts/nu9ve-controls-v1.md#3-números) |
| §6.2 Fechas, horas y tiempo | [§4](../contracts/nu9ve-controls-v1.md#4-fechas-horas-y-tiempo) |
| §6.3 Textos largos | [§5](../contracts/nu9ve-controls-v1.md#5-textos-largos) |
| §6.4 Listas, tablas y selección | [§6](../contracts/nu9ve-controls-v1.md#6-listas-tablas-y-selección) |
| §6.5 Mapas, imágenes y medios | [§7](../contracts/nu9ve-controls-v1.md#7-mapas-imágenes-y-medios) |
| §6.6 Teclado, voz, seguimiento y gestos | [§8](../contracts/nu9ve-controls-v1.md#8-voz-seguimiento-y-gestos) |
| §6.7 Estados | [§9](../contracts/nu9ve-controls-v1.md#9-estados) |
| §6.8 Accesibilidad | [§10](../contracts/nu9ve-controls-v1.md#10-accesibilidad) |

## 7. Orden de extracción

La regla de [AGENTS](../AGENTS.md): una pieza entra cuando dos apps la usan igual o nació genérica, en un cambio propio, con su prueba y su línea en el CHANGELOG, y la app de origen deja su primera copia fijada en el mismo paso.

| # | Pieza | Destino | App de origen | Segundo usuario | Condición |
| --- | --- | --- | --- | --- | --- |
| C1 | **Hecho el 2026-11-22** (Swift; C# sin correr en Windows). `Nu9veFormat`: `ParamSpec`, `ValueScale`, pasos, código de tiempo, duraciones, fechas, bytes, con `data/ui-format-vectors-v1.json` | `swift/`, `Nu9ve.Core/Format` | SoundBender (`ParameterScale`), neuf (`NeufTimecode`), nüün (`Timestamps`) | kyu (escala logarítmica y snap), Stalker (reloj), curado (vectores en TS) | Nace genérica. Que el usuario apruebe este documento |
| C2 | Estados: `Nu9veLoadingState`, `Nu9veEmptyState` (con sus dos causas), `Nu9veCallout`, `Nu9veVolumeBadge` | `swift/` (`Nu9veControls`) | Fractos (`FractosEmptyState`, `FractosCallout`, `LoadingMessage`) | nüün (`ContentUnavailableView` y «desconectado»), kyu (fuera de línea) | La insignia, con `nu9ve-volume-v1` ya en el core. Fractos se sigue viendo igual por omisión |
| C3 | `SettingsCard`, `StatusDot`, `AlertCard` | `Nu9ve.WinUI` | nove (Windows) | nüün (Windows), que hoy los arma con `UI.Section` | Junto con [lo que se repite en Windows](windows-shared-next.md) |
| C4 | `Nu9veLevelMeter` / `NuLevelMeter` | `swift/`, `Nu9ve.WinUI` (reemplaza `UI.Meter`) | SoundBender (`MasterMeterBars`) | Stalker (`LevelMeter`); curado, por las reglas | Ninguna: tres copias del mismo control |
| C5 | `Nu9veKnob`, `Nu9veSlider`, `Nu9veXYPad`, `Nu9veNumberField` sobre C1 | `swift/`, después `Nu9ve.WinUI` | SoundBender (`Knob`, `XYPad`), con las reglas de curado | kyu (el deslizador del inspector, con `interactive`/`commit`) | C1 hecho |
| C6 | Catálogo de atajos, keycap, ayuda contextual, panel ⌘/ y HUD en Swift, con la misma forma que `Nu9ve.Core/Shell/Shortcuts.cs` | `swift/` | SoundBender (`ShortcutCatalog`, `WorkspaceHelp`) | nüün en Mac (su primer paso sugerido en [adopción](adoption.md#nu9ve-interaction-v1)) o kyu | Es la fila de Ola 3 «`ShortcutCatalog` y ayuda contextual»: la cumple la sesión de nüün o kyu que adopte el contrato |
| C7 | `Nu9veTimeRuler` y `Nu9veScrubber` sobre C1 | `swift/` | SoundBender (`TimeRuler`) | neuf (su barra de posición), cuando salga de la pausa | Que Clipper cierre su migración (neuf espera) |
| C8 | `Nu9veDataTable` | `swift/` | nove (`DataTable`) | nüün (`PlanTable`, `DisksView`) | Comparar las dos tablas de nüün con la de nove antes |
| C9 | `Nu9veDateRangeField`, `Nu9veScheduleField`, `Nu9veMonthGrid` sobre C1 | `swift/` | nove (rango y horario), Fractos (mes) | Fractos (rango de eventos) o nüün | Una segunda app con rango de fechas o horario |
| C10 | `Nu9veTextEditor` y `Nu9veFindBar` | `swift/` | Fractos (editor), con las reglas de búsqueda de Clipper | neuf (transcripción) | Que neuf o una app de Mac necesite buscar en un texto |
| C11 | `Nu9veGrid` y `Nu9veImageCanvas` | `swift/` | kyu | nüün (vista de lo que llega) o SoundBender (biblioteca) | Una segunda rejilla grande en Mac |
| C12 | `Nu9veMap` (capas del motor, lienzo sin red) | `swift/` | nove (`AtlasPage`) | Fractos (`MapScreen`) | Las dos usan MapKit de forma distinta: comparar antes. En Windows, lo más nativo posible y si no una página empaquetada, al probarlo en una máquina con Windows |
| C13 | Seguimiento por cámara (histéresis, pérdida, visibilidad) sobre `Nu9veVision` | `swift/` | curado | — | Una segunda app que lo use |
| C14 | Core de web | Por decidir, fuera de este repo o en él | Web de Fractos (su librería) | curado | Que Fractos documente su librería de interfaz, y decidirlo allá (pregunta 2) |
| G1 | Estudio de gráficas, sin código: Swift Charts, la página común de nove y un motor propio (cálculo en Rust, la vista sólo muestra), medidos con las series de nove por cuánto se configuran, cuánto cuestan con muchos puntos y qué dejan crear o explorar | `docs/` | nove | — | Ninguna; cuando convenga. Mientras, nove sigue con su página |

C1 a C4 se pueden hacer ya. Las reglas ya están en el contrato propio [`nu9ve-controls-v1`](../contracts/nu9ve-controls-v1.md) (pregunta 8, decidida); cada pieza que entra lo cumple y lo prueba. Las gráficas no se extraen hasta el estudio G1: sólo nove tiene.

## 8. Cómo lo adopta una app

Una app adopta un control en su repo, en un cambio propio:
1. vuelve a fijar el core;
2. reemplaza su copia por el control común, sin cambiar cómo se ve con su skin original;
3. borra la copia;
4. anota en [adopción](adoption.md) qué reglas de la §6 cumple.

Lo que su dominio necesita y el común no tiene (la cabina de curado, el pincel de SoundBender) queda en la app y se compone con los comunes.

## 9. Preguntas para el usuario

1. ~~**Ajuste fino:** ¿⌥, como el contrato y SoundBender, o ⇧, como curado?~~ **Decidido por el usuario el 2026-11-22:** ⌥ para el ajuste fino y ⇧ para el paso grande, como dice el [contrato](../contracts/nu9ve-interaction-v1.md#2-todo-por-teclado). El contrato no cambia; curado se alinea en su sesión.
2. ~~**TypeScript en el core.**~~ **Decidido por el usuario el 2026-11-22:** todavía no, y la web no es el foco por ahora. La web de Fractos (`nu9ve.xyz`) ya tiene una librería de interfaz avanzada. Primero Fractos deja documentado qué tiene y dónde. Con eso a la vista se decide allá si se muda a un core de web aparte, que podría integrar también `nu9ve/inner-core` o usar el core por WebAssembly. Mientras, el core da a la web sólo reglas y vectores (§5.3).
3. ~~**iOS.**~~ **Decidido por el usuario el 2026-11-22:** se decide control por control, según qué es mejor en cada caso. Compila también en iOS lo que se puede reusar o se relaciona, pensando también en una Mac táctil; lo que es propio de AppKit queda sólo en macOS (§5.1).
4. ~~**Gráficas.**~~ **Dirección del usuario, 2026-11-22:** a la larga, lo más configurable y eficiente, que dé más opciones para crear y entender los datos. Puede ser un motor propio que calcula en Rust y una vista que sólo muestra el resultado. La página web común de nove se hizo porque era rápida de tener en las dos plataformas, y se queda mientras tanto. Falta un estudio que compare Swift Charts, la página y un motor propio con los datos de nove (G1 en la §7).
5. ~~**Mapas en Windows.**~~ **Decidido por el usuario el 2026-11-22:** lo más nativo posible. Si no hay nada nativo sin servicio de pago, una página web empaquetada sin CDN. Se decide cuando haya una máquina con Windows para probarlo. WinUI no trae un mapa que funcione sin un servicio con clave: el de UWP dependía de Bing Maps, ya retirado.
6. ~~**Voz.**~~ **Decidido por el usuario el 2026-11-22:** no es prioridad, sólo se documenta. Hoy, buena parte se logra hablando con un agente por MCP y con la voz en la entrada del agente. Puede volver con interfaces de chat internas para hablar directo con distintos agentes o modelos y variar cómo se usa el MCP. Las reglas de la §6.6 quedan escritas para ese momento.
7. ~~**Fechas relativas.**~~ **Aprobado por el usuario el 2026-11-22:** fecha absoluta por omisión, y «hace 5 min» sólo en listas de actividad (§6.2).
8. ~~**Dónde se escriben las reglas.**~~ **Decidido por el usuario el 2026-11-22: un contrato propio, `contracts/nu9ve-controls-v1.md`**, con su tabla en [adopción](adoption.md). Nace como propuesta, igual que los demás, con las reglas de la §6. El de interacción no cambia. Lo que se pesó:

   Este documento es una propuesta. Las reglas de la §6 (cómo se comporta una perilla, cómo se muestra una fecha, qué dice un disco desconectado) tienen que quedar en un contrato, que es lo que las apps fijan y cumplen. Hay dos caminos:
   - **Un contrato nuevo, `nu9ve-controls-v1`** (recomendado): un documento propio para los controles por tipo de dato, al lado del de tema y el de interacción. Cambia y sube de versión sin tocar los otros, y tiene su propia tabla de qué cumple cada app.
   - **Una sección nueva en `nu9ve-interaction-v1`:** menos documentos, pero ese contrato es sobre teclado, ayuda y atajos, y crecería mucho. Cada cambio de un control lo movería entero.

[nuun-src]: ../../../nu9ve/n-creator-utils/native/MediaMenu/Sources/MediaMenu
[nuun-win]: ../../../nu9ve/n-creator-utils/native/windows
[nuun-win-main]: ../../../nu9ve/n-creator-utils/native/windows/Nuun/MainWindow.cs
[nuun-ts]: ../../../nu9ve/n-creator-utils/native/MediaMenu/Sources/MediaMenuCore/Timestamps.swift
[nuun-disks]: ../../../nu9ve/n-creator-utils/native/MediaMenu/Sources/MediaMenu/DisksView.swift
[nuun-plan]: ../../../nu9ve/n-creator-utils/native/MediaMenu/Sources/MediaMenu/PlanTable.swift
[nuun-projects]: ../../../nu9ve/n-creator-utils/native/MediaMenu/Sources/MediaMenu/ProjectsView.swift
[nuun-inbox]: ../../../nu9ve/n-creator-utils/native/MediaMenu/Sources/MediaMenu/InboxView.swift
[nuun-progress]: ../../../nu9ve/n-creator-utils/native/MediaMenu/Sources/MediaMenu/ActionProgress.swift
[nove-src]: ../../nove/apps/macos/Sources/Nove
[nove-win]: ../../nove/apps/windows/Nove
[nove-win-pages]: ../../nove/apps/windows/Nove/Pages
[nove-win-controls]: ../../nove/apps/windows/Nove/Controls
[nove-look]: ../../nove/apps/windows/Nove/Look.cs
[nove-market]: ../../nove/apps/macos/Sources/Nove/MarketHistory.swift
[nove-cal]: ../../nove/apps/macos/Sources/Nove/Calendaring.swift
[nove-chart]: ../../nove/apps/windows/Nove/Chart/chart.js
[nove-conv]: ../../nove/apps/macos/Sources/Nove/ConversationsPage.swift
[nove-table]: ../../nove/apps/macos/Sources/Nove/DataTable.swift
[nove-atlas]: ../../nove/apps/macos/Sources/Nove/AtlasPage.swift
[clip-src]: ../../clipper/src/clipper_tui
[clip-app]: ../../clipper/src/clipper_tui/app.py
[clip-transcribe]: ../../clipper/src/clipper_tui/transcribe.py
[cur-src]: ../../curado/src
[cur-controls]: ../../curado/src/components/Controls.tsx
[cur-xy]: ../../curado/src/lib/xy.ts
[cur-library]: ../../curado/src/components/Library.tsx
[cur-wave]: ../../curado/src/components/Waveform.tsx
[cur-keys]: ../../curado/src/lib/keyboard.ts
[cur-hints]: ../../curado/src/lib/hints.ts
[cur-tracking]: ../../curado/src/lib/tracking.ts
[cur-tracker]: ../../curado/src-tauri/native/CuradoTracker.swift
[sb-src]: ../../soundshop/Sources/SoundShop
[sb-param]: ../../soundshop/Sources/SoundShop/ParameterWidgets.swift
[sb-timeline]: ../../soundshop/Sources/SoundShop/TimelineViews.swift
[sb-tempo]: ../../soundshop/Sources/SoundShop/TempoControls.swift
[sb-meter]: ../../soundshop/Sources/SoundShop/AudioMeter.swift
[sb-shortcuts]: ../../soundshop/Sources/SoundShop/ShortcutsPanel.swift
[sb-help]: ../../soundshop/Sources/SoundShop/WorkspaceHelp.swift
[sb-spectrum]: ../../soundshop/Sources/SoundShop/SpectrumView.swift
[kyu-src]: ../../kyu/apps/macos/Sources/Kyu
[kyu-inspector]: ../../kyu/apps/macos/Sources/Kyu/InspectorView.swift
[kyu-grid]: ../../kyu/apps/macos/Sources/Kyu/GridController.swift
[kyu-sidebar]: ../../kyu/apps/macos/Sources/Kyu/SidebarView.swift
[kyu-content]: ../../kyu/apps/macos/Sources/Kyu/ContentController.swift
[kyu-canvas]: ../../kyu/apps/macos/Sources/Kyu/ImageCanvasView.swift
[kyu-theme]: ../../kyu/apps/macos/Sources/Kyu/Theme.swift
[neuf-src]: ../../neuf/apps/Neuf/Neuf
[neuf-layout]: ../../neuf/apps/Neuf/Neuf/WorkspaceLayout.swift
[neuf-detail]: ../../neuf/apps/Neuf/Neuf/Features/Details/ItemDetailViews.swift
[neuf-timeline]: ../../neuf/apps/Neuf/Neuf/Features/Timeline/TimelineViews.swift
[neuf-preview]: ../../neuf/apps/Neuf/Neuf/PreviewTimelineViews.swift
[st-src]: ../../stalker/Sources/Stalker
[st-views]: ../../stalker/Sources/Stalker/Views.swift
[st-input]: ../../stalker/Sources/StalkerCapture/InputEventRecorder.swift
[video-src]: ../../video/src
[atlas-viewer]: ../../../data/atlas/viewer/index.html
[atlas-label]: ../../../data/atlas/label
[atlas-cells]: ../../../data/atlas/label/cells/cells.js
[fr-mac]: ../../../nu9ve/ios-fractos/Apps/macOS/FractosMac
[fr-design]: ../../../nu9ve/ios-fractos/Packages/FractosKit/Sources/FractosDesign
[fr-controls]: ../../../nu9ve/ios-fractos/Packages/FractosKit/Sources/FractosDesign/FractosControls.swift
[fr-cue]: ../../../nu9ve/ios-fractos/Packages/FractosKit/Sources/FractosDesign/FractosCue.swift
[fr-cal]: ../../../nu9ve/ios-fractos/Apps/macOS/FractosMac/CalendarScreen.swift
[fr-work]: ../../../nu9ve/ios-fractos/Apps/macOS/FractosMac/WorkView.swift
[fr-text]: ../../../nu9ve/ios-fractos/Apps/macOS/FractosMac/TextLogsScreen.swift
[fr-map]: ../../../nu9ve/ios-fractos/Apps/macOS/FractosMac/MapScreen.swift
[fr-profile]: ../../../nu9ve/ios-fractos/Apps/macOS/FractosMac/ProfileScreen.swift
[fr-gallery]: ../../../nu9ve/ios-fractos/Apps/macOS/FractosMac/ComponentGalleryScreen.swift
