# Contratos

Reglas que toda la suite sigue. Cada contrato dice la regla; qué cumple cada app está aparte, en [adopción por app](../docs/adoption.md). Se mudaron de `n-creator-utils/docs/contracts/` el 2026-11-16 y desde entonces se editan aquí.

| Contrato | Qué define | Estado |
| --- | --- | --- |
| [Apps de macOS v1](macos-app-v1.md) | Barra de título, menús, Acerca de, documentos, distribución | En adopción |
| [Apps de Windows v1](windows-app-v1.md) | Su par en Windows: motor compartido con la app de Mac, ventana, instalador | Propuesta |
| [Tema visual v1](nu9ve-theme-v1.md) | Roles de color, opciones de Apariencia, skins, identidad de cada app; vectores en [data/](../data/theme-vectors-v1.json) | Propuesta |
| [Teclado e interacción v1](nu9ve-interaction-v1.md) | Catálogo de atajos, badges con capas por modificador, ayuda contextual, panel ⌘/ | Propuesta |
| [Controles por tipo de dato v1](nu9ve-controls-v1.md) | Números con `ParamSpec`, fechas y tiempo, textos largos, listas, mapas e imágenes, voz y seguimiento, estados (también «disco no conectado») y accesibilidad de los controles | Propuesta (2026-11-22) |
| [Servidores MCP v1](nu9ve-mcp-v1.md) | Cuatro niveles (leer, mostrar, editar, actuar), permiso por sesión, datos que no son instrucciones | Propuesta |
| [Avisos del sistema v1](nu9ve-notifications-v1.md) | Notificaciones del sistema: cola de entregas, clic al objeto exacto, límites, sonido | Propuesta |
| [Sonido de tema v1](nu9ve-sound-v1.md) | Sonidos de interfaz por skin, con el motor v2 de Fractos como fuente | Propuesta |
| [JSON canónico v1](nu9ve-json-v1.md) | Bytes canónicos compatibles con Python, digest, ids, sobre de errores; vectores en [data/](../data/json-vectors-v1.json) | Implementado en nüün |
| [Archivos v1](nu9ve-fs-v1.md) | Escritura y publicación duraderas, candados, lecturas de verificación | Implementado |
| [Motor detrás de una app v1](nu9ve-engine-v1.md) | Transporte (líneas, marcos, una vez), petición e id, sobre, `engine_info`, `not_ported`, códigos de error, ciclo de vida, seguridad local, sombra contra un oráculo; ejemplos en [data/](../data/engine-examples-v1.json) | Propuesta |
| [Registro de motores v1](nu9ve-registry-v1.md) | Archivo por app en `nu9ve/engines/`: transporte, pid con hora de inicio, versión, capacidades; registro viejo ignorado; ejemplos en [data/](../data/registry-examples-v1.json) | Propuesta |
| [Catálogo de datos v1](nu9ve-data-v1.md) | `data_catalog`: datasets con modalidad, ejes, tamaño, ruta, hash, dueño y privacidad; `project_key` común y claves conocidas; raíces en un disco; agregación; ejemplos en [data/](../data/data-examples-v1.json) | Propuesta |
| [Artefactos de datos v1](nu9ve-artifact-v1.md) | Paquete de una gráfica, mapa o tabla: datos exactos, render PNG/SVG, receta, linaje, privacidad y animación en cuadros o declarada; validador y paquete de ejemplo en [data/](../data/artifact-examples-v1.json) | Propuesta |
| [Perfiles en esta máquina v1](nu9ve-profile-v1.md) | `profiles.json` de cada máquina: los perfiles de la persona (`e64`), su tipo y, por app, dónde vive la biblioteca de cada uno; cada app escribe sólo su clave, con candado; ejemplos en [data/](../data/profile-examples-v1.json) | Propuesta (2026-11-25) |
| [Identidad de dispositivo v1](nu9ve-device-v1.md) | `device.json` de cada máquina: id aleatorio, nombre que elige la persona, sin serial ni nombre del equipo, y cómo se detecta una copia a otra máquina; ejemplos en [data/](../data/device-examples-v1.json) | Propuesta (2026-11-22) |
| [Agente de la suite v1](nu9ve-agent-v1.md) | Un solo agente `nu9ve` por máquina: dónde se instala, `agent.json`, gana la versión más nueva y nunca se baja, instalar y quitar sin tocar la identidad, qué ve una app; ejemplos en [data/](../data/agent-examples-v1.json) | Propuesta (2026-11-22) |
| [Carpeta de intercambio v1](nu9ve-exchange-v1.md) | Una carpeta por máquina con registros firmados en Ed25519 (identidad, discos, catálogos, dueñas, pedidos, objetos), confianza por emparejado, orden por `seq`, lo personal y los pedidos; ejemplos en [data/](../data/exchange-examples-v1.json) | Propuesta (2026-11-22) |
| [Exportar un dataset v1](nu9ve-export-v1.md) | `data_export`: el motor dueño entrega al agente los bytes de un dataset pequeño, listos en un archivo con su hash, o el disco donde está; lo personal sólo si la persona lo compartió, y cómo una app ofrece sola lo compartido; ejemplos en [data/](../data/export-examples-v1.json) | Propuesta (2026-11-22) |
| [Identidad de volumen v1](nu9ve-volume-v1.md) | Marcador `.nu9ve/volume.json` con rol y segundo factor, estados al conectar un disco, copias y clones, quién escribe según el rol, carpeta e índice de cada app en el disco; ejemplos en [data/](../data/volume-examples-v1.json) | Propuesta (2026-11-22) |
| [Registro de benchmarks v1](nu9ve-bench-v1.md) | Una corrida por línea de un NDJSON que sólo se agrega: suite, caso, sujeto, máquina, entradas con hash, métricas con mediana y p95, calidad y privacidad; cómo se comparan y cómo se traducen los formatos anteriores | Propuesta (2026-11-21); ejemplos en [data/](../data/bench-examples-v1.json) |

Los contratos de media (Stalker, Clipper, Atlas, ingreso, nombres, cierre de proyectos) se quedan en [n-creator-utils](https://github.com/nu9ve/n-creator-utils/blob/master/docs/contracts/README.md): son entre nüün y otra app.

Un cambio que rompe sube de versión (`-v2`) y las dos conviven mientras las apps migran. Un contrato cambia sólo cuando cambia su regla. Cada app actualiza su fila en [adopción por app](../docs/adoption.md) desde su propio repo, con un cambio aquí, y eso no deja atrás a las apps que fijan el contrato.
