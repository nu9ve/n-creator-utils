# Apps de macOS de nu9ve v1

Estado: **en adopción**. Su par para Windows, con la tabla de equivalencias, es [apps de Windows v1](windows-app-v1.md); el mapa de apps está en [la suite](../suite.md). Qué debe cumplir una app nativa de nu9ve para sentirse de Mac y poder distribuirse, sin importar lo que haga. Igual que los demás contratos, separa la regla de quién la adoptó: ver [adopción](#adopción). Cumplirlo no declara una app lista para producción; eso lo dicen sus propios [requisitos de release](../release.md).

Referencias: **SoundShop** para una app de documento en ciclo de vida SwiftUI (`Sources/SoundShop/WindowChrome.swift`, `AppCommands.swift`, `AboutWindow.swift`, `DocumentSession.swift`) y **nüün** para una app con ciclo de vida AppKit y barra de menús ([Application.swift](../../native/MediaMenu/Sources/MediaMenu/Application.swift)).

**Debe** es obligatorio salvo que la app documente por qué no aplica. **Conviene** es lo recomendado.

## 1. Barra de título

- **Debe** poner lo global en una sola fila junto a los botones de ventana, sin franja vacía encima: `NSToolbar` con `toolbarStyle = .unifiedCompact`, `titleVisibility = .hidden`, `titlebarSeparatorStyle = .none` y `titlebarAppearsTransparent = true`.
- **Debe** mostrar la identidad a la izquierda (logo y nombre, y el documento si lo hay), la navegación principal al centro y las acciones a la derecha, con la acción principal destacada.
- **Debe** usar un elemento flexible con `isBordered = false` (macOS 26 lo envolvería en una plataforma de vidrio) y `NSHostingView` con `sizingOptions = []`. Esa vista no hereda el entorno de SwiftUI: tema, stores y acciones se pasan explícitamente.
- **Debe** hacer que el espacio vacío arrastre la ventana y que el doble clic respete `AppleActionOnDoubleClick` (zoom, minimizar o nada). Fijar el zoom o consumir el evento con un monitor global no cumple.
- **Debe** ceder por pasos al estrechar, con una regla pura que se pueda probar: primero textos secundarios y nombres; la navegación al final. Las acciones no se truncan.
- `Label` dentro de un toolbar `.iconOnly` pierde el texto: usar `.labelStyle(.titleAndIcon)` donde deba verse.

## 2. Ventana y tamaños

- **Debe** tener tamaño mínimo real, probado con todo su contenido, y recordar su marco. Los paneles recuerdan visibilidad y tamaño si el usuario los cambia.
- **Debe** usar `Window` y no `WindowGroup` si hay un único estado por proceso, y desactivar las pestañas automáticas.
- **Debe** funcionar en pantalla completa sin perder navegación ni acciones, y sin depender de nombres de clases privadas. Dos formas válidas: `window(_:willUseFullScreenPresentationOptions:)` si la app posee el delegado, o llevar la barra al contenido si es de SwiftUI. Tras salir, comprobar que el toolbar volvió: AppKit restaura el estado guardado al entrar.
- En SwiftUI, **no se reemplaza** `window.delegate`. Usar notificaciones de `NSWindow`.
- **Debe** minimizar el chrome: el contenido es la razón de ser de la ventana. Medir el alto y ancho que ocupa, no estimarlo.

## 3. Temas

Qué se configura, cómo se nombran los colores y qué es un skin: [tema visual v1](nu9ve-theme-v1.md). Esta sección cubre solo su aplicación a la ventana.

- **Debe** aplicar el tema a la ventana (`backgroundColor`, `appearance`), no solo al contenido, para que barra, sheets y controles nativos coincidan. Un solo punto, por valor. Vale para **todas** las ventanas: Ajustes, Acerca de, sheets, paneles y auxiliares, con su barra ([tema §8](nu9ve-theme-v1.md#8-barras-del-sistema-menús-y-controles)). Una barra de título sin tema en una subventana es un error.
- **Debe** verificar contraste de texto (4.5:1) en el tema por defecto, y texto sobre el acento.
- Los menús respaldados por AppKit pueden conservar el color de texto al cambiar de claro a oscuro: probar ese cambio con la app abierta.
- **Conviene** ofrecer Claro, Oscuro y Sistema. Observar `NSApp.effectiveAppearance`, no notificaciones privadas.

## 4. Barra de menús

- **Debe** dejar todo accesible desde menús, con estado (`disabled`) y atajo, también lo que ya tiene botón. El catálogo de atajos, los badges, la ayuda contextual y el panel ⌘/ están en [teclado e interacción v1](nu9ve-interaction-v1.md).
- **Debe** conservar los elementos del sistema: Servicios, Ocultar otras, Mostrar todo, Zoom, Traer todo al frente, Dictado, Emojis. En SwiftUI, reemplazar grupos con `CommandGroup` y no el menú entero. En AppKit, añadirlos a mano.
- **Debe** declarar `CFBundleDevelopmentRegion` y `CFBundleLocalizations`, o el sistema mezcla idiomas: los elementos de macOS salen en el idioma del sistema y los propios en el de la app.
- **Debe** respetar un campo de texto activo: ⌘Z, ⌘C, ⌘X, ⌘V y ⌘A no actúan sobre el documento mientras se escribe.
- **Conviene** que Deshacer nombre la operación. El menú Ayuda **debe** llevar a contenido real, no al aviso de que no hay ayuda.

## 5. Acerca de

Toda app **debe** tener un elemento «Acerca de <App>» en el menú de la app que abra una ventana con:

| Elemento | Regla |
| --- | --- |
| Logo | Icono de la app, a 64 pt o más |
| Nombre y frase | Una línea que diga qué es |
| Versión | `Versión 1.2.3 (45)`, leída del bundle. Clic copia versión, compilación, macOS y arquitectura para reportes |
| Sitio | [nu9ve.xyz](https://nu9ve.xyz) |
| Código | [github.com/nu9ve](https://github.com/nu9ve) |
| Copyright | `© <año> nu9ve`, igual que `NSHumanReadableCopyright` |

Sigue el tema de la app, no cambia de tamaño y no se restaura al abrir. Hechos en un solo lugar (`AppInfo`), con una prueba que exija que la versión del código coincida con el `Info.plist`. Si incluye dependencias con obligación de atribución, sus licencias van aquí.

## 6. Documentos

Solo para apps que editan archivos propios.

- **Debe** declarar `CFBundleDocumentTypes` y `UTExportedTypeDeclarations`, y abrir desde Finder y el Dock. En SwiftUI, usar `onOpenURL` con `handlesExternalEvents`: con `application(_:open:)`, lanzar la app con un archivo no abre la ventana. Probarlo por Launch Services con la app cerrada.
- **Debe** ofrecer ⌘S (guarda donde está, pregunta solo sin ubicación), ⇧⌘S, ⌘N, ⌘O y Abrir reciente, con `noteNewRecentDocumentURL`.
- **Debe** mostrar cambios sin guardar con `isDocumentEdited`, y dentro de la barra si el título está oculto. El indicador se compara con lo guardado, de modo que deshacer hasta ahí lo borra.
- **No debe** perder trabajo: si hay autoguardado, salir no pregunta. Cambiar de documento con trabajo sin guardar sí.
- Escrituras atómicas. Un fallo conserva sesión y estado.

## 7. Ajustes

- **Debe** abrirse con ⌘, en el menú de la app, recordar su sección y seguir el tema.
- Cada opción cambia algo que ya funciona; no hay controles decorativos.

## 8. Sistema

- **Debe** respetar Reducir movimiento en animaciones decorativas.
- **Debe** pedir permisos cuando el usuario inicia lo que los necesita, no al abrir.
- **Debe** avisar por notificación solo si un trabajo largo termina con la app en segundo plano. `UNUserNotificationCenter` aborta sin un bundle real: protegerlo.
- Icono en la barra de menús solo si la app trabaja en segundo plano. Apps de audio: teclas multimedia y Now Playing, sin publicar por tick.
- **Debe** nombrar los controles con icono para VoiceOver y marcar el elemento activo con `.isSelected`.

## 9. Bundle y distribución

- **Debe** declarar en `Info.plist`: `CFBundleIdentifier`, versión y compilación, `CFBundleDevelopmentRegion`, `CFBundleLocalizations`, `LSApplicationCategoryType`, `NSHumanReadableCopyright`, `LSMinimumSystemVersion`, y una descripción por permiso.
- **Debe** empaquetar siempre con hardened runtime (`--options runtime`), también ad hoc, y entitlements mínimos. El micrófono necesita `com.apple.security.device.audio-input`.
- **Debe** aceptar una identidad Developer ID por parámetro, firmar binarios anidados de dentro hacia fuera y no usar `--deep` al firmar.
- **Debe** tener una ruta explícita de notarización que rechace firmas ad hoc antes de subir, grape el ticket y compruebe Gatekeeper. Una app sin esto se describe como build de desarrollo.
- **Debe** reemplazar sin destruir: no borrar una app en uso ni cerrar sesiones del usuario para probar.
- Las actualizaciones se declaran: o hay actualizador, o la app dice cómo actualizarse.

## 10. Validación

- **Debe** tener un modo de QA aislado por variable de entorno: no lee ni escribe datos, preferencias ni recientes de producción.
- **Conviene** un modo de depuración que recorra la ventana real a varios tamaños sin permiso de grabación de pantalla: geometría, hit tests de la barra y PNG por `cacheDisplay`.
- **Debe** registrar lo no comprobado físicamente. Una prueba automática no cubre arrastrar, doble clic, trackpad ni Gatekeeper en un Mac limpio.

## Adopción

Revisar el código al actualizar; esta tabla no sustituye el estado de cada app.

| Regla | SoundShop 0.27 | nüün 0.2 |
| --- | --- | --- |
| 1 Barra de título | Sí | Sí |
| 2 Ventana | Sí | Parcial: la pantalla completa recorre clases privadas |
| 3 Temas | Sí | Sí |
| 4 Menús | Sí | Parcial: menú manual sin Servicios, Ocultar otras, Mostrar todo ni Zoom |
| 5 Acerca de | Sí | Sí (2026-11-07): ventana propia con tema y skin, versión copiable, enlaces y `NSHumanReadableCopyright` |
| 6 Documentos | Sí | No aplica |
| 7 Ajustes | Sí | Sí |
| 8 Sistema | Sí, sin pasada completa de VoiceOver | Sí, sin pasada completa de VoiceOver |
| 9 Bundle y distribución | Parcial: scripts listos, nunca notarizada, sin actualizador | Parcial: sin notarización ni actualizador |
| 10 Validación | Sí | Sí |

Stalker, Clipper y otras apps: sin revisar. Una app adopta el contrato añadiéndose aquí con lo que le falta.

## Cambios

Regla nueva o más estricta: añadirla, actualizar esta tabla y avisar en cada app afectada. Las reglas no se dan por adoptadas por escribirlas.
