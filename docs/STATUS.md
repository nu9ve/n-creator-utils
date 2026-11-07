# Implementation status

Updated: 2026-11-07, America/Hermosillo. Current contract closure is recorded below; entries describe their own snapshots/builds. Check Git status and HEAD before resuming. It is **not a production release**. [Next priorities](../PLAN.md), [release gates](release.md).

## Coordinación de la suite: mapa, Windows e interacción · 2026-11-07

Solo documentación, sin commit. Se agregan:
- [Mapa de la suite](suite.md): apps, para qué sirven, identidad, plataformas, UI, motor y transporte, flujos que cruzan apps, contratos y piezas candidatas a paquete por plataforma.
- [apps de Windows v1](contracts/windows-app-v1.md): par de macos-app-v1, con motor compartido, equivalencias Mac↔Windows (barra, teclas, segundo plano, avisos, secretos, paquete y QA) y cómo se aplican tema y teclado. Toma como referencia lo que nove ya tiene en Windows (lo último revisado: `a34e5f4`, con lector desde Chrome y A21) y Tauri en curado.
- [Teclado e interacción v1](contracts/nu9ve-interaction-v1.md): catálogo único, todo por teclado, badges con capas por modificador, ayuda contextual y panel ⌘/.

Las tablas de adopción se llenaron leyendo los repos; nada se probó en Windows desde esta sesión.

## Terminal reemplaza a Grano en nüün · 2026-11-07

