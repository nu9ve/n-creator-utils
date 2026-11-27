# Una sola barra en Macintosh y Windows 9x: propuesta

Estado: **hecho en el core el 2026-11-26**: `PeriodWindowBar(singleRow: true)`, `periodClearing()` y `periodPlate()` en `NuMacShell`, la cara de Windows 9x con «Con tu color» (`PeriodBarColors.face` y `faceInk`), `AboutView(systemButtonStyle: false)` y la regla en [tema §8](../contracts/nu9ve-theme-v1.md#8-barras-del-sistema-menús-y-controles). Una diferencia con nüün: el core escribe el nombre una vez en las dos épocas (en Macintosh sobre su margen de papel), así que la fila de la app no lo repite. La sección [«Época por omisión y color de las rayas»](#época-por-omisión-y-color-de-las-rayas-decisión-del-usuario-2026-11-26) también quedó hecha: «Con tu color» por omisión sólo como preferencia (la API y los vectores originales no cambian), Macintosh con tu color empieza en claro (`defaultDark(era:)`), y `line` lleva el 65 % del color con 3:1 como mínimo, en rayas, reglas, bordes y caja de agarre (`PeriodBarColors.line`); el marco de la ventana es de cada app y sigue esa regla. Propuesta original para una sesión del core (2026-11-26). La escribió una sesión de nüün a pedido del usuario; nüün ya la usa en su ventana principal (`n-creator-utils`, `native/Nuun/Sources/Nuun/SkinChrome.swift`, `SkinnedWindow(singleBar:)`), con su comentario de origen. No cambia ningún contrato.

## El problema

Con `PeriodWindowBar` de dos filas, una app con navegación dice su nombre dos veces: la barra de título de la época («nüün») y, debajo, la fila de la app, que vuelve a empezar con el nombre antes de las pestañas. En palabras del usuario: se ve raro, y en Windows la barra de título no se aprovecha para la navegación, los discos o Ajustes. Pidió parecerse al resto de los temas (Cristal, Terminal, Gótico: una sola barra con controles, nombre, navegación al centro y acciones) sin perder la esencia de cada época.

## Lo que hace nüün

Una sola barra de `rowHeight + 4` para la ventana principal; Ajustes y Acerca de conservan la barra clásica.

- **Macintosh:** rayas a lo ancho y × − + sobre ellas, cada uno en su caja, como siempre. El nombre, la navegación y las acciones van sobre un **margen de papel** (la superficie de la época, 7 pt a cada lado y casi todo el alto), como el título de System 7: las rayas terminan antes y nunca los tocan. Sin caja propia para el nombre ni las acciones; la navegación conserva su borde y su sombra dura.
- **Windows 9x:** todo sobre el degradado de la barra de título: × − +, el nombre en blanco (Tahoma Bold 12) y cada grupo (navegación, acciones) en un panel plateado con bisel en relieve, como una banda de herramientas de Windows 98.
- Con «Con tu color» funciona igual: papel y tinta salen de los roles.

Capturas de QA antes y después en las dos épocas, en claro y en «Con tu color» oscuro (`--qa-skins`).

## Qué subir

1. Una variante de `PeriodWindowBar` de una sola fila (`singleRow: true`, o un tipo aparte) para ventanas con navegación: controles de la época, y la fila de la app encima de la barra de título.
2. Dos modificadores públicos para lo que la app pone encima: el margen de papel de Macintosh y el panel en relieve de Windows 9x (en nüün, `PeriodClearing` y `PeriodPlate`).
3. Decir en [tema §8](../contracts/nu9ve-theme-v1.md#8-barras-del-sistema-menús-y-controles) cuándo va una fila y cuándo dos: dos para ventanas sin navegación (Ajustes, Acerca de, Ayuda), una para la principal.

## Época por omisión y color de las rayas (decisión del usuario, 2026-11-26)

- **«Con tu color» (`tinted`) pasa a ser la época por omisión** de Macintosh y Windows 9x en toda la suite: al elegir el skin sin haber elegido «Original», se usa el color y el modo de la persona. «Original» queda como opción explícita. Cambia la regla de [tema §5](../contracts/nu9ve-theme-v1.md#5-catálogo-de-skins) que hoy dice «`original` (por omisión)» y «no tener `era` guardado da los mismos colores que antes»; los vectores de `theme-vectors-v1.json` sin `era` cambiarían, o se pide `era` explícito en ellos.
- **Las rayas y reglas de Macintosh deben llevar el color.** Hoy, aun con `tinted`, salen casi negras (o blancas en oscuro): `ThemeSurfaces.tinted` mezcla la tinta con sólo un 10 % del color, y `PeriodBarBackground` dibuja las rayas (`ClassicPinstripes`) y la regla con `colors.ink`, no con `line`, que es la que lleva el color; `PeriodBarColors` no tiene `line`. Propuesta: sumar `line` a `PeriodBarColors` y usarlo en rayas, reglas, bordes de los controles y marco de la ventana con `tinted`, manteniendo los mínimos de contraste de la §2. En nüün, el marco de la ventana (`SkinnedWindow`) hoy usa `ink`; seguirá lo que decida el core.

Otras dos cosas vistas al adoptar `NuMacShell` en nüün (F3):

- Con «Con tu color», los controles de Windows 9x siguen plateados: `Win9xBevel.face` es fijo.
- `AboutView` impone `.buttonStyle(.bordered)` del sistema en lugar del estilo del skin de la app.
