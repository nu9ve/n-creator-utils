# Adopción por app

Qué parte de cada contrato cumple cada app. Se mudó aquí desde los contratos el 2026-11-16: un contrato cambia cuando cambia la regla, y esta página cuando una app avanza. Así fijar un contrato no deja a una app atrás porque otra registró su adopción.

Cada app actualiza su fila desde su propio repo, con un cambio aquí y su línea en el [CHANGELOG](../CHANGELOG.md). Estas tablas resumen; el estado real está en el repo de cada app y, si difieren, manda el de la app. Las apps no necesitan fijar esta página.

## macos-app-v1

[Apps de macOS de nu9ve v1](../contracts/macos-app-v1.md)

Revisar el código al actualizar; esta tabla no sustituye el estado de cada app.

| Regla | SoundShop 0.27 | nüün 0.2 | Fractos Mac 0.1 |
| --- | --- | --- | --- |
| 1 Barra de título | Sí | Sí | Sí: cada material salvo cristal dibuja su barra y sus controles; arrastre con ratón real sin probar |
| 2 Ventana | Sí | Parcial: la pantalla completa recorre clases privadas | Sí, con desvío declarado: `ShellWindow` en vez de `Window`, por las esquinas cuadradas |
| 3 Temas | Sí | Sí | Sí; contraste probado sólo en los temas por defecto |
| 4 Menús | Sí | Parcial: menú manual sin Servicios, Ocultar otras, Mostrar todo ni Zoom | Parcial: la Ayuda abre la web; interfaz sólo en español; ⇧⌘Q cierra la sesión de la app y es un atajo del sistema |
| 5 Acerca de | Sí | Sí (2026-11-07): ventana propia con tema y skin, versión copiable, enlaces y `NSHumanReadableCopyright` | Sí, con sello de build (`Nu9veBuildID`) |
| 6 Documentos | Sí | No aplica | No aplica |
| 7 Ajustes | Sí | Sí | Sí: tema, paleta, material, logo, sonido y háptica; sin sincronizar con la cuenta |
| 8 Sistema | Sí, sin pasada completa de VoiceOver | Sí, sin pasada completa de VoiceOver | Parcial: sonido y háptica sin escuchar ni probar con trackpad; sin pasada de VoiceOver |
| 9 Bundle y distribución | Parcial: scripts listos, nunca notarizada, sin actualizador | Parcial: sin notarización ni actualizador | Parcial: firma ad hoc, sin Developer ID, notarización ni actualizador; icono de iOS |
| 10 Validación | Sí | Sí | Sí: modo QA con capturas y prueba de ventana; falta registro de pruebas físicas |

La columna de Fractos resume su propia tabla (`ios-fractos`: `docs/MONOREPO.md`, «Adopción del contrato de Mac»), leída el 2026-11-10 junto con su código; nadie vio la app corriendo desde esta sesión. Si las dos tablas difieren, manda la de su repo.

Stalker, Clipper y otras apps: sin revisar. Una app adopta el contrato añadiéndose aquí con lo que le falta.

## nu9ve-interaction-v1

[Teclado e interacción de las apps de nu9ve v1](../contracts/nu9ve-interaction-v1.md)

Leído en el código el 2026-11-07, sin probar a mano. Revisar al actualizar. La columna de Fractos se releyó el 2026-11-10 en la app de Mac de `ios-fractos` (`f1a7bcb`).

