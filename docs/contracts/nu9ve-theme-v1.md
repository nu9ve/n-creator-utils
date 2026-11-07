# Tema visual de las apps de nu9ve v1

Estado: **propuesta** (2026-11-06). Define cómo se ven las apps de nu9ve, lo que el usuario puede configurar y cómo cada app conserva su propia identidad dentro de un mismo lenguaje. Junto con [teclado e interacción v1](nu9ve-interaction-v1.md) (cómo se maneja) complementa a [apps de macOS v1](macos-app-v1.md), que trata el comportamiento: barra de título, menús, Acerca de, documentos y distribución. Igual que los demás contratos, una cosa es la regla y otra quién la adoptó: ver [adopción](#adopción). Escribir la regla no la da por adoptada.

**Debe** es obligatorio salvo que la app documente por qué no aplica. **Conviene** es lo recomendado.

Referencias leídas en el código el 2026-11-06:

| App | Dónde | Qué toma este contrato |
| --- | --- | --- |
| Fractos (Mac) | `ios-fractos/Packages/FractosKit/Sources/FractosDesign`, `NuMacShell`, `docs/DESIGN_SYSTEM.md`; canon web en `nu9ve.xyz/packages/styles` y `nu9ve.xyz/docs/themes.md` | Las tres capas, los materiales, el vocabulario de señales de interacción y la regla de ornamento «uno por contexto» |
| curado | `src/lib/theme.ts`, `src/lib/dither.ts`, `docs/THEMES.md` | La paleta como función pura, las opciones de Apariencia, la matriz de pruebas de contraste y el dither Bayer |
| SoundShop | `Sources/SoundShop/Appearance.swift` | Los roles de dominio, `readable(on:)` y la rampa que sale del acento |
| nüün | [Appearance.swift](../../native/MediaMenu/Sources/MediaMenu/Appearance.swift) | El tema aplicado a la ventana, el vidrio con respaldo sólido y un único botón principal por vista |
| kyu | `apps/macos/Sources/Kyu/Theme.swift` | Los presets culturales y el entorno neutro para las fotos |

Fractos es la expresión máxima y la referencia. Es la única app que lleva el lenguaje hasta sonido y háptica por tema, y la que define los materiales con ventana cuadrada; nüün los replica como skins opcionales. Las demás toman de ella poco a poco, sin perder usabilidad.

## 1. Tres capas

| Capa | Decide | Compartida |
| --- | --- | --- |
| **Skin** (material) | Forma, radios, bordes, textura, tipografía por rol y tratamiento de la ventana | Sí: un mismo nombre se ve igual en todas las apps |
| **Color** | Color base del usuario, secundario y roles derivados con contraste garantizado | La fórmula sí; el color lo elige el usuario en cada app |
| **Identidad** | Presets con nombre, marca, un motivo ornamental, skin y color por defecto | No: cada app tiene la suya |

- **Debe:** ninguna vista pregunta qué skin está activo. Lee roles y medidas del entorno (`@Environment` en Swift, variables CSS en web/Tauri). Un `switch` sobre el skin vive en el módulo de tema, nunca en una pantalla.
- **Debe:** cualquier color funciona sobre cualquier skin. Un skin puede *sugerir* un color, pero elegir un skin nunca cambia el color del usuario.

## 2. Roles de color

Nombres comunes. Cada app puede conservar sus nombres internos mientras migra, pero documenta su equivalencia con esta tabla.

| Rol | Uso | Contraste mínimo |
| --- | --- | --- |
| `canvas` | Fondo de la ventana, la barra de título y el chrome | — |
| `surface` | Contenido elevado: hoja, panel, tarjeta | — |
| `raised` | Controles, campos y popovers sobre `surface` | — |
| `line` | Bordes y divisores | — |
| `ink` | Texto principal | 7:1 sobre `canvas`, `surface` y `raised` |
| `muted` | Texto secundario | 4.5:1 sobre las tres |
| `accent` | Relleno: selección, acción principal, insignias y marca | Sin mínimo como relleno: lo que va encima usa `onAccent` y la selección se marca además con peso o forma. Para trazos e iconos se usa `accentText` |
| `accentText` | El acento usado como texto o enlace | 4.5:1 |
| `onAccent` | Texto sobre un relleno de acento | 4.5:1; negro o blanco, el que contraste más |
| `accentSoft` | Fondo de un elemento seleccionado | `ink` sobre él a 7:1 |
| `secondary` | Segundo color derivado (deck B, comparación, segunda serie) | 3:1 sobre `surface` |
| `danger`, `warning`, `success`, `info` | Estado | 4.5:1 como texto |

Equivalencias actuales:

| Común | nüün | curado | SoundShop | Fractos | Web |
| --- | --- | --- | --- | --- | --- |
| canvas | `canvas` | `--chrome` / `--bg` | `background` | atmósfera | `--background` |
| surface | `card` | `--surface` (sheet) | `panel` | `card` | `--card` |
| raised | `inset` | `--surface-raised` | `raised` | — | `--popover` |
| line | `border` | `--line` | `line` | `edge` | `--border` |
| ink / muted | `ink` / `muted` | `--text` / `--text-muted` | `text` / `muted` | `ink` / `mutedInk` | `--foreground` / `--muted-foreground` |
| accent / onAccent | `accent` / `accentInk` | `--accent` / `--accent-ink` | `accent` / `onAccent` | `signal` / `onSignal` | `--primary` / `--primary-foreground` |
| secondary | — | `--deck-b` | — | `glow` | `--secondary` / `--accent` |

**Roles de dominio.** Una app añade los que necesita con prefijo y los deriva de los comunes: `wave-*` en curado; `thermal*`, `canvas` y `playhead` en SoundShop; `photoSurround` en kyu; `series-*` en nove. Si un rol de dominio tiene colores propios (bandas de waveform, rampa térmica), **debe** ajustarlos al modo claro como hace curado con las bandas.

- **Debe:** los colores salen solo de la función de paleta. No hay hex en componentes. Son excepciones intencionales los presets, las semillas de skin y las constantes de dominio documentadas.
- **Debe:** los estados mezclan su color semántico con `ink` y `surface` para conservar el contraste. No son rojo, verde o amarillo puros, ni toman el acento del usuario. El estado se dice también con texto o icono, nunca solo con color.
- **Debe:** el contenido que el usuario evalúa no se tiñe. Una foto va sobre gris neutro (kyu: 13 % en oscuro, 42 % en claro), una gráfica no hereda el dither y un video se ve sin velo.

## 3. Derivación y contraste

Una sola implementación por plataforma, pura y probada:

1. **Base:** un color que el usuario elige como preset, HSV o hex, y que se guarda como hex.
2. **Secundario:** el matiz de la base desplazado lo que diga el skin, con S y V fijos por modo (`secondary.shift` en curado).
3. **Superficies:** neutros del skin por modo, mezclados con la base según el contraste. Neutro 3 %, tonal 16 %, alto contraste 0 % y además más profundos. Son los valores de curado.
4. **Texto:** `readable(color, on: fondos, mínimo)`. Si el color ya cumple, no cambia. Si no, se mezcla hacia la polaridad con mejor contraste, buscando el punto mínimo por bisección (SoundShop).
5. **Sobre el acento:** negro o blanco, el que dé más contraste.

- **Debe:** luminancia relativa WCAG (sRGB lineal, coeficientes 0.2126/0.7152/0.0722) y la razón `(L1 + 0.05) / (L2 + 0.05)`.
- **Debe:** probar la matriz completa: skins × al menos 10 colores × 3 modos × 3 contrastes. Los colores incluyen gris, amarillo puro, un casi negro, un casi blanco y los presets de la app (curado ya lo hace con 5×10×3×3).
- **Conviene:** compartir vectores de prueba (base y opciones de entrada, hex esperado por rol) entre Swift y TypeScript, para que las dos implementaciones den el mismo color. Pendiente: no existe todavía.

## 4. Lo que el usuario configura

**Ajustes → Apariencia**, en este orden y con estos nombres:

| Opción | Valores | Por defecto |
| --- | --- | --- |
| Skin | Los de la [§5](#5-catálogo-de-skins) que la app soporte | El de su identidad |
| Color | Presets de la app, selector HSV y hex | El de su identidad |
| Modo | Claro, Oscuro, Sistema | El de su identidad; Sistema si no hay razón para otro |
| Contraste | Neutro, Tonal, Alto | Neutro |
| Textura | Automática, Dither, Suave, Plana | Automática: la del skin |
| Tipografía | Automática, Display, Pixel, Mono, Sistema | Automática: la del skin |
| *Opciones de dominio* | Waveform, rampa del lienzo… | Las de la app |
| Restablecer | Vuelve exactamente a los valores por defecto | — |

- **Debe:** si una app no soporta una opción, la oculta; no la renombra ni la reordena. Cada control cambia algo que se ve en ese momento, con vista previa en el propio panel.
- **Debe:** «Sistema» observa `NSApp.effectiveAppearance` (o `prefers-color-scheme`) y no tira el tema de la app al tema genérico del sistema.
- **Conviene:** cada tarjeta de skin en Ajustes se pinta con su propia paleta (curado `ThemePanel`, pestaña Componentes de Fractos).

**Formato guardado.** Se valida campo por campo: un valor desconocido toma el valor por defecto de ese campo y no reinicia todo (`validateTheme` de curado).

```json
{
  "version": 1,
  "skin": "dither",
  "base": "#d66154",
  "mode": "system",
  "contrast": "neutral",
  "texture": "auto",
  "typography": "auto",
  "app": { "waveform": "bands" }
}
```

- **Debe:** migrar las claves actuales de cada app (`accentHue`… en nüün, `appearance.v1` en SoundShop, `theme.*` en kyu, Theme v2 en curado) sin perder la elección del usuario.
- **Propuesta, fase 2 (no adoptada):** una opción «Seguir el skin de nu9ve» que lea un archivo común, por ejemplo `~/Library/Application Support/nu9ve/theme.json`, para aplicar el mismo skin y color en todas las apps. Es opt-in por app, cada app conserva su tema propio por defecto y ninguna escribe en las preferencias de otra. Antes de construirla hay que decidir el dueño del archivo y el comportamiento si se aplica en sandbox.

## 5. Catálogo de skins

Un skin es un conjunto de medidas, no una paleta. Medidas en pt (Swift) y px (web). Se tolera ±1 por plataforma.

| Skin | Radio control / panel / hoja | Borde | Textura | Display | UI | Carácter |
| --- | --- | --- | --- | --- | --- | --- |
| `glass` · Cristal | 7 / 10 / 14 | 0–1 línea fina | Ninguna; Liquid Glass en macOS 26 | Sistema | Sistema | Nativo, tranquilo; el acento tiñe el vidrio |
| `minimal` · Mínimo | 4 / 6 / 10 | 1 | Plana | Sistema | Sistema | Solo el color esencial, sin glow ni grano |
| `dither` · Grano | 4 / 7 / 14 | 1 | Dither Bayer, grano 0.07–0.22 | Jersey 15 | Sistema | El grano del colectivo; degradados suaves con detalles pixel |
| `pixel` · Pixel | 0 / 0 / 0 | 2 | Dither, sin antialias, sombras escalonadas | Jersey 15 | Pixelify Sans | Consola de 8 bits / Macintosh II: 1 bit hasta que el usuario elige color |
| `gothic` · Gótico | 1 / 2 / 3 | 1–2, marco doble en paneles de marca | Suave, viñeta | UnifrakturMaguntia | Sistema | Negro profundo, carmesí, noche |
| `y2k` · Cromo | 9 / 14 / 24 | 1 | Suave, brillo diagonal | Michroma / Audiowide | Sistema | Cromo líquido, botones píldora |

Solo en Fractos: `neon`, `parchment` y `ledger`; `terminal` es uno de los comunes y le falta a las demás apps. `win9x` y `classicMac` (el Macintosh de `pixel`) también están en nüün desde 2026-11-07, copiados de `ThemeMaterial` y `ThemedWindowBar`. Otra app puede adoptar uno de estos materiales solo si sigue la especificación de Fractos.

**Skins opcionales.** Una app puede ofrecer skins de época como extra para quien quiera personalizar su espacio. El skin por defecto sigue siendo el sobrio (Cristal), y elegir otro siempre es reversible desde Apariencia. Un skin de época **puede** fijar su modo (Macintosh y Windows 9x claros, Gótico oscuro). En ese caso Apariencia lo dice y conserva la preferencia de modo del usuario para cuando vuelva a otro skin.

### Juego de skins por app

Decisión del usuario (2026-11-07). Cada app ofrece el mismo juego en tres partes, para que se note a la vez lo común de nu9ve/Fractos y la personalidad de cada una:

| Parte | Qué es | Regla |
| --- | --- | --- |
| **Original** | El skin de la identidad de la app (§7): el de todos los días, con sus presets y ornamentos | El por defecto. Sobrio, legible y con la personalidad más marcada de la app |
| **Contraste** | Un skin que se opone a la identidad original | Si hay un opuesto claro, ese. Si no, al implementarlo **se pregunta al usuario** cuál de los materiales de Fractos (9 o más) usar. No se elige solo |
| **Comunes** | Macintosh (`classicMac`), Windows 9x (`win9x`) y Terminal (`terminal`) | Los mismos en todas las apps y con las medidas de Fractos: son el tema central compartido, el «skin de nu9ve» que se reconoce en cualquier app |

- Los tres comunes son iguales en todas partes (barra, controles × − + a la izquierda, botones, bordes, tipografía). Lo único que varía es la paleta que les da el color base. Si cambia un común, cambia en todas las apps que lo tienen.
- El original y el contraste son los que llevan la personalidad: presets culturales, ornamentos y tipografía display de la app. Pueden ser más expresivos que los comunes, siempre con la matriz de contraste del §3.
- Extras como Grano son opcionales y no reemplazan ninguna de las tres partes.
- Al adoptar una app: implementar primero el original, luego los comunes y luego el contraste.

| App | Original | Contraste | Comunes |
| --- | --- | --- | --- |
| Fractos | Claro/Oscuro (`glass`) | Todo su catálogo | Fuente de los tres |
| nüün | Cristal suizo (`glass` + presets suizos) | Gótico (ornamento frente a retícula) | Macintosh, Windows 9x y Terminal hechos (2026-11-07). Grano se retiró: no aportaba frente a Macintosh |
| curado | `curado` (dither cálido) | Por preguntar (tiene gótico, y2k, pixel, mínimo) | Pendientes |
| kyu | Kyū·washi | Por preguntar | Pendientes |
| nove | Por definir (Atlántico / Carta náutica / Anochecer) | Por preguntar | Pendientes |
| neuf | Por definir (cine francés) | Por preguntar | Pendientes |
| SoundShop | Original (lienzo espectral) | Por preguntar | Pendientes |
| Stalker | Sistema (utilidad de barra de menús) | No aplica | Opcionales en su ventana de ajustes, si la tiene |

- **Debe:** el mismo skin tiene las mismas medidas, textura y fuentes por rol en todas las apps. Eso es lo que hace que se sienta como un skin compartido.
- **Debe:** dither con la matriz Bayer 8×8 común, umbral `(m + 0.5) / 64`:

  ```text
   0 32  8 40  2 34 10 42
  48 16 56 24 50 18 58 26
  12 44  4 36 14 46  6 38
  60 28 52 20 62 30 54 22
   3 35 11 43  1 33  9 41
  51 19 59 27 49 17 57 25
  15 47  7 39 13 45  5 37
  63 31 55 23 61 29 53 21
  ```

  Las texturas son estáticas: se generan una vez por paleta y escala, se guardan en caché y se dibujan sin interpolación (`interpolation(.none)`, `image-rendering: pixelated`). No hay shader a pantalla completa ni un tick permanente para decorar.
- **Debe:** «Reducir movimiento» detiene lo decorativo. «Reducir transparencia» cambia vidrio y cromo por superficies sólidas. «Aumentar contraste» equivale a Contraste: Alto.
- **Debe:** lo decorativo no intercepta clics ni llega a VoiceOver.
- **Debe:** el vidrio tiene un respaldo sólido con la misma forma. Las capturas de QA (`cacheDisplay`) no dibujan Liquid Glass, así que el vidrio real se verifica en la app.
- **Debe:** la forma de la ventana la decide [macos-app-v1 §2](macos-app-v1.md#2-ventana-y-tamaños). La excepción de ventanas cuadradas (`ShellWindow`) solo está documentada en Fractos y en nüün (Macintosh, Windows 9x y Gótico). En esas ventanas se reponen teclado, ⌘W, ⌘M, zoom, doble clic según `AppleActionOnDoubleClick`, bordes para redimensionar y pantalla completa, y el skin dibuja su propia barra con controles de época. En las demás apps, un skin retro cambia el contenido y deja la ventana del sistema.

## 6. Tipografía por roles

Roles: `display` (marca, títulos grandes), `heading`, `body`, `ui` (controles), `label` (etiquetas pequeñas) y `mono` (números, tiempos, tamaños).

- **Debe:** un componente elige el rol y el skin decide la familia. Los números van en `mono` o con `monospacedDigit()`.
- **Debe:** fuentes empaquetadas localmente con licencia OFL, nunca remotas, siempre con la del sistema como respaldo. Una fuente decorativa solo se usa en `display` y `heading`; el cuerpo y los formularios se mantienen legibles.
- **Conviene:** compensar la escala óptica de las fuentes decorativas, como hace la web con Nora94 y Sangre.

## 7. Identidad de cada app

Cada app se llama «nueve» en un idioma, y ese idioma da su referencia cultural. La identidad es una capa encima de lo compartido, nunca un reemplazo.

| App | «Nueve» en | Referencia | Skin / color por defecto | Presets y ornamentos (dirección) |
| --- | --- | --- | --- | --- |
| **nüün** | Suizo alemán | La navaja suiza: multiherramienta precisa, plegable, cada utilidad en su sitio | `glass` · ámbar `#FFBB5C` (actual) | Estilo tipográfico internacional: retícula, grotesca, alineación estricta. Presets posibles: ámbar (actual), rojo navaja, acero, glaciar. Las herramientas se despliegan como hojas de la navaja |
| **curado** | — (México / Sonora) | «Curar» una selección y «curado» como algo chido: latino, de barrio | `dither` · coral `#D66154` | Rotulación a mano, sonidero, cromo lowrider (`y2k`), noche (`gothic`). Mantener sus presets y explorar nombres del barrio |
| **kyu** | Japonés | Las cámaras profesionales japonesas | Kyū·washi (`dither` de baja densidad) · 朱 `#DB4D2E` | Presets con kanji (朱 藍 抹茶 桜 金 墨), Mincho y sello hanko. La foto siempre sobre gris neutro |
| **nove** | Portugués | La exploración europea: navegantes portugueses, italianos, españoles | Por definir | Atlántico, Carta náutica y Anochecer ([product.md de nove]); puerto con barcos solo para ejecuciones activas, rosa de los vientos y curvas de nivel. Las gráficas no llevan textura |
| **neuf** | Francés | El cine francés | Por definir | Direcciones posibles: blanco y negro de nouvelle vague, grano de película, cartela con serif Didone. El video y el preview no se tiñen |
| **Fractos** | — | El universo de Fractos | Por tema | Todo el catálogo, sonido y háptica por tema y el lenguaje Crystal |
| **SoundShop** | — | El sonido como lienzo | Original · lima | La pincelada de espectrograma de `BrandMark` |
| **Stalker** | — | Utilidad de barra de menús | Sistema | Sigue al sistema; color solo para estados (grabando, problema) |

[product.md de nove]: ../../../../ai/nove/docs/product.md

- **Debe:** la identidad se expresa en presets con nombre, marca, una fuente display, un motivo ornamental y los valores por defecto. No cambia roles, controles ni comportamiento.
- **Debe:** el ornamento va solo en superficies de marca: barra de título (marca), Acerca de, estados vacíos e inicio. No va en listas densas, formularios ni contenido que el usuario evalúa. Uno principal por contexto, como el Crystal de Fractos.
- **Debe:** un nombre de preset en otro idioma o escritura (kanji) lleva una etiqueta accesible traducida.
- **No debe** usar marcas o símbolos protegidos como motivo: la cruz o el escudo suizo, logotipos de fabricantes de navajas o de cámaras. La referencia se evoca con forma, tipografía y color.
- Las direcciones de esta tabla son puntos de partida para diseñar, no paletas validadas.

## 8. Barras del sistema, menús y controles

- **Debe: toda ventana sigue el tema y el skin, también su barra de título.** Es decir: la principal, Ajustes, Acerca de, sheets, popovers, paneles flotantes, el panel de atajos, los diálogos propios y las ventanas auxiliares (vista mini, ventana de visuales, broker, importador rápido). Cada una aplica el tema al crearse, sin esperar a que el contenido lo pinte, y entra en la QA de skins. Con un skin de ventana cuadrada, también es cuadrada y lleva la misma barra de época con × − +.
- La excepción son los paneles del sistema: Abrir/Guardar, Quick Look, alertas del sistema y `NSMenu`. Esos conservan el aspecto del sistema, pero reciben el modo claro u oscuro de la app.

El comportamiento está en [macos-app-v1](macos-app-v1.md). Aquí va solo el aspecto.

- **Barra de título:** a la izquierda, la marca a 20 pt y el nombre en minúsculas a 14 pt semibold con la fuente de identidad de la app. En el centro, la navegación como cápsula sobre vidrio (`GlassSurface`), con el elemento activo en `accentSoft` y semibold. A la derecha, las acciones, con un único relleno de acento. Todo con el tema de la app, también en pantalla completa.
- **Controles de ventana propios** (skins con ventana cuadrada): **deben** ir en el orden y lugar de macOS, es decir cerrar, minimizar y ampliar a la izquierda, con ×, − y + siempre visibles, para que quien nunca usó un Mac los entienda. La época cambia solo su dibujo (caja de 1 bit, bisel gris, losa gótica). Cada uno lleva nombre para VoiceOver y ayuda al pasar el puntero; Opción + ampliar entra a pantalla completa. Fractos conserva por ahora los glifos y lugares de cada época (Win9x a la derecha, cajas vacías en Macintosh) como excepción documentada.
- **Botones:** un solo botón principal (relleno `accent`, texto `onAccent`) por vista. Los secundarios son claros sobre `surface`, con borde `line` tenue. El destructivo usa `danger`. Los radios salen del skin.
- **Ajustes:** barra lateral con secciones; la sección activa en `accentSoft`. Apariencia e Idioma son secciones separadas.
- **Menús (`NSMenu`) y menús contextuales:** nativos y sin tema. Los popovers propios siguen el skin.
- **Icono en la barra de menús:** SF Symbol en plantilla monocromo, que el sistema adapta. La actividad se indica con un sufijo: `•` necesita atención, `…` trabaja (nüün). Solo los estados críticos llevan color de sistema: rojo al grabar, naranja con problemas (Stalker). Nunca se tiñe con el acento de la app.
- **Señales de interacción:** una app que añade sonido o háptica usa el vocabulario de Fractos (`hover`, `press`, `select`, `tab`, `toggleOn`/`toggleOff`, `expand`/`collapse`, `popupOpen`/`popupClose`). La háptica es solo para Force Touch y se puede apagar en Ajustes. Un gesto da una sola señal.

## 9. Idioma

- **Debe:** español como idioma base y también inglés. En Swift, `String(localized:)` con `Localizable.strings` o `.xcstrings`, y `CFBundleLocalizations` declarado ([macos-app-v1 §4](macos-app-v1.md#4-barra-de-menús)). En web y Tauri, un catálogo. El `tr(es, en)` de curado sirve mientras la app sea pequeña.
- **Debe:** los identificadores de skin, modo y contraste son estables en código (`glass`, `dither`…) y su nombre visible se traduce.
- **Debe:** probar en los dos idiomas cómo cede la barra de título al estrecharse: el inglés y el español no miden lo mismo.

## 10. Implementación compartida

Decisión (2026-11-06): **primero replicar, después extraer.** Todavía no hay un paquete ni un repo común. Cada app copia las piezas que adopta, y lo que de verdad se comparte se decide cuando la misma pieza haya quedado igual en al menos tres apps.

- **Debe:** cada copia empieza con un comentario que dice de dónde viene y qué sección cubre, para poder encontrar todas las copias y compararlas: `// nu9ve-theme-v1 §3 · origen: soundshop Appearance.swift ThemeColor.readable`.
- **Debe:** si una copia se aparta del original, el comentario lo dice y explica por qué. Una divergencia sin explicar se trata como un error de alguna de las dos.
- Origen sugerido de cada pieza:

| Pieza | Origen | Sección |
| --- | --- | --- |
| Luminancia, contraste, `readable(on:)`, HSV | SoundShop `ThemeColor` | §3 |
| Paleta pura desde las opciones y matriz de pruebas | curado `theme.ts`, `theme.test.ts` | §3, §4 |
| Validación de preferencias campo por campo | curado `validateTheme` | §4 |
| Tile Bayer en caché | kyu `Dither` (Swift), curado `dither.ts` (web) | §5 |
| Vidrio con respaldo sólido | nüün `GlassSurface` | §5 |
| Barra de título, arrastre y doble clic | nüün / SoundShop `TitleBarDragArea` | §8 |
| `AppIdentity` y Acerca de | Fractos `NuMacShell` | [macos-app-v1 §5](macos-app-v1.md#5-acerca-de) |

- **Más adelante:** cuando una pieza se repita igual, extraerla a un paquete (por ejemplo `NuTheme` / `NuMacShell`). Antes de crearlo hay que decidir en qué repo vive y cómo lo consumen las apps.

## 11. Validación

- **Debe:** pruebas puras de la matriz de contraste de la [§3](#3-derivación-y-contraste) y de la migración de preferencias.
- **Debe:** capturas de QA de cada skin soportado en claro y oscuro, con el color por defecto, mediante el modo de QA aislado de [macos-app-v1 §10](macos-app-v1.md#10-validación).
- **Debe:** revisión manual, registrada como tal: cambiar de modo con la app abierta (menús y sheets), Reducir movimiento y transparencia, Aumentar contraste y vidrio real en macOS 26.
- Las cuentas de contraste no sustituyen una revisión visual. Lo que no se vio se anota como no verificado.

## 12. Sonido y háptica de tema

Cada skin puede sonar a su época, como Fractos (`FractosSoundV2`, voces sintetizadas sin archivos de audio y portadas de la web). Es un extra, igual que los skins de época.

- **Debe:** apagado por defecto. Ajustes → Apariencia → **Sonidos de interfaz** (apagado), con volumen, y **Háptica** (solo trackpad Force Touch). Quien lo quiera lo enciende una vez, y la preferencia sobrevive a reinstalar. En los equipos del colectivo se enciende desde Ajustes como cualquier usuario; no hay un valor por defecto distinto escondido en el build.
- **Debe:** usar el vocabulario de señales de [interacción](nu9ve-interaction-v1.md#7-respuesta-al-usar-una-tecla): `hover`, `press`, `select`, `tab`, `toggleOn`/`toggleOff`, `expand`/`collapse`, `popupOpen`/`popupClose`, entrar/volver, éxito, aviso y error. Un gesto da un solo sonido. Repetir rápido suaviza (fatiga). El sonido nunca es la única señal.
- **Debe:** cada skin elige su voz (Macintosh 1 bit, Windows 9x, Terminal, Gótico, la del original de cada app), con el mismo vocabulario.
- **Debe:** nunca suena encima del trabajo. Se calla durante una grabación (Stalker y cualquier captura de audio del sistema, para no quedar grabado), mientras una app de audio reproduce o graba (curado, SoundShop), y en pantalla completa de video. Sale por la salida predeterminada del sistema, nunca por la salida de mezcla o de cue.
- **Conviene:** respetar «Reproducir efectos de sonido de la interfaz» de macOS y el esquema de sonidos de Windows. Si el sistema los tiene apagados, Ajustes lo explica.
- Origen para replicar: Fractos `Packages/FractosKit/Sources/FractosSoundV2` (motor) y `SoundCues`/`HapticPlayer` (puente con la UI), según [§10](#10-implementación-compartida).

## Adopción

Leído en el código el 2026-11-06, sin revisión visual. Revisar al actualizar.

| Regla | Fractos | curado | SoundShop | nüün | kyu | nove | neuf | Stalker |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2 Roles | Parcial: roles propios | Parcial: roles propios | Parcial: roles propios | Sí, con estados; sin `secondary` (2026-11-07) | No: solo acento | No: colores fijos | No: colores del sistema | No aplica: sistema |
| 3 Contraste | Parcial: `mutedInk` | Sí, con matriz | Sí, sin matriz | Sí, con matriz (`ThemeRolesTests`) | No | No | Parcial: texto sobre acento | No aplica |
| 4 Apariencia | Parcial: tema, paleta, material | Sí, el más completo | Parcial: preset, superficie, brillo | Parcial: presets, HSV y modo | Parcial: estilo, modo, acento | No | Parcial: modo | No aplica |
| 5 Skins | Sí, catálogo completo | Sí: `dither`, `pixel`, `gothic`, `y2k`, `minimal` | No: uno | Sí: `glass`, `classicMac`, `win9x`, `terminal`, `gothic` | Parcial: washi, dither, cristal, mínimo | No | No | No aplica |
| 7 Identidad | Sí | Parcial: falta lo de barrio | Sí | Parcial: presets suizos | Sí | Parcial: solo escrita | No | No aplica |
| 8 Todas las ventanas | Sin revisar | Sin revisar | Sin revisar | Sí (2026-11-07): principal, Ajustes y Acerca de en los 5 skins | Sin revisar | Sin revisar | Sin revisar | No aplica |
| 9 Idioma | Sin revisar | Parcial: mensajes nativos en español | No | Sí | No | Sin revisar | Sin revisar | Sin revisar |

Una app adopta el contrato añadiéndose aquí con lo que le falta. Primeros pasos sugeridos:

- **nüün:** roles, contraste y presets suizos hechos (2026-11-07). Skins Grano, Macintosh, Windows 9x y Gótico (2026-11-07). Terminal hecho; Grano retirado. Faltan la opción de Contraste y la tipografía suiza del wordmark.
- **kyu:** derivar toda la paleta desde el acento.
- **nove:** sus tres direcciones como presets y quitar los colores fijos.
- **neuf:** elegir su dirección de cine.
- **SoundShop:** añadir i18n.

## Cambios

Regla nueva o más estricta: añadirla, actualizar esta tabla y avisar en cada app afectada. Cambiar la medida de un skin la cambia en todas las apps que lo soportan.
