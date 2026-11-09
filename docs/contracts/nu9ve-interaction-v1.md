# Teclado e interacción de las apps de nu9ve v1

Estado: **propuesta** (2026-11-07). Es la base de interacción de la suite, al mismo nivel que el [tema visual](nu9ve-theme-v1.md) y el [comportamiento de Mac](macos-app-v1.md): todo se puede hacer con el teclado, las teclas se ven en el control que las usa, y cada app explica lo que el puntero o el foco tienen debajo. Cada app lo aplica según su UX (un editor de fotos no navega igual que un grabador), pero el vocabulario, los atajos comunes y las piezas (catálogo, badges, ayuda contextual y panel de atajos) son los mismos. Escribir la regla no la da por adoptada: ver [adopción](#adopción).

**Debe** es obligatorio salvo que la app documente por qué no aplica. **Conviene** es lo recomendado.

Referencias leídas en el código el 2026-11-07:

| App | Dónde | Qué toma este contrato |
| --- | --- | --- |
| Clipper TUI | `clipper_tui/app.py` (`ShortcutSpec`, `SHORTCUT_SPECS`), `docs/tui-architecture.md`, `tests/test_tui_shortcuts.py` | Una sola tabla de atajos con alcance (`local`, `panel`, `global`) y visibilidad según el contexto; la barra de contexto con los atajos del foco; editar, seleccionar y exportar sin ratón |
| curado | `src/lib/keyboard.ts`, `src/lib/hints.ts`, `src/components/ContextHelp.tsx`, `KeyboardMap.tsx`, `docs/CONTROLS.md` | Badge con la tecla en cada control; capa de Option que muestra otras teclas mientras se mantiene; HUD «tecla · acción»; footer de contexto con «Más info»; remapeo con detección de conflictos; códigos físicos; campos de texto respetados |
| SoundShop | `Sources/SoundShop/ShortcutsPanel.swift` (`ShortcutCatalog`, `ShortcutBadge`, `ShortcutsContent`), `WorkspaceHelp.swift`, `WorkspaceBottomBar.swift` | Un catálogo que leen panel, ayuda y barra de estado, con prueba de duplicados; el seguimiento del puntero que escribe en la barra de estado sin volver a dibujar la ventana; el botón del footer que muestra la tecla del control apuntado y abre el panel de atajos (⌘/) por tarea o por teclado |
| Fractos | `docs/DESIGN_SYSTEM.md` (señales de interacción) | Que una acción se sienta igual con teclado, ratón o trackpad (⌘1…⌘6 suenan como un clic de pestaña) |

## 1. Un solo catálogo

- **Debe:** cada acción de la app se declara una vez en un catálogo con: identificador estable, título, grupo, teclas, alcance (global, vista, panel o foco), cuándo está visible o activa y un nombre corto que quepa en un keycap. Es el `ShortcutSpec` de Clipper y el `ShortcutCatalog` de SoundShop.
- **Debe:** menús, badges, ayuda contextual, panel de atajos, Ayuda y HUD leen ese catálogo. No hay texto de atajo escrito a mano en una vista.
- **Debe:** una prueba falla si dos acciones del mismo alcance comparten tecla, si un atajo pisa uno del sistema (§8) o si una acción del menú no tiene entrada en el catálogo.

## 2. Todo por teclado

La regla es la del TUI de Clipper: se puede navegar, seleccionar, editar, variar y aplicar sin tocar el ratón.

| Para | Teclas base | Notas |
| --- | --- | --- |
| Moverse entre zonas | Tab / ⇧Tab; ⌘1…⌘9 entre vistas | El foco recorre barra → lista → detalle → acciones, en orden de lectura |
| Recorrer listas y rejillas | ↑ ↓ ← →; ⇧ extiende la selección; ⌘A todo | Una lista con foco nunca roba las flechas a un control ajustable |
| Abrir o aplicar | Return | En un elemento: abrir. En un plan o diálogo: la acción principal |
| Vista previa o alternar | Espacio | Quick Look, play o marcar, según la app |
| Salir | Esc | Cierra lo último que se abrió (búsqueda, popover, edición) sin perder trabajo |
| Variar un valor | ← → (y ↑ ↓ en controles verticales); ⇧ paso grande; ⌥ paso fino | Home vuelve al valor por defecto, como en curado |
| Buscar | ⌘F; `/` donde no hay campo de texto activo | `/` es el atajo del TUI y de la web |
| Presets y recetas | 1…9 o la tecla del preset; ⇧⌘C / ⇧⌘V copiar y pegar ajustes | Aplica a la selección y se puede deshacer |

- **Debe:** el foco siempre se ve (anillo o fondo en `accentText`/`accentSoft` del tema), también en los skins de época.
- **Debe:** un campo de texto activo se queda con sus teclas. Letras, Espacio y flechas no disparan acciones mientras se escribe ([macos-app-v1 §4](macos-app-v1.md#4-barra-de-menús)).
- **Debe:** toda acción de ratón tiene equivalente de teclado, o la app documenta por qué no (rayar un plato, dibujar un trazo). Un gesto puede ser más rico, pero no puede ser la única forma de hacer algo.
- **Conviene:** atajos de una letra para las acciones frecuentes de la vista (como `P`/`E` en Clipper o `V`/`B`/`E` en SoundShop), activos solo con el foco en esa vista y siempre en el catálogo.

## 3. Badges de teclas

Las teclas se ven donde se usan, sin cambiar el layout.

- **Debe:** un control con atajo puede mostrar un keycap pequeño con su tecla, en el rol `label` del tema, con `raised` y `line`. Va sobre el control (overlay), nunca desplaza otros elementos. Lo dibuja el skin: caja de 1 bit en Macintosh, bisel en Windows 9x, `[K]` en Terminal.
- **Debe:** al mantener un modificador (⇧, ⌃, ⌥ o ⌘), los badges cambian para mostrar qué hace cada control con esa combinación, como la capa de Option en el mixer de curado. Al soltarlo, vuelve la capa base. Los controles sin acción en esa capa atenúan su badge en vez de esconderlo, para que la capa se lea como un mapa.
- **Debe:** la capa aparece tras una pausa corta (~300 ms) para no parpadear cuando alguien usa un atajo de corrido, y respeta «Reducir movimiento» (aparece sin animación).
- **Debe:** una opción en Ajustes → Teclado: **Teclas en pantalla**: Siempre · Al mantener un modificador (por defecto) · Nunca. curado y su cabina pueden usar Siempre por defecto, porque su UX es un instrumento.
- **Debe:** el badge es ayuda, no la única fuente: el nombre y la tecla también llegan por `help` y por VoiceOver.

## 4. Ayuda contextual y seguimiento del puntero

Para apps complejas (curado, SoundShop, kyu, neuf, nove) es **Debe**; para utilidades pequeñas, **Conviene**.

- Una línea en el footer o en la barra de estado dice el nombre del control bajo el puntero **o con el foco**, qué hace y sus teclas en la capa actual. El seguimiento es un solo almacén delegado: entrar a un control actualiza esa línea, no vuelve a evaluar la ventana (`WorkspaceHelpStore` en SoundShop, `ContextHelp` de curado).
- Al pasar a la ayuda misma, se conserva el último control, para poder leer o expandir.
- **Conviene:** «Más info» expande la explicación en el footer sin tapar el contenido, y Esc la cierra devolviendo el foco.
- El texto vive junto al catálogo, no se genera solo. Una acción sin explicación muestra al menos su título y sus teclas.

## 5. Panel de atajos

- **Debe:** ⌘/ abre el panel de atajos, y también un botón de teclado en el footer. Ese botón muestra el keycap del control apuntado o, sin control, «Atajos» (`ShortcutBadge` de SoundShop). También está en el menú Ayuda.
- **Debe:** dos vistas: **por tarea** (grupos del catálogo) y **por teclado** (un mapa con teclas ocupadas, libres y la capa del modificador que se mantenga). Se puede buscar por nombre o por tecla, y filtrar a la vista actual.
- **Debe:** sigue el tema y el skin, y se maneja entero con teclado.

## 6. Remapeo

- **Conviene:** Ajustes → Teclado permite reasignar con detección de conflictos y volver a los valores de fábrica (curado). Lo reasignado aparece en badges, menús, panel y ayuda porque todos leen el catálogo.
- Las teclas que forman un layout físico (las dos manos de curado, la rejilla de pads) se guardan por código físico; los atajos mnemónicos (`P` de preview) por carácter, para que sigan teniendo sentido en otros idiomas de teclado.

## 7. Respuesta al usar una tecla

- **Debe:** al usar un atajo, el control que lo recibe se ilumina brevemente. Un atajo que no aplica en ese momento dice «no disponible» en la línea de ayuda o en un HUD, en vez de no hacer nada.
- **Conviene:** un HUD breve «K · Acción» en apps donde la acción no se ve en pantalla (curado, SoundShop). Respeta «Reducir movimiento».

### Acciones largas

Una acción que tarda más de un par de segundos (copiar, verificar, liberar, ingresar, exportar) se muestra igual en todas las apps y sistemas. La referencia es `ActionProgress.swift` de nüün, hecho para servir de base a otros loaders.

- **Debe:** al empezar, la acción **reemplaza** la pantalla desde la que se lanzó (el plan, el formulario) por una vista centrada que dice:
  - qué se hace, con un verbo concreto («Copiando y verificando finales», no «Procesando»);
  - el archivo en mano, con origen → destino;
  - qué pasa si se pausa o se cancela.
- **Debe:** una **barra siempre visible** con porcentaje, bytes y archivos. Mide el trabajo real de la acción completa, no solo del paso actual: el motor emite `overall_done`/`overall_total` y nunca retrocede. Sin total conocido, la barra es indeterminada, pero visible.
- **Debe:** el **detalle** va plegado por defecto. Al abrirlo muestra el archivo actual y un log corto tipo terminal de los pasos terminados («✓ copiado y verificado X → MEDIA502», «− retirado de edición Y · ESSD»). El log lo emite el motor y la vista no lo reconstruye.
- **Debe:** la animación la dibuja el skin: documentos que viajan entre discos en Cristal, hormigas de selección en Macintosh, papeles con bisel en Windows 9x y `[====>····]` con un `$ cp --verify` en Terminal. Con «Reducir movimiento» queda quieta y conserva todos los números. Es decorativa: no intercepta clics ni llega a VoiceOver, y la barra lleva su valor accesible.
- **Debe:** los colores del efecto son los de estado del tema (§2): azul para copiar, ámbar para retirar, verde para lo hecho y rojo para descartar. Así se lee de un vistazo qué se respalda, qué se retira y qué no cambia.
- Al terminar, la vista deja paso al resultado. Si se reabre lo mismo después, primero se dice qué ya pasó y qué falta (en nüün: «Cierre aplicado… · N entregas siguen en edición esperando publicarse»).
- En Windows aplica igual, con la barra y la animación dibujadas por el tema y el progreso también en la barra de tareas (`ITaskbarList3`), como conviene.
- **nüün (2026-11-07):** el motor da la medida con `media/progress.py` (`ActionMeter`, el patrón de `closure.ActionProgress`). La usan el cierre y «Respaldar publicados…» desde la hoja, el ingreso y «Respaldar publicados» desde Discos (Transfer: leer + verificar en cada destino + releer antes de limpiar), Revisar liberación (`apply_release`: verificar el respaldo + releer cada original) y Copiar al SSD desde Biblioteca (`deliver_work`: copiar + verificar). El ingreso lo publica en el progreso del job; liberar y recuperar, en `action_progress`. Cada una tiene una prueba `test_apply_reports_one_overall_measure_with_a_log`. Junto a la barra global va un indicador pequeño del archivo actual (`file_done`/`file_total`, que emiten la revisión, la liberación y el ingreso), y el resumen añade el tiempo restante estimado con el ritmo real de la medida global después de unos segundos. La animación se dibuja en un `Canvas` asíncrono a 12 cuadros por segundo y se pausa con la app inactiva; la medición está en STATUS.

## 8. Atajos comunes de la suite

Los mismos en todas las apps cuando la acción existe. Command y las combinaciones del sistema son de macOS: no se reasignan.

| Atajo | Acción |
| --- | --- |
| ⌘, | Ajustes |
| ⌘/ | Panel de atajos |
| ⌘? | Ayuda |
| ⌘1…⌘9 | Vistas o espacios principales, en el orden de la navegación |
| ⌘\ | Alternar vista compacta / completa, si la app tiene ambas |
| ⌃⌘F | Pantalla completa |
| ⌘F, `/` | Buscar |
| Espacio | Vista previa / play / marcar |
| Return | Abrir o acción principal |
| Esc | Cancelar, cerrar lo último o salir de la edición |
| ⌘Z / ⇧⌘Z | Deshacer / Rehacer, nombrando la operación |
| ⇧⌘C / ⇧⌘V | Copiar y pegar ajustes o receta (kyu, SoundShop, neuf) |

Atajos de dominio que ya existen y conviene conservar entre apps del mismo tipo: calificar 0–5, P/X/U y etiquetas 6–9 en kyu (como Lightroom); J/K/L e I/O para video en neuf y Clipper.

## 9. Accesibilidad

- **Debe:** cada control nombrado para VoiceOver incluye su atajo en el hint; funciona con «Acceso total por teclado» de macOS.
- **Debe:** los keycaps cumplen el contraste del tema (texto 4.5:1 sobre su fondo) en todos los skins.
- **Debe:** «Reducir movimiento» quita las animaciones de badges, HUD y capas; el contenido sigue ahí.

## 10. Validación

- **Debe:** la prueba del catálogo de la §1.
- **Debe:** un recorrido de QA solo con teclado por app: abrir, navegar, seleccionar, variar, aplicar un preset o plan y cerrar, sin ratón. Se registra qué tareas lo lograron.
- Lo que no se puede probar automáticamente (Full Keyboard Access, VoiceOver, teclados de otros idiomas) se anota como no verificado.

## Adopción

Leído en el código el 2026-11-07, sin probar a mano. Revisar al actualizar.

| Regla | curado | SoundShop | Clipper TUI | nüün | kyu | Fractos | nove · neuf · Stalker |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 Catálogo | Sí (`keyboard.ts`) | Sí, con prueba | Sí (`SHORTCUT_SPECS`), con prueba | No: atajos solo en el menú | Sin revisar | Sin revisar | Sin revisar |
| 2 Todo por teclado | Sí en la cabina | Parcial | Sí | Parcial: menús, ⌘1…⌘6, pasos de Ingreso | Parcial: cribado con 0–5, P/X/U, flechas | Parcial | Sin revisar |
| 3 Badges y capas | Sí, con capa de Option | Sí en el footer; en los controles sin revisar | Barra de contexto (terminal) | No | Sin revisar | No | Sin revisar |
| 4 Ayuda contextual | Sí (footer) | Sí (barra de estado) | Barra de contexto | No | Sin revisar | No | Sin revisar |
| 5 Panel de atajos | Pestaña teclado | Sí (⌘/) | Sin revisar | No | Sin revisar | No | Sin revisar |
| 6 Remapeo | Sí | Sin revisar | Sin revisar | No | Sin revisar | No | Sin revisar |

Una app adopta el contrato añadiéndose aquí con lo que le falta. Primeros pasos sugeridos:
- **nüün:** catálogo a partir de su menú, badges con capa de modificador, línea de ayuda en el footer del Ingreso y ⌘/. En Windows (maqueta, 2026-11-09) ya hay catálogo con pruebas de duplicados y de teclas del sistema, aceleradores y tooltips generados desde él, `AccessKey` en la navegación y panel Ctrl+/; es la referencia para portar el catálogo a la app de Mac.
- **kyu y neuf:** el recorrido completo por teclado de la §10, porque son las apps de edición más densas.

## Cambios

Regla nueva o más estricta: añadirla, actualizar esta tabla y avisar en cada app afectada.
