# Un navegador en la suite, versión 2: tu navegador diario sobre un Chromium ajeno

Estado: **propuesta para decidir, nada implementado** (2026-11-21). Reemplaza la conclusión de la [versión 1](suite-browser.md), que sigue valiendo para el inventario, los contratos de la §7 y las fuentes.

La versión 1 decía «no hagas un navegador diario propio». El usuario pide ahora justo eso, al nivel de usarlo por defecto:

- una experiencia coherente, minimalista y configurable;
- funciones propias que hoy resuelve instalando extensiones sueltas: modo oscuro, compresor o limitador del audio de una pestaña o un video;
- perfiles seguros y fáciles de cambiar;
- una capa de manejo y configuración sobre la que se puedan integrar flujos de la suite;
- un motor eficiente y siempre al día, probablemente Chromium.

Leído en los repos el 2026-11-21: nove (`docs/acquisition.md`, `docs/architecture.md`, `apps/chrome-extension`), Stalker (`docs/2026-11-18-suite-y-prioridades.md`) y Clipper (sin planes de navegador; lo que llama «navegador» es el de proyectos).

## 1. Respuesta corta

- **El navegador diario sí, el motor no.** Lo que hace tuyo un navegador es la capa de encima: perfiles, funciones, atajos, estilo e integración. Esa capa se construye. El motor lo mantiene otro equipo, que sigue a Chromium cada cuatro semanas y saca los parches de seguridad entre versiones.
- **Tres piezas:**
  1. **Base:** un Chromium mantenido por otros, Brave o Helium (§3);
  2. **Capa nu9ve** (§4): un gestor de perfiles nativo, una extensión de la suite con módulos y un host nativo en Rust que se anuncia en la suite como cualquier motor;
  3. **Runtime de fondo** (§5): un Chromium fijado para navegar sin ventana, compartido por nove (recolectar) y Clipper (grabar páginas).
- **El escenario de la versión 1 se simplifica:** su primera fase es una ventana `--app` o `--kiosk` del mismo Chromium, abierta por la capa con un perfil de grabación (§6). WKWebView queda sólo si después hace falta.
- **Sigue sin convenir:** un fork de Chromium, CEF (casi no admite extensiones, así que no sirve de navegador diario) y Electron.
- **Arc deja de funcionar el 22 de diciembre de 2026.** La fase 0, mover los perfiles a la base elegida, va ya, aunque no exista nada más.

## 2. Qué cambia respecto a la versión 1

| Tema | Versión 1 | Versión 2 |
| --- | --- | --- |
| Navegador diario | Brave tal cual; no construir | Brave o Helium **con la capa nu9ve encima**, que es lo que se usa a diario |
| Extensión | Puente mínimo (pestaña activa, «mandar a…») | Extensión de la suite con módulos por perfil, puente incluido |
| Perfiles | Los de Brave | Gestor propio: un `--user-data-dir` por perfil, cada uno con sus módulos y extensiones |
| Escenario | App nueva con WKWebView y WebView2 | Primero una ventana `--app`/`--kiosk` del mismo Chromium; WKWebView después, si hace falta |
| Navegar de fondo | Fuera del alcance (era cosa de nove) | Runtime compartido por nove y Clipper |

## 3. La base: qué Chromium

| Base | A favor | En contra |
| --- | --- | --- |
| **Brave** | Equipo grande, actualizaciones rápidas, perfiles, pestañas verticales, bloqueador propio; nove ya instala ahí su extensión | Trae cosas que no pides (cripto, noticias, IA) que hay que apagar; su interfaz no se puede cambiar |
| **Helium** (imputnet, sobre ungoogled-chromium) | Minimalista, sin telemetría, bloqueador incluido, extensiones completas; macOS es su plataforma principal. Versión 0.15.7.1 sobre Chromium 151 | Equipo pequeño; todavía en 0.x. Hay que medir cuánto tarda en sacar un parche de seguridad después de Chrome |
| Chrome | La referencia, al día siempre | Telemetría y cuenta de Google; contradice lo de minimalista |

