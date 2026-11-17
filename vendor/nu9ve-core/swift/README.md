# Swift

Paquete de Swift (`Package.swift`, paquete `nu9ve-swift`), para las apps de Mac. Una app lo usa por su copia fijada: declara `swift` en `vendor/nu9ve-core.paths`, sincroniza y apunta su proyecto de Xcode (o su `Package.swift`) a `vendor/nu9ve-core/swift` como paquete local.

| Pieza | Qué hace | De dónde | Estado |
| --- | --- | --- | --- |
| `NuMacShell` | `ShellWindow` y `ShellWindowHost` (ventana propia, esquinas cuadradas, bordes de arrastre, zoom que llena la pantalla), `WindowChromeBridge` (barra junto a los botones de ventana, pantalla completa), `TitleBarDragArea` y doble clic según el sistema, `AboutView`, `AppIdentity` con el sello `Nu9veBuildID`/`Nu9veBuildDate`, `HapticPlayer` y `PressureMonitor` | Fractos (`ios-fractos/Packages/FractosKit`, mudado el 2026-11-16) | Aquí. Consumidores: Fractos |
| `Nu9veTheme` | Color sRGB, `readable`, 15 roles, el tipo `ThemePreset` y las medidas de los skins del catálogo, con `ThemeSkin.commons` (Macintosh, Windows 9x y Terminal) y `ThemeSkinPart`. Prueba los 110 casos de `data/theme-vectors-v1.json`. No trae colores de ninguna app: cada una declara su color por omisión y sus presets | nüün (`ThemeColor.swift`) | Aquí. Consumidores: nüün y Kyū |
| Motor de sonido de interfaz | | Fractos (`FractosSoundV2`) | Según el contrato de sonido |

## macOS mínimo: 15

`Package.swift` pide macOS 15. Es lo más bajo que permiten las APIs del shell, comprobado compilando con un mínimo menor: con macOS 13 o 14 sólo fallan `NSCursor.frameResize(position:directions:)` y `NSCursor.FrameResizePosition`, los cursores de borde y esquina de `ShellWindowResizeOverlay`, que llegaron en macOS 15. Lo demás pide macOS 13 a lo sumo (`URL.host()`, `Task.sleep(for:)`, `NSHostingView.sizingOptions`).

- **Fractos** pide macOS 26 y lo usa sin cambios.
- **nüün y Kyū** suben su mínimo de macOS 14 a 15 al adoptar `Nu9veTheme`. SoundBender sigue en 14 y queda pendiente su adopción.

## Reglas

- **La identidad es de cada app** ([tema §5 y §7](../contracts/nu9ve-theme-v1.md#juego-de-skins-por-app)). `Nu9veTheme` da lo común: roles, contraste, los tres comunes y los skins del catálogo con sus medidas. El **original** y el **contraste** de cada app, su color por omisión y sus presets los declara la app. Si su original no está en el catálogo, la app le da sus `ThemeSurfaces` y usa `ThemeRoles(dark:base:surfaces:)`, que aplica las mismas reglas de contraste. Así una app que adopta el paquete se sigue viendo igual por omisión, y suma los comunes.
- **No conoce ninguna app.** `scripts/check-boundary.sh` lo comprueba: el código de `swift/` sólo importa frameworks de Apple y sus propios targets, y ningún archivo de código nombra una app de la suite. Lo que es de una app llega como parámetro: `ShellWindowHost(frameAutosavePrefix:)` recibe el nombre con que la app guarda sus ventanas (Fractos pasa `fractos.window`, el de siempre, para no perder los marcos guardados), `AppIdentity` recibe nombre, lema y enlaces, y `AboutView` el logo y los colores.
- **Plataformas.** El código AppKit/SwiftUI de `NuMacShell` va dentro de `#if os(macOS)`. `Nu9veTheme` contiene datos y funciones de color, con Foundation como única dependencia.
- **Gate:** `scripts/check-swift.sh` corre la frontera, `swift test` y un build de Release con los avisos como errores (`NU9VE_STRICT_WARNINGS=1`). No puede ser siempre: Xcode compila los paquetes locales con `-suppress-warnings`, y el compilador rechaza las dos opciones juntas. El CI lo corre en macOS 26 (Swift 6.2).
- **Las apps no corren `swift test` dentro de `vendor/nu9ve-core/swift`:** dejaría `.build/` ahí y `sync-core.sh --check` lo rechaza por archivos fuera del manifiesto. Las pruebas del shell corren aquí; la app prueba que lo usa bien.
- El nombre `NuMacShell` viene de Fractos y se mantiene para no romper los `import`. La regla del prefijo pediría `Nu9veMacShell`; renombrarlo sería un cambio propio, con `-v2` conviviendo.
