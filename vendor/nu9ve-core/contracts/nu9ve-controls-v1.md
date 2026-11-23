# Controles por tipo de dato v1

Estado: **propuesta** (2026-11-22). Dice cómo se comportan los controles de la suite según el dato que manejan: números, fechas y tiempo, textos largos, listas, mapas e imágenes, la voz, el seguimiento y los gestos, los estados y la accesibilidad. Va al lado del [tema visual](nu9ve-theme-v1.md), que dice cómo se ven, y de [teclado e interacción](nu9ve-interaction-v1.md), que trata el catálogo de atajos, los badges y la ayuda. Vale igual en [macOS](macos-app-v1.md) y en [Windows](windows-app-v1.md). Es contrato propio por decisión del usuario del 2026-11-22. El inventario de lo que hace cada app, las librerías y el orden de extracción están en [controles comunes](../docs/suite-ui.md). Escribir la regla no la da por adoptada: ver [adopción](#adopción).

**Debe** es obligatorio salvo que la app documente por qué no aplica. **Conviene** es lo recomendado.

Referencias leídas en el código el 2026-11-22 (el detalle, con archivos, en [controles comunes §2](../docs/suite-ui.md#2-inventario-por-tipo-de-interacción)):

| App | Qué toma este contrato |
| --- | --- |
| kyu | El parámetro lo declara el motor; vista previa al arrastrar y una sola entrada de deshacer al soltar; la cifra en `ink` si cambió; rejilla diffable; lo fuera de línea atenuado y contado |
| curado | El manejo completo de una perilla (rueda, flechas con foco, Inicio, `origin`, muescas, presión); los problemas como filtros; el seguimiento por cámara |
| SoundBender | `ParameterScale` y `XYPad`; una sola regla de tiempo para todas las pistas |
| nove | Zona y bordes escritos en un rango; el horario lo calcula el motor; huecos sin interpolar; tabla con texto exacto y `∅`; mapa con lienzo sin red |
| nüün | Fechas del motor en la zona de quien mira; dos vacíos distintos; «desconectado» con lo que se sabe del disco |
| neuf | Código de tiempo fijo y dos relojes con nombre |
| Fractos | Mes por teclado; editor con el problema en línea; piezas de estado |
| Clipper | Buscar en un texto: siguiente, anterior, marcar todas |
| Stalker | «Entrada guardada · no disponible» |

## 1. Vocabulario

- **Debe:** un control se llama igual en documentos, código y ayuda, con los nombres de la tabla de [controles comunes §4](../docs/suite-ui.md#4-vocabulario-común): Valor, Perilla, Deslizador, Plano XY, Campo numérico, Medidor, Código de tiempo, Regla de tiempo, Barra de posición, Fecha, Rango de fechas, Mes, Horario, Editor de texto, Buscar en el texto, Tabla de datos, Rejilla, Filtro, Mapa, Lienzo de imagen, Cargando, Vacío, Aviso, Sin conexión, Disco no conectado. En Swift llevan el prefijo `Nu9ve`; en `Nu9ve.WinUI`, `Nu`.
- **Debe:** donde el sistema tiene el control (`Slider`, `DatePicker`, `Table`, `NumberBox`, `CalendarDatePicker`, `InfoBar`), se usa el nativo con las reglas de este contrato. Sólo se dibuja uno propio cuando el sistema no lo tiene (perilla, plano XY, regla de tiempo, medidor, insignia de disco).

## 2. Tema y skins

- **Debe:** un control lee roles y medidas del entorno y no pregunta qué skin está activo ([tema §1](nu9ve-theme-v1.md#1-tres-capas)). Su dibujo por skin vive en un solo lugar del módulo de controles, no en cada pantalla.
- **Debe:** roles fijos:
  - pista en `line` y relleno del valor en `accent`;
  - perilla y asa en `raised`, con borde `line`;
  - cifra en `mono`: `ink` si el valor cambió, `muted` si es el de omisión;
  - foco en `accentText`/`accentSoft` ([interacción §2](nu9ve-interaction-v1.md#2-todo-por-teclado));
  - selección en `accentSoft`, con `ink` a 7:1;
  - estados en `danger`, `warning`, `success` e `info`, siempre con texto o icono además del color.
- **Debe:** con la época `tinted` ([tema §5](nu9ve-theme-v1.md#época-de-los-skins-de-época)) el control conserva sus medidas y sólo toma los roles nuevos.
- **Debe:** lo que el usuario evalúa no lleva textura ni tinte. Eso incluye medidores, formas de onda, espectros, gráficas, mapas base y fotos. El dither decora el marco del control, nunca la pista ni el valor ([tema §2](nu9ve-theme-v1.md#2-roles-de-color)).
- **Debe:** en alto contraste de Windows mandan los colores del sistema; en Mac, «Aumentar contraste» equivale a Contraste: Alto. «Reducir movimiento» quita las animaciones de perillas, medidores (los picos saltan, sin caída animada) y HUD.
- **Conviene:** las señales de interacción de [tema §8](nu9ve-theme-v1.md#8-barras-del-sistema-menús-y-controles) salen del control, no de la pantalla. Una perilla que pasa una muesca da una señal `step` y un pulso háptico, una sola vez por muesca.

## 3. Números

### 3.1 El parámetro se declara una vez

- **Debe:** cada valor ajustable tiene un `ParamSpec`:

  | Campo | Qué es |
  | --- | --- |
  | `id` | Identificador estable |
  | `label` | Nombre visible y para VoiceOver o Narrador |
  | `min`, `max` | Límites |
  | `step` | Paso normal, en las unidades del valor; con escala `log`, una fracción del recorrido (0,01 es un centésimo). El grande son 4 pasos (o los que declare `largeSteps`) y el fino, una décima |
  | `default` | Valor al restablecer |
  | `origin` | De dónde sale el relleno (el centro de un EQ); por omisión, `min` |
  | `scale` | `linear` o `log` |
  | `unit`, `format` | Unidad y formato de la cifra |

- **Debe:** si el valor vive en el motor, el `ParamSpec` también lo da el motor ([Windows §0](windows-app-v1.md#0-principio-dos-apps-nativas-un-producto)). El control no inventa límites.
- **Debe:** Hz, Kelvin y los tiempos de efectos usan escala `log`.

### 3.2 El mismo manejo en todos

Vale para perilla, deslizador, plano XY y campo numérico:

| Para | Mac | Windows |
| --- | --- | --- |
| Cambiar | Arrastrar: la perilla en vertical (y a medias en horizontal); el deslizador, en su eje | Igual |
| Ajuste fino | ⌥ al arrastrar o con las flechas; más presión de Force Touch afina también | Alt al arrastrar o con las flechas |
| Un paso | ← → (y ↑ ↓ en controles verticales) | Igual |
| Paso grande | ⇧ con las flechas; Re Pág / Av Pág | Igual |
| Restablecer | Doble clic, Inicio o «Restablecer a …» en el menú contextual | Igual |
| Máximo | Fin | Igual |
| Rueda | Cambia el valor sin desplazar la página | Igual |

- **Debe:** las teclas actúan sólo con el foco en el control. Una lista nunca roba las flechas a un control ajustable, y un campo de texto activo se queda con las suyas ([interacción §2](nu9ve-interaction-v1.md#2-todo-por-teclado)).
- **Debe:** mientras se arrastra, la vista previa es en vivo y no se guarda nada. Al soltar queda una sola entrada de deshacer, con nombre («Deshacer Temperatura»).
- **Debe:** la cifra siempre se ve, en `mono`, con su unidad y el separador decimal del idioma: `−6,0 dB`, `440 Hz`, `5600 K`, `1,25×`, `50%`. Lleva el signo menos U+2212, sin separador de miles, redondeada a la mitad lejos del cero, y la unidad va tras un espacio de no separación salvo `×`, `%` y `°`. Un cero redondeado no lleva signo, y un valor que no es número se lee `—`.
- **Debe:** un valor exacto del motor (precio, conteo, medida) se muestra como llegó. No se redondea en una tabla ni en una leyenda, y un nulo es `∅`, nunca cero ni vacío.
- **Conviene:** muescas hápticas en `origin`, en `default` y en cada paso grande.

## 4. Fechas, horas y tiempo

- **Debe:** el motor manda instantes en ISO 8601 con zona (`Z`, `±HH:MM` o `±HHMM`, con o sin segundos y fracción). Una fecha sola o una hora sin zona no es un instante. La app los muestra **en la zona de quien mira**, con una sola función para toda la app. Lo que no se puede leer se muestra como llegó, nunca vacío.
- **Debe:** si el dominio se define en otra zona, la etiqueta lo dice: «Desde UTC», «09:00 en America/Hermosillo».
- **Debe:** fecha absoluta por omisión. «Hace 5 min» sólo en listas de actividad, con la fecha absoluta en la ayuda del puntero y en la etiqueta accesible (decisión del usuario, 2026-11-22).
- **Debe:** un rango dice sus bordes: inicio incluido y fin excluido por omisión, escrito junto al control. Un rango que cuesta aplicar (consulta, cálculo) se aplica con un botón, y la vista muestra el rango aplicado; uno barato, en vivo.
- **Debe:** un horario lo calcula el motor. La app manda zona, hora local y días, y muestra la vista previa de las próximas ejecuciones que devuelve. La app no suma días ni horarios de verano.
- **Debe:** un selector de día se maneja por teclado: ← → un día, ↑ ↓ una semana, Re Pág/Av Pág un mes, y «Hoy» con `T`. Su etiqueta accesible es la fecha completa con lo que tiene ese día.
- **Debe:** el código de tiempo es `m:ss.cc` o `h:mm:ss.cc`, con dígitos monoespaciados, el signo U+2212 en los negativos y `--:--.--` para un tiempo que no es finito. Donde importan los cuadros y la cadencia es conocida, `h:mm:ss:ff`. Con dos relojes (fuente y editado), cada tiempo dice cuál es.
- **Debe:** en una línea de tiempo hay una sola regla para todos los carriles:
  - clic mueve el cabezal;
  - arrastrar selecciona tiempo;
  - la franja superior define el loop;
  - el snap a la rejilla da una muesca háptica por compás.

  Con teclado: ← → mueven el cabezal un paso de rejilla, ⇧ extiende la selección e `I`/`O` marcan entrada y salida. J/K/L se conservan en video ([interacción §8](nu9ve-interaction-v1.md#8-atajos-comunes-de-la-suite)).
- **Debe:** una serie con huecos los muestra como huecos; no los interpola ni los rellena, y la leyenda dice el lapso real.

## 5. Textos largos

- **Debe:** el texto que se lee se puede seleccionar, va en el rol `body` y con el ancho de lectura limitado. Si no se puede editar, la vista dice por qué.
- **Debe:** para editar se usa el editor del sistema, con ⌘S y los cambios sin guardar a la vista ([macOS §6](macos-app-v1.md#6-documentos)). Un problema del borrador se dice en línea, junto al botón que deshabilita.
- **Debe:** ⌘F (Ctrl+F) abre **Buscar en el texto**:
  - siguiente y anterior con ⌘G y ⇧⌘G (F3 y Shift+F3 en Windows);
  - marcar todas y la cuenta («3 de 12»);
  - la consulta no distingue mayúsculas ni acentos;
  - las coincidencias van en `accentSoft`, y la actual además con borde;
  - Esc cierra y devuelve el foco al texto.
- **Debe:** en un texto con tiempos (una transcripción), cada segmento muestra su tiempo en el reloj elegido. Return o un clic llevan el cabezal ahí.
- **Conviene:** en textos muy largos, la posición («línea 120 de 900») en la barra de estado.

## 6. Listas, tablas y selección

- **Debe:** listas virtualizadas con el control nativo: `Table`/`NSTableView` o `NSCollectionView` en Mac; `ItemsRepeater`, `ItemsView` o `ListView` en Windows.
- **Debe:** se ordena por cualquier columna según su tipo: números como números, fechas como instantes y texto con el orden del idioma. Los nulos van al final.
- **Debe:** los filtros se ponen como fichas o como menú, cada uno con su cuenta. El filtro activo siempre se ve, con una forma de quitarlo. Los problemas (falta, sin conexión, duplicado) son filtros.
- **Debe:** selección múltiple con ⇧ (rango), ⌘/Ctrl (alternar) y ⌘A/Ctrl+A. La cuenta de seleccionados va en la barra de estado. Las acciones aplican a toda la selección y se deshacen como una sola.
- **Debe:** las flechas recorren la lista, y los atajos de una letra actúan sólo con el foco en ella, nunca mientras se escribe.
- **Debe:** dos vacíos distintos (§9): «aún no hay nada» y «el filtro lo oculta todo».

## 7. Mapas, imágenes y medios

- **Debe:** el contenido que se evalúa va sin tinte: la foto sobre gris neutro y el mapa base sin textura.
- **Debe:** un mapa funciona sin red. Las capas del motor se sirven en local, y si el mapa base no está queda un lienzo en blanco. Las librerías web se empaquetan con la app, nunca se cargan de un CDN.
- **Debe:** el mapa es lo más nativo que tenga el sistema (MapKit en Mac). En Windows, si no hay uno nativo que funcione sin servicio de pago, va una página web empaquetada; se decide al probarlo en una máquina con Windows (decisión del usuario, 2026-11-22).
- **Debe:** en un lienzo de imagen:
  - ajustar y 100 % con teclado (⌘0, ⌘1, ⌘+ y ⌘−; Ctrl en Windows);
  - pellizco para ampliar;
  - a 100 %, la rueda desplaza la imagen.
- **Debe:** Espacio es vista previa o reproducir ([interacción §8](nu9ve-interaction-v1.md#8-atajos-comunes-de-la-suite)).
- **Conviene:** con el foco en un mapa, las flechas desplazan y + − amplían; el elemento elegido se anuncia con su nombre.

## 8. Voz, seguimiento y gestos

El teclado está en [interacción](nu9ve-interaction-v1.md). Cada control publica sus teclas al catálogo de su §1; no las escribe a mano.

- **Voz** (sin usuario todavía; se documenta por si llega, decisión del usuario del 2026-11-22). Si una app la usa:
  - **debe** tener equivalente escrito para toda entrada por voz;
  - **debe** pedir el micrófono al usarlo ([macOS §8](macos-app-v1.md#8-sistema));
  - **debe** mostrar un medidor de nivel y un estado visible mientras escucha;
  - **debe** callar los sonidos de interfaz ([tema §12](nu9ve-theme-v1.md#12-sonido-y-háptica-de-tema));
  - **debe** mostrar lo transcrito para corregirlo antes de actuar.
- **Seguimiento por cámara.** **Debe:**
  - histéresis al empezar y al soltar un gesto;
  - soltar lo que se tenía agarrado al perder la mano (unos 300 ms);
  - el centro del encuadre es todo el rango;
  - sólo corre con la ventana visible y enfocada;
  - un indicador mientras la cámara está encendida;
  - todo lo que se hace así tiene equivalente con teclado o ratón.
- **Gestos.** **Debe:** un gesto nunca es la única forma de hacer algo ([interacción §2](nu9ve-interaction-v1.md#2-todo-por-teclado)). La presión de Force Touch sólo afina, nunca cambia el valor de golpe, y la háptica se puede apagar en Ajustes.

## 9. Estados

| Estado | Qué dice | Cómo se ve |
| --- | --- | --- |
| **Cargando** | Qué se carga («Consultando entregas…») | Indicador pequeño en el sitio de lo que carga, nunca un velo sobre la ventana. Si tarda más de un par de segundos y el trabajo se puede medir, es una acción larga ([interacción §7](nu9ve-interaction-v1.md#acciones-largas)) |
| **Vacío: aún no hay nada** | Qué aparecerá y cómo llega («Conecta una SD o termina una sesión») | Icono, título y la acción principal si existe. El ornamento de la identidad va sólo aquí ([tema §7](nu9ve-theme-v1.md#7-identidad-de-cada-app)) |
| **Vacío: filtrado** | «Ningún elemento con estos filtros» | El mismo, con «Quitar filtros» |
| **Error** | Qué falló, qué se conserva y qué hacer | Aviso en `danger` dentro de la vista, con «Reintentar» y el detalle técnico plegado |
| **Sin conexión** | Qué servicio no responde y desde cuándo | Aviso en `warning` arriba de la vista. Lo último leído sigue a la vista con su hora. Reintenta solo, con espera creciente, y tiene un botón |
| **Disco no conectado** | El nombre del disco, «desconectado» y lo que se sabe de él (tamaño, última vez visto) | El elemento sigue en su lugar, atenuado, con su insignia, y la barra cuenta «N fuera de línea». Las acciones que necesitan el archivo se deshabilitan, con la razón en la ayuda. Nunca es un error ni desaparece |
| **No disponible** | Un dispositivo guardado que no está: «Entrada guardada · no disponible» | La elección del usuario se conserva en la lista, marcada |

- **Debe:** los estados de un disco son los de [`nu9ve-volume-v1`](nu9ve-volume-v1.md); la insignia los traduce y no inventa otros.

## 10. Accesibilidad

- **Debe:** cada control llega a VoiceOver y a Narrador con nombre, valor (con unidad), estado y su atajo en el hint ([interacción §9](nu9ve-interaction-v1.md#9-accesibilidad)). Un control de valor tiene la acción ajustable, con el mismo paso que las flechas. Lo decorativo (dither, ornamento, animación de progreso) no llega.
- **Debe:** «Acceso total por teclado» en Mac y Tab en Windows recorren todos los controles en orden de lectura.
- **Debe:** cada control entra en la matriz de contraste por superficie ([Windows §3](windows-app-v1.md#3-tema)): normal, sobre, presionado, deshabilitado y foco, × skin × modo. Vale también en Swift.

## 11. Plataformas

- **Mac:** los controles comunes viven en `swift/` (`Nu9veFormat` sin interfaz y `Nu9veControls`), según [controles comunes §5](../docs/suite-ui.md#5-dónde-va-cada-librería).
- **iOS, control por control** (decisión del usuario, 2026-11-22):
  - compila también en iOS lo que es SwiftUI puro y sirve a una pantalla táctil, con zonas de toque de 44 pt;
  - lo que depende de AppKit queda en macOS hasta que haga falta su par táctil;
  - cada pieza dice en qué sistemas compila.
- **Windows:** la lógica va en `Nu9ve.Core` y los controles en `Nu9ve.WinUI`. Los colores llegan a los controles nativos por sus claves ([Windows §3](windows-app-v1.md#3-tema)).
- **Web** (curado, Atlas, la página de gráficas de nove): el core da este contrato y sus vectores, sin código (decisión del usuario, 2026-11-22). Si se muda la librería de la web de Fractos a un core de web, se decide allá.
- **Gráficas:** este contrato fija lo que se ve (huecos, valores exactos, sin textura), no con qué se dibujan. La herramienta espera el estudio G1 de [controles comunes §7](../docs/suite-ui.md#7-orden-de-extracción).

## 12. Validación

- **Debe:** las funciones puras (escala, pasos, formato de cifras, código de tiempo, fechas) se prueban con vectores comunes en `data/` ([`ui-format-vectors-v1.json`](../data/ui-format-vectors-v1.json)), en Swift, C# y TypeScript. El cambio de zona se prueba con la hora de pared en tres zonas (UTC, una sin horario de verano y una con él). El texto localizado de una fecha lo da el formateador de cada sistema y no entra en los vectores.
- **Debe:** una prueba por control comprueba que tiene etiqueta y valor accesibles y que su catálogo de teclas no choca con el de la app.
- **Debe:** el recorrido sólo con teclado de [interacción §10](nu9ve-interaction-v1.md#10-validación) incluye variar un valor, elegir un rango de fechas, buscar en un texto y seleccionar varios elementos, donde la app los tenga.
- Lo que no se puede probar automáticamente (Force Touch, háptica, VoiceOver, Narrador, cámara) se anota como no verificado.

## Adopción

Quién adoptó qué está en [adopción por app](../docs/adoption.md#nu9ve-controls-v1). Esa tabla cambia cuando una app avanza, sin tocar este contrato.

## Cambios

Regla nueva o más estricta: añadirla, actualizar su tabla en [adopción por app](../docs/adoption.md#nu9ve-controls-v1) y avisar en cada app afectada. Un cambio que rompe sube a `nu9ve-controls-v2`.

- **2026-11-22:** `Nu9veFormat` (C1) precisa cuatro reglas sin cambiarlas. El paso de una escala `log` es una fracción del recorrido. La cifra lleva U+2212, sin miles, y la unidad tras un espacio de no separación. Un instante lleva su zona. Los vectores prueban la hora de pared, no el texto localizado.
- **2026-11-22:** primera versión, propuesta, desde [controles comunes §6](../docs/suite-ui.md). Lleva las decisiones del usuario de ese día: ⌥ para el ajuste fino, fechas absolutas por omisión, iOS control por control, mapas de Windows lo más nativos posible y voz sólo documentada.
