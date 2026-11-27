# Acciones, progreso y estados: qué subir al core

Estado: **propuesta** para una sesión del core (2026-11-26). La escribió una sesión de nüün a pedido del usuario, después de usar nüün en la M1 y la M4 con varios procesos a la vez. No cambia ningún contrato ni ninguna pieza: dice qué hay, qué falla en el uso real y en qué orden subirlo. Cada pieza entra en un cambio propio, con sus pruebas y su línea en el [CHANGELOG](../CHANGELOG.md) ([AGENTS](../AGENTS.md)). Lo que nüün hace en su repo está en su `PLAN.md`, sección «UX · acciones y procesos».

Leído en: `n-creator-utils` `a85ade4` (`native/Nuun/Sources/Nuun/ActionProgress.swift`, `engine/crates/nuun-media/src/{delivery,transfer,living,work_copy,conversations}.rs`), Clipper `docs/export-progress.md`, este repo en `f267022`. Antes de empezar, la sesión del core **debe** releerlos (`git log`) por si cambiaron.

## 1. Lo que pidió el usuario (2026-11-26)

En sus palabras, resumidas:

- Estandarizar progreso, barras de carga y animaciones entre temas (o por tema), con el manejo de estados en el core, reutilizable y escalable. Clipper tiene mucho progreso en su TUI y nüün un buen progreso nativo con Rust y animaciones por tema.
- Mensajes y animaciones de éxito, y manejo de errores, con el mismo lenguaje.
- Que **todo proceso** use esas animaciones adaptadas a su vista: o todo pasa por una vista principal de proceso (como la de Ingreso, con formatos distintos), o, si la persona está en otra vista, un diálogo compacto de progreso que se transforma en ese proceso y luego regresa.
- Hover y animaciones estandarizados por tema, también en los controles × − + de la barra (al menos un brillo o *glow*).
- Color del tema Mac en bordes y líneas de la barra de título, y temas correctos también en subventanas.
- Documentarlo para que otros procesos lo usen bien (docs públicos de la pieza).

## 2. Lo que ya hay