| Regla | curado | SoundShop | Clipper TUI | nüün | kyu | Fractos | nove · neuf · Stalker |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 Catálogo | Sí (`keyboard.ts`) | Sí, con prueba | Sí (`SHORTCUT_SPECS`), con prueba | No: atajos solo en el menú | Sin revisar | No (Mac): atajos en `AppCommands.swift` y sueltos en pantallas | Sin revisar |
| 2 Todo por teclado | Sí en la cabina | Parcial | Sí | Parcial: menús, ⌘1…⌘6, pasos de Ingreso | Parcial: cribado con 0–5, P/X/U, flechas | Parcial (Mac): menús, flechas en Trabajo y Calendario; las secciones van en ⌥⌘1… y ⌘1/⌘2 son filtros de Trabajo, no las vistas de la §8 | Sin revisar |
| 3 Badges y capas | Sí, con capa de Option | Sí en el footer; en los controles sin revisar | Barra de contexto (terminal) | No | Sin revisar | No | Sin revisar |
| 4 Ayuda contextual | Sí (footer) | Sí (barra de estado) | Barra de contexto | No | Sin revisar | No | Sin revisar |
| 5 Panel de atajos | Pestaña teclado | Sí (⌘/) | Sin revisar | No | Sin revisar | No | Sin revisar |
| 6 Remapeo | Sí | Sin revisar | Sin revisar | No | Sin revisar | No | Sin revisar |

Una app adopta el contrato añadiéndose aquí con lo que le falta. Primeros pasos sugeridos:
- **nüün:** catálogo a partir de su menú, badges con capa de modificador, línea de ayuda en el footer del Ingreso y ⌘/. En Windows (maqueta, 2026-11-09) ya hay catálogo con pruebas de duplicados y de teclas del sistema, aceleradores y tooltips generados desde él, `AccessKey` en la navegación y panel Ctrl+/; es la referencia para portar el catálogo a la app de Mac.
- **kyu y neuf:** el recorrido completo por teclado de la §10, porque son las apps de edición más densas.

## nu9ve-engine-v1

[Protocolo de un motor detrás de una app v1](../contracts/nu9ve-engine-v1.md)