Recomendación: **probar Helium dos semanas como diario**, con Brave como respaldo. Elegir según dos medidas: cuánto tarda en publicar los parches de Chrome y si las extensiones de siempre (1Password incluido) funcionan. La capa nu9ve sirve igual con cualquiera de los dos, porque los dos son Chromium y aceptan las mismas extensiones y los mismos argumentos de arranque.

## 4. La capa nu9ve

### 4.1 Gestor de perfiles

Una app nativa de barra de menús (SwiftUI con `NuMacShell` y `Nu9veTheme`; WinUI 3 después), con el núcleo en Rust.

- **Un perfil es una carpeta propia** (`--user-data-dir`), no un perfil dentro de la misma carpeta. Así, cada perfil es un proceso aparte con sus cookies, su historial, sus extensiones y su caché, y cerrarlo o borrarlo no toca a los demás.
- **Cambiar de perfil es un atajo:** el gestor abre la ventana del perfil o la trae al frente. Cada perfil tiene su color e ícono en el Dock, que salen de los roles del tema.
- **Cada perfil declara qué lleva:** módulos de la extensión de la suite, extensiones de terceros, buscador, si guarda historial y si es privado (§4.4).
- **Perfiles efímeros:** una carpeta temporal que se borra al cerrar, para clientes, pruebas o grabaciones.
- **Abrir links:** el gestor puede ser el navegador por defecto del sistema y mandar cada link al perfil que le toca, con reglas por dominio o por la app que lo abrió (un link desde Slack de trabajo va al perfil de trabajo).

### 4.2 La extensión de la suite

Una sola extensión MV3, con módulos que cada perfil enciende o apaga. Esta extensión no es la de nove, que sigue siendo sólo para su broker.