| Qué | Dónde | Estado |
| --- | --- | --- |
| Regla de las acciones largas: reemplaza la pantalla, barra siempre visible, detalle plegado con log, animación del skin, colores de estado | [interacción §7](../contracts/nu9ve-interaction-v1.md#acciones-largas) | Propuesta; referencia nüün |
| El progreso se lee sondeando un método (`action_progress`, `closure_progress`, `storage_progress`; nove `events`) | [motor §6.2](../contracts/nu9ve-engine-v1.md#62-progreso-y-cancelación) | Propuesta; sin forma común del registro |
| Vista y animación por skin (`ActionProgressView`, `ActionTravel` a 12 fps fuera del hilo principal, `SkinProgressBar`) | nüün `ActionProgress.swift` | En uso en ingreso, cierre, entregas, liberación y copia entre discos |
| Medidor del motor (`Meter`, `ActionProgress` con `stage`, `files_*`, `overall_*`, `current`, log) | nüün `transfer.rs`, `delivery.rs` | En uso; una sola app |
| `ProgressPlan`: pesos por etapa, nunca retrocede, ETA tras 5 s y 2 %, «ETA pending» a los 15 s sin avance, *watchdog* de 120 s, `Stage`/`Step` | Clipper `docs/export-progress.md` | En uso en CLI/TUI |
| `TransferAnimation` (copia de `ActionTravel`, sin variantes de época) | `windows/Nu9ve.WinUI/Controls/TransferAnimation.cs` | En el core |
| `Nu9veActionProgress` / `Nu9veLoadingState` como nombres | [suite-ui](suite-ui.md) | Sin código |
| Época `tinted` (Macintosh y Windows 9x con el color de la persona, también barra y controles) | [tema §5](../contracts/nu9ve-theme-v1.md#5-catálogo-de-skins), `ThemeSurfaces` | Hecho en el core; **nüün no la adopta todavía** |
| Toda ventana sigue el tema, también su barra | [tema §8](../contracts/nu9ve-theme-v1.md#8-barras-del-sistema-menús-y-controles) | Regla; por comprobar en sheets y popovers de nüün |

Hay tres implementaciones del mismo progreso (Swift de nüün, C# del core, TUI de Clipper): ya se cumple la regla de extraer cuando una pieza se repite.

## 3. Lo que falla en el uso real (nüün, 2026-11-26)

1. **Procesos secundarios sin progreso.** El respaldo de conversaciones (4,4 GB, 7 403 archivos a MEDIA502) tardó ~37 min con sólo un *spinner* junto a «Respaldar ahora». El motor sí guardaba el progreso (`action-progress:<plan>` con etapa, archivo, archivos y bytes), pero la llamada era de una vez y la vista no lo leía. La persona no sabía si iba bien o se había trabado.
2. **Preparar y revisar no se ven.** «Copiar al otro disco» muestra un *loader* en la esquina mientras revisa. Esa revisión hashea en los dos discos cada archivo que ya existe en el destino, así que crece con los datos sin decir qué hace.
3. **Escaneos con su propia animación** (Acumulados, mapa de discos), distinta de la de las acciones.
4. **Al cambiar de vista no queda rastro** de lo que corre. Sin indicador en la barra ni en el Dock.
5. **Terminal:** las dos líneas (`ESSD [====>···] SUBSSD` y `$ cp --verify …`) se alinean a la izquierda entre sí, así que el bloque se ve descentrado.

## 4. Propuesta

### 4.1 Un registro de acción común (contrato)

Sumar al [motor §6.2](../contracts/nu9ve-engine-v1.md#62-progreso-y-cancelación) (o a un contrato propio, decide el core) la forma del registro que el cliente sondea, con lo que nüün y Clipper ya escriben:

- `action` (verbo), `stage` de un vocabulario cerrado: `preparing`, `listing`, `reviewing` (hashear lo que ya existe), `copying`, `verifying`, `releasing`, `writing_receipt`, `complete`, `failed`, `cancelled`; una app puede añadir etapas con prefijo propio.
- `overall_done`/`overall_total` con `unit` (`bytes`, `files`, `items`); nunca retrocede (regla de Clipper: se redistribuye lo pendiente).
- `files_done`/`files_total`, `current {action, from, to, name, file_done, file_total}`, `log[]` corto emitido por el motor.
- `started_at` y `updated_at` como latido: el cliente dice «sin avance desde hace N s» (no «trabado») con las reglas de ETA de Clipper.
- Al terminar: `result` (resumen y recibo) o `error {code, message}` en palabras, y si se puede reanudar.
- Lo pide quien lanza la acción con un `progress_id`; el motor no lo inventa.

### 4.2 Tres tamaños de la misma vista (interacción §7)

1. **Pantalla** (lo de hoy): cuando la acción se lanzó desde esa vista.
2. **Compacta**: un panel flotante con la misma barra y animación, cuando la acción corre y la persona está en otra vista o en la ventana compacta. Se expande a (1) con un clic y vuelve.
3. **Indicador**: un chip en la barra de título con el porcentaje (clic → 2), progreso en el Dock y la barra de tareas ([os-integration](platforms/os-integration.md)), y un aviso al terminar según [avisos v1](../contracts/nu9ve-notifications-v1.md). Las acciones de fondo (respaldo programado) usan sólo este tamaño.

Varias acciones a la vez: una fila por acción en (2), la más reciente arriba.

### 4.3 Final: éxito, aviso y error

Un mismo cierre para todas: un estado `complete` con animación corta del skin y la señal de éxito de [sonido v1](../contracts/nu9ve-sound-v1.md); `complete` con pendientes (p. ej. «8 archivos cambiaron durante la copia: entran en el próximo respaldo») como aviso; `failed` con el error en palabras, qué quedó hecho y «Reanudar» o «Reintentar». Respeta «Reducir movimiento».

### 4.4 Carga corta y escaneos

`Nu9veLoadingState` para lo que tarda menos de un par de segundos. Un escaneo largo es una acción más (`unit: items`), con la misma vista.

### 4.5 Hover y controles de ventana por skin

Sumar al [tema §5](../contracts/nu9ve-theme-v1.md#5-catálogo-de-skins) una columna de tratamiento de `hover` y `press` por skin (brillo en Terminal y Gótico, inversión en Macintosh, bisel en Windows 9x, tinte en Cristal), que también aplica a × − + de las barras propias. Con la señal `hover` de [interacción](../contracts/nu9ve-interaction-v1.md#7-respuesta-al-usar-una-tecla).

## 5. Piezas y orden

| Orden | Pieza | Destino | De dónde | Condición |
| --- | --- | --- | --- | --- |
| 1 | **Hecho el 2026-11-26:** [`nu9ve-action-v1`](../contracts/nu9ve-action-v1.md), contrato propio con el método común `action_progress` (decisiones del usuario), y los tres tamaños y el cierre en [interacción §7](../contracts/nu9ve-interaction-v1.md#acciones-largas). Registro de acción (§4.1) y tres tamaños (§4.2–4.3) | contratos de motor e interacción, con ejemplos en `data/` | nüün `delivery.rs`, Clipper `ProgressPlan` | Ya hay dos productores |
| 2 | **Hecho el 2026-11-26**, salvo `Nu9veLoadingState`: `Nu9veAction` (registro, reglas de lectura y monitor) y `Nu9veActionProgress` (pantalla, compacta, indicador, Dock y cierre en los cinco skins) en [`swift/`](../swift/README.md#nu9veactionprogress-la-vista-de-una-acción-larga). `Nu9veActionProgress` (vista, travesía por skin, barra, compacta, indicador) y `Nu9veLoadingState` | `swift/` | nüün `ActionProgress.swift`; Fractos `FractosProgressBar` | Tres implementaciones del mismo progreso |
| 3 | Ponerle a `TransferAnimation` las variantes de época y el tamaño compacto | `windows/Nu9ve.WinUI` | La pieza 2 | Cuando nüün retome Windows |
| 4 | Medidor `nu9ve-progress` (escribe el registro, ETA, latido) | `rust/` | nüün `Meter`/`ActionProgress`; Clipper `ProgressPlan` | Cuando Clipper o nove lo use como segundo motor. **Cumplida el 2026-11-27:** nove produce `action_progress` desde su build 36 (`a3a38be`) con su propio código, y nüün lo adoptó con el suyo (F4). Comparadas el 2026-11-27 ([§6](#6-los-dos-productores-comparados-2026-11-27)): se muda el núcleo, no el almacén; el usuario decidió cerrar como `failed` un registro de un proceso muerto |
| 5 | Hover y press por skin, también en × − + (§4.5) | tema §5 y `Nu9veTheme`/`NuMacShell` | Fractos (vocabulario de señales) | Con la pieza 2 o antes |

nüün adopta cada pieza en su repo. Mientras tanto corrige en su propio código lo que no depende de esto (su `PLAN.md`), dejando el comentario de origen para mudarlo después.

## 6. Los dos productores, comparados (2026-11-27)

Se leyó el código, sin correrlo: nüün `engine/crates/nuun-media/src/action.rs` (966 líneas, F4 y U1.3) y nove `core/src/actions.rs` (279 líneas, build 36). Clipper (`core/src/rpc_jobs/progress.rs`, `ProgressPlan`) todavía no escribe registros: sólo sirve de referencia para `phase` y `step`.

| Aspecto | nüün | nove |
| --- | --- | --- |
| Patrón de `progress_id`, `unknown`, latido de 5 s y retiro a las 24 h | Igual | Igual |
| Dónde vive | Fila de su catálogo SQLite (`action-progress:<id>`) | Archivo con `nu9ve-fs` en `runtime/action-progress/<digest>` |
| Quién escribe | El hilo del paso; el latido, un hilo con su conexión, con un `UPDATE` condicional para no pisarlo | Un hilo supervisor por motor, que recibe por canal el fin de cada comando |
| Límite de escrituras | 250 ms, siempre al cambiar de etapa | No hace falta: una escritura por comando |
| Fracción que no retrocede | Suelo y tope `total − 1` mientras corre; varios pasos continúan el registro | Por construcción (`unit: work`, un punto por comando) |
| Etapas, log, `current`, archivos | Mapa a las comunes o `nuun.*`; log de hasta 60, archivo en mano | Fijas (`analyzing`, `writing`); sin log ni archivos |
| Cancelar y reanudar | Por acción | Nunca `cancellable`; `resumable` siempre falso |
| Un registro `running` de un proceso muerto | Se puede reemplazar cuando el latido pasa de 60 s | Se cierra como `failed` al arrancar el motor y en cada lanzamiento |

**Desvíos del contrato encontrados** (cada app los arregla en su repo):
- **nüün:** al cerrar con `complete` sube `overall_done` al total, pero no mueve `progressed_at`, que la §3.1 pide mover (`action.rs` ~375). Abre con `files_total: 0` aunque no lo sepa. Un archivo terminado no fuerza la escritura retenida (es un «conviene» de la §3.3).
- **nove:** el `summary` de un fallo dice siempre «Se publicó la cobertura. Faltantes quedó pendiente.» (`actions.rs` ~97), para cualquier acción. Hoy sólo se ve con Atlas, pero no sirve en cuanto haya otra.

**Recomendación: mudar el núcleo a `rust/nu9ve-progress`, no el almacén.** Las reglas puras coinciden, y las de nüün ya están probadas; lo que cambia es la concurrencia y dónde se guarda. El crate tendría:
- validar el id y la etapa;
- `open`, `apply` (suelo, tope `total − 1`, `progressed_at`) y `finish` (100 %, sin `current`, `cancellable: false`);
- `unknown`, `may_start` (`action_running`) y `expired`;
- un `Throttle` de 250 ms y un `Heartbeat` que llama a un `trait Store { load, save, touch }`. Cada app pone su almacén.

Se queda en cada app: el almacén y sus claves, el `within` y el alias `closure-progress` de nüün, su mapa de etapas, los textos de `summary`, y el hilo, `busy` y `nove_result` de nove. Compila con Rust 1.89 (las `let`-chains de nüün son estables desde 1.88 en edition 2024).

**Decisión del usuario (2026-11-27): un registro `running` cuyo proceso murió se cierra como `failed`**, como hace nove, «por ahora». nüün hoy lo reemplaza al pasar 60 s sin latido, y cambia al adoptar el crate. El contrato admite las dos, así que no cambia.