Leído en el código el 2026-11-16: nüün `7737e2f`, Clipper `41084ccb`, nove `9935227`. Los casos de motor de [`data/engine-examples-v1.json`](../data/engine-examples-v1.json) se corrieron ese día contra los binarios de nüün y Clipper; nove no, porque su petición todavía no es v1. Qué falta en detalle y en qué orden está en la [§12 del contrato](../contracts/nu9ve-engine-v1.md#12-diferencias-por-app-y-camino-a-v1).

| Regla | nüün (Python y `nuun-engine`) | Clipper (Python y `clipper-core`) | nove |
| --- | --- | --- | --- |
| 1 Transporte | Sí: líneas (`rpc-serve`) y una vez (`rpc`). Los dos motores descartan líneas de más con lectura acotada a 1 MiB. Rust usa el servidor de `nu9ve-engine` | Sí: líneas (`serve`), con `nu9ve-engine` en Rust | Sí: marcos por stdio, socket y pipe; una vez (`call`) |
| 2 Petición e id | Sí: el `id` se lee antes que `method` y sólo vale entero, cadena o `null`. Los dos motores devuelven ids de 350 dígitos exactos en ambos modos (Rust con `raw_value`, sin `arbitrary_precision`). `params` se omite o es un objeto | Sí | No: `request_id`, `protocol` |
| 3 Sobre | Sí en los dos modos. Python conserva `command` en los éxitos de `rpc` | Sí | Paso 1 implementado: los clientes leen el sobre viejo y v1. El motor emite el sobre viejo. |
| 4 `engine_info` | Sí en los dos: `contract`, `protocol` (`nuun-media/1`), `engine`, `version`, `capabilities`, `ported` y `native`, más sus `writes` | Los dos motores incluyen `contract`, `version`, `capabilities` y `queue_views`. Al cliente le falta validar `contract` | No: `hello` |
| 5 `not_ported` | Sí: Rust incluye `reason`, entre ellos `method_unavailable` y `platform_unavailable` | Sí | No aplica |
| 6 Grandes, progreso, escrituras | Progreso por sondeo; escrituras sólo en una vez, sin clave | Sí: `data` ya serializado, `request_id`, `write_status` | Parcial: claves e `idempotency_key` sí; una respuesta de más de 1 MiB se pierde |
| 7 Códigos | Sí: `invalid_request`, `invalid_params`, `unknown_method` antes que `read_only` e `internal_error` (antes `operation_failed`), en Python y Rust a la vez | Sí | Parcial: `invalid_json`, `frame_size` |
| 8 Ciclo de vida | Sí | Sí | Sí: `--parent-pid`, `engine_shutdown` |
| 9 Seguridad | Sí (stdio) | Sí (stdio) | Sí: socket `0600` con uid del par; pipe con DACL y PID |
| 10 Sombra | Sí: compara tipos y el registro guarda sólo rutas, códigos, `reason` y tiempos. Quita `not_ported` de `data` antes de comparar (respuesta parcial) | Sí | No aplica: sin oráculo |
| Casos de motor (§11) | 20 de 20 en ambos motores (`d329092` en `n-creator-utils`). Gate de macOS con Rust 1.99.1, 23 diferenciales y sombra sobre el catálogo del dorado sin diferencias nuevas. Windows queda en CI | 20 de 20 en los dos, en su CI | Sin medir |

**nove (2026-11-16).** Contrato y ejemplos fijados desde `nu9ve/core@9df396d` en `8a7efd6`; lectores de Swift, C#, host del broker y `pipe-call` en `83fd776`. Las pruebas de ambos sobres pasan en Rust y Swift en Mac. Desde `42719f6`, Clippy 1.89 pasa y `.github/workflows/engine.yml` corre el gate de Mac; **Pasos 2 y 3 hechos** (`6ceb214` a `8fb870f`): el motor responde v1 a quien manda `id` y el sobre viejo a quien manda `request_id`, y Swift, el cliente del broker y MCP, `pipe-call` y C# envían `id`. El motor pasa los 20 casos de los ejemplos en modo de marcos. `8fb870f` arregla en un commit aparte las tres pruebas Swift previas (esquema 27, `calendarDefaultZone` opcional) y el formato. El gate de Mac pasa entero con Rust 1.89: fmt, clippy, pruebas, build de release, replay offline, 29 pruebas Swift, `swift format lint --strict` y `sync-core.sh --check`. Falta compilar y probar C# y los clientes por named pipe en Windows. El paso 4, retirar el sobre viejo, llega con la primera versión de nove que publique estos clientes con C# probado en Windows ([§12.3](../contracts/nu9ve-engine-v1.md#123-nove-transición-con-los-dos-sobres)).

## nu9ve-fs-v1

[Escritura duradera y acceso a archivos v1](../contracts/nu9ve-fs-v1.md)

| App | Implementación | Alcance |
| --- | --- | --- |
| nüün | `nu9ve-volumes::fs` usa `nu9ve-fs` desde la copia fijada | `Full`, creación exclusiva, reemplazo, rename exclusivo bajo el candado del escritor y lectura de verificación. El motor conserva sus errores y sus comprobaciones de archivos regulares. |
| Clipper | `rpc_writes/fsio.rs` y `rpc_jobs/store.rs` usan `nu9ve-fs` desde la copia fijada | Las escrituras que siguen a Python usan `Plain`. Usa candados exclusivos y compartidos, y sondea el candado del runner. La cola distingue el fallo anterior al rename del fallo de sincronización posterior. |

## nu9ve-json-v1

[JSON canónico, digest, ids y sobre de errores v1](../contracts/nu9ve-json-v1.md)

| App | Implementación | Prueba los vectores | Nota |
| --- | --- | --- | --- |
| nüün | Python `media/common.py` (oráculo) y Rust `nu9ve-json` | Rust: `engine/nuun-engine/tests/json_vectors.rs`, con el crate y los vectores de su copia fijada | El gate compara además el motor con el dorado de Python (`scripts/engine_golden.py --check` y 23 diferenciales en macOS) |
| Clipper | Rust `nu9ve-json` desde su copia fijada, para el sobre de `clipper-engine/1`, `sha256_hex` y `uuid4` (desde el 2026-11-16); su JSON propio sigue en `py.rs`/`pyval.rs` | No todavía: el canónico no lo usa | Compila `serde_json` con `arbitrary_precision` y `preserve_order` (§8) |
| nove | Sobre `{code, message}` propio | — | Ola 3 |

## nu9ve-mcp-v1

[Servidores MCP de las apps de nu9ve v1](../contracts/nu9ve-mcp-v1.md)

nove y nüün leídos en su código el 2026-11-09. Las demás apps no están clonadas en esta máquina: sin revisar.

| Regla | nove 0.12 (Windows) | nüün (`media mcp`) | curado | Clipper · kyu · Stalker · neuf · SoundShop · Atlas |
| --- | --- | --- | --- | --- |
| 0 Local stdio, sin HTTP | Sí | Sí | No tiene servidor (revisado: sin `mcp` en el código) | Sin revisar |
| 0 Cliente del motor en ejecución | Sí: named pipe, usuario y PID verificados | **No:** abre el catálogo con `Service` en su proceso | — | Sin revisar |
| 1 Protocolo y errores | Sí; versiones hasta 2025-06-18 | Sí; versiones hasta 2025-11-25; `serverInfo` `ncreator-media` 0.2.0, no el nombre y la versión de la app | — | Sin revisar |
| 2 Niveles | Leer, Mostrar, Editar, Actuar | Sólo Leer (7 herramientas: biblioteca, recurso, jobs, job, volúmenes, almacenamiento) | — | Sin revisar |
| 3 Permiso por sesión y registro | Sí (datos 22, 60 min, deshacer) | No aplica todavía | — | Sin revisar |
| 4 `_untrusted` e instrucciones | Sí | Parcial: instrucciones de una línea; nombres de archivo y proyecto sin marcar como datos | — | Sin revisar |
| 5 Registro e instalación | Alias MSIX `nove-mcp.exe`; Mac responde «sólo Windows» | `nu9ve media mcp` desde el checkout; sin alias ni ruta en el bundle | — | Sin revisar |
| 6 Validación | Motor real + MCP en `tests/mcp.rs`; **cliente real comprobado el 2026-11-09**: esta sesión de Claude Code llamó `nove_status` y respondió (motor en segundo plano, esquema 24) | `test_mcp_is_read_only_and_does_not_invoke_mutations` (en proceso); sin cliente real registrado | — | Sin revisar |

Primeros pasos sugeridos:

- **nüün:** que el MCP pida al lector persistente (`snapshot`) en lugar de abrir el catálogo, `serverInfo` `nuun` con la versión de la app, campos `_untrusted` para nombres de archivo y proyecto, y `show_in_app` para abrir un proyecto o un plan revisado en la ventana. Escribir `docs/mcp.md`.
- **nove:** servidor también en Mac; `<app>_status` ya existe como `nove_status`.
- **curado, kyu, Clipper:** empezar por Leer (biblioteca, proyectos, estado) y Mostrar; Editar sólo con registro deshacible.

## nu9ve-notifications-v1

[Avisos del sistema de las apps de nu9ve v1](../contracts/nu9ve-notifications-v1.md)

Leído en el código el 2026-11-10; nadie probó un aviso desde esta sesión.

| Regla | nove (Windows) | nove (Mac) | nüün (Mac) | nüün (Windows) | curado | Fractos Mac |
| --- | --- | --- | --- | --- | --- | --- |
| 1 Cola en el motor | Sí (`deliveries.rs`) | No: publica la app | No: publica la app | No | No | No |
| 2 Catálogo de tipos | Implícito en el motor (web, OTC, prueba) | Implícito | Un tipo: tarjeta detectada. Falta fin de trabajo | No | No | No |
| 3 Por defecto y control | Sí: «todo salvo excepciones» (A36), prueba de canal | Opt-in por vigilancia | Sin ajustes de avisos | No | No | No |
| 4 Contenido | Sí: tres líneas, botón «Ver en el Buzón», argumentos validados | Parcial | Parcial: título y recomendación; sin botones | No | No | No |
| 5 Identidad y límites | Sí: etiqueta y grupo, 30/h, 12 h, reintentos | Identificador por aviso | Identificador por tarjeta; sin límite | No | No | No |
| 6 Urgencia | Sí: recordatorio para señales de estrategia | No | No | No | No | No |
| 7 Sonido | Del sistema | Del sistema (pide `.sound`) | Silencioso (sólo pide `.alert`) | No | No | No |
| 8 Adaptador | Sí: rol `--deliver-toast`, activación COM | En la app | En la app; el clic abre Ingreso | No | No | No |
| 9 Validación | Automática sí; clic físico pendiente | Con ayudante ficticio; aceptación del sistema pendiente | Sin pruebas vistas | — | — | — |

Primeros pasos sugeridos:

- **nüün (Mac):** avisos de trabajo terminado y fallido con la app en segundo plano, con «Ver resultado» y «Expulsar tarjeta»; sección Avisos en Ajustes; sonido.
- **nüün (Windows):** el adaptador copiando el rol de nove con comentario de origen, cuando `plan`/`apply` estén en la ventana ([brief S9](audits/session-briefs-2026-11-10.md#s9--integración-de-sistema-en-la-ventana-de-windows)).
- **nove (Mac):** llevar el canal del sistema a la cola del motor al alcanzar el esquema actual, para que Mac y Windows compartan límites, alcance y caducidad.
- **nove (Windows):** registrar el clic físico; decidir si el canal nace encendido.
- **curado y Fractos Mac:** sólo cuando tengan algo que avisar en segundo plano.

Brief: [S18](audits/session-briefs-2026-11-10.md#s18--avisos-del-sistema-en-nüün-para-mac).

## nu9ve-sound-v1

[Sonido de tema de las apps de nu9ve v1](../contracts/nu9ve-sound-v1.md)

Leído en el código el 2026-11-10; nada escuchado.

| Regla | Fractos Mac | Fractos iOS | nüün | nove | curado |
| --- | --- | --- | --- | --- | --- |
| 1 Motor v2 | Sí, receta 3 | No: motor v1 | No | No | No |
| 2 Señales y gestos | Sí: es el origen de la tabla | Vocabulario propio de 11 acciones | No | No | No |
| 3 Voz por skin | Sí, por tema | Por tema | No | No | No |
| 4 Entrega | Motor Swift | Motor v1 | — | — | — |
| 5 Reproducción | Sí: precarga, límites, fatiga, reposo a los 20 s | `AVAudioEngine` con grupos y caché | No | No | No (tiene respuesta visual y háptica de teclado, no sonido de interfaz) |
| 6 Silencio | No: sólo en QA; no lee la preferencia del sistema | Coordinador de sesión de audio | No | No | No |
| 7 Por defecto y ajustes | Encendido por defecto (cumple la decisión); dos interruptores en Ajustes, sin volumen, sin «Silenciar» a un paso ni aviso de primera vez | Efectos, voz, volúmenes y háptica; sincroniza con su API | Parcial: Windows guarda `app.interfaceSounds`; nada suena | No | No |
| 8 Háptica | Sí, sin probar con trackpad | Sí | No | No | Háptica propia en Mac |

Primeros pasos sugeridos:

1. **Mac, con el repo de la web:** confirmar este contrato contra el motor de la web, resolver la pregunta de niveles y exportar el paquete (S19).
2. **Usuario:** confirmar las voces de los skins que Fractos no tiene (§3).
3. **Fractos:** decir en sus documentos que el v2 es el vigente; en la app de Mac, «Silenciar» a un paso, volumen y reglas de silencio.
4. **nüün Windows** como primera app consumidora (paquete): ya tiene los cinco skins y un `ThemeService` central.
5. **nüün Mac** con el motor Swift, cuando se decida dónde vive lo compartido.

## nu9ve-theme-v1

[Tema visual de las apps de nu9ve v1](../contracts/nu9ve-theme-v1.md)

Leído en el código el 2026-11-06, sin revisión visual. Revisar al actualizar. La columna de Fractos se releyó el 2026-11-10 en `ios-fractos` (`f1a7bcb`, app de Mac y `FractosDesign`), también sin verla.

| Regla | Fractos | curado | SoundShop | nüün | kyu | nove | neuf | Stalker |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2 Roles | Parcial: roles propios | Parcial: roles propios | Parcial: roles propios | Sí, 15 roles desde `Nu9veTheme`; falta `secondary` | Sí, 15 roles desde `Nu9veTheme`, más `secondary` y el entorno fotográfico neutro | No: colores fijos | No: colores del sistema | No aplica: sistema |
| 3 Contraste | Parcial: `mutedInk` y pruebas en Claro, Oscuro y Matrix; sin matriz | Sí, con matriz y los 110 vectores para `accent` y `onAccent` (TypeScript, 2026-11-16) | Sí, sin matriz | Sí, matriz y [110 vectores fijados](../data/theme-vectors-v1.json) con `Nu9veTheme` | Sí, matriz y los 110 vectores con `Nu9veTheme` (Gótico se prueba en la biblioteca) | No | Parcial: texto sobre acento | No aplica |
| 4 Apariencia | Parcial: tema, paleta, material, logo, sonido y háptica; claves propias, no el formato común | Sí, el más completo | Parcial: preset, superficie, brillo | Parcial: presets, HSV y modo | Sí: skin, modo, color, contraste, textura y tipografía con migración | No | Parcial: modo | No aplica |
| 5 Skins | Sí: nueve materiales (`glass`, `classicMac`, `win9x`, `terminal`, `neon`, `gothic`, `parchment`, `ledger`, `chrome`) | Sí: `dither`, `pixel`, `gothic`, `y2k`, `minimal` | No: uno | Sí: `glass`, `classicMac`, `win9x`, `terminal`, `gothic` | Sí: washi, Macintosh, Windows 9x, Terminal, Neón, Cristal, Grano y Mínimo | No | No | No aplica |
| 7 Identidad | Sí | Parcial: falta lo de barrio | Sí | Parcial: presets suizos | Sí | Parcial: solo escrita | No | No aplica |
| 8 Todas las ventanas | Sí en código (Mac): `ThemedWindowContent` da barra y atmósfera a Ajustes y Acerca de | Sin revisar | Sin revisar | Sí (2026-11-07): principal, Ajustes y Acerca de en los 5 skins | Sin revisar | Sin revisar | Sin revisar | No aplica |
| 9 Idioma | No en Mac: sólo español, sin catálogo de textos; iOS con catálogo incompleto | Parcial: mensajes nativos en español | No | Sí | No | Sin revisar | Sin revisar | Sin revisar |

Una app adopta el contrato añadiéndose aquí con lo que le falta. Primeros pasos sugeridos:

- **nüün:** roles, contraste y presets suizos hechos (2026-11-07). Skins Grano, Macintosh, Windows 9x y Gótico (2026-11-07). Terminal hecho; Grano retirado. Faltan la opción de Contraste y la tipografía suiza del wordmark. **Windows (2026-11-09, maqueta):** mismos roles y cinco skins portados a C# con su matriz, más una prueba de contraste por superficie con skin (`SurfaceChromeTests`). En Windows la ventana y sus botones siguen siendo del sistema ([windows-app-v1 §3](../contracts/windows-app-v1.md#3-tema)).
- **SoundBender (2026-11-16, `e1635e8`):** juego de skins completo en colores: Original (el suyo, sin cambios y por omisión, con capturas idénticas byte a byte), Cristal claro como contraste (decisión del usuario) y los tres comunes, con `Nu9veTheme` desde su copia fijada en macOS 15. El lienzo y el pincel conservan los colores del usuario. Faltan la ventana cuadrada de época, los radios y la tipografía de cada skin.
- **Identidad propia (2026-11-16, core `ace6d97`):** `Nu9veTheme` ya no trae colores de ninguna app. nüün declara su ámbar y sus presets suizos en `MediaMenuCore/ThemeIdentity.swift` (`1b83d23`), con los mismos valores. kyu ya tenía los suyos (Kyū·washi, neón) en `KyuSkin` y no cambió (`2de4e0f`). Una app que adopte el paquete conserva su original y su contraste, y suma los comunes de `ThemeSkin.commons`.
- **Kyū (2026-11-16):** usa `Nu9veTheme` desde su copia fijada en macOS 15. Cristal, Macintosh, Windows 9x y Terminal toman los roles comunes en contraste neutro. El fondo de la foto y de la cuadrícula se comprueba en gris neutro.
- **nove:** sus tres direcciones como presets y quitar los colores fijos.
- **neuf:** elegir su dirección de cine.
- **SoundShop:** añadir i18n.

- **curado (2026-11-16):** `bun test src/lib/theme.test.ts` comprueba los hex de `accent` y `onAccent` en los 110 casos fijados y añade sus 11 bases a la matriz de los cinco estilos propios. La comparación de los demás hex queda pendiente de adoptar las paletas comunes.

## windows-app-v1

[Apps de Windows de nu9ve v1](../contracts/windows-app-v1.md)

nove y curado leídos en sus repos el 2026-11-07; nüün probado en Windows 11 el 2026-11-09 (compilación, pruebas, capturas y recorrido por UI Automation).

| Regla | nove 0.12 Windows | curado 0.1 (Tauri) | nüün 0.1 Windows (maqueta) |
| --- | --- | --- | --- |
| 0 Motor común | Sí: Rust, named pipe, pruebas de contrato | Sí: Rust + comandos Tauri | Parcial: protocolo de `media rpc-serve`, transporte probado en memoria; motor no portado (volúmenes de Mac) |
| 1 Barra de título | Sí: `TitleBar` | Barra decorada del sistema en Windows (THEMES.md) | Sí: `TitleBar`, botones del sistema teñidos, dos filas en skins de época |
| 2 Ventana | Parcial | Sin revisar | Parcial: tres modos con tamaños de Mac por DPI, compacto encima; no recuerda el marco |
| 3 Tema | Parcial: «Puerto» claro/oscuro/alto contraste en código, sin captura de alto contraste | Sí en la WebView; alto contraste sin revisar | Sí en código y capturas: cinco skins, roles y superficies probados, vectores compartidos; alto contraste sin revisar a mano |
| 4 Teclado | Parcial: menús de bandeja con teclado; catálogo y access keys sin revisar | Sí en la WebView (remapeo); access keys no aplican | Parcial: catálogo con pruebas, aceleradores, `AccessKey`, panel Ctrl+/; sin badges de Ctrl/Shift |
| 5 Acerca de | Sí | Sin revisar | Sí (maqueta) |
| 7 Ajustes | Sí | Sí | Parcial: Apariencia y Teclado |
| 8 Sistema | Sí: host, bandeja, StartupTask, avisos, Credential Manager; falta aceptación física de sesión/suspensión | Sin segundo plano | No aplica todavía: sin segundo plano |
| 9 Paquete | Parcial: MSIX con certificado de desarrollo | Parcial: instaladores generados, uso en Windows pendiente | Parcial: MSIX empaquetado sin firmar; Setup con identidad en demostración |
| 10 Validación | Sí: `NOVE_QA_ROOT`, UI Automation | Pruebas automáticas en Windows 11; GUI pendiente | Sí: `NUUN_QA_ROOT`, `qa-skins.ps1`, UI Automation; 55 pruebas |

Las demás apps no tienen cliente Windows. El [mapa de la suite](suite.md#plataformas) dice cuál podría tenerlo y con qué motor.