| Módulo | Qué hace | Cómo | Permisos |
| --- | --- | --- | --- |
| **Modo oscuro** | Oscurece las páginas que no lo traen, con reglas por sitio | Hoja de estilos y filtros inyectados; Dark Reader (MIT) es la referencia | `scripting` y acceso a los sitios elegidos |
| **Audio de la pestaña** | Compresor, limitador y nivel objetivo por pestaña o video | `tabCapture` desde el service worker y un documento *offscreen* con Web Audio (Chrome 116+). El limitador y el LUFS son los de SoundBender y `nu9ve-loudness`, compilados a WASM | `tabCapture`, `offscreen`. Se activa con un clic o atajo por pestaña, porque la captura pide un gesto del usuario |
| **Grabación** | Página lista para grabar: fuente más grande, banners ocultos, cursor resaltado | Igual que el modo grabación de la [versión 1 §5](suite-browser.md#5-pieza-a--puente-con-brave) | `scripting` sólo mientras se graba |
| **Línea de tiempo para Stalker** | `web.ndjson` con dominio y, si está permitido, URL, mientras Stalker graba | [Versión 1 §7](suite-browser.md#7-contratos-nuevos) | `tabs` |
| **Mandar a…** | La pestaña o los links elegidos, a la bandeja de la suite o a un proyecto | Menú contextual y atajo | `contextMenus` |
| **Atajos de la suite** | El catálogo común y su panel de ayuda dentro del navegador | [Contrato de interacción](../contracts/nu9ve-interaction-v1.md) | `commands` |

Límites que hay que saber de entrada:

- Al capturar una pestaña, Chrome la silencia: el documento *offscreen* tiene que volver a sacar el audio procesado. Chrome muestra además su indicador de captura.
- La alternativa sin captura, procesar cada `<video>` o `<audio>` de la página con Web Audio, no sirve con el medio de otro origen sin CORS: sale en silencio. Por eso el módulo usa `tabCapture`.
- Una extensión no cambia el cromo del navegador (barra, pestañas). Lo coherente se logra con el tema de la base, el gestor y la extensión, no dentro de la ventana.

Instalación: al principio, desempaquetada en modo desarrollador, como hace nove. Después, publicada sin listar en la Chrome Web Store, o instalada con la política `ExtensionInstallForcelist` y un servidor de actualizaciones propio. Hay que validar cómo aplica cada base esa política en macOS sin un perfil de configuración gestionado.

### 4.3 Host nativo

Un binario en Rust que la extensión llama por *native messaging*.

- Se anuncia con [`nu9ve-registry-v1`](../contracts/nu9ve-registry-v1.md) usando `nu9ve-ipc`, como el motor de nove. Así `nu9ve mcp` y las demás apps lo encuentran.
- Responde con el sobre de [`nu9ve-engine-v1`](../contracts/nu9ve-engine-v1.md).
- Publica sus herramientas de MCP por niveles: Leer (pestañas, perfiles, dominio activo) y Mostrar (abrir una URL en un perfil). Actuar, sólo con diálogo nativo. Nunca: cookies, contraseñas ni formularios ([MCP v1](../contracts/nu9ve-mcp-v1.md)).
- Recibe la señal de grabación de Stalker y avisa a la extensión.

### 4.4 Perfiles seguros

- **Aislados por carpeta**, como en §4.1.
- **Contraseñas:** 1Password por perfil, con su extensión. La capa nunca las lee ni las guarda.
- **Perfiles privados:** sin historial ni línea de tiempo, y nunca visibles por MCP.
- **Lo que no da el aislamiento por carpeta:** Chromium en macOS cifra las cookies con una clave del Llavero que es la misma para todos los perfiles de una app. Un perfil separado no tiene otra clave. Si un perfil de cliente pide más, va en un volumen cifrado aparte (APFS), montado sólo cuando se usa.
- **Borrar un perfil** borra su carpeta entera, con confirmación y sin papelera.

## 5. Runtime de fondo

Un Chromium fijado para navegar sin ventana: Chrome for Testing o su `chrome-headless-shell`, que son las versiones que Playwright espera. No es la base diaria; se actualiza a propósito, con su versión anotada.

- **Lo supervisa la suite:** grupo de procesos, plazo, cancelación y limpieza, idealmente con `nu9ve-jobs` cuando exista (sale del `jobkit` de Clipper).
- **Perfiles con *lease*:** nunca dos escritores en un perfil, *pools* con TTL y máximo de páginas, como ya diseñó nove ([`architecture.md`](../../nove/docs/architecture.md)).
- **Visible u oculto:** visible para iniciar sesión o diagnosticar, oculto para lo ya probado. Pasar de oculto a visible recrea la sesión.

Quién lo usa:

| App | Para qué | Hoy |
| --- | --- | --- |
| nove | Recolectar páginas con sesión, capturar las respuestas que la página ya pide | Diseñado (P3 de su plan), sin código. Su extensión del broker y WebView2 en Windows ya existen |
| Clipper | Grabar recorridos de una página automatizados, para demos o material de un video | Nada todavía. Ver §6 |
| `ai/video` | Ya renderiza con un Chromium sin ventana (Remotion) | Fuera de la suite; podría compartir el binario fijado |

## 6. Grabar páginas: el escenario simplificado

- **Ventana limpia:** la capa abre el Chromium base con `--app=<url>` (o `--kiosk` para pantalla completa), con un perfil de grabación efímero y el tamaño fijado (1920×1080 o 1080×1920). Sin barras ni pestañas.
- **Grabación de calidad:** Stalker graba esa ventana por su nombre, con su reloj. Es mejor que el video de Playwright, que sale con poca calidad, o que los cuadros de `Page.startScreencast`, que van a pocos fps.
- **Automatizado:** el runtime de §5 la maneja por CDP con un guion (desplazarse, hacer clic, esperar), y la extensión escribe `web.ndjson`. Clipper recibe el bundle de Stalker con la línea de tiempo, de donde salen capítulos, links y la placa de URL ([versión 1 §7](suite-browser.md#7-contratos-nuevos)).
- **Aviso:** con CDP, el navegador muestra la barra «está depurando este navegador» (nove lo vio). Para grabar, el guion corre en el Chromium del runtime, no en el diario.

## 7. Fases

| Fase | Trabajo | Criterio de cierre | Costo |
| --- | --- | --- | --- |
| 0 | Elegir base (Helium o Brave) y mover los perfiles de Arc | Arc sin uso diario antes del 22 de diciembre | Horas |
| 1 | Gestor de perfiles mínimo: perfiles como carpetas, atajos, ícono y color, efímeros, navegador por defecto con reglas por dominio | Una semana de uso diario sin volver a Arc | 1–2 semanas |
| 2 | Extensión de la suite con dos módulos: audio de la pestaña y modo oscuro | Un video con picos se oye nivelado; las mediciones con los vectores de `nu9ve-loudness` | 1–2 semanas |
| 3 | Host nativo: registro, MCP (Leer y Mostrar), señal de grabación y `web.ndjson` | Una grabación de 10 min con `web.ndjson` alineado; `nu9ve mcp` lista los perfiles | Días |
| 4 | Runtime de fondo y grabación automatizada de una página para Clipper | Un guion de 30 s grabado por Stalker e importado por Clipper | 1–2 semanas |
| 5 | nove recolecta con el runtime | Lo decide nove en su plan | nove decide |
| — | Windows: el gestor en WinUI 3; la extensión y el host ya sirven | Según uso | Por medir |

## 8. Riesgos

- **El parche de seguridad lo pone la base.** Si Helium tarda días en seguir a Chrome, se cambia a Brave sin tocar la capa.
- **Políticas y extensiones fuera de la tienda.** Cada base las trata distinto en macOS (§4.2). Se valida en la fase 1.
- **Captura de audio:** pide un gesto por pestaña y Chrome muestra su indicador. Si molesta, el módulo puede recordar los sitios donde se encendió y pedir el gesto con un atajo.
- **DRM:** un video protegido no se puede procesar con Web Audio. El módulo de audio no sirve ahí.
- **Mantenimiento:** la extensión sigue los cambios de la API de extensiones de Chrome. Es mucho menos trabajo que un motor, pero no es cero.

## 9. Lo que decides tú

1. ¿Helium o Brave como base? La propuesta: Helium dos semanas, con Brave como respaldo.
2. ¿Un repo nuevo para la capa (gestor, extensión y host)? ¿Con qué nombre nueve y qué nombre Bender? La versión 1 proponía *naŭ*, *nio* o *naw*.
3. ¿Qué módulos primero? La propuesta: audio de la pestaña y modo oscuro.
4. ¿El runtime de fondo es de la capa o del core (un `nu9ve-browser` en Rust)? Con dos usuarios a la vista (nove y Clipper), la regla de entrada del core se cumple.
5. Siguen abiertas las preguntas 3 a 5 de la [versión 1 §11](suite-browser.md#11-riesgos-y-preguntas-abiertas): dónde va `web.ndjson`, de quién es la bandeja de links y SwiftTerm.

## 10. Fuentes

Externas, consultadas el 2026-11-21:

- Helium: [imputnet/helium](https://raw.githubusercontent.com/imputnet/helium/main/README.md) (basado en ungoogled-chromium), [imputnet/helium-macos](https://upd.dev/imputnet/helium-macos) y la versión 0.15.7.1 sobre Chromium 151 en [Neowin](https://www.neowin.net/software/helium-browser-01571/).
- `tabCapture` desde el service worker y documento *offscreen* (Chrome 116+), la pestaña silenciada al capturar y el gesto del usuario: [Audio recording and screen capture](https://developer.chrome.com/docs/extensions/mv3/screen_capture).
- El cierre de Arc y lo demás, en las [fuentes de la versión 1](suite-browser.md#12-fuentes).
