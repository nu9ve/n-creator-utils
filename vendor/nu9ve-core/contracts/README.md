# Contratos

Reglas que toda la suite sigue. Cada contrato dice la regla; qué cumple cada app está aparte, en [adopción por app](../docs/adoption.md). Se mudaron de `n-creator-utils/docs/contracts/` el 2026-11-16 y desde entonces se editan aquí.

| Contrato | Qué define | Estado |
| --- | --- | --- |
| [Apps de macOS v1](macos-app-v1.md) | Barra de título, menús, Acerca de, documentos, distribución | En adopción |
| [Apps de Windows v1](windows-app-v1.md) | Su par en Windows: motor compartido con la app de Mac, ventana, instalador | Propuesta |
| [Tema visual v1](nu9ve-theme-v1.md) | Roles de color, opciones de Apariencia, skins, identidad de cada app; vectores en [data/](../data/theme-vectors-v1.json) | Propuesta |
| [Teclado e interacción v1](nu9ve-interaction-v1.md) | Catálogo de atajos, badges con capas por modificador, ayuda contextual, panel ⌘/ | Propuesta |
| [Servidores MCP v1](nu9ve-mcp-v1.md) | Cuatro niveles (leer, mostrar, editar, actuar), permiso por sesión, datos que no son instrucciones | Propuesta |
| [Avisos del sistema v1](nu9ve-notifications-v1.md) | Notificaciones del sistema: cola de entregas, clic al objeto exacto, límites, sonido | Propuesta |
| [Sonido de tema v1](nu9ve-sound-v1.md) | Sonidos de interfaz por skin, con el motor v2 de Fractos como fuente | Propuesta |
| [JSON canónico v1](nu9ve-json-v1.md) | Bytes canónicos compatibles con Python, digest, ids, sobre de errores; vectores en [data/](../data/json-vectors-v1.json) | Implementado en nüün |
| [Archivos v1](nu9ve-fs-v1.md) | Escritura y publicación duraderas, candados, lecturas de verificación | Implementado |
| [Motor detrás de una app v1](nu9ve-engine-v1.md) | Transporte (líneas, marcos, una vez), petición e id, sobre, `engine_info`, `not_ported`, códigos de error, ciclo de vida, seguridad local, sombra contra un oráculo; ejemplos en [data/](../data/engine-examples-v1.json) | Propuesta |

Los contratos de media (Stalker, Clipper, Atlas, ingreso, nombres, cierre de proyectos) se quedan en [n-creator-utils](https://github.com/nu9ve/n-creator-utils/blob/master/docs/contracts/README.md): son entre nüün y otra app.

Un cambio que rompe sube de versión (`-v2`) y las dos conviven mientras las apps migran. Un contrato cambia sólo cuando cambia su regla. Cada app actualiza su fila en [adopción por app](../docs/adoption.md) desde su propio repo, con un cambio aquí, y eso no deja atrás a las apps que fijan el contrato.