Commit `1bff4b4`. Instalada como build **`f5cefb24e2b9`** con el árbol limpio y la app cerrada; la anterior quedó en `app-backups/ca7d7bfd.app`. Grano se retiró porque no aportaba frente a Macintosh; quien lo tenía guardado vuelve a Cristal. Terminal completa los tres comunes del [contrato §5](contracts/nu9ve-theme-v1.md#juego-de-skins-por-app):
- Siempre oscuro y con ventana cuadrada.
- El color base se vuelve fósforo: mismo matiz, a 7:1.
- Todos los estilos de texto en monoespaciada, con líneas de barrido estáticas y botones en video inverso.
- Barra con línea de señal, `[×] [−] [+]` a la izquierda y `~$ nüün`.

Evidencia: `swift test` **22/22** (matriz con Terminal y prueba de matiz de la señal), localización sin faltantes ni sobrantes, `--qa-skins` aislado en los 5 skins. Se revisaron las capturas de Terminal: solo fixtures. **Sin probar:** clics reales.

## Controles de ventana legibles en los skins · 2026-11-07

En working tree, sin commit. Instalada como build **`1c16ca1cb645`** (desde el árbol sin commit; la anterior quedó en `app-backups/8d4c31cd.app`). A pedido del usuario, los controles propios de Macintosh, Windows 9x y Gótico ya no usan cajas vacías ni rombos sin significado. Van siempre a la izquierda en el orden de macOS (cerrar, minimizar, ampliar) con ×, − y + visibles. La época solo cambia el dibujo: caja de 1 bit, bisel gris o losa con el acento en cerrar. En Win9x el título sigue a los controles. La regla quedó en el [contrato §8](contracts/nu9ve-theme-v1.md#8-barras-del-sistema-menús-y-controles). `--qa-skins` con `HOME` y `NU9VE_MEDIA_HOME` temporales pasa igual en los 5 skins. Se revisaron las capturas de la ventana principal y de Ajustes: solo fixtures. Al pedir revisión, el botón de compacto/completo quedaba pegado al borde derecho en los skins de ventana cuadrada: el toolbar del sistema era el que daba ese margen. Ahora la barra propia usa 8 pt a ambos lados, alineada con los controles del caption, y en las capturas de la ventana real queda a la misma distancia que en Cristal. **Sin probar:** clics reales en los controles nuevos.

## Instalación con skins y colores de estado · 2026-11-07

Instalada en `~/Applications/nuun.app` la build **`d220fa121c80`**, desde `278b62a` con el árbol limpio y la app cerrada por el usuario. Incluye el tema `nu9ve-theme-v1`, los skins opcionales y los colores de estado. La anterior quedó en `app-backups/a6f67e7c.app`. El instalador informa `release_ready: false` (faltan validación con equipo, aceptación GUI y notarización). No se abrió después de instalar.

## Colores de estado por skin en nüün · 2026-11-07

En working tree, sin commit. `ThemeRoles` agrega los estados del contrato (§2): `danger`, `warning`, `success` e `info`, y `warningSoft` para el fondo de los avisos. Cada uno sale del matiz del sistema mezclado 12 % con `ink` y ajustado a 4.5:1 sobre las tres superficies. El aviso se ajusta también sobre su propio banner. Nunca toman el acento, así que un color base rojo no confunde un error con una selección. Unos 35 colores fijos (`.orange`, `.red`, `.green`, `.blue`) de Inicio, Ingreso, Discos, Ajustes, cierre de proyectos y liberación pasan a estos roles, igual que el botón destructivo. Esto corrige el aviso «Sin proyecto» sobre el gris de Windows 9x.

Siguen como colores de dominio documentados la paleta categórica del mapa de discos (`diskClassColor`) y los puntos de madurez de Flujos, que van acompañados de texto.

Evidencia:
- `swift test`: **22/22**. La matriz de contraste cubre los estados en los 5 skins × claro/oscuro × 196 colores, y una prueba confirma que con un color base rojo cada estado conserva su matiz.
- `check_localization.py`: sin faltantes.
- `git diff --check`.
- Capturas `--qa-snapshots` en Cristal claro, Windows 9x y Gótico.

**Incidente de QA corregido:** una corrida de `--qa-snapshots` en Win9x capturó en Inicio sesiones reales de `~/Movies/Stalker` (solo lectura). El vigilante de capturas refrescó después del fixture, y el motor lee Stalker desde el `HOME` real aunque `NU9VE_MEDIA_HOME` sea temporal. Las imágenes se borraron. Ahora las builds de depuración congelan las capturas al poner el fixture, y [la QA visual se corre con `HOME` temporal](development.md#qa-visual-aislada). Las capturas válidas de Win9x y Gótico se hicieron así y solo muestran fixtures.

**Sin probar:** clics reales y build instalada.

## Skins opcionales en nüün · 2026-11-07

En working tree, sin commit. **Ajustes → Apariencia → Skin** ofrece, además de **Cristal** (por defecto, sin cambios), cuatro skins para quien quiera personalizar su espacio. **Restaurar claro** vuelve a Cristal.

- **Grano:** dither Bayer estático sobre el fondo y radios menores. Los títulos en Silom.
- **Macintosh:** 1 bit con tarjeta de color. Barra de título con rayas, caja de cerrar, de colapsar y de zoom; botón principal con anillo; al pulsar, se invierte.
- **Windows 9x:** caption con degradado del color base, biseles y botones de época.
- **Gótico:** siempre oscuro, viñeta, marco doble, controles en rombo y títulos en Luminari.

Macintosh y Windows 9x son siempre claros, y Apariencia lo explica sin perder la preferencia de modo.

Los tres de época usan **ventana cuadrada** como Fractos. `ShellWindow.swift` es una copia comentada de `NuMacShell`, con un cambio: el zoom propio solo aplica con esquinas cuadradas. La barra es propia y tiene dos filas en Macintosh y Win9x (caption de época arriba, barra de nüün abajo). Hay bordes para redimensionar, caja de redimensionar dibujada, y ⌘W, ⌘M, zoom y Opción para pantalla completa. Ajustes usa el mismo skin. Cambiar de skin conserva el marco de la ventana. Todos los skins pasan la matriz de contraste del contrato. Solo se usan fuentes del sistema (Silom, Tahoma, Luminari), sin assets nuevos.

Evidencia:
- `swift test`: **21/21**. La matriz cubre 5 skins × claro/oscuro × 196 colores base, más la regla de modo fijo por skin y la matriz Bayer.
- `check_localization.py`: 671 claves, sin faltantes ni sobrantes.
- `git diff --check`.
- `--qa-snapshots` de cada skin con fixtures.
- **`--qa-skins`** (nuevo), con la ventana real y datos ficticios: en los 5 skins, título, toolbar, borde izquierdo con `ShellWindowResizeOverlay` en los cuadrados, zoom 1530×856 → 1920×1080 → 1530×856 y Ajustes con la misma forma. Además, un viaje a pantalla completa en Win9x que al salir conserva la ventana cuadrada, y la vuelta a Cristal que restaura título y toolbar.
  - La primera corrida de `--qa-skins` leyó en solo lectura las sesiones reales de `~/Movies/Stalker`, porque el fixture se aplicaba antes del primer refresco. Esas capturas se borraron, el orden se corrigió y la corrida que vale usa solo fixtures.

**Sin probar:** clics y arrastres reales en las cajas, bordes y doble clic de la barra propia; VoiceOver en los controles de época; Aumentar contraste. El aviso naranja fijo «Sin proyecto» (`InboxView`) tiene poco contraste sobre el gris de Win9x. Sin build instalada.

## Tema visual nu9ve-theme-v1 en nüün · 2026-11-07

Primera adopción del [contrato de tema](contracts/nu9ve-theme-v1.md), en working tree y sin commit. La paleta pasa a los roles comunes (`surface`, `raised`, `line`, `ink`, `muted`, `accent`, `onAccent`, `accentText`, `accentSoft`). Se resuelve en `MediaMenuCore/ThemeColor.swift`, que es pura y lleva la copia comentada de `readable` de SoundShop. Las superficies conservan la receta anterior. Los textos solo se ajustan cuando no llegarían al mínimo: `ink` 7:1, `muted` y `accentText` 4.5:1, e `ink` sobre `accentSoft` 7:1. Antes, un color base oscuro en tema oscuro dibujaba casillas y radios a ~1:1. Los trazos e iconos que usaban el acento crudo ahora usan `accentText`. En Apariencia hay presets suizos: Ámbar (por defecto), Navaja, Acero y Glaciar. Elegir uno solo cambia el color, no el modo.

Evidencia:
- `swift test`: **19/19**, incluida una matriz de 196 colores base × claro/oscuro, y la prueba de que el color por defecto conserva sus valores.
- `check_localization.py`: 654 claves, sin faltantes ni sobrantes.
- `git diff --check`.
- Capturas offscreen `--qa-snapshots` con estado aislado (`NU9VE_MEDIA_HOME` temporal, preferencias por argumento) en tres casos: claro por defecto, oscuro con Navaja y oscuro con azul a V=0.15.
  - El claro por defecto se ve como antes.
  - En el azul oscuro los radios y casillas se leen, pero sin matiz. El relleno del segmento seleccionado casi no se distingue: el texto en negrita lo sostiene.
  - En los fixtures de estas corridas aparece el aviso «Root unavailable» de la carpeta de media temporal; no tiene que ver con el tema.

**Sin probar:** clics reales en los presets, cambio de modo con la app abierta y Aumentar contraste. Sin build instalada.

## Aceptación reportada, Sony y trabajo por defecto · 2026-11-06

**Reporte del usuario:** flotante funciona; detección/popup incierta (posible tardanza o apertura manual), se mantiene pendiente sin pedir repetir ahora porque necesita editar. Completó ingreso de cámara y Stalker, pero Sony mostró `[Errno 2] No such file or directory: 'C0001.MP4'`. No declarar aceptado el cierre Sony.

**Lectura operativa:** job Sony `060b585d-2f3f-5b37-8020-65ff0dc8ca71` quedó `failed/FileNotFoundError`, 15 miembros con copias de archivo registradas, seis miembros `cleaned=1` (dos MP4, dos XML, dos thumbnails); nueve índices conservados en journal. Los dos assets editoriales quedaron `skipped` para trabajo. Job Stalker posterior `ed3f70ec-d37a-58e7-815c-f9abfa8d86e9` terminó `completed`, dos sesiones del mismo proyecto en trabajo ready. Esto no identifica el punto exacto del FileNotFoundError ni demuestra doble borrado; falta traceback/reproducción dirigida. No se reanudó ni cambió a completado el job Sony. Planes futuros de cierre pueden seguir bloqueados por ese job abierto.

**Entrega física pedida expresamente:** `media deliver <dos assets Sony> --work ESSD` reutilizó `deliver_work` existente para `buscando-baches-con-el-dron`: **2 MP4, 7,026,723,235 bytes**, desde MEDIA502 a `001_proyectos/e64_raw/buscando-baches-con-el-dron/` en ESSD. SHA verificado por el motor, recibos de trabajo y estados ready registrados; archivos presentes y tamaños comprobados. Conservó archivo, no leyó/borró SD ni modificó proyectos de Clipper. Snapshot SQLite previo en `/private/tmp/nuun-before-work-delivery-20261106.sqlite3`. No se asocia esta operación al replay de limpieza Sony.

**Producto:** nuevos ingresos arrancan Todo el lote para trabajo; la elección Ninguno/selección es por ingreso y restaurar borrador respeta su política. Destinos/opciones siempre visibles en Flujo. Vista previa de nombre en proyecto general, unidad y asignación masiva mediante `project_preview` read-only, con `common.slug` compartido y sin crear proyecto. Sugerencias incluyen proyectos revisados de esta sesión, borradores y capturas pendientes. No se implementa un único plan multifuente: cámara/Stalker siguen por fuente/raíz. [Contrato](contracts/ingest-flow-v1.md#copias-de-trabajo-y-nombres--actualización-2026-11-06).

**Validación:** 66 pruebas de `test_media_engine.py` aprobadas, incluido preview contra planner y ausencia de creación de proyecto; Swift 14/14. Localización 650 claves sin faltantes/sobrantes. QA `--qa-ingest-choices` con catálogo/preferencias temporales comprobó default/reset, sugerencias y RPC de normalización: OK. Los PNG de esa QA resultaron negros y **no acreditan revisión visual**; falta aceptación visual de nombres/destinos en la app. Sin nuevos tests automáticos sobre discos personales.

**Instalada build `17acd51df25c`**, artefacto `/private/tmp/nuun-work-default-final-20261106.app`, firma estricta verificada. App anterior conservada en `~/Library/Application Support/n-creator-utils/app-backups/08490985.app`; sin operación activa (sólo lector) al cerrar por UI y actualizar. Ejecutable idéntico al artefacto, 28 Python de media idénticos al checkout; RPC `project_preview` instalado respondió correctamente con catálogo temporal. Reabierta por UI; primer estado observado fue Inicio sin datos todavía, no acredita velocidad de refresco ni popup. Runtime externo/ad-hoc, sin declaración de release.

**Pendiente:** detección/latencia, diagnóstico de fallo Sony y reconciliación segura, selección unificada de pendientes y aceptación visual de esta entrega. El usuario decidió dejar la captura del README para después; no se reemplazó ninguna captura. Sin commits, mensajes a otros chats ni edición de JSON externos.

## Compacto flotante y popup · 2026-11-06

Usuario reportó que insertar SD con nüün en menubar no abrió compacto y pidió mantenerlo encima hasta cerrar o ampliar. Diagnóstico de sólo lectura: `showQuickImporter` persistido en **false**; el default de Model y Ajustes ya era **true**. Se respeta el opt-out, sin migración que lo sobreescriba para otros usuarios. Preferencia de esta instalación habilitada por petición explícita.

`Application.present` aplica nivel `floating` y no oculta al desactivar sólo en compacto; selección/completo y entrada a fullscreen restauran `normal`. Cerrar baja el nivel y mantiene menubar; reabrir compacto recupera flotación. [Contrato](contracts/ingest-flow-v1.md#barra-y-montaje). Sin cambios de planner, copia, catálogo ni limpieza.

**Validación:** Swift **14/14**. QA `--qa-window-levels` con preferencias/estado temporal y backend inexistente: ventanas AppKit reales compacto → selección → completo → compacto con otra ventana en key; niveles 3/0/0/3, visibilidad y cierre/reapertura correctos. No acredita cambio entre apps, fullscreen/Spaces físicos ni reconexión SD. Localización: 650 claves, sin faltantes/sobrantes; `git diff --check` correcto y 235 enlaces locales revisados. Build release compilada y firma estricta verificada.

**Instalada** build `88fa9b79a571` desde `/private/tmp/nuun-floating-20261106.app`. Sólo lector persistente activo antes de actualizar; script de instalación aborta ante backend one-shot y la app conserva su rechazo de quit durante operación. App anterior en `~/Library/Application Support/n-creator-utils/app-backups/65a88eda.app`. Ejecutable idéntico al artefacto y **28 archivos Python de media** instalados idénticos al checkout. `showQuickImporter=1` verificado, app reabierta `--background` con lector de su backend instalado. Observación UI posterior mostró Inicio; no se aplicó ningún plan ni se observó un nuevo montaje. Firma ad-hoc/runtime externo; no release de distribución.

**Siguiente:** usuario expulsa de forma segura y reinserta la SD con ventana cerrada, verifica popup, cambio de app y ampliación; aceptación física pendiente. No se copió/borró media ni se confirmó ningún job de Sony. Sin actualizar capturas del producto: no cambió el diseño estático.

## Handoff vigente y aceptación preparada · 2026-11-06

Revisión documental y lectura dirigida del motor/UI; **sin nueva build, instalación ni prueba física**. Última instalación registrada: `78790688291f`; comprobar versión efectiva antes de operar. [PLAN](../PLAN.md) consolidado por prioridad, separando implementación, aceptación y propuestas; historial de builds conservado abajo.

**Reporte del usuario:** Untitled se ha formateado y reutilizado varias veces para copiar/mover Sony. El incidente de montaje del 27 octubre deja de ser bloqueo actual; causa/reparación y recuperación del contenido antiguo no acreditadas. Persiste aviso de formato/limpieza pendiente. Lectura de `transfer.confirm_camera_cleanup` y UI: Sony retira clips tras verificar, conserva índices y termina `camera_action_required`; Actividad → «Ya la formateé…» comprueba ausencia de clips antiguos y cambio de índices. No se reconcilian automáticamente jobs por reutilizar la tarjeta. Falta identificar mensaje/job antes de diagnosticar defecto o cerrar históricos.

**Contratos reconciliados:** Clipper ya ofrece retiro editorial con desregistro por CLI/TUI (5 noviembre); nüün conserva editables y consulta deps frescos, sin invocación automática de retire. Atlas actualizó propuesta el 6 noviembre a A1–A10: no nadir, trabajo selectivo, estado/exports y productos ODM costosos; nüün sin implementar. Limpieza/espacio reportados por Atlas no son una medición de esta sesión.

**Aclaración del usuario:** todo el recorrido se hará desde nüün, sin preconfiguración ni planes por CLI. Reporta borrado externo de intermedios y eliminación de algunos proyectos desde Clipper. Lectura de `closure.py`, `library.py` y `clipper.py`: planes nuevos inventarían archivos presentes; no adoptan desapariciones durante apply como retiro propio; `removed_from_work` distingue copias de trabajo ausentes; referencias/offline de Clipper pueden bloquear liberación aunque una carpeta haya desaparecido. Esto no verifica cuáles archivos/proyectos reales se borraron ni si Clipper los desregistró. [Semántica y siguiente comprobación](contracts/project-closeout-v1.md#cambios-externos-y-borrados-previos--2026-11-06).

**Siguiente acción:** aceptación acompañada con dos videos multicam nuevos; asignaciones elegidas por el usuario desde la app. [Recorrido y registro pendiente](sony-validation.md#aceptación-acompañada-sony-y-dos-multicams--2026-11-06). Empezar en menubar con ventana cerrada, insertar SD, observar popup y revisar plan antes de ejecutar. No se copió ni borró media, no se cerraron jobs ni se enviaron mensajes a otras sesiones. Validación documental: `git diff --check` sin errores y 223 enlaces locales de 11 Markdown modificados sin destinos faltantes. Sin suites ni apps por ser cambio sólo documental; las suites/builds de abajo siguen siendo evidencia de sus fechas.

## Ingreso directo y publicación por final · 2026-11-05

Implementado e **instalado a petición del usuario**, build **`78790688291f`** en `~/Applications/nuun.app`. [Ingreso](contracts/ingest-flow-v1.md), [publicados/cierre](contracts/project-closeout-v1.md#actualización-vigente--2026-11-05). Entrada reúne fuente y contenido; Flujo reúne destinos y revisión agrupada; Resultado muestra el avance global. El botón del plan autoriza copiar/verificar/liberar según la política visible, sin otro modal. La excepción por SSD esperado ausente queda inline; el descarte sin réplica del cierre sigue requiriendo revisión explícita.

Proyecto general opcional si cada contenido principal ya tiene asignación. Soporte de cámara agrupado, respaldado/verificado y oculto por defecto, sin proyecto editorial ficticio. Cada bundle Stalker tiene una fila desplegable con destinos, acciones y miembros. Recibos con soporte versionados `/2`, recovery compatible con `/1`; revisión del planner participa en el ID para no reutilizar un job antiguo con otro digest. Progreso global monotónico entre archivos/fases/sesiones y reanudación; ETA aproximada medida, sin inventar duración de dependencias/retiro. Sony mantiene índices y el formato manual sigue pendiente aunque el trabajo automático llegue a 100 %.

Popup de tarjeta vuelve a habilitarse con montaje nuevo aunque exista un resultado anterior; conserva borradores durante la sesión y no interrumpe jobs activos ni escanea profundamente por defecto. Navegación principal centrada en la ventana. Publicación **sin canales**, por carpeta `subidos`/configurada o marca individual; guardar decisiones registra evento atómico. **Respaldar publicados…** permite retirar sólo finales y conservar raws/renders para seguir editando. El cierre completo posterior vuelve a verificar fuentes, reenlaza con ACK y retira raws/intermedios identificados; asociaciones de finales archivados, estado terminal y evento se publican en una transacción. El movimiento independiente de finales mantiene el bloqueo de Clipper durante consulta y retiro. Clipper editable se conserva mientras no exista API pública de retiro/desregistro; Final Cut y desconocidos se conservan.

**Evidencia:** `/usr/bin/python3 -m unittest discover -s tests -v`: **286 pruebas, 4 omitidas** por JSON Schema ausente. Contratos con el Python existente `3.9.6`: **19/19**; el entorno antiguo `data` no resolvió `$ref` y no se usó como evidencia de aprobación. Sin instalar dependencias. Swift **14/14**; localización **650 claves**, sin faltantes ni sobrantes; `compileall`, `git diff --check` y **208 enlaces locales** comprobados. Fixtures de recuperación/versiones, asignaciones, soporte, progreso con fallos/replay, publicación parcial, asociación archivada y atomicidad del cierre.

QA nativa con engine real y RPC aislado: Sony sin proyecto general → plan de media+soporte → `camera_action_required`, índices conservados y global 1.0; Stalker agrupado, borradores, popup simulado y bloqueo durante job; centrado a 760/1180/1600 px y fullscreen (desviación máxima 0.5 px). Ventana real con eventos sintéticos: doble clic en espacio vacío amplía/restaura; pestañas cambian ruta sin ampliar. Cierre: publicados primero conserva render y actualiza ubicaciones; cierre completo después sin recopia retira raw/render, conserva editable y resuelve el aviso. Clipper deps/guard simulados explícitamente en esa QA; no acredita integración física nueva. Captura del README actualizada con **datos ficticios**, autorizada por el usuario; PNG nativo revisado.

Artefacto `/private/tmp/nuun-ingreso-directo-20261105.app`, firma estricta verificada antes/después de instalar. Binario instalado idéntico al artefacto y **36 archivos Python** empaquetados/instalados idénticos al checkout; `closure_overview` del backend instalado respondió `ok` con estado temporal aislado. La app estaba cerrada al instalar y se abrió la nueva build para pruebas; anterior conservada en `~/Library/Application Support/n-creator-utils/app-backups/59589db7.app`. Runtime externo `/usr/bin/python3`, firma ad-hoc.

**Pendiente:** aceptación del usuario desde la app instalada: Sony/reinserción SD, Stalker con Clipper real y discos, proyecto publicado pequeño, desconexión/reanudación y mediciones con media grande. Fotos/audio/dron, cámaras, carpetas vivas y recetas editables siguen pendientes de priorizar. No se movió ni borró media personal, no se editaron JSON externos, no se hizo commit ni se enviaron mensajes a otras sesiones. No se declara production/release ready: siguen los [gates de release](release.md).

## Completar y liberar proyectos · 2026-11-04

Implementado e **instalado a petición del usuario**, build **`1b13d0ff83e1`** en `~/Applications/nuun.app`. [Contrato y límites](contracts/project-closeout-v1.md). Proyectos → **Completar y liberar…** vincula finales de trabajo o ya archivados con selección confirmada, requisitos por proyecto/perfil (`9logs`: H; `e64`: H/V/thumbnail), publicación manual y «ya no habrá más». Guardar no mueve media. Un cierre revisado puede copiar/verificar sólo los finales elegidos, reenlazar Clipper con ACK y retirar raws verificados y `renders/` identificados. Descarte sin réplica explícito, confirmado y registrado con el digest. Inicio recuerda cierres declarados completos aún pendientes; Actividad y Proyectos reabren planes parciales. Desconocidos, bibliotecas y editables se conservan. «Mover al respaldo» ya no ignora Clipper desconocido/offline.

**Límite material:** el editable Clipper se conserva y la UI indica retirarlo desde Clipper: su borrado debe quitar también el registro y aún no tiene API pública. No se editó su repo ni sus JSON. Final Cut fuera de alcance. Tampoco se implementaron recetas, registro de cámaras, agrupación de Ingreso ni porcentaje/ETA global; el cierre informa fases y conteos reales. Material sin fuentes catalogadas se bloquea en vez de asumir cobertura por tener un final.

**Evidencia:** `/usr/bin/python3 -m unittest discover -s tests -v`: **270 pruebas, 4 omitidas** por faltar JSON Schema en ese runtime; suite de contratos con el Python existente que sí lo tiene: **19/19**, sin instalaciones. Las 13 pruebas nuevas enfocadas volvieron a pasar tras el ajuste de consultas de recibos. Caso adicional con **CLI real de Clipper**, fuentes/editor/media temporales: relink/ACK → retiro raw/render → editable conservado. Swift **14/14**; localización **644 claves**, sin faltantes ni sobrantes; `compileall`, `git diff --check` y 200 enlaces locales de documentos revisados sin faltantes.

QA `--qa-closeout` en claro y oscuro: catálogo y volúmenes fixture, engine real, deps/guard de Clipper simulados explícitamente, sin discos personales. Modelo nativo: buscar → vincular H/V/thumbnail → revisar cinco archivos/tres copias → aplicar → completado, render retirado, editable conservado y aviso resuelto. PNG temporales revisados; no reemplazan capturas del producto ni acreditan clics reales. Lectura operativa aparte: nombres de carpetas ESSD, metadata de proyectos Clipper y consulta a **copia temporal del catálogo**; no se usaron como fixtures ni para liberar.

Artefacto `/private/tmp/nuun-project-closeout-20261104.app`; firma estricta verificada. Las **36 secciones Mach-O** coinciden con el release compilado; la firma cambia bytes del ejecutable. Tras instalar: binario idéntico al artefacto, **36 archivos Python** instalados idénticos al checkout; `closure_overview` del backend instalado respondió correctamente con estado temporal aislado. Build anterior conservada en `~/Library/Application Support/n-creator-utils/app-backups/b1617e2d.app`. No había instancia activa que cerrar; se abrió la nueva app. Runtime externo `/usr/bin/python3`, firma ad-hoc.

**Pendiente:** aceptación del usuario con un proyecto pequeño desde la app instalada, incluida selección, confirmación destructiva, rendimiento de hashes/relink con discos y eventual pausa/reanudación física. No se movió ni borró media personal, no se hizo commit ni se enviaron mensajes a otras sesiones. No se declara production/release ready: siguen los [gates de release](release.md).

## Doble clic en la barra y pasos centrados · 2026-11-01

La vista propia de la barra llena el renglón del toolbar y AppKit no recibía los clics. Ahora el espacio vacío mueve la ventana y el doble clic sigue el ajuste del sistema (por defecto, zoom). En fullscreen no hace nada, como el titlebar nativo. Los pasos de Ingreso quedan centrados. `--qa-titlebar`, ventana real con eventos sintéticos: antes 1180×900 sin cambio; después alterna 1180×900 y 1920×1080, y al barrer la barra las pestañas cambian de vista sin hacer zoom. Python **255 OK**, Swift **14/14**. Sin probar con mouse real ni arrastre. Instalada build **`f7bb2e2740e9`**, firma verificada, binario y backend idénticos; anterior en `app-backups/bf3f69d5.app`. La instancia abierta estaba inactiva; se cerró desde AppleScript y se reabrió. No es una build de producción.

## Perfil erikiado64 y ruta de finales · 2026-11-01

**Perfil, instalado:** build `58384ccdb046` en `~/Applications/nuun.app` a petición del usuario, con la app ya cerrada por él: firma verificada, binario y backend idénticos; anterior en `app-backups/b1747bc6.app`. Incluye lo de la entrada siguiente. Ingreso, Proyectos, Archivos, Finales y Flujos muestran el nombre del perfil; Ajustes → Perfiles edita el perfil de Clipper y ya no lo borra al guardar sin cargar. Guardado en el catálogo real: ID `e64`, nombre `erikiado64`, `e64_raw`, ESSD, MEDIA502, `clipper_profile: erikiado64`. No se movieron ni renombraron carpetas.

**Mover finales e Inicio, instalado:** build **`79b37ccee200`** en `~/Applications/nuun.app`, con la app ya cerrada por el usuario: firma verificada, binario y backend idénticos; anterior en `app-backups/2dabac18.app`. **Mover al respaldo…** copia, verifica y libera en una sola acción, con opción de conservar. En Inicio, un proyecto asignado se resume en un badge con icono para cambiarlo. [Reglas](features/product-flows.md#finales-y-subidos). Plan real de e64 en modo mover, con copia del catálogo y sin ejecutar: 34 archivos, sin bloqueos de discos ni de Clipper, y los archivos de MEDIA502 y `03_subidos` quedaron iguales. El primer movimiento real sigue sin correrse. Plan real de e64 con copia del catálogo, sin copiar: 34 archivos, 12.86 GB, 21 a `shorts`, 10 a `thumbnails` y 3 a `2026`, sin conflictos; MEDIA502 quedó con los mismos 1,571 archivos en `e64`.

Evidencia: `python3 -m unittest discover -s tests` **255 OK**, `swift test` **14/14**, localización sin faltantes, `compileall`, `git diff --check`. `--qa-finals` con motor real y archivos inventados en carpetas temporales: detectar → plan → volver a revisar en la misma ruta → copiar y verificar → estado al día, origen conservado y lo no publicado sin copiar. **Sin probar:** clics reales, el primer plan real con discos y tema oscuro. La firma ad-hoc cambia en cada build, así que macOS puede volver a pedir permisos de discos. No es una build de producción.

## Flujos, discos en la barra, sesiones y finales · 2026-11-01

Pedido del usuario, en working tree sobre `9ee6121`, sin commit. **Barra:** los discos pasaron de Inicio a la barra en modo completo, junto a Ajustes; sin espacio libre al angostar, ocultos antes que las pestañas, y abren Discos. **Sesiones en Inicio:** Vista rápida con Quick Look (botón, doble clic, o seleccionar y espacio) y **Mover a la Papelera…**. No borra: `capture_discard_check` exige sesión terminada de una biblioteca conocida, sin respaldo, sin operación abierta que la incluya y sin uso reportado por Clipper; Clipper desconocido sólo avisa porque la Papelera se puede deshacer. Se vuelve a comprobar antes de mover y se guarda nota de la decisión. **Finales:** sólo lo que está en la carpeta de publicados cuenta como por respaldar; se detecta y se elige por perfil. **Corrige la entrada de abajo:** «e64 74 de 74» mezclaba material aún en uso. **Flujos:** pestaña principal de sólo lectura con bloques y líneas; todavía no se edita. [Propuesta y ruta de finales](features/product-flows.md#finales-y-subidos).

Finales reales con copia del catálogo, sin escribir en discos, 1.2 s contra MEDIA502, nombre y tamaño:

| Perfil | Publicados | Por respaldar | Esperan publicarse | Ya en respaldo |
| --- | --- | --- | --- | --- |
| e64 | `03_subidos` | 32 · 12.86 GB | 42 · 10.91 GB | 0 |
| 9logs | `subidos` | 0 | 5 · 4.18 GB | 0 |
| 9sets | ninguna | 0 | 0 | 43 · 17.86 GB |
| a9sets | ninguna | 0 | 0 | 2 · 2.15 GB |
| fractos | ninguna | 0 | 0 | 1 · 0.05 GB |
| nu9vexyz | ninguna | 1 · 0.29 GB | 0 | 0 |

Evidencia: `python3 -m unittest discover -s tests` **242 OK**, `swift test` **14/14**, `check_localization.py` 591 claves sin faltantes ni sobrantes, `compileall`, `git diff --check`. Capturas offscreen revisadas de Flujos, Inicio y barra a varios anchos, con fixtures. `--qa-projects` con motor real y una sesión inventada recorrió revisar → Papelera → salir de la lista; retiró esa misma sesión de la Papelera. **Sin probar:** clics reales, teclado y panel de Quick Look, hoja de confirmación en ventana, tema oscuro y vista previa con video real. No cambia planner, transferencia ni liberación. Artefacto sin instalar: `/private/tmp/nuun-flujos-20261101.app`, build **`c3903c702723`**, firma ad-hoc verificada, runtime externo `/usr/bin/python3`. No es una build de producción.

## Proyectos, avisos y fuentes coherentes · 2026-11-01

**Instalada a petición del usuario** la build `9e669ccffea7` en `~/Applications/nuun.app` el 2026-11-01: firma verificada, binario idéntico al artefacto y backend `media/` idéntico al repo; la huella del árbol coincide con el build. Anterior (`d3760d868bff`) en `~/Library/Application Support/n-creator-utils/app-backups/d1e28134.app`. La instancia abierta sólo tenía su lector `rpc-serve`, sin jobs ni escaneos activos; se cerró desde AppleScript y se reabrió. Sin operaciones de media. Aceptación del usuario en la app pendiente.

Pedido del usuario sobre la build instalada, implementado en working tree sobre `b41ad08`, sin commit. **Biblioteca → Proyectos** es el subtab por defecto: etapa calculada de recibos y sesiones (`draft`, `in_progress`, `backed_up`), estado declarado (`active`, `paused`, `finished`; no borra nada), filtros, conteos ligados, renombrar y subir a recientes. Los borradores existen desde que una sesión recibe nombre, así que ya aparecen como proyecto rápido en las demás sesiones e Ingreso. **Inicio:** guardar con Enter, sólo si cambió; un clic en un proyecto reciente lo asigna (cinco); «Usar X en las N sin proyecto»; una sola sección Stalker con Preparar lote junto a Actualizar y sin tarjetas de carpetas vacías; cada aviso se puede descartar sin cambiar el job. **Ingreso:** un solo Stalker y sin selector de tipo; la fuente lo sabe. `~/Movies/Stalker` y `~/Downloads` se revisan aunque el registro no las liste, y sólo cuentan con sesiones. Varias carpetas con sesiones siguen siendo planes separados: no hay transacción entre raíces. **Superficies:** Ajustes → Apariencia, listas y tablas sin tarjeta blanca ni filas alternas.

**Finales, sólo lectura:** Inicio lista por perfil lo que está en `002_finales` de un SSD y no en el del respaldo, comparando nombre y tamaño. Con el catálogo copiado y los discos reales, sin escribir: 2.0 s; e64 74 de 74 y 23.8 GB, 9logs 5 de 5 y 4.2 GB, nu9vexyz 1 de 1 y 0.3 GB; 9sets, a9sets y fractos completos. Copiarlos **no** está disponible y «Entrega final» salió de la UI: el planner los mandaría a `002_finales/camera-bundles/AAAA/MM/...` y duplicaría la carpeta en cada corrida. Falta una ruta por perfil con adopción de idénticos.

Defectos corregidos de paso: una tarjeta salía como «original Stalker liberado» según la fuente elegida; elegir otra carpeta heredaba tipo y limpieza, lo que fallaba al planear; fechas en UTC o ISO crudo; ID repetido y comando de reanudar en jobs terminados; guardar varias filas seguidas podía perder la petición; planear N sesiones lanzaba N procesos; `projects` leía cada plan en cada snapshot.

Evidencia: `python3 -m unittest discover -s tests` **233 OK** (13 nuevas en `test_media_projects.py`), `swift test` **13/13**, `check_localization.py` 512 claves sin faltantes ni sobrantes, `compileall`, `git diff --check`. Capturas offscreen revisadas con fixtures de Inicio, Proyectos, Ingreso, Archivos, Actividad, Discos y Apariencia. `--qa-projects` (motor real, estado aislado, bundles inventados) recorre asignar, reutilizar, renombrar y subir a recientes; `--qa-ingest` sigue llegando a respaldo verificado. **Sin probar:** clics reales en la ventana, tema oscuro de estas vistas, descartar y estados pausado/terminado sólo tienen prueba de motor. No cambia planner, transferencia ni liberación. Artefacto: `/private/tmp/nuun-proyectos-20261101.app`, build **`9e669ccffea7`**, firma ad-hoc verificada, runtime externo `/usr/bin/python3`. No es una build de producción: siguen los [gates de release](release.md) y el ciclo físico.

## Esta Mac en Discos y mensajes vacíos · 2026-10-30

Reporte del usuario sobre la build anterior: faltaba el disco de la Mac y «Todo al día» salía a la izquierda. **Esta Mac** ya aparece siempre y analiza `/System/Volumes/Data`; nunca es destino ni se puede olvidar. El mensaje de Inicio se centró y se revisaron con capturas offscreen Inicio, Contenido, Archivos y Discos vacíos. Corrida real de sólo lectura con catálogo temporal, ya borrado: 11,230,064 archivos en 424 s, 952 de 985 GB contados, 347 carpetas sin acceso, índice de 8 MB. Pruebas: Python **220 OK, 4 omitidas**, Swift **12/12**, localización 464 claves sin faltantes. Instalada la build **`d3760d868bff`** (`/private/tmp/nuun-discos-20261030b.app`, firma verificada, binario y backend idénticos); la instancia estaba inactiva y se reabrió. Clics reales pendientes.

## Mapa de discos, fase 1 · 2026-10-30

Implementado en working tree sobre `b501864`, sin commit: [mapa de almacenamiento](features/storage-map.md), [navegación](features/product-flows.md#navegación). Biblioteca tiene subtabs Archivos, Discos y Actividad; Espacio desapareció y la liberación es una hoja abierta desde Discos, Inicio o Actividad, sobre el mismo plan. Sin cambios en planner, transferencia ni liberación.

Evidencia: `python3 -m unittest discover -s tests -q` **219 OK, 4 omitidas** (78 s; 31 nuevas en `test_media_storage.py`), `swift test` **12/12** (nuevas: treemap y frontera real), `check_localization.py` 462 claves sin faltantes ni sobrantes, `compileall`, `git diff --check`, 108 enlaces sin faltantes. QA offscreen de la vista con fixture y con datos reales (`--qa-disks`).

Corridas reales de sólo lectura, con **una copia temporal del catálogo**, ya borrada; el catálogo real no se tocó y no se escribió en los discos:

| Disco | Archivos | Tiempo | Usado / contado | Índice |
| --- | --- | --- | --- | --- |
| ESSD | 79,939 | 5.8 s | 482.8 / 481.7 GB; 1.1 GB sin contar y 4 carpetas de sistema sin acceso | 62 MB |
| Carpeta personal | 8,759,049 | 355 s | 763 GB de 985 GB; el resto queda fuera de esa carpeta o sin acceso | 5.9 MB |

Los recibos clasifican 20.5 GB de ESSD como «En respaldo» sin hashear. SIGINT real durante un escaneo de ESSD: estado `cancelled`, índice anterior intacto. La primera corrida en la carpeta personal encontró un índice de más de 600 MB; de ahí salieron los límites de carpetas elegidas, la recuperación de espacio y el alto con poco disco.

**Sin probar:** clics reales en la app (la QA es offscreen), Comprobar contra el MEDIA502 real, que estaba desconectado, y cuánto tarda el HDD. Artefacto release `/private/tmp/nuun-discos-20261030.app`, build **`0d873c96934f`**, firma ad-hoc verificada. Instalado a petición del usuario en `~/Applications/nuun.app`, con binario y backend idénticos al artefacto; anterior en `~/Library/Application Support/n-creator-utils/app-backups/9ca5fc40.app`. La instancia abierta estaba inactiva, se cerró desde AppleScript y se reabrió; el motor instalado respondió `storage_overview`. Runtime externo `/usr/bin/python3`, sin notarizar. No es una build de producción: siguen los [gates de release](release.md) y el ciclo físico pendiente.

Límite conocido: **Revisar liberación…** rechaza carpetas fuera del prefijo de trabajo como `0000_acomodar`; esa salida llega en la fase 3.

## Propuesta de flujos y almacenamiento · 2026-10-30

Sólo documentación, sin cambios de comportamiento: [flujos y cierre](features/product-flows.md), [mapa de almacenamiento](features/storage-map.md) y nuevas prioridades en [PLAN](../PLAN.md). Compara la landing y Kyū contra el código.

Observado en lectura de código, sin ejecutarlo: Kyū sólo importa `kind: file`, por lo que los pares RAW+JPG (`camera_bundle`) no le llegan. Medido en sólo lectura, sin escribir en discos: ESSD 445 de 465 GB con 80,061 archivos, recorrido de metadatos en 10.4 s; el catálogo conoce 19.1 GB de ESSD; el interno tiene 12 GB disponibles. MEDIA502 estaba desconectado. La auditoría de cobertura sigue sin correrse sobre discos reales.

## Borde negro de la barra en fullscreen · 2026-10-29

En fullscreen la toolbar vive en `NSToolbarFullScreenWindow`; su fondo ya era el
canvas, pero el sistema pinta encima dos `NSVisualEffectView`, `NSTitlebarBackgroundView`
y `_NSTitlebarDecorationView`, que alrededor de la barra dejan ver el fondo negro de
fullscreen (reporte del usuario con captura). `flattenFullScreenToolbar()` oculta sólo
esas capas del sistema al entrar en fullscreen (y otra vez 400 ms después), sin tocar
la vista de la barra. `--qa-fullscreen` confirmó las capas ocultas y la barra visible;
el color final no se puede capturar sin permiso de pantalla y queda a aceptación visual.
`swift test` 10/10. Instalada build **`54638498ad9e`**
(`/private/tmp/nuun-fsbar-20261029-698b.app`, firma verificada, binario idéntico); la
instancia abierta sólo tenía su lector `rpc-serve` y se reabrió.

Seguimiento: el usuario aún veía negro alrededor de la barra (con la barra de menús
revelada u oculta). Un render fuera de pantalla de esa ventana sale crema, así que el
negro viene del compositor: `NSGlassContainerView` (1920×40) muestrea el fondo negro
detrás de la ventana y sólo la vista de la barra quedaba crema con esquinas redondeadas.
Ahora también se oculta ese contenedor y se inserta una capa del color del canvas bajo
la barra y los botones de ventana, que se retira al salir de fullscreen. QA: capas
ocultas, relleno presente, barra visible; `swift test` 10/10. Instalada build
**`81fac0c9cf40`** (`/private/tmp/nuun-fsfill-20261029-70d5.app`, firma verificada,
binario idéntico). Aceptación visual pendiente del usuario.

## Barra superior y Liquid Glass · 2026-10-29

El usuario reportó en macOS 26.5 que la barra quedaba dentro de una burbuja de vidrio
grande (con borde oscuro y sombra en fullscreen). Causa confirmada en la jerarquía:
macOS 26 envuelve los ítems de toolbar con vista propia en `NSToolbarPlatterView` +
`NSGlassEffectView`. Corrección: `isBordered = false` en el ítem (la jerarquía ya no
tiene platter), sin separador de titlebar y más margen para el nombre. El vidrio se
aplica a propósito con `GlassSurface`: pestañas, barra de pasos y pies de acción
flotantes; en macOS 14–15 cae a superficies opacas. Plurales es/en con
`Localizable.stringsdict` (verificado: «1 operación requiere atención» / «1 operation
needs attention»).

Evidencia: `swift test` 10/10, `test_media_snapshot` + `test_media_install` 10/10,
`check_localization.py` 418 claves sin faltantes, `--qa-fullscreen` con la barra
visible en fullscreen y sin platter. Instalada la build release **`7a8069ed3042`**
(`/private/tmp/nuun-glass-20261029-7534.app`, firma verificada, binario idéntico;
anterior en `app-backups/f046a6f7.app`); la instancia abierta sólo tenía su lector
`rpc-serve`, se cerró y se reabrió. Después de instalar sólo cambió
`QASnapshots.swift` (sólo debug, para la imagen del README), por eso el fingerprint
del árbol ya no coincide con ese build ID. El vidrio real no se puede renderizar fuera
de pantalla: su aspecto queda a aceptación visual del usuario. La captura
`docs/images/nuun-compacto.png` se regeneró con ese render y datos de ejemplo.

## Implementación de la revisión · 2026-10-29

Implementado en working tree sobre `f3d1651` (sin commit) a partir de la
[revisión](features/app-review-2026-10.md#implementado-el-2026-10-29): toolbar
nativa con pestañas también en fullscreen, Ingreso en cinco pasos (rápido/guiado),
selección múltiple y asignación masiva, Biblioteca con tabla/búsqueda, Actividad con
progreso, Inicio con recomendaciones por tarjeta, Ajustes en ventana propia, `snapshot`
y lector persistente de sólo lectura, `@Observable`, bloqueo por ámbito, inglés/español.
Flujo vigente: [nuun-window](features/nuun-window.md#ingreso-por-pasos-2026-10-29);
métodos nuevos: [contrato](contracts/media-library-v1.md#fuentes-y-expulsión-local-2026-10-27).

Evidencia: `python3 -m unittest discover -s tests -q` **188/188** (100.8 s; 5 nuevas
de snapshot/huellas/lector), `compileall`, `swift test` **10/10** (nuevo: lector
persistente con catálogo aislado), `scripts/check_localization.py` 418 claves sin
faltantes, `git diff --check`. QA con build debug, catálogo aislado
(`NU9VE_MEDIA_HOME`) y media sintética de ffmpeg: render offscreen de cada pestaña,
paso y modo en español e inglés; `--qa-fullscreen` en la pantalla real registró la
toolbar visible a y=1045 de 1080 y sin `autoHideToolbar` en fullscreen; `--qa-ingest`
con el motor real recorrió fuente → contenido → asignación masiva → sólo respaldo
confirmado → ejecutar: 9/9 archivos verificados en carpetas temporales, dos proyectos,
origen conservado. El motor rechazó correctamente archivo y SSD en el mismo disco físico.

Instalado en `~/Applications/nuun.app`: build release **`eb9c719514fb`**, artefacto
`/private/tmp/nuun-steps-20261029-61fa.app`, firma ad-hoc verificada, binario idéntico,
backend `media/` idéntico al repo; anterior en
`~/Library/Application Support/n-creator-utils/app-backups/5f8fa2ad.app`. La instancia
abierta estaba inactiva (sin procesos hijos) y se cerró desde su menú antes de instalar;
se volvió a abrir. Sin planes ni operaciones de media reales, sin SDs/MEDIA/ESSD.
El sistema tiene `en-MX` primero, así que la app sigue al sistema en inglés salvo que se
elija Español en Ajustes → Idioma.

Pendiente: aceptación visual/interactiva del usuario (clics reales, hojas, Quick Look,
atajos), matriz de pantallas/Spaces y accesibilidad, ciclo físico pequeño autorizado y
los gates de release; DiskArbitration, eventos de progreso empujados, agrupación de la
grilla y miniaturas de cámara siguen como propuesta.

## Revisión de app y propuestas · 2026-10-29

Revisión de código (sin abrir la app ni usar volúmenes) de ventana, navegación,
Ingreso, detección de SD, puente RPC, estilo, idiomas y plataformas:
[propuestas](features/app-review-2026-10.md). Bug reportado por el usuario: en
fullscreen con la barra de menús en ocultación automática las pestañas sólo aparecen
al revelar la barra; **no reproducido**, causa probable y corrección propuesta en el
documento. Sin cambios de comportamiento.

## Presentación del proyecto · 2026-10-29

README reorganizado para presentar nüün como multiherramienta para creadores,
con media como área más desarrollada y audio, MIDI y futuras conexiones de
dispositivos diferenciados por su estado. Cambio de documentación, sin cambios
de comportamiento ni nueva validación de producto. La revisión local y la consulta
del remoto encontraron únicamente la rama `master` en `e149503`; los hashes de
otras entradas de este documento identifican snapshots históricos.

Se añadió una captura de la vista compacta de la app abierta para ilustrar el
README; no se ejecutó un plan ni una operación de media.
El [flujo de capturas](development.md#capturas-del-producto) pide consultar al
usuario cuando cambios importantes justifiquen actualizarlas.

## Instalación del cierre contractual · 2026-10-29

A petición del usuario se compiló en release y se instaló la build local
`969812829241` en `~/Applications/nuun.app`. Artefacto firmado ad-hoc:
`/private/tmp/nuun-contract-closure-20261029-a91e.app`. Anterior conservada en
`~/Library/Application Support/n-creator-utils/app-backups/1f00446c.app`.
Builder/instalador y verificación posterior comprobaron la firma. Los 32 archivos
del backend instalado coinciden byte a byte con el repo; binario nativo idéntico
al artefacto. El CLI instalado con `/usr/bin/python3` publica `publish-revision`
e `integration-capabilities` en su ayuda. No se abrió la app ni se ejecutaron
operaciones de media; el instalador comprobó ausencia de instancias activas.
Runtime externo `/usr/bin/python3`, sin notarización ni aceptación física nueva.
Esta instalación sustituye el límite «sin nueva instalación» del cierre siguiente;
no actualiza ni certifica las instalaciones de Clipper/Stalker.

## Cierre nüün / Clipper CLI-TUI / Stalker · 2026-10-29

Implementado en working tree sobre `5a8fdf4`, siguiendo el bloque «Para nüün»
del [prompt de cierre](../../../ai/clipper/docs/2026-10-29-nuun-stalker-contract-prompts.md).
Se conservaron `.gitlab-ci.yml` y `.vscode/` preexistentes; sin commits/restage/reset,
sin editar los repos de Clipper/Stalker, sin cambios en Studio ni media personal.

- Delivery completo en sources (`media_members` con rutas exactas, hashes/tamaños)
  y handoff (todos los auxiliares). Selección de volumen de handoff explícita.
  Retiro solicita `library relink --handoff`, journaliza el documento/operación,
  exige ACK de todos los miembros y bloquea colas de filas históricas del asset.
- `publish-revision` CLI/RPC: copia nueva verificada en archivo registrado →
  escritor real Stalker con CAS/operation ID/dos FDs → validación project-only y
  hashes/membresía completos → recibo portable durable → publicación SQLite
  transaccional. Mismo asset/session/proyecto de catálogo/perfil; revisión vieja,
  jobs/copias/recibos intactos. Historial `asset_revisions`, resolve histórico y
  recovery desde ambos recibos; copia de trabajo nueva diferida vía `deliver`.
  El retitle in-place no recibe excepciones. El productor Stalker conserva su
  journal y actualiza su registro según el protocolo existente.
- Discovery opcional con timeout breve y errores por integración. La UI oculta
  Reenlazar Clipper sin capability. Ingest/catálogo/verificación/handoff funcionan
  sin esas apps; consumidor configurado/descubierto previamente, registro o cola
  conocidos siguen bloqueando retiro si quedan offline/incompatibles. No se
  instalan/inician apps ni se importan sus repos en runtime.

Evidencia: `python3 -m unittest discover -s tests -q`: **183/183** (68.345 s).
Suite de cierre real **6/6** (39.432 s), posterior al ajuste de recovery/work;
con catálogos/identidades exclusivamente temporales y FFmpeg sintético. Incluye
sources/import y handoff, dos videos + auxiliar, nombres/carpetas diferentes
entre réplicas, parcial/hash diferente/offline y reintento; cola real → bloqueo →
relink integral/ACK → deps → liberación fixture. Check final adicional del recorrido
delivery: **1/1** (13.990 s), con ACK incompleto rechazado y reintento real
idempotente del mismo operation ID. Publicación real → plan/adopción
por CLI Clipper → cola vieja intacta y bloqueante → desbloqueo al retirarla.
Cinco interrupciones de publicación (coordinador en copy_verified, producer_applied,
receipt_durable y published, más SIGKILL del escritor Stalker en manifest), mismo
ID y conflicto de payload; reconstrucción de seis revisiones desde recibos.
Capacidad ausente, major desconocido, standalone y conocido offline cubiertos.
No se simula el registro de la revisión en catálogo ni se modifica JSON editorial
para preparar estos recorridos: análisis/colas/adopción usan los CLIs reales.

Puente `python3 scripts/test_stalker_retitle_bridge.py`: **22/22** (10.617 s),
incluye diez SIGKILL y muerte del coordinador con ambos locks heredados retenidos.
`swift test --package-path native/MediaMenu`: **9/9**, compiló los cambios de UI.
Compileall y `git diff --check` correctos; 152 enlaces locales comprobados sin faltantes.
Los mensajes Installed de la suite son fixtures del instalador en tmp.

**Implementado/probado, sin nueva instalación ni aceptación física.** Publicación
explícita disponible por CLI/RPC; no se añadió diálogo nativo de publicación.
El usuario planea/confirma la adopción en Clipper CLI/TUI; published no es ACK
editorial y un fallo no libera las copias. Nuevo archivo/revisión queda en el
volumen elegido, con trabajo pendiente; no hay staging interno ni limpieza de
copias ambiguas. Consumidor antiguo sin `--handoff` falla reteniendo el origen.
Exclusión limitada a apps participantes del mismo usuario/host/catálogo; Final
Cut/Atlas, procesos antiguos y hardware siguen fuera. No se ejecutó ASR remoto,
export largo ni captura/retiro personal. Siguiente gate: builds efectivas
reiniciadas y ciclo físico pequeño autorizado, según [PLAN](../PLAN.md).
[Contrato](contracts/media-library-v1.md#cierre-productor-clitui--29-octubre-2026),
[índice/adopción](contracts/README.md#cierre-contractual-vigente--2026-10-29).

## Instalación de recuperación pending · 2026-10-28

A petición del usuario se compiló e instaló la build local `4b0c31b7e4a5` en
`~/Applications/nuun.app`, con el despacho de recuperación pending. Builder e
instalador verificaron firma ad-hoc; backend media/manager instalado comparado
byte a byte contra el repo. Artefacto `/private/tmp/nuun-pending-recovery-20261028.app`.
Anterior preservada en `~/Library/Application Support/n-creator-utils/app-backups/3a73d024.app`.
No se abrió la app ni se ejecutaron operaciones de media. Runtime externo
`/usr/bin/python3`, sin notarización; aceptación física sigue pendiente.
Esta instalación sustituye el límite de build anterior de la entrada siguiente.

## Recuperación Stalker pending · 2026-10-28 · implementada, sin reinstalar

Se cerró el pedido del índice común tras la adopción del productor Stalker.
`media/retitle.py` valida el resultado journalizado pending (IDs, proyecto,
SHA-256 anterior/nuevo y destino seguro en el mismo parent), revalida archivo,
planes y dependencias para origen **y destino**, y repite apply con el request
original y ambos FDs heredados, bajo la misma exclusión. No exige que siga
existiendo la ruta vieja. El resultado de recuperación debe conservar destino
/hash journalizados y pasa la validación final existente antes de publicarse.
Fallos/colas/referencias conservan pending; no se escribe ningún manifest desde
nüün ni se adopta automáticamente otra revisión en Clipper.

Evidencia: **177/177 Python** (30.997 s); tras la comprobación adicional de
coincidencia con el journal, **19/19 contratos** (4.932 s) y **22/22 puente real**
(13.992 s). Incluye diez SIGKILL y muerte del coordinador con locks retenidos,
recuperados por reintento normal (sin apply manual), bytes de media/eventos
comparados por la suite del productor. Puente repetible añadido en
`scripts/test_stalker_retitle_bridge.py`: carga sólo lectura la suite del repo
Stalker y reemplaza su recuperación manual histórica por el despacho nüün.
Catálogos, dependencias/colas y sesiones exclusivamente temporales.
Compileall y diff check correctos. Schema actualizado para exigir campos
journalizados en pending; contratos y PLAN actualizados.

No se modificó el repo Stalker ni Clipper, ni capturas/catalogo personales.
Sin commits, compilación/instalación/apertura de apps: la instalación previa
`b0c413211b4d` todavía no incluye este ajuste. Los mensajes Installed de la suite
corresponden sólo a fixtures de instalador en tmp. Adopción física, builds
reiniciadas y consumidores externos siguen siendo gates separados.
[Protocolo vigente](contracts/stalker-retitle-v1.md#recuperación-pending-adoptada-por-nüün--2026-10-28).

## Instalación solicitada · 2026-10-28

Build release local `b0c413211b4d`, compilada desde el working tree actual,
firmada ad-hoc y verificada por builder/instalador. Instalada a petición del
usuario en `~/Applications/nuun.app`, sin abrirla; backend Python instalado
comparado byte a byte con el repo. Artefacto `/private/tmp/nuun-latest-20261028.app`;
anterior conservada en `~/Library/Application Support/n-creator-utils/app-backups/55eaeaec.app`.
Runtime externo `/usr/bin/python3`, sin notarización ni nueva aceptación física.
Los límites de adopción Stalker/Clipper del cierre siguiente permanecen vigentes.

## Cierre productor Stalker/Clipper · 2026-10-28 · fixtures, sin instalación

Implementación en working tree sobre HEAD `3360d1d`. Se preservaron `.gitlab-ci.yml`
y `.vscode/` preexistentes sin editar; no commits/restage/reset. No se escribieron
repos/proyectos Clipper ni manifests Stalker externos, no se usaron SD, MEDIA,
ESSD o capturas personales. Nombres MEDIA/ESSD en tests son identidades fixture.

- `capture_time` aditivo en sources/handoff congela evidencia original y origen,
  instante absoluto sólo con offset, zona IANA/offset cuando existen. Prioridad
  Stalker epochWallClock → createdAt, Sony/ffprobe con procedencia. mtime/nombre
  legacy no inventan UTC; catálogos sin evidencia siguen legibles con null.
- `editable_members`, `media_bytes`, total `bytes`, hash crudo de manifest,
  revision/content_revision explícitos y mapping opaco `clipper_profile` (sin
  heurística, guardado por RPC de perfil, omisión de cliente antiguo lo conserva).
  Listar usa recibos/stat; no abre manifests ni hashea media para la lista.
- Handoff/schema/ejemplo real sintético: identidad, selección/orden y miembros
  por volumen con sus paths/hashes/recibos. No exige nombres iguales entre
  réplicas; mantiene documentos v1 anteriores. No es prueba de presencia actual.
- `retitle_capture`, `retitle_operations`, `cancel_plan` implementados en servicio
  y CLI. Archivo verificado de cualquier revisión, jobs/planes revisados activos,
  planes de release sin completar, referencias/colas/offline/desconocidos bloquean.
  Exclusión existente Clipper + writer de catálogo durante checks/mutación;
  ambos FDs heredados mantienen locks en el productor si muere nüün. Intent
  durable pending, recuperación mismo operation_id por status, validación CAS/ACK,
  publicación atómica de resultado + invalidación de planes anteriores. Nüün no
  escribe manifests ni adopta la revisión nueva en Clipper/catálogo de assets.

Evidencia: **174/174 Python** (`python3 -m unittest discover -s tests -q`,
27.048 s, pase final). Incluye **16 contratos** y **44 de liberación**, también
ejecutados de forma dirigida durante el cierre. Hay subprocess productor sintético
real, muerte de coordinador mientras el hijo retiene ambos locks, recuperación
sin repetir apply, muerte de propietario del writer, referencias/cola/offline,
planes cancelados/antiguos, CAS/ACK inconsistente, idempotencia/conflicto,
Stalker 1/2/3 y rechazo major desconocido, schema y handoff v1 antiguo. JSON Schema
con herramienta ya instalada, sin dependencia runtime nueva. El ejecutable fixture
sólo escribe su raíz temporal y no sustituye el journal/fsync de Stalker.
Los mensajes “Installed” de la suite son tests de instalador **simulados en tmp**;
no se construyó, instaló ni abrió una app real. Sin cambios Swift; no se ejecutó
captura, export, relink o limpieza personal. Compileall y `git diff --check` correctos; 119 enlaces locales revisados sin destinos faltantes.

**Límites / siguiente dueño:** Stalker todavía debe implementar/anunciar
`stalker-coordinated-retitle/1` (capabilities/status/apply, CAS propio, journal
manifest/rename/library y FDs heredados). Sin capability el coordinador bloquea
con guía; `archived:false` no autoriza. Clipper tiene que adoptar campos nuevos;
handoff portable aún sin consumidor. Exclusión de usuario/host/catálogo y apps
participantes; no cubre procesos viejos, otro catálogo/host ni Final Cut/Atlas.
No hay validación física ni nueva build instalada ni declaración de release.
[Contrato productor](contracts/media-library-v1.md#productor-aditivo-de-biblioteca--28-octubre-2026),
[handoff Stalker/API/schema/fixtures/aceptación](contracts/stalker-retitle-v1.md),
[siguientes dos sesiones](contracts/next-sessions.md#prioridad-vigente--2026-10-28).

## Contrato con Clipper · 2026-10-28 · backend instalado; aceptación física pendiente

`media/clipper.py` y `release.py` coordinan relink/retiro con Clipper mediante
`media-release-lock/1`: nüün posee el flock exclusivo durante comprobaciones y
mutación; token de operación autoriza sólo sus subprocesses. CLI anterior sin
capability y trabajo activo bloquean conservando origen. Cierre/crash libera
el lock del kernel. Requiere actualizar/reiniciar consumidores; otros hosts,
Final Cut y retitle de Stalker no participan. No cambia planner, hashes,
política de limpieza ni los gates de corrupción/SD documentados abajo.

Evidencia: 44/44 pruebas de `test_media_release.py` (20.294 s), con guard
inyectado en fixtures del motor; pruebas de protocolo real en Clipper dentro
de 146/146 dirigidas (41.69 s), incluyendo deps → relink por CLI → ACK → deps,
versión anterior, exclusión y recuperación después de kill. Todo con carpetas
sintéticas/catálogo aislado; sin copias ni limpieza de media personal.

Build release **9a2c9764c7eb**, `/private/tmp/nuun-contract-20261028.app`, firmado
ad-hoc y verificado, instalado en `~/Applications/nuun.app` sin abrirlo.
Anterior: `~/Library/Application Support/n-creator-utils/app-backups/fc442411.app`.
Backend de `media/clipper.py`/`media/release.py` coincide byte a byte entre repo,
bundle de revisión e instalación. Runtime externo `/usr/bin/python3`, sin
notarización. Sin cambios Swift, sin aceptación GUI/física ni declaración de
production-ready. [Contrato](contracts/media-library-v1.md#exclusión-local-de-liberación--28-octubre-2026),
[entrega Clipper](../../../ai/clipper/docs/2026-10-28-contract-hardening.md).

## Incidente operativo Untitled · 2026-10-27 · histórico; reutilización reportada 2026-11-06

**Actualización:** el usuario reporta formatos y uso normal posteriores. El diagnóstico y propuesta de preservación siguientes describen el 27 octubre; no son la siguiente operación vigente. Ver handoff del 6 noviembre al inicio de este archivo.

Solicitud explícita del usuario: tarjeta antes legible ahora inaccesible. macOS detecta SD USB removible de 127,865,454,592 B, partición ExFAT `disk4s1`, UUID `C8BF5026-0077-3F03-AA08-019E0A1DC444`, sin MountPoint. `/Volumes/Untitled` no existe. El tamaño/espacio de volumen reportado como cero mientras está desmontada no demuestra que esté vacía. No se pudo listar su contenido ni confirmar borrado/formateo.

Logs locales: Finder solicitó expulsión a las 03:39:34 (America/Hermosillo); tras reconectar a las 03:40:16, FSKit ExFAT devolvió `Code=201`, seguido de fallo de montaje `0x00001001`. Las reconexiones/intentos de montaje registran además fases automáticas `repaired disk`; ese texto no prueba qué bytes se modificaron. Se intentó una vez `diskutil mount readOnly disk4s1` y falló. No se lanzó reparación ni formateo explícitos. `fsck_exfat -n /dev/rdisk4s1` fue rechazado por permisos y `sudo -n` exige contraseña; el diagnóstico no pudo ejecutarse. No hay fsck activo en segundo plano. Se pidió al usuario ejecutar el diagnóstico `sudo ... -n` directamente en Terminal, sin compartir su contraseña y reidentificando primero si reconecta.

Catálogo consultado con SQLite `mode=ro`: no hay job reciente de Untitled/terra-6-1. El único ingreso registrado de esa raíz es `0de68317-6a1d-573b-944a-0720db1dc1d5`, del 25, proyecto `debloateando-mi-tele`, 30 archivos, `cleaned_files=0`, acción/formato en cámara confirmado históricamente. Sus 30 copias de MEDIA502 siguen presentes con tamaño de recibo (sólo stat, sin rehash); de siete copias de trabajo, seis coinciden y una falta/cambió. Esto no confirma respaldo del material reciente de la tarjeta. El rechazo `Plan digest mismatch` ocurre antes de registrar job/copiar/limpiar.

El usuario ejecutó `sudo /sbin/fsck_exfat -n /dev/rdisk4s1` y compartió la salida: main boot region inválida (se pudo consultar la alternativa), sin upper-case translation table ni main allocation bitmap, entradas de directorio raíz inesperadas/fuera de fin, y volumen declarado corrupto. Todas las propuestas Extend/Delete/Replace/Create/Update se respondieron NO. Esto confirma corrupción de estructuras, no demuestra borrado de los archivos ni establece la causa. Se revalidó el mismo UUID y estado desmontado. MEDIA502 dispone de 2,691,439,788,032 B libres; ESSD sólo 23,113,080,832 B y no cabe una imagen completa de 127,865,454,592 B. Se ofreció preservar imagen en una carpeta nueva de MEDIA502 y revisar desde copia, sin reparar la SD. Esa copia aún no está autorizada/creada. Siguiente paso: confirmar el resguardo y ejecutarlo con acceso de lectura al dispositivo antes de considerar reparaciones. No usar esta tarjeta como fixture ni ejecutar plan/cleanup para probar la app. Estado: corrupción ExFAT confirmada, causa exacta y contenido recuperable todavía sin confirmar.

## Corrección de digest y overflow en Ingreso · 2026-10-27

La captura reportada combina `Plan digest mismatch` al ejecutar y un pie cortado cuando el plan/error hacen crecer compacto. El bridge anterior reconstruía el objeto JSON con Codable antes de devolverlo a Python; se reprodujo en fixtures un mismatch por representación numérica (`1.0` convertido a `1`). El JSON original del plan real fallido no se conservó para comparar sus campos, por lo que no se atribuye a un campo concreto de esa tarjeta. `plan_document` devuelve el documento canónico como string; la app conserva ese string y decodifica una copia sólo para presentación. Apply lo recibe intacto y mantiene todas las validaciones de digest/plan/preflight. El JSON general conserva además UInt64 para identidades. Un documento/digest alterado sigue bloqueado antes de crear job o copiar. Ante `plan_changed`, la UI descarta la revisión y pide otra.

Compacto y selección usan scroll para encabezado/opciones/revisión/error y mantienen el pie fuera. El texto del error crece y se puede seleccionar; al aparecer una revisión/error se desplaza hacia ese contenido. QA visual aislada reprodujo copia de todo el lote + trabajo diferido + plan + un error sintético de más de mil caracteres: texto y rutas accesibles, scroll funcional y pie/botón completos dentro de 440×700. El error se inyectó antes de transferir, sólo en el backend QA; no se ejecutó el plan real de la tarjeta.

Evidencia: **158/158 Python** (44.380 s), incluidos cuatro tests de documento/apply, y **9/9 Swift**. La regresión Swift usa proceso Python real con catálogo/volúmenes fixture: transporte reconstruido falla, no crea job, documento original verifica dos copias y conserva origen, modificación posterior vuelve a fallar. Se cubren UInt64 máximo y representación float integral. `git diff --check` y compileall. Build **`4e3c1fd0214b`**, [nuun-plan-document-20261027.app](../dist/nuun-plan-document-20261027.app), instalado en `~/Applications/nuun.app`. Bundle anterior en `~/Library/Application Support/n-creator-utils/app-backups/597f6193.app`. Firma ad-hoc, runtime externo `/usr/bin/python3`, sin notarizar. Se verificó la firma y que el backend/binario instalado coinciden con el artefacto; no se abrió la instalación principal ni se ejecutó el plan del usuario. Reintentar desde **Revisar plan** en el build actualizado; no reutilizar la revisión del proceso viejo. Aceptación del caso real pendiente.

## Ingreso, expulsión y barra nativa · 2026-10-27

Implementado el selector de SDs removibles/fuentes Stalker conocidas, con carpeta manual detrás de «Otra…». Completo agrupa fuente/expulsión a la izquierda, perfil/país/ciudad al centro y archivo/SSD a la derecha en un renglón; compacto/selección usan dos. Tres proyectos recientes son badges directos, con «Más recientes» y texto detrás de «Otro…». Cambiar el lote conserva proyectos individuales; Stalker requiere inspección antes de planear. Tipo de fuente, copia y limpieza siguen debajo y «Revisar plan» sigue como primaria. Elegir fuente/cambiar modo no escanea: inspección, elegir archivos y preparar lote son solicitudes explícitas.

`source_cards`/`eject_card` en el servicio compartido agregan expulsión normal y explícita, nunca automática. Revalidan identidad/montaje, rechazan archivo/SSD e imágenes y comparan stores actuales tras remount. Comparten el bloqueo interproceso de copia/verificación/limpieza; registro de destino también lo respeta. La UI bloquea expulsión durante operación/inspección. No se llama diskutil desde Swift ni se añade expulsión a transfer. El bloqueo cubre el mismo catálogo, no otras apps ni catálogos independientes. [Semántica local](contracts/media-library-v1.md#fuentes-y-expulsión-local-2026-10-27).

Pestañas, nombre/icono y modo pasan al renglón nativo con los controles del sistema. Se recompone el accesorio al terminar el resize animado para corregir clipping al reducir; en fullscreen se fija la misma barra sobre el contenido mientras el accesorio nativo está oculto. El pie conserva el estado final visible. Previews 16:9 reparten el ancho disponible, overlays de perfil tienen contraste explícito y la caché distingue centro/hover y reutiliza resultados de peticiones en espera. No hay benchmark que cuantifique mejora de velocidad de copia o de tarjetas grandes.

Validación: **154/154 Python** (`python3 -m unittest discover -s tests -v`, 42.256 s, incluidos **11 casos de expulsión**), **7/7 Swift** tras el último ajuste de titlebar, `compileall` y `git diff --check`. QA visual con app de identidad separada y backend limitado a `/private/tmp/nuun-ingest-qa`: completo/compacto/selección, tres badges, entrada manual, cuatro screenshots generados, cambio de proyecto de lote que conserva una unidad propia, plan con ambos proyectos, copia/verificación sin limpieza y expulsión simulada posterior. Elegir SD no produjo discovery antes de pedir inspección. Fullscreen conserva navegación y el regreso a compacto conserva barra/modelo. No se usaron contenidos de SDs personales, MEDIA o ESSD como fixtures; la expulsión física no fue ejecutada.

Build **`d272b7e2cb92`**, artefacto [nuun-ingreso-20261027-validated.app](../dist/nuun-ingreso-20261027-validated.app), instalado en `~/Applications/nuun.app`. Versión previa preservada en `~/Library/Application Support/n-creator-utils/app-backups/da85f69e.app`. Firma ad-hoc verificada, runtime externo `/usr/bin/python3`, sin notarizar. La instalación principal no se abrió tras reemplazarla; la revisión visual se hizo en QA aislada. **111 enlaces locales** revisados sin destinos faltantes. La aceptación física de remount/popup/expulsión y el ciclo pequeño Stalker siguen pendientes, junto con benchmarks grandes, pantallas/Spaces/accesibilidad y gates de release. Este build es de desarrollo; no demuestra preparación para producción.

## Stage progress and Stalker residue · code in working tree · 2026-10-27

Transfers now report verified-file counts per destination (MEDIA/ESSD), verified bytes across all destinations, and copy bytes counted once per source file. Stalker release reports its current stage, session, file, and bytes read from that file: source hashing, archive matching, bundle validation, Clipper/dependencies, work-copy verification, revalidation, catalog snapshots, and journaled release. Stage changes persist immediately; updates within a stage are limited to about one per second. The UI uses actual counts/bytes rather than an estimated percentage.

The job shown in the screenshot (`22ee38a6-5622-502c-b52e-0bcdcecf02e5`) finished `completed`: 14/14 verified copies on each of MEDIA502 and ESSD, two sessions released, and two local release plans completed with 7/7 files each. The `manifest.json` error came from the post-operation refresh: the source retained one `.stalker` folder with only `.DS_Store` (6148 B), which release intentionally ignores, so the directory itself stayed. Discovery now ignores a manifest-less Stalker folder only when every entry is Finder metadata or an empty directory; missing-manifest folders containing any other file still fail. No metadata was manually removed.

Validation: **65/65** `test_media_engine.py`, **44/44** `test_media_release.py`, **7/7** Swift tests, `compileall`, `git diff --check`, and bundle signature verification. Build **`8dd235483514`** at [Nu9veMedia-local-20261027-progress.app](../dist/Nu9veMedia-local-20261027-progress.app) uses external `/usr/bin/python3`, is ad-hoc signed, and is not notarized. Installed to `~/Applications/nuun.app`; prior app preserved at `~/Library/Application Support/n-creator-utils/app-backups/aea5e3e3.app`. No media contents were opened and the app was not launched or visually checked. Verify the real UI refresh/progress on next use. Investigation of generic Stalker folder naming and large microphone tracks is recorded for the next Stalker session.

## Stalker batch + compact window · 2026-10-27 · installed; visual acceptance pending

Stalker planning now allows an empty batch project when each session already has a project; the batch value fills only sessions without one, and an incomplete batch gets assignment guidance. The pre-plan and reviewed-plan summaries list the projects per session. Stalker preview cards overlay date/duration/tracks and profile on the midpoint video thumbnail; an assigned project is a compact label with a pencil, while quick project choices appear only for unassigned/editing sessions. Editing a project/profile/country/camera/copy selection invalidates any reviewed plan. The full window moves centered navigation into a 50 pt native titlebar accessory, removes the repeated logo/header row, and places the mode button at the far right. Compact/selection uses the same top row for “Ingreso” and expand.

Evidence: **43/43 release/Stalker fixtures**, **64/64 media-engine fixtures**, **7/7 Swift tests**, release bundle build/signature verification, and `git diff --check` passed. Build **`e9e84e130b5c`** (`dist/Nu9veMedia-local-20261027-0207-stalker-batch.app`) is installed at `~/Applications/nuun.app`; the installer preserved the previous bundle at `~/Library/Application Support/n-creator-utils/app-backups/9b8aefe7.app`. The app was not opened or visually inspected, and no real media or volumes were accessed. Check visual spacing/titlebar accessory behavior and the reported two-session workflow before treating the UX as accepted. This is not a production/release-ready build.

## UI follow-up · 2026-10-27 · code only

Added an AVFoundation video thumbnail path that samples the midpoint, then samples one of six coarse positions when hovering; image thumbnails remain Quick Look and thumbnail concurrency/cache limits remain bounded. Stalker discovery now exposes the manifest's safe screen-video path, duration and local timestamp to the Inbox and ingest grid. Inbox rows show video preview/date/duration/track count, three recent project buttons are directly selectable, the library-level action prepares every pending session under that root in one reviewed plan, and the app has fixed contextual footers for Ingreso, Biblioteca, Actividad, Bandeja and Espacio. The compact/selection importer also pins its plan action in a footer. The mount-popup preference now defaults to enabled for new users; a saved user opt-out still wins.

Checks at that point: **3/3 Stalker engine fixtures**, **9/9 capture workflow fixtures**, and **7/7 Swift tests/build** passed; `git diff --check` passed. The app was not launched or visually inspected, and no personal volumes/media were read, copied or removed. Visual acceptance of video thumbnails/hover and footers, and a real remount-popup check, remain pending. A batch is one library root per plan; separate Stalker roots still require separate plans.

## Stalker backup + local release · 2026-10-27 · installed; visual acceptance pending

`Ejecutar plan revisado` now owns the full Stalker flow: write/verify required MEDIA and optional ESSD copies, inspect Clipper, relink through its public CLI and validate ACK when needed, then run the catalog-recorded release journal for each local `.stalker` bundle. `Ajustes → Capturas → Conservar el original Stalker después del respaldo` defaults off; plans freeze this choice, and turning it on preserves originals for manual release later. Unknown dependencies, queued exports, unavailable Clipper or failed revalidation retain the source and report the reason. An interrupted per-session release resumes from the same recorded plan. This is a journaled workflow, not an ACID transaction across volumes, the catalog and Clipper. Current follow-up checks and installation evidence are recorded in the newer entry above.

Evidence: **40/40 release/Clipper fixtures**, **64/64 media-engine fixtures**, **7/7 Swift tests**, `compileall` and `git diff --check` passed. The local development bundle `dist/Nu9veMedia-local-20261027-013555-3f10d1.app`, build ID **`ed53ad7dc3da`**, is installed at `~/Applications/nuun.app`. The installer preserved the previous build at `~/Library/Application Support/n-creator-utils/app-backups/3b71b831.app`. The app was not opened or visually inspected; no real volumes or media were accessed. This is not a production/release-ready build.

## Stalker / espacio · implementación 2026-10-26

Bandeja descubre `stalker-library/1` actual + anteriores, muestra sesiones sin proyecto y guarda etiquetas por UUID sin reescribir manifests. Sugerencia de proyecto/país versión 1 confirmable y asignación por unidad. Espacio agrega auditoría con rehash actual, planes registrados de liberación por carpeta de raws/original Stalker, revisión de Clipper vía CLI/ACK, preferencia de ESSD verificado para reenlace local y bloqueo por colas/proyectos offline/antiguos no adoptados. Backup completo SQLite/perfiles/tags/decisiones a MEDIA, verificado, explícito y antes/después del retiro; también tras ingreso por `apply`.

[Uso y límites](operations.md#bandeja-stalker-proyectos-y-liberar-espacio). **Operación solicitada 2026-10-26:** cinco bundles Stalker completos de `~/Movies/Stalker`, asignados a `nuun-creator-utils` (1), `terra-6-1` (2) y `soundshop-tests` (2), liberados mediante cinco planes registrados después de hash actual en origen/MEDIA/ESSD. Total: **31 archivos, 1,593,007,238 bytes**. Los cinco terminaron `completed`; 7/7 o 3/3 copias por bundle permanecen en MEDIA502 y ESSD, los cinco originales ya no existen y los snapshots previos/finales del catálogo se guardaron sin errores. Clipper no reportó dependencias, colas ni relinks para esas fuentes; no había procesos abiertos de Clipper/nüün. La operación se lanzó por el mismo servicio local, sin abrir la UI.

La eliminación posterior al backup sigue siendo una prioridad de producto, no un comportamiento automático: hoy la oferta/revisión y confirmación de liberar es un segundo paso. No se abrió la app ni se hicieron pruebas visuales en este cierre. No significa aceptación física ni release de producción. La fila de Clipper del contrato documenta CLI/TUI ya implementados; aún no hay lease compartido ni garantía para proyectos/consumidores fuera de registros conocidos. Los proyectos antiguos no adoptados bloquean el retiro y requieren resolver su enlace en Clipper. No se archivan/retiran proyectos editoriales ni renders desde el nuevo plan de raws.

Evidencia final de este cambio: **134/134 Python** (`python3 -m unittest discover -s tests -v`, 37.848 s; 36 pruebas nuevas de este flujo), **7/7 Swift** y build release/firma ad-hoc verificados. `compileall`, `git diff --check` y 115 enlaces locales revisados. Artefacto `dist/nuun-stalker-space-20261026.app`, build **`affd140113f9`**, instalado en `~/Applications/nuun.app` después de que el usuario cerró la instancia anterior; no se abrió después. [Cierre y aceptación pendiente](releases/2026-10-26-stalker-space.md).

## Implemented

| Capability | Owner | Current limit |
| --- | --- | --- |
| Safe `save`, explicit legacy command, project/profile/country/city | [save](../media/save.py), [entry](../nu9ve.py) | Flags plus `save SOURCE COUNTRY PROJECT` shorthand; `--no-work` explicitly archives with work skipped. Historical engine remains `legacy-save`. Named layout restores historical names/folders (Sony XML duration/date verified on a real ILCE-6400 card; H6/DJI/GoPro naming only fixture-level). |
| Stable plan/digest, paired-file assignment, actual destination preview, required disks and space preflight | [planner](../media/planner.py) | Simple stem/sidecar pairing; DJI category explicit; date routing uses file mtime; no full EXIF/device registry. |
| Volume identity + physical-store overlap checks | [volumes](../media/volumes.py) | macOS diskutil/df; fixtures simulate devices. Exact cloned filesystem identity remains a limitation; other OS backends are incomplete. |
| Copy in bounded blocks, destination read-back SHA-256, per-file reattempts, mutation lock and journal | [transfer](../media/transfer.py), [IO](../media/common.py) | `F_FULLFSYNC` for files/dirs/receipts and SQLite `fullfsync`; `F_NOCACHE` on copy/verify reads is best effort (already-cached pages can still answer). No multi-GB hardware benchmark yet; F_FULLFSYNC support on ExFAT not measured. No rollback promise. |
| Cleanup after all required receipts; interruption recovery | [transfer](../media/transfer.py) | Each source file is re-read and must match the verified hash right before unlink. Finder metadata (`.DS_Store`, `._*`, `.TemporaryItems`) is ignored, not deleted. Material appearing during cleanup ends `source_has_new_material` (terminal). Stalker retains source; Sony media-only cleanup leaves camera indexes intact and remains fixture-tested (physical acceptance pending). |
| Explicit archive-only + pending/skipped work | [planner](../media/planner.py), [library](../media/library.py) | Registered expected work target required for pending. Later delivery is explicitly selected, not a silent bulk copy. |
| Creative profiles/workspace folders | [workspaces](../media/workspaces.py), [layout](../media/layout.py), [settings](../native/MediaMenu/Sources/MediaMenu/SettingsView.swift) | One work disk per job; profile disks are suggestions. Registry itself still needs metadata backup; plan routes are frozen. |
| SQLite catalog, stable project IDs/rename aliases, asset/project associations | [catalog](../media/catalog.py) | Shared-asset resolution still primarily uses the initial job. Broader relink/location policy needs review. |
| Restore, work delivery, handoff export and archive receipt recovery | [library](../media/library.py), [recovery](../media/recovery.py) | Recovery restores receipt-time metadata, not every later local decision. Recovered work-location/ownership reconciliation needs more coverage. |
| Per-variant/channel publication metadata | [publication](../media/publication.py) | Explicit registration only; does not upload or automatically archive `subidos`. |
| Completed Stalker discovery and incremental index | [captures](../media/captures.py), [planner](../media/planner.py) | Completed manifest revisions tracked; no producer lock/lease or cross-app relink ACK. |
| nüün menubar, unified compact/selection/full window, automatic discovery, inbox, assignments, plan review, library and sectioned settings | [app](../native/MediaMenu/Sources/MediaMenu/Application.swift) | Main flows implemented; final physical/UX/accessibility/performance acceptance pending. |
| Native mount/wake events and configured Stalker FSEvents | [app](../native/MediaMenu/Sources/MediaMenu/Application.swift), [watcher](../native/MediaMenu/Sources/MediaMenuCore/CaptureWatcher.swift) | Initial implementation; sustained real capture/idle tests and event-loss scenarios remain. No periodic idle scan. |
| Shared one-request JSON service; read-only MCP stdio | [service](../media/service.py), [MCP](../media/mcp.py) | MCP has no destructive tools or HTTP server. No installed external client integration yet. |
| App packaging helper | [builder](../scripts/build_media_app.py) | Development external runtime or supplied relocatable runtime; no provisioned release runtime/Developer ID/notarization. |

## Validation evidence

- 2026-10-26, nüün unified-window closeout: final artifact `dist/nuun-final-20261026.app`, installed at `~/Applications/nuun.app`, build **`2386647728a2`**, closed after installation (not reopened/tested visually after the user requested stopping visual tests). One AppKit window now renders compact/selection/full through shared WindowRoot/Model, with animated size transitions, Reduce Motion support, retained mode on reopen, manually controlled host sizing and native fullscreen handling. Auto discovery has an independent cancellable reader, generation IDs to discard obsolete responses and reuse by source/type/archive; lazy thumbnails remain bounded. Flat previews expose per-unit project/recent project/profile and camera classification; source counts/camera evidence/root routing summary precede the authoritative plan. Source kind and main navigation use icon buttons. Inbox and settings sections (profiles/disks/appearance/captures/diagnostics) added. Profiles are local config with relative raw folders and normalized volume IDs; routes are frozen into plans/receipts. Stalker-only discovery/planning/source validation now skip unrelated folders in a shared library, exclude active/already archived sessions consistently and expose manifest metadata. **98/98 Python tests passed** after final backend edits (including profile path/role validation, frozen routes, Stalker metadata/archived filtering/unrelated editor folder and installer migration). **7/7 Swift tests passed** after final read-generation changes; final release build, ad-hoc signature verification and installer passed. Before the stop request, UI build `1b1ddd17ea61` verified auto load of 104 JPG + Canon support file, same-window compact→selection→full, marking one item, review of a 105-file mixed plan without applying, preserved plan/selection on expansion, and profile/disk settings. Marks/plan were discarded; final pre-close state remained only archive. No live media copied/deleted. Search uses ASCII filename plus registered keywords; Spotlight metadata returned null and automated Spotlight UI was unavailable, so actual nuun/nun matching/ranking is pending. Final Stalker-only filter and fast-cancellation fixes were technically checked, not visually re-tested, as requested. [User acceptance and artifact record](releases/2026-10-26-local.md).
- 2026-10-26, coherent surface theme (UI-only): installed build **`bb3684deb158`**, release build + ad-hoc codesign verification passed. New [Appearance.swift](../native/MediaMenu/Sources/MediaMenu/Appearance.swift) owns shared seed-derived canvas/cards/fields/borders/buttons, native window background/appearance and transparent titlebar. Fresh defaults are Light + **#FFBB5C**; existing custom settings are preserved until explicit reset. The reset preset was applied for this user as requested. Selected-action text chooses black/white by luminance; native interactive tint is darker in light mode for legibility. Compact importer is now 408 pt wide, with vertical choices, wrapped explanations and explicit accessible selection values. Visual review on the final installed build covered light/dark surfaces and native chrome; returned to light and left the vertical importer open for EOS_DIGITAL / `kasakarla-electroritual`, none/only archive, cleanup enabled. No plan applied or media copied/deleted. No Python/transfer changes or backend-suite rerun; verification was compile/sign/install plus native UI review. System-theme switching and full accessibility/contrast matrix remain outside this check.
- 2026-10-26, native identity/grid/quick importer: installed **n creator utils.app**, build **`1cc34b5f090f`** at `~/Applications/`, with code-generated ICNS and LaunchServices registration; old Nu9ve Media install moved to app-backups. Normal app menus/Dock while windows are visible; accessory after all close. `Application.swift` now owns model/menu/events, `ImportViews.swift` owns grid, Quick Look thumbnails, appearance and compact importer. Three batch choices: none/all/selected editing copies, per-unit project entry, advanced profile/camera, cached thumbnails (4 active, 24 MB/160 entries), persisted system/light/dark + HSV accent, opt-in mount popup and manual demo/⌘I. **93/93 Python tests passed** on the final backend (including selected-only SSD copies, full archive+cleanup, later explicit delivery/handoff, partial deferred work, all-deselected confirmation and renamed installer migration). **7/7 Swift tests passed** before final copy-only label changes; final release build and ad-hoc signature checks passed after those labels. UI review on preceding build `828fb2613b65`: actual EOS_DIGITAL thumbnail grid, native menus, settings, and compact importer for the same project; no copies/cleanup. Final installed build opened and retains appearance preferences; its EOS_DIGITAL grid was loaded again for `kasakarla-electroritual`, left at none/only archive with cleanup enabled and no plan applied. Physical reconnect popup, selection performance on huge cards and broader accessibility/theme contrast acceptance remain untested; manual popup is not evidence of a mount event. No release-ready claim, no bundled Python runtime.
- 2026-10-26, native archive-only + local install (uncommitted): explicit “Sólo respaldo” works even with ESSD online and omits both work target and pending delivery. Plan invalidates when that choice changes; summary shows work status, bytes and cleanup. **7/7 Swift tests passed**, including two destination-policy tests and isolated Python RPC. **3/3 installer fixture tests passed** (preserved previous bundle, running-app refusal, rollback on swap failure). Release-mode Swift build, ad-hoc codesign verification and installation/LaunchServices registration succeeded at `~/Applications/Nu9ve Media.app`, build **`c4820e569776`**. Python is external `/usr/bin/python3`; backend source is bundled and resolved relative to installed Resources, overriding old runtime preferences. Launcher opens/reopens the window; `--background` keeps menubar only. UI review on the installed build, normal catalog: EOS_DIGITAL → MEDIA502, project `kasakarla-electroritual`, 105 files / 211.2 MB, work skipped, cleanup enabled, paths expanded. Left at **Ejecutar plan revisado**, never clicked; no media copied/deleted. Canon cleanup remains untested physically. Future code changes require rerunning [installer](../scripts/install_media_app.py); no automatic updater or portable runtime is claimed.
- 2026-10-26, CLI archive-only usability (uncommitted): restored `save SOURCE COUNTRY PROJECT` shorthand on the verified engine, with explicit `--no-work` (`skipped`, no SSD dependency), per-operation archive confirmation and per-file destination preview. **6/6 focused fixture tests passed** (`python3 -m unittest discover -s tests -p test_media_engine.py -k MediaSaveTests -v`): argument conflicts, separate city, preview without copying, `--yes` cannot authorize archive-only, declined confirmation, verified archive/cleanup with work offline and default SSD preserved. Full suite not re-run for this change. Read-only operational preview using a temporary copy of the catalog: EOS_DIGITAL has 104 JPGs + Canon CTG, 211,248,779 bytes, no recognized videos; MEDIA502 routing passed (images → `images/2026/09`, support → `metadata/generic/support`). No media copied/deleted. Model remains `generic` without metadata/device identification; this preview is not Canon cleanup hardware acceptance.
- 2026-10-25 on `5346663` (before the fixes below): Python 3.9.16 full suite **63/63 OK**; Swift **5/5 OK**.
- 2026-10-25 after review fixes (uncommitted): Python 3.9.16 and 3.12 full suite **68/68 OK** (new: transient bad SD read, Finder metadata, crash before owner marker, disk-image replica, new file during cleanup); Swift build without warnings and **5/5 OK**. Suite time rose from ~2 s to ~15 s because of real `F_FULLFSYNC`.
- Named layout (uncommitted): 71/71 Python tests on 3.9 and 3.12, including a Sony fixture card (dated names from XML, work gets only clips, support tree, handoff) and same-second collisions.
- **First live operational run, 2026-10-25** (user-requested): Sony ILCE-6400 SD (`Untitled`, ExFAT, 30 files, 6.35 GB) plus 3 completed Stalker sessions from `~/Downloads` (1.87 GB) → MEDIA502 (ExFAT) + ESSD (HFS+), project `debloateando-mi-tele`. Results: Sony `camera_action_required` in 829 s (~7.7 MB/s effective over copy+verify+second copy+verify), Stalker `source_retained` in 28/65/101 s. Receipts on both disks, no `.ncpartial` left, independent `shasum` matched SD/ESSD/MEDIA for a sampled clip and `mic.wav`. Nothing deleted: SD still holds its clips (format in camera), sessions remain in Downloads. An earlier attempt with the schema-1 nested layout was stopped by the user's choice; its partial work copy and catalog job were removed.
- 2026-10-25, second pass (uncommitted): Stalker bundles named `YYMONDD-HHMMSS_STALKER_MM-SS_…[].stalker` from the manifest so they interleave with clips; `media relayout JOB_ID` renamed the 3 real sessions in place on MEDIA502 and ESSD (no bytes copied); a fresh `recover-catalog` from MEDIA502 into a temporary catalog re-hashed all 4 jobs (8.2 GB, 70 s) and verified. That check found and fixed a real ExFAT bug: macOS `._*.json` AppleDouble companions broke receipt recovery.
- Copy engine now reads each source once and writes all destinations in parallel threads, verifies them in parallel, and records live progress (`phase`, copied/verified bytes, MB/s) shown by the app. Fixture-tested (75 tests); **speedup not yet measured on hardware**.
- 2026-10-25, third pass (uncommitted): `media coverage` read-only audit (fixtures only, **not yet run on ESSD/MEDIA**); `media captures` CLI. Fixed a real bug: Stalker's uppercase `sessionID` made archived sessions look pending in `captures` and in the app's badge; the 3 real sessions now report `archived: true`. Added `media confirm-cleanup` (plus an app button) to close a Sony job after the camera format; the pre-format index snapshot of the real card is kept for comparison. Python 78/78.
- **First complete card cycle, 2026-10-25:** Sony SD backed up → formatted in camera → `confirm-cleanup` → job `completed`. Pre/post index diff in [sony-validation](sony-validation.md). Found and fixed: a stalled `diskutil`/`df` on a freshly mounted card crashed the CLI instead of returning a JSON error.
- Editor interface (uncommitted): read-only `media projects` and `media sources --project` (ordered sources, local paths, IDs, hashes, `captured_at`/`duration_seconds`) for Clipper quick import; 1.7 s on the real project. Contract index now has an adoption table, the stable local interface and edit rules for other sessions. Python 80/80 (also under TZ=UTC).
- Uncommitted: `sources` checks presence/size and reports hand-deleted work copies (`removed_from_work`) and lost archive copies (`warnings`) instead of failing. Sony profile `sony_clips_then_format` (fixture-tested only; next real card pending). Camera/screen N:M grouping case documented for Clipper's autogroup. Python 80/80.
- Clipper check (read-only, in its repo): `expand_inputs` on the real project folder lists 7 clips + 3 renamed bundles in capture order.
- [CI](../.gitlab-ci.yml) added: Python 3.9/3.12 in Linux containers, Swift as a manual macOS job. **Not yet run on GitLab or Linux** (no local Docker daemon).
- Independent data-loss review (2026-10-25): no logic path deletes a source without same-run verified copies on every required destination. Fixed: drive-cache durability, single-SD-read replicas, Finder metadata blocking jobs, crash before owner marker, disk images counted as independent replicas. Open: see blockers 1.
- Failure fixtures include same-size corruption, missing/replaced/overlapping disks, insufficient space, symlinks, concurrent writer, changed source, partial cleanup and cancel/resume. Three real child-process hard exits exercise recovery during copy, after receipts and immediately after unlink.
- Native: latest recorded build completed; **5 Swift tests passed**, including a real Python RPC call with isolated state and exact 64-bit nanosecond roundtrip. Swift tools 6.3.3, target macOS 14+.
- Native UX fixes (uncommitted, compiled, **not visually re-tested**): a mounted card notifies once and appears as a suggestion instead of opening the window or replacing an in-progress source; the last project is offered ("Usar último") instead of prefilled; labels for Stalker-on-card and `source_has_new_material`.
- UI smoke test (earlier snapshot): a development .app, isolated catalog and two tiny local fixture files. Inspected paired files, showed SSD absent, requested archive-only confirmation, applied verified copy with source retained and displayed work pending. No real SD cleanup. Latest FSEvents/routing refinements were compiled but not re-tested visually.
- Earlier wheel/schema/link checks belonged to the earlier inventory-only snapshot. They are **not** evidence that the latest full package/handoff is release-validated. Rebuild/validate current outputs before release.

## Artifacts and local test context

| Item | Meaning |
| --- | --- |
| `native/MediaMenu/.build/.../debug/MediaMenu` | Most recently compiled native source. Ignored build output. |
| `dist/Nu9veMedia-dev.app`, `dist/Nu9veMedia-review.app` | Ad-hoc development snapshots. Existing outputs are deliberately not overwritten by the builder. Neither is notarized/current-release quality. |
| `/private/tmp/ncreator-media-ui-20261025` | Isolated UI catalog. Never confuse with the normal user catalog. |
| `/private/tmp/ncreator-media-ui-fixture-20261025` | Tiny test source/archive/work folders; no personal media. |

The review app was opened using the isolated `NU9VE_MEDIA_HOME`; its development runtime was pointed at the checkout's `nu9ve.py`. A running app may therefore use newer Python than its bundled source snapshot. Rebuild a fresh artifact for reproducible review; do not distribute this session's test bundle.

## Release blockers / known gaps

1. Remaining review items: plan digest is unauthenticated, so `apply` trusts plan-time checks (DJI review, archive-only confirmation) a caller could skip — record reviewed digests in the catalog or re-run checks; jobs have no owner/lease, so a killed run looks active; `progress.verified_copies` also counts delivery/recovery rows. Fresh package pass and physical SD/ExFAT disconnect trials remain necessary.
2. Sony cleanup acceptance and samples/rules for the actual DJI models; full metadata dates and device persistence are not implemented.
3. Full SQLite snapshots now back up human decisions/profiles; automatic retention and in-app full-state restoration remain open. Secondary-job location resolution, recovery/work ownership reconciliation and active-consumer leases still need hardening. Receipt recovery alone is not a complete backup of human metadata.
4. Clipper CLI/TUI has library sources/relink/deps (see its adoption row); nüün now requests relinks and validates ACKs. Handoff v1 consumption, consumer leases and real cross-app acceptance remain open. Old unadopted projects block release. Atlas/FCP integrations are design only.
5. No automatic archive of published deliveries or complete dashboard/launcher/onboarding. No runtime provisioning, notarization, Gatekeeper signoff or reproducible installer. Python package/native app versioning must be aligned before release.
6. Windows/Linux product support is not validated. Current secure IO/locking and native UI are macOS/Unix-oriented.

Do not use this status to authorize live media operations. Registration and backup runs must use the user's chosen disks and reviewed plans; automated tests stay in fixtures. No changes were made to the other repositories or sent to other chats.


## Coordinación documental con Clipper · 2026-10-29

Índice común alineado con adopción efectiva de handoff y zoom sostenido Stalker
1.4 en Clipper CLI/TUI. Se enlazan delivery de un archivo, adopción explícita
project-only, audio por contribuciones y refinamiento PCM implementados en
Clipper. Se conserva la separación entre adopción del consumidor y publicación
productora de una nueva revisión; retitle con referencias permanece bloqueado.
Deps histórico de una revisión fijada en cola sigue bloqueando retiro mediante
el guard existente, sin modificar backend nüün. Evidencia de la sesión Clipper:
96 pruebas dirigidas de biblioteca/relink/preparación/audio/sync aprobadas;
incluyen delivery con ingest/recibos/handoff reales y adopción con registro de
revisión simulado únicamente en el catálogo fixture. No se ejecutaron nuevas
operaciones físicas ni instalaciones, ni la suite completa del productor.
[Entrega actual](../../../ai/clipper/docs/2026-10-29-contract-coordination.md).
