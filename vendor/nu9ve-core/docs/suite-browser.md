# Un navegador en la suite: propuesta

> **Versión 2 (2026-11-21):** el usuario quiere un navegador diario propio. La conclusión nueva (base Chromium ajena con una capa nu9ve encima) está en [la versión 2](suite-browser-v2.md). De aquí siguen valiendo el inventario (§2), los contratos (§7) y las fuentes (§12).

Estado: **propuesta para decidir, nada implementado** (2026-11-15). Responde a la pregunta del usuario: ¿tiene sentido que la suite tenga un navegador propio que ayude a grabar, que tenga shells mínimas con el estilo de nu9ve, fullscreen real y coordinación de lo que se navega (links compartidos, modo grabación de Stalker, señales para Clipper, MCP)? Y, de paso, ¿qué hacer ahora que Arc se acaba? ¿Hacer un fork de Chromium, usarlo como librería u otra cosa? ¿Rust en el núcleo y vistas nativas? Lo leído en los repos es del 2026-11-15. Los datos externos llevan su fuente en la [§12](#12-fuentes).

## 1. Respuesta corta

- **No conviene un fork de Chromium ni un navegador de uso diario propio.** Un fork obliga a seguir las versiones de Chromium (cada cuatro semanas, con parches de seguridad en medio) y a compilar un proyecto enorme. Brave, Vivaldi y Arc tienen equipos dedicados a eso. Un navegador diario además necesita gestor de contraseñas, extensiones, sincronización y DRM, y nada de eso aporta a la suite.
- **Sí conviene sacar provecho de dos piezas más pequeñas:**
  1. **Un puente con Brave/Chrome**: una extensión con *native messaging*, que es el mismo patrón que [nove ya usa](../../nove/apps/chrome-extension/README.md). Sigues navegando en Brave con tus perfiles, y la extensión le pasa a la suite lo que hace falta: la pestaña activa mientras Stalker graba, links para compartir y un «modo grabación» para la página. Es barato y da valor la primera semana.
  2. **Un «escenario» nativo para grabar y presentar.** Es una ventana de la suite con el motor web del sistema (WKWebView en Mac, WebView2 en Windows), fullscreen real, cromo mínimo con el tema de nu9ve, un panel de shell opcional, línea de tiempo con el reloj de Stalker y servidor MCP. No reemplaza a Brave. Es la superficie que se graba.
- **Rust en el núcleo y vistas nativas, sí**, igual que nove y nüün. El núcleo lleva la sesión, la línea de tiempo, los links, el MCP y el IPC. El motor web **no** es Rust: Servo ya se publica como librería, pero no sirve todavía para navegar a diario.
- **Lo de Arc es una decisión aparte y urgente.** Según reportes de octubre, Arc deja de funcionar el **22 de diciembre de 2026** ([§10](#10-arc-perfiles-y-el-navegador-diario)). La salida razonable es mover los perfiles a Brave ahora, sin esperar a que exista nada de esta propuesta.

## 2. Lo que ya hay en la suite y sirve

| Pieza | Dónde | Qué aporta a un navegador |
| --- | --- | --- |
| Extensión de Chrome/Brave + conector nativo | nove `apps/chrome-extension` (decisión A17) | Patrón probado: MV3, `nativeMessaging`, permisos mínimos, la sesión del usuario sin tocar cookies ni contraseñas, instalación para Chrome y Brave |
| WebKit aislado en Mac | nove [selector W2](../../nove/docs/contracts/web-picker-lab-v1.md) | `WKWebView` no persistente, mundo aislado, puente mínimo con nonce, red deny-all, ciclo de vida medido |
| WebView2 en Windows | nove WN2/WN5 | Perfiles InPrivate y persistentes, CDP pasivo, `WebResourceRequested`, worker sin UI |
| IPC y MCP en Rust | nove `core` (`mcp.rs`, `agent.rs`) | Servidor MCP por niveles con permiso por sesión ([MCP v1](../contracts/nu9ve-mcp-v1.md)); candidato a crate `nu9ve-mcp` |
| Eventos de Stalker | [`input-events.md`](../../stalker/docs/input-events.md) | `events.ndjson` ya registra la **app activa** por bundle, con el reloj de la sesión. No sabe qué página o pestaña había |
| Vistas de Stalker | [contrato stalker-clipper](../../stalker/docs/clipper-contract.md) | Recortes con nombre que Clipper importa como lienzos: una ventana «escenario» encaja como fuente |
| Overlays y highlights de Clipper | `clipper/docs/overlays.md`, highlights | Clipper ya usa señales de la captura (atajos, comandos, Ctrl+Fn) para decidir zoom y cortes |
| Enlaces `nu9ve://` | [integración con el OS §4](platforms/os-integration.md#4-enlaces-nu9ve) | Esquema propuesto para abrir un objeto en otra app |
| Tema y teclado | [tema v1](../contracts/nu9ve-theme-v1.md), [interacción v1](../contracts/nu9ve-interaction-v1.md) | Roles, skins, identidad por app y atajos para el cromo propio |
| forja | `ai/forja` (base T3 Code) | GUI para agentes. Si el escenario expone MCP, un agente lo puede manejar, como el panel de navegador de Claude Desktop |

Lo que falta: ninguna app sabe **qué se navegaba** durante una grabación, no hay una bandeja común de links, y ninguna superficie web se ve bien grabada (barras, pestañas, banners, tipografía chica).

## 3. Lo que se quiere, por caso de uso

| Caso | Qué hace falta | Quién lo hace mejor |
| --- | --- | --- |
| Grabar un tutorial o demo web | Ventana limpia, fullscreen sin barras, URL legible, cursor visible, sin notificaciones | **Escenario** |
| Saber qué página había en el segundo X de una grabación | Línea de tiempo de navegación con el reloj de Stalker | Extensión (en Brave) **o** escenario |
| Que Clipper use la navegación | Capítulos por página, lista de links para la descripción, placa de URL, desenfocar dominios sensibles | Clipper, a partir de la línea de tiempo |
| Compartir links entre apps o a otra persona | Bandeja común: «manda esta pestaña a…» | Extensión |
| Shell al lado de la página para demos de código | Terminal mínima con el tema y la fuente grande | Escenario (panel opcional) |
| Perfiles: trabajo, personal, clientes | Datos separados por perfil | **Brave** hoy; el escenario puede tener los suyos |
| Que un agente navegue, lea o muestre | Servidor MCP con niveles Leer/Mostrar/Actuar | Escenario (y la extensión en modo lectura) |
| Navegar todo el día | Contraseñas, extensiones, sync, DRM, actualizaciones | **Brave** (no construir) |

## 4. Estrategias de motor, comparadas

| Estrategia | Costo de mantener | Extensiones y gestor de contraseñas | Perfiles | Fullscreen y estilo propio | Windows | Veredicto |
| --- | --- | --- | --- | --- | --- | --- |
| **Fork de Chromium** (como Brave o Arc) | Muy alto: rebase cada 4 semanas, compilaciones de horas, parches de seguridad | Completas | Completos | Total | Sí | **No.** Es un trabajo de tiempo completo para un equipo |
| **CEF** (Chromium como librería) | Medio-alto: binario de ~200 MB, actualizar CEF a mano para estar seguro | Parciales | Sí | Total | Sí | Sólo si se llega a necesitar Chromium idéntico en Mac y Windows |
| **Electron** | Medio | Parciales | Sí | Alto | Sí | No: contradice la regla de clientes nativos de la suite (nove la descartó con Tauri/UI web) |
| **Webview del sistema** (WKWebView · WebView2 · WebKitGTK) | **Bajo**: el sistema lo actualiza y parcha | WKWebView: `WKWebExtension` (formato Safari, macOS 15.4+), no Chrome Web Store. WebView2: soporte limitado | WKWebView: varios almacenes persistentes desde macOS 14. WebView2: perfiles | Total, con vistas nativas | WebView2, ya probado en nove | **Sí, para el escenario** |
| **wry/Tauri** (Rust sobre los webviews del sistema) | Bajo | Igual que la fila anterior | Igual | Alto, pero las vistas no son nativas | Sí | Útil si algún día se quiere un cliente Linux rápido. curado ya usa Tauri, pero el cromo de un navegador pide vistas nativas |
| **Servo** (motor en Rust) | Bajo en código, alto en riesgo | No | — | Total | Sí | **Explorar después**: v0.1 como crate (abril 2026), compatibilidad web aún incompleta |
| **Ladybird** | — | No | — | — | No | No se puede embeber. Alfa todavía sin publicar |
| **Puente con Brave** (extensión + native messaging, CDP sólo si hace falta) | **Muy bajo** | Las de Brave | Los de Brave | Limitado: una extensión no cambia el cromo de Brave. Puede poner la ventana en fullscreen e inyectar estilos en la página | Sí (nove ya lo instala) | **Sí, primero** |

La combinación recomendada es **puente con Brave + escenario con webview del sistema**. Con eso no se mantiene ningún motor, se reusa lo que nove ya probó y cada pieza sirve aunque la otra no exista.

## 5. Pieza A · Puente con Brave

Una extensión MV3 de la suite (no la de nove, que es sólo para el broker) más un host de native messaging en Rust que habla con el motor de la app dueña.

- **Permisos mínimos:** `tabs` (URL y título de la pestaña activa), `nativeMessaging`, `storage`. Sin `debugger`, sin acceso a cookies ni al contenido de las páginas. `scripting` sólo si se activa el modo grabación de página.
- **Línea de tiempo de navegación (opt-in, sólo mientras Stalker graba):** cambios de pestaña activa, navegación y cierre. Por defecto guarda **sólo el dominio**. La URL completa se guarda sólo para los dominios de una lista que eliges, y nunca la query ni el fragmento. Los perfiles marcados como privados y las ventanas de incógnito no se registran nunca. Los títulos van como datos `_untrusted` ([MCP §4](../contracts/nu9ve-mcp-v1.md#4-datos-que-no-son-instrucciones)).
- **Modo grabación de página:** al empezar a grabar (o con un atajo) inyecta una hoja de estilos en la pestaña activa: fuente más grande, banners de cookies ocultos, cursor resaltado y foco visible. También puede poner la ventana en fullscreen (`chrome.windows.update`). No puede ocultar la barra de Brave dentro de la ventana. Para eso está el escenario.
- **Mandar a…:** menú contextual y atajo para mandar la pestaña o una selección de links a la bandeja de la suite, o a un proyecto de nüün/Clipper como referencia.
- **Lo que no hace:** no lee contraseñas, cookies ni formularios. No navega sola, no envía nada fuera de la computadora y no es un MCP con acciones.

Costo estimado: una extensión pequeña más un host. El patrón de instalación (copiar la carpeta, registrar el host en Chrome y Brave, quitar la conexión) ya está resuelto en nove.

## 6. Pieza B · El escenario

Una app de la suite, con su repo, que abre una o pocas páginas para **mostrarlas y grabarlas**, no para navegar todo el día.

- **Motor:** WKWebView en Mac y WebView2 en Windows. Perfiles con `WKWebsiteDataStore(forIdentifier:)` (macOS 14+) y perfiles de WebView2, uno por proyecto o cliente. Por defecto, perfil efímero.
- **Fullscreen real:** una ventana sin bordes que tapa la barra de menús y el Dock con las opciones de presentación de AppKit, o fullscreen nativo en su propio Space, a elegir. En Windows, ventana sin bordes en el monitor elegido. La resolución se puede fijar (1920×1080 o 1080×1920 para verticales), así la grabación sale a la medida sin recortar.
- **Cromo mínimo con estilo:** barra fina con los roles del tema y una placa de URL grande y legible que puede ocultarse al grabar. Atajos del catálogo común y skin propio según el [tema §7](../contracts/nu9ve-theme-v1.md#7-identidad-de-cada-app). El ornamento va sólo en la marca, nunca sobre la página.
- **Shell mínima (panel opcional):** una terminal al lado o debajo de la página, con la paleta del skin y la fuente grande, para demos de código. En Mac la vía corta es SwiftTerm (MIT, sería una dependencia nueva que hay que decidir). En Windows, ConPTY. Va en una fase posterior ([§9](#9-fases-y-criterios)) y sólo si el escenario se usa.
- **Coordinado con Stalker:** el escenario se presenta como **una ventana con nombre** que Stalker puede elegir como fuente o como vista. Escribe su línea de tiempo con el mismo reloj ([§7](#7-contratos-nuevos)) y se silencia solo mientras se graba: sin avisos, sin sonidos de interfaz y sin cursor de carga.
- **MCP:** servidor según [MCP v1](../contracts/nu9ve-mcp-v1.md). Nivel Leer: pestañas, URL y estado. Nivel Mostrar: abrir una URL de la lista permitida o pasar a fullscreen. Nivel Actuar, con diálogo nativo: navegar fuera de la lista o desplazarse. Nunca: llenar formularios, leer cookies ni teclear contraseñas.
- **Lo que no tiene:** sync, gestor de contraseñas propio, historial global ni tienda de extensiones.

## 7. Contratos nuevos

Cada contrato lo decide su dueño. Aquí sólo se propone la forma.

| Contrato | Dueño | Propuesta |
| --- | --- | --- |
| **Señal de grabación** | Stalker | Stalker publica «empezó / terminó» con `sessionID` y epoch, por notificación distribuida local en Mac (`NSDistributedNotificationCenter`) y algo equivalente en Windows cuando exista captura allí. Lo consultable va por `stalker-session status`. Sin contenido, sólo el estado |
| **Línea de tiempo web** `web.ndjson` | Productor: extensión o escenario. Formato: revisión aditiva del contrato `stalker-clipper` | Mismo origen y unidad de `t` que `events.ndjson`. Tipos `tab_active`, `navigate`, `tab_close`, `fullscreen`, con `domain`, `url` sólo si está permitida y `title_untrusted`. Stalker lo guarda dentro del bundle sin interpretarlo, como hace nüün con el bundle completo |
| **Bandeja de links** | Por decidir; candidata nüün, como «inbox» de referencias de proyecto | Entrada con URL, título `_untrusted`, origen (Brave, escenario o agente), proyecto opcional y hora. Se abre con `nu9ve://…/link/<id>` |
| **Uso en Clipper** | Clipper | Capítulos sugeridos por navegación, links para la descripción, placa de URL como overlay, desenfoque por lista de dominios sensibles. Todo como sugerencia y nada automático, igual que hoy con los highlights |

Mientras no exista `nu9ve/core`, estos contratos se escriben aquí como propuesta, igual que los demás ([cómo se comparte](suite-sharing.md)).

## 8. Arquitectura

```text
Brave/Chrome ──extensión MV3──► host native messaging (Rust) ─┐
                                                              │ IPC enmarcado (patrón nove core)
Escenario: vistas nativas (SwiftUI/AppKit · WinUI 3)          ▼
   └─ WKWebView / WebView2 ◄──── órdenes ──── núcleo Rust del escenario
                                              ├─ sesión, perfiles, lista permitida
                                              ├─ línea de tiempo web.ndjson (reloj Stalker)
                                              ├─ bandeja de links
                                              └─ servidor MCP (stdio, niveles)
Stalker ──señal de grabación──► núcleo        Stalker ◄── web.ndjson en el bundle ── Clipper
```

- **Rust en el núcleo**, porque ahí está lo que se comparte entre Mac y Windows (IPC, MCP, línea de tiempo, reglas de privacidad) y porque nove ya tiene esas piezas en Rust. Se replican con comentario de origen antes de extraer un crate ([replicar primero](../contracts/nu9ve-theme-v1.md#10-implementación-compartida)).
- **Vistas nativas**, porque el fullscreen, el cromo, los atajos y la integración con el sistema son justo lo que el escenario necesita hacer bien.
- **El motor web es del sistema** y no se reimplementa.

## 9. Fases y criterios

| Fase | Trabajo | Criterio de cierre | Costo |
| --- | --- | --- | --- |
| 0 | Mover perfiles de Arc a Brave (decisión del usuario, fuera de código) | Arc sin uso diario antes del 22 dic | Horas |
| 1 | Extensión + host: línea de tiempo por dominio sólo durante grabación (con señal de Stalker) y «mandar a…» | Una grabación de 10 min con `web.ndjson` alineado (comprobado contra `events.ndjson` y el video), sin URLs fuera de la lista | Días |
| 2 | Escenario mínimo en Mac: una página, fullscreen real, placa de URL, tema, perfil efímero, ventana elegible por Stalker | Grabar una demo con el escenario y que Clipper la importe sin recortes. Revisión visual tuya | 1–2 semanas |
| 3 | Clipper consume `web.ndjson`: capítulos y links sugeridos | Prueba con fixture en Clipper, sin aceptación física aún | Clipper decide |
| 4 | MCP del escenario (Leer y Mostrar), después Actuar con diálogo | Pruebas de [MCP §6](../contracts/nu9ve-mcp-v1.md#6-validación) y un cliente real | Días |
| 5 | Panel de shell (decidir dependencia), perfiles persistentes, gemelo WebView2 en Windows | Según uso real de las fases 2–4 | Por medir |
| — | Navegador diario propio | **Sólo** si, tras meses, el escenario se usa para todo y Brave estorba. Entonces se reevalúa CEF o Servo | — |

## 10. Arc, perfiles y el navegador diario

- Arc entró en mantenimiento en mayo de 2025. Atlassian compró The Browser Company (cerrado en octubre de 2025). En octubre de 2026 se reportó el cierre de Arc para el **22 de diciembre de 2026**, con importación a Dia hasta esa fecha. Conviene confirmarlo en el aviso oficial.
- **Brave** ya cubre perfiles separados, pestañas verticales y extensiones, y el puente de la fase 1 funciona ahí.
- Otras opciones para probar a mano antes de decidir: **Dia** (sucesor de Arc, con IA), **Zen** (fork de Firefox con estilo Arc) y **Nook** (proyecto de terceros, código abierto sobre WebKit, con perfiles por `WKWebsiteDataStore` y extensiones por `WKWebExtensionController`). Nook vale como **referencia de código** para el escenario, no como algo que adoptar.
- No hay que elegir el navegador diario pensando en esta propuesta. El puente sirve con cualquier navegador Chromium, y Zen pediría su propia extensión WebExtension, casi igual.

## 11. Riesgos y preguntas abiertas

- **Contraseñas en el escenario.** Ni WKWebView ni WebView2 traen el gestor de Brave. Habría que validar si 1Password funciona dentro de WKWebView, por su extensión de Safari con `WKWebExtension` o por el autollenado del sistema. Mientras no esté validado, el escenario sirve para páginas sin sesión o con sesión iniciada a mano en un perfil propio. Es una de las razones para no hacerlo navegador diario.
- **DRM:** WKWebView no reproduce todo lo que Brave sí (Widevine). Para grabar video protegido, el escenario no sirve.
- **Privacidad de la línea de tiempo:** una URL puede revelar más que una tecla. Por eso el defecto es dominio, lista explícita y nada en perfiles privados. Clipper nunca debe quemar una URL en un video sin que tú lo elijas.
- **CDP en Brave** muestra la barra «está depurando este navegador» (nove lo vio). La fase 1 no lo usa.
- **Nombre y repo:** la convención es «nueve» en otro idioma ([tema §7](../contracts/nu9ve-theme-v1.md#7-identidad-de-cada-app)). Una candidata es **naŭ** (esperanto: la lengua universal, como la web), pero suena cerca de nüün. Alternativas: *nio* (sueco) o *naw* (galés). Lo decides tú.

**Lo que decides tú:**

1. ¿Brave como navegador diario tras Arc, o probar Zen/Dia primero?
2. ¿Empezamos por la fase 1 (puente) y en qué repo vive: uno nuevo del escenario o junto a Stalker?
3. ¿La línea de tiempo web va dentro del bundle de Stalker (revisión del contrato `stalker-clipper`) o como archivo aparte que nüün asocia?
4. ¿La bandeja de links es de nüün o de la app nueva?
5. ¿Se acepta SwiftTerm como dependencia cuando llegue el panel de shell?

## 12. Fuentes

Externas, consultadas el 2026-11-15:

- Cierre de Arc el 22 dic 2026: [AlternativeTo, oct 2026](https://alternativeto.net/news/2026/10/the-browser-company-confirms-arc-browser-sunset-directs-users-to-ai-powered-successor-dia/). Mantenimiento desde mayo de 2025: [The Register](https://www.theregister.com/2025/05/27/arc_browser_development_ends/). Compra de Atlassian: [Neowin](https://www.neowin.net/news/atlassian-acquires-the-browser-company-creators-of-arc-browser/).
- Perfiles en WebKit (varios almacenes persistentes, macOS 14): [WebKit blog](https://webkit.org/blog/14423/building-profiles-with-new-webkit-api/). Nook y `WKWebExtensionController` (macOS 15.4+, según el análisis de su código): [DeepWiki](https://deepwiki.com/nook-browser/Nook/5.6-browser-extensions-system).
- Servo v0.1 en crates.io (una sola fuente secundaria; confirmar en el proyecto): [Machine Herald](https://machineherald-io.pages.dev/article/2026-04/14-servo-reaches-cratesio-with-v010-giving-rust-developers-an-embeddable-browser-engine-with-long-term-support). Ladybird, alfa anunciada sin publicar: [ladybird.org](https://ladybird.org/).

Internas: [mapa de la suite](suite.md), [MCP v1](../contracts/nu9ve-mcp-v1.md), nove [adquisición](../../nove/docs/acquisition.md) y [decisiones](../../nove/docs/decisions.md), Stalker [README](../../stalker/README.md) y [eventos](../../stalker/docs/input-events.md).
