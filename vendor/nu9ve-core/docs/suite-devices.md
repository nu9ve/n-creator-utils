# Varias máquinas, discos y nube en la suite

Estado: **propuesta** (2026-11-22), pedida por el usuario desde una sesión del core. Ese mismo día el usuario respondió nueve preguntas y parte de una décima, marcadas como decididas en la [§12](#12-preguntas-para-el-usuario): **F0 ya no tiene bloqueos**, y F1 y F2 tienen su regla de privacidad, su ruta de login y su recuperación. Lo demás sigue abierto. **El paso del core en F0 está hecho** (2026-11-22): [`nu9ve-device-v1`](../contracts/nu9ve-device-v1.md), [`nu9ve-volume-v1`](../contracts/nu9ve-volume-v1.md) y la versión menor de [`nu9ve-data-v1`](../contracts/nu9ve-data-v1.md), con ejemplos y validadores. Siguen nüün, Atlas, nove y Clipper en sus repos. nüün tiene su primer corte (2026-11-22, sin commit): MEDIA502 y ESSD registrados, con su índice, y «adoptar este disco» probado por el motor ([adopción](adoption.md#nu9ve-volume-v1)). Parte de una revisión de la suite hecha el mismo día ([§2](#2-dónde-estamos-revisión-del-2026-11-22)). Relacionados: [licencias](licensing.md), [MCP de la suite](suite-mcp.md), [registro](../contracts/nu9ve-registry-v1.md), [catálogo de datos](../contracts/nu9ve-data-v1.md), [auditoría A3](audits/fable-audit-2026-11-10.md#a3--la-identidad-de-un-disco-no-viaja-entre-equipos-ni-entre-sistemas).

El problema, en palabras del usuario: hay dos Macs lado a lado (la M1 y la M4) que pueden correr nüün, Atlas, nove y Clipper. A veces Atlas procesa misiones y baches en la M4 y los datos se analizan en nove en la M1. Un disco de archivo, MEDIA501, a veces está en una Mac y a veces en la otra, y en las dos se querría seguir el ingreso o los respaldos del perfil. Ya no queda claro qué disco tiene qué ni qué máquina tiene qué estado. El usuario quiere una nube común en el core privado, un MCP que habilite más capacidades cuando la nube esté activa, registrar dispositivos y que todo sea compatible con la cuenta de Fractos: la misma cuenta y el mismo método de pago para modelos y almacenamiento, con perfiles locales.

## 1. Respuesta corta

- **El primer problema no es la nube. Es la identidad.** Hoy un disco se reconoce por la ruta donde se monta (`/Volumes/ESSD`) o por un id que cambia de un sistema a otro, y ninguna app sabe en qué máquina corre. Eso se arregla sin red ni cuenta: los discos y las máquinas reciben una identidad que viaja con ellos, y las raíces de datos apuntan a un disco, no a una ruta de montaje (F0).
- **Después, las máquinas se cuentan qué tienen.** Cada máquina escribe su estado en una carpeta de intercambio, en archivos firmados, y escribe sólo en la suya. Las demás leen y piden lo que quieran traer. Esa carpeta puede viajar por Syncthing, iCloud o un disco, y más tarde por la nube propia. Así no hay servidor ni una base de datos con varios escritores (F1).
- **No hace falta una Mac principal.** Basta con que cada cosa tenga una sola dueña a la vez. Lo que está en un disco es del disco, y llevar el disco es llevar el proyecto. Lo procesado es de la máquina que lo procesó, y las demás lo traen. Lo que sólo vive en una máquina tiene una dueña por proyecto. Las decisiones de nüün en dos Macs se juntan por proyecto (§6.4).
- **La nube entra en tercer lugar, y es la cuenta de Fractos.** El mismo login, por OAuth, y los mismos pagos. Cada dispositivo se registra con su propia clave y se puede revocar. Todo va cifrado de punta a punta: sólo ve el contenido quien tiene acceso. La nube sirve primero como un transporte más de esa carpeta, y después para guardar derivados y artefactos y, si se quiere, pagar modelos con el saldo de la cuenta (F2 y F4).
- **Procesar en otra máquina es un paso aparte.** Por ejemplo, que nove en la M1 le pida a la M4 que procese una tanda de Atlas. Es lo que más riesgo tiene (ejecuta trabajo pedido desde fuera) y no hace falta para dejar de confundirse (F3).
- **Dónde va cada cosa.** Lo que es protocolo local (identidad de disco y de dispositivo, formato de intercambio, emparejado) va en `nu9ve/core` y puede abrirse algún día. Lo que es cuenta, nube y cobro va en un **repo privado nuevo**, no en carpetas de `nu9ve/core`. El servidor va en el API de Fractos, bajo su marca.

## 2. Dónde estamos (revisión del 2026-11-22)

### 2.1 Lo que ya funciona y sirve aquí

| Pieza | Estado | Qué aporta a este diseño |
| --- | --- | --- |
| [`nu9ve-json-v1`](../contracts/nu9ve-json-v1.md) y `rust/nu9ve-json` | En uso (nüün, Clipper, SoundBender) | Bytes canónicos y digest: lo que se firma en la carpeta de intercambio |
| [`nu9ve-fs-v1`](../contracts/nu9ve-fs-v1.md) y `rust/nu9ve-fs` | En uso (nüün, Clipper, curado) | Escritura atómica de cada archivo de intercambio |
| [`nu9ve-engine-v1`](../contracts/nu9ve-engine-v1.md), `rust/nu9ve-engine`, `swift/Nu9veEngine` | Clipper y nüün pasan los ejemplos; cliente Swift desde hoy | El protocolo con el que el agente pregunta a cada motor |
| [`nu9ve-registry-v1`](../contracts/nu9ve-registry-v1.md) y `rust/nu9ve-ipc` | Sólo nove, sólo en Mac | Qué motores están vivos **en esta máquina** |
| [`nu9ve-data-v1`](../contracts/nu9ve-data-v1.md) | Sólo nove lo sirve | Qué datos tiene cada app, con raíces, estado y clase de privacidad. Es casi todo lo que una máquina necesita contarle a otra |
| [`nu9ve-artifact-v1`](../contracts/nu9ve-artifact-v1.md) y `rust/nu9ve-artifact` | nove produce, Clipper consume | La unidad natural de «dato procesado clave» que se comparte: datos exactos, render, receta y linaje |
| `nu9ve mcp` ([diseño](suite-mcp.md)) | Primer corte, probado con motores falsos y con nove aislado; sin cliente real | La puerta donde aparecen las capacidades nuevas |
| [`nu9ve-bench-v1`](../contracts/nu9ve-bench-v1.md) | Clipper escribe | Ya compara máquinas por modelo y chip, sin nombre ni serial (§9 de ese contrato) |
| Snapshot del catálogo de nüün en el disco | MEDIA502 ya guarda uno con su recibo; se recuperó en sólo lectura sin la SD ([STATUS de nüün](https://github.com/nu9ve/n-creator-utils/blob/master/docs/STATUS.md)) | Un disco que ya lleva su propio índice |
| Fractos: MCP público de sólo lectura (`/mcp`, siete tools) | Validado en staging el 2026-09-01 | Ya existe un MCP de la cuenta, del lado de la nube |
| Fractos: pagos con Stripe y Mercado Pago, suscripciones, créditos y ledger | En la web | El método de pago de la cuenta |

### 2.2 Lo que está a medias y bloquea o toca este diseño

| Pendiente | Dónde | Por qué importa aquí |
| --- | --- | --- |
| **Identidad portable de un disco:** decidida el 2026-11-22 en esta propuesta (§6.2); falta el contrato (A3, brief S6) | [Auditoría](audits/fable-audit-2026-11-10.md#a3--la-identidad-de-un-disco-no-viaja-entre-equipos-ni-entre-sistemas) | Es la base de todo. `nu9ve-volumes` no se muda hasta decidirla ([plan, Ola 2](suite-adoption.md#ola-2--con-dos-usuarios-a-la-vista)) |
| **Quién instala el binario `nu9ve`** | [MCP §9, decisión 4](suite-mcp.md#9-decisiones-abiertas) | El agente de este diseño sería ese mismo binario. La decisión pasa de «al primer usuario real» a bloqueante para F1 |
| nüün y Clipper sólo hablan por stdio | [Adopción, registro](adoption.md#nu9ve-registry-v1) | Sin un socket no se registran, y el agente no puede pedirles su catálogo |
| `nu9ve-ipc` sin Windows | [MCP §10](suite-mcp.md#10-primer-corte-2026-11-19) | El PC no aparece en nada de esto hasta tenerlo |
| `nu9ve mcp` sin cliente real | [Plan, fila de nove](suite-adoption.md#cada-app) | Antes de sumarle herramientas, probar las que tiene |
| Licencias: ningún repo tiene `LICENSE`; el «inner core» no tiene nombre ni repo | [Licencias §7](licensing.md#7-preguntas-para-el-usuario) | Este diseño es el primer contenido real del inner core |
| Fractos: servidor OAuth sin elegir (Hydra, el de Supabase en beta o propio) | Web, `docs/domains/auth/architecture.md` | El login de escritorio con scopes propios depende de esto |
| Fractos: cuatro cierres de seguridad antes de una API delegada (altas con `role` del cliente, sesiones de servidor compartidas, autorización, ciclo de vida) | Mismo documento, «Cierres previos» | Ningún dispositivo debería autenticarse contra una frontera que todavía tiene esos huecos |
| Fractos Mac: login sin cuenta real | `ios-fractos` `docs/MONOREPO.md` (P0) | Es el único cliente de escritorio que ya inicia sesión |
| nüün: «multi-host catalogs» marcado como entrega aparte | `n-creator-utils` `docs/release.md` | El caso MEDIA501 en dos Macs ya es real |
| Atlas: raíces con ruta fija (`/Volumes/ESSD/001_proyectos/...` en `config/atlas.json`) | Atlas | Si el disco se monta con otro nombre u otra Mac lo tiene, Atlas no lo encuentra |
| nove: las revisiones reales de Atlas están «en el XSSD de la M4»; S4b espera ese disco | nove `docs/decisions.md`, `docs/status-history.md` | Es el caso exacto del problema |
| curado: su `check.yml` corre en cada push y no está en Git | [CI sólo a mano](suite-adoption.md#ci-sólo-a-mano) | Ajeno a este diseño; se reporta, no se arregla aquí |

### 2.3 Las máquinas y los discos, según los repos

| Máquina o disco | Qué se sabe | Fuente |
| --- | --- | --- |
| M1 (Apple M1, macOS 26.5) | nove y el core; gates de nove medidos aquí; datos reales de nove | nove `docs/STATUS.md` |
| M4 | Gates anteriores de nove; Atlas procesa aquí; XSSD con las revisiones de Atlas | nove `docs/STATUS.md`, `docs/decisions.md` |
| PC con Windows | nove 0.12 (datos 27 frente a 33); nüün en maqueta; curado compilado | [Mapa](suite.md) |
| ESSD (HFS+) | Disco de trabajo: proyectos (`e64_raw`, `e64`), raíz de imágenes y derivados de Atlas. Windows no lo lee | nüün `docs/CONTEXT.md`, [auditoría A5](audits/fable-audit-2026-11-10.md#a5--nombres-fechas-y-durabilidad-dependen-del-sistema-de-archivos) |
| MEDIA500, MEDIA501, MEDIA502 | Archivo. MEDIA502 es exFAT (intercambiable con Windows) y lleva un snapshot del catálogo | nüün `docs/STATUS.md` |
| SSD1 | Carpeta viva de música, al 99 % | nüün `docs/CONTEXT.md` |
| XSSD | Revisiones de Atlas, conectado a la M4 | nove `docs/decisions.md` |

No consta cuál es el sistema de archivos de MEDIA500, MEDIA501, XSSD ni SSD1. Lo confirma el inventario de F0.

### 2.4 Reglas vigentes que este diseño tiene que respetar o cambiar

- **Catálogo de datos:** «Local. Nunca por red» y «un dataset `personal` nunca sale del equipo» ([data §0 y §6](../contracts/nu9ve-data-v1.md#6-privacidad)). Compartir con **otra máquina propia** es salir del equipo. **Decidido (el usuario, 2026-11-22, pregunta 3):** lo personal no sale salvo que la persona lo decida (subirlo, llevarlo en un disco, respaldarlo), y una vez decidido puede verse junto desde todas sus máquinas. El formato no cambia, pero la regla sí: el texto de data §6 cambia en el mismo cambio que `nu9ve-exchange-v1` (§6.5), y ahí se decide si es una versión menor o `-v2` ([AGENTS](../AGENTS.md)). Lo que sale por MCP no cambia: de lo personal, sólo su forma.
- **nove:** «nove **nunca escribe** en una raíz en sitio» (nove [modelo §9](../../nove/docs/contracts/data-model-v1.md#9-en-sitio)). El usuario quiere que nove, Clipper y otras apps también puedan escribir en los discos de trabajo (pregunta 10). Con la regla de una carpeta por app (§6.4), nove puede escribir la suya sin tocar lo que lee en sitio. Esa excepción la decide y la escribe la sesión de nove.
- **Registro:** «Un archivo por motor vivo» en una carpeta del usuario; nunca por red. Sigue igual: lo de otra máquina no entra en el registro, va por la carpeta de intercambio.
- **Benchmarks:** sin nombre de equipo, usuario ni serial. Un id de dispositivo aleatorio es compatible, pero sólo si la persona lo pide al compartir.
- **nüün:** «SQLite local por equipo… No mantener una SQLite WAL activa compartida entre varios discos o equipos» (nüün `docs/media-architecture.md`). Se mantiene: nunca se abre una base de datos por red.
- **nove:** «cuentas, red, cifrado y sincronización serían un proyecto adicional» (nove `docs/architecture.md`). Es este.
- **Fractos:** «Fractos es la marca y frontera para capacidades consumibles fuera del sistema. `nu9ve` identifica el núcleo… no se expone como el nombre de un protocolo público» (web, `docs/domains/integrations/README.md`). La cuenta, las URLs y la interfaz de la nube dicen Fractos. El usuario lo dijo así (2026-11-22, pregunta 19): todo es parte del mismo universo. Fractos es el «citybender», y estas apps son sus herramientas de creador, que más adelante podrían disparar logros en Fractos (§6.12).

## 3. Los dos casos, hoy y con esta propuesta

**Atlas en la M4, análisis en la M1.**

| | Hoy | F0 | F1 | F3 |
| --- | --- | --- | --- | --- |
| ¿Dónde están las fotos y los derivados? | En la memoria del usuario. `config/atlas.json` dice `/Volumes/ESSD/…` | El ESSD y el XSSD tienen identidad. Atlas apunta a «volumen X, carpeta Y» | nove en la M1 ve: «tanda 12, terminada en la M4 hace 2 h, derivados en XSSD (conectado a la M4)» | Igual |
| ¿Cómo llega el resultado a nove? | Mover el disco o copiar a mano | Mover el disco: nove lo reconoce aunque se monte con otro nombre | Además, lo pequeño (baches, tramos, artefactos) llega por la carpeta de intercambio | nove pide a la M4 la siguiente tanda y recibe el artefacto |

**MEDIA501 en la otra Mac.** El recorrido que describe el usuario es éste: ir a la M4, conectar MEDIA y un disco de trabajo, respaldar y dejar la M4 con ese disco lista para un proyecto; después volver con MEDIA a la M1, seguir con otros proyectos y que todo quede al día.

| | Hoy | F0 | F1 |
| --- | --- | --- | --- |
| ¿Qué sabe nüün en la otra Mac? | Su propio catálogo SQLite, sin la historia de la primera | Lee el índice que el disco lleva consigo: recibos, manifiestos y snapshot del catálogo («adoptar este disco») | Además, ve las decisiones de la otra Mac (proyectos, asignaciones, cierres) y las junta con las suyas |
| ¿Puede seguir ingresando? | Sí, pero con dos catálogos que divergen | Sí: el disco dice qué tiene. Las decisiones de cada Mac todavía no se juntan | Sí, en las dos (decidido, pregunta 11). Lo que se suma se une solo; lo que choca se pregunta (§6.4) |
| ¿Y el disco de trabajo que se quedó en la M4? | Nadie lo sabe fuera de la M4 | Lleva su identidad y su índice: si vuelve a la M1, se reconoce | La M1 ve que está en la M4, con qué proyecto y desde cuándo |

## 4. Principios

1. **Local primero.** Sin cuenta, sin red y sin nube, todo sigue como hoy. Cada fase suma algo y ninguna lo quita.
2. **Un escritor por carpeta.** Cada máquina escribe sólo su carpeta de intercambio, y en un disco de trabajo cada app escribe sólo la suya. Cada app sigue siendo dueña de sus datos ([data §0](../contracts/nu9ve-data-v1.md#0-principios)), y nüün sigue siendo el único que escribe en un disco de archivo (MEDIA). Lo que dos máquinas podrían editar a la vez tiene una dueña por proyecto (§6.4). Lo que no se puede evitar se junta con las reglas de cada app, y si choca se le pregunta a la persona.
3. **Los bytes pesados se quedan en los discos.** Viaja la descripción (catálogo, estado, avistamientos de discos) y lo pequeño y ya procesado (artefactos, tablas, GeoJSON). Los raw y los derivados grandes viajan con el disco o con nüün, que es quien copia y verifica.
4. **Lo que viene de otra máquina dice de cuándo es.** «Visto en la M4 hace 2 h», nunca como si fuera de ahora. Es la regla de [data §5](../contracts/nu9ve-data-v1.md#5-agregar-catálogos) aplicada a máquinas.
5. **No se inventa criptografía ni sincronización.** Firmas y cifrado con bibliotecas estándar. OAuth con un motor existente. Nada de CRDT ni de sincronización con varios escritores mientras se pueda evitar.
6. **Las credenciales viven en un solo proceso por máquina.** Las apps no hablan con la nube: le piden al agente local (§6.6). Así la sesión de la cuenta está en un solo llavero y se revoca en un solo lugar.

## 5. Modelo

```text
Cuenta (Fractos, quien paga)
 ├─ Perfiles (identidades creativas: e64…; locales, vínculo opcional con el perfil público de Fractos)
 ├─ Dispositivos (M1, M4, PC: cada uno con su id y su clave)
 │    └─ Avistamientos de volúmenes (qué disco vio, cuándo, montado ahora o no)
 └─ Volúmenes (ESSD, XSSD, MEDIA501…: identidad que viaja con el disco)
      └─ Raíces (volumen + carpeta) ← datasets de cada app (nu9ve-data)
```

| Concepto | Identidad | Dónde vive | Sin cuenta |
| --- | --- | --- | --- |
| **Cuenta** | El principal estable de Fractos (UUID de `profiles`, según su documento de auth) | Fractos | No existe; todo funciona igual |
| **Perfil** | La clave estable de nüün (`e64`, alias `erikiado64`) | En cada app que lo usa; vínculo con la cuenta y con el perfil público de Fractos opcional | Igual que hoy |
| **Dispositivo** | UUID 4 aleatorio, creado la primera vez; nombre que elige la persona («M4») | `…/nu9ve/device.json` en cada máquina (§6.1) | Existe igual, sin registrar |
| **Volumen** | Marcador en el disco y un segundo factor del sistema (A3) | `.nu9ve/volume.json` en la raíz del disco (§6.2) | Existe igual |
| **Raíz** | `root_id` de [data §3](../contracts/nu9ve-data-v1.md#3-respuesta), que gana un volumen | En cada app | Igual |

Una cuenta paga y agrupa. Un perfil es «a nombre de quién» va el trabajo. Son cosas distintas: la misma persona puede tener dos perfiles creativos, y un perfil no paga.

## 6. Las piezas

### 6.1 Identidad de dispositivo · `nu9ve-device-v1` (contrato nuevo, local)

```json
{"contract":"nu9ve-device/1","device_id":"6f0c…","name":"M4","created_at":"2026-11-22T20:00:00Z","os":"macos","arch":"aarch64"}
```

- La crea el primero que la necesita (el agente o una app) con `nu9ve-fs`, en la carpeta de la suite de cada sistema (la misma raíz que el [registro §1](../contracts/nu9ve-registry-v1.md#1-dónde): `~/Library/Application Support/nu9ve/`, `%LOCALAPPDATA%\nu9ve\`). `NU9VE_DEVICE_FILE` la reemplaza en QA, como `NU9VE_ENGINES_DIR`.
- Sin serial, nombre de equipo ni usuario. El nombre lo elige la persona.
- La clave privada del dispositivo no va en este archivo: va en el llavero del sistema (Keychain, si se puede en el Secure Enclave; en Windows, protegida con DPAPI o TPM). Este archivo guarda sólo la huella de la clave pública.
- Una copia de la máquina (migrar a una Mac nueva) copia el archivo pero no la clave del Secure Enclave. El dispositivo nuevo se detecta por eso y pide un nombre nuevo. Es lo que se quiere: la M4 nueva no es la M4 vieja.

### 6.2 Identidad de volumen · `nu9ve-volume-v1` (contrato nuevo, cierra A3/S6)

Lo propuesto en la [auditoría A3](audits/fable-audit-2026-11-10.md#a3--la-identidad-de-un-disco-no-viaja-entre-equipos-ni-entre-sistemas), generalizado a la suite:

- **Marcador** `.nu9ve/volume.json` en la raíz del disco: `volume_id` (UUID 4), `role` (`archive`, `work` u `other`), etiqueta al registrar, fecha y dispositivo que lo registró.
- **Quién lo escribe.** Lo crea la primera app que registra el disco, siempre con `nu9ve-volumes` y sin reemplazar uno que ya exista (creación exclusiva). Después nadie lo reescribe. El `role` sólo lo cambia nüün, que es quien conoce la política de los discos.
- **Decidido (el usuario, 2026-11-22, pregunta 10):** sí al marcador y al segundo factor, con una precisión: en los discos de trabajo también escriben otras apps (nove, Clipper, Atlas). Cada una escribe en su carpeta (§6.4). En un disco `archive` (MEDIA) sólo escribe nüün.
- **Segundo factor** del sistema (serial del sistema de archivos y tamaño), para que un clon bit a bit no pase por el original. Un formateo lo invalida.
- Funciona igual en Mac y en Windows: el marcador viaja con el disco y el GUID de volumen de Windows deja de ser la identidad.
- El contrato dice qué pasa con un clon (dos discos con el mismo marcador y distinto segundo factor: se pregunta), un disco formateado, un disco sin marcador (se usa como hoy, sin identidad de suite) y un disco de sólo lectura (HFS+ en Windows: se reconoce, no se marca).
- Sustituye el `.ncreator/volume.json` que proponía la auditoría. nüün decide en su sesión si migra o lee los dos.

### 6.3 Raíces que apuntan a un volumen

- [`nu9ve-data-v1`](../contracts/nu9ve-data-v1.md) gana en una versión menor un campo opcional `roots[].volume_id`. Un cliente que no lo conoce lo ignora ([data §4.8](../contracts/nu9ve-data-v1.md#48-lo-demás)), así que no rompe a nadie.
- Cada app resuelve «volumen X, carpeta Y» a la ruta de montaje actual leyendo los marcadores de los discos montados. Si el disco no está, la raíz está `missing`, como ya dice el contrato.
- **Atlas:** `config/atlas.json` pasa de `/Volumes/ESSD/…` a `{volume_id, path}`, con la ruta absoluta como respaldo mientras tanto. Es un cambio en la sesión de Atlas, que ya trabaja aparte.

### 6.4 El disco como mensajero, y una dueña por proyecto

Un disco sólo está montado en una máquina a la vez. Por eso **lo que describe al disco vive en el disco**, sin red y sin conflictos:

- **Una carpeta por app.** En cada disco, `.nu9ve/apps/<app>/index.json`, que escribe sólo esa app: qué proyectos, raíces y datasets tiene ahí, y de qué snapshot sale cada cosa. Cualquier app puede leer los índices de todas.
  - nüün ya deja recibos, manifiestos y un snapshot del catálogo; eso pasa a ser su carpeta.
  - Clipper, Atlas y nove escriben la suya en los discos de trabajo donde guardan cosas.
- **Adoptar este disco en otra Mac** es leer esos índices y, si la persona lo pide, importarlos al catálogo local. Es la segunda parte de A3.
- **Nunca** se abre una base de datos SQLite por una carpeta compartida en red (SMB, AFP) ni por una carpeta sincronizada. Se copia un snapshot (`VACUUM INTO`) y se lee la copia. Abrir el SQLite de un disco **conectado a esta Mac** sí se puede, como hoy hace Atlas con su `atlas_state.sqlite3`.

**¿Hace falta una Mac principal? No.** El usuario lo preguntó al responder la pregunta 11, y la respuesta es que basta con que cada cosa tenga **una sola dueña a la vez**. Casi siempre ya la tiene:

| Qué | Su dueña | Cómo cambia |
| --- | --- | --- |
| Bytes y recibos de un disco | El disco, o sea la Mac que lo tiene conectado | Desconectarlo y conectarlo en otra |
| Copia de trabajo de un proyecto en un disco de trabajo, con el estado que la app guarda ahí | El disco de trabajo | Igual: llevarse el disco es llevarse el proyecto |
| Estado de un proyecto que sólo vive en una máquina (la biblioteca de nove, o algo de Atlas en el disco interno) | Una máquina por proyecto | «Trabajar aquí» en la app. La otra queda en sólo lectura hasta que se devuelve. Se anota en `leases.json` (§6.5) |
| Datos procesados (misiones, tandas de baches, artefactos) | La máquina que los produjo | No cambia. Las demás los ven y los traen cuando quieren (§6.5) |
| Decisiones de nüün (proyectos, nombres, asignaciones, cierres, publicaciones) | Cada Mac escribe las suyas y nüün las junta | Ver abajo |

Una dueña por proyecto alcanza porque un proyecto se trabaja en una máquina a la vez. Un «flujo» (ingreso, respaldo) no necesita dueña, porque escribe en discos y el disco ya la tiene.

#### nüün en dos Macs

Decidido (el usuario, 2026-11-22, pregunta 11): **las dos Macs pueden ingresar.** Cómo:

- **El ingreso no choca.** Copias, recibos y verificaciones quedan en el disco destino, y sólo una Mac lo tiene conectado.
- **Las decisiones son eventos.** Crear un proyecto, renombrarlo, asignar un asset, cerrar o publicar se anota en un diario de cada Mac, `devices/<id>/nuun/decisions.ndjson`, dentro de la carpeta de intercambio y con el contador de esa Mac. nüün junta los diarios de todas:
  - Lo que se suma (asignar un asset, añadir un recibo, crear un proyecto con una clave nueva) se une sin preguntar.
  - Dos Macs que crean la misma clave de proyecto hablan del mismo proyecto ([data §2](../contracts/nu9ve-data-v1.md#2-clave-de-proyecto-común)).
  - Dos cambios distintos al mismo campo (dos títulos para el mismo proyecto) son un conflicto: nüün lo muestra y la persona elige.
- **Lo destructivo se decide con el disco, no con lo sincronizado.** Liberar espacio, limpiar una SD o borrar un raw exige, como hoy, los recibos verificados de los discos conectados. Un diario atrasado nunca autoriza borrar.
- **El catálogo SQLite de cada Mac pasa a ser una caché.** Se reconstruye con los índices de los discos y los diarios de las Macs, y sigue siendo local, con un solo escritor. El modelo de nüün ya tiene leases y dependencias (`docs/media-architecture.md`); esto las extiende a varias máquinas.

Es la parte más trabajosa de la propuesta, y le toca a nüün. Por eso entra en dos pasos: primero «adoptar este disco» (F0) y después los diarios (F1).

#### Una app en dos máquinas

Decidido (el usuario, 2026-11-22, pregunta 20): dos apps no escriben la misma carpeta. Lo que sí pasa es **la misma app en dos máquinas, en momentos distintos**:

- Normalmente cada disco de trabajo está conectado a una computadora distinta.
- A veces es más cómodo conectar el disco a la otra computadora que copiar por red, porque es más rápido.
- Con Clipper, la idea es configurar un proyecto en una máquina y exportarlo o procesar su cola en la otra, llevando el disco o copiando por red.

Las reglas:

- **Cada app escribe en su carpeta, y dentro de ella escribe una máquina a la vez:** la que tiene el disco conectado o, si el proyecto se copió por red, la que tiene su lease.
- **Llevar un proyecto a otra máquina** es mover el disco (la dueña va con él) o copiar el proyecto **con su lease**. La copia vieja queda en sólo lectura hasta que el proyecto vuelve. Copiar sin pasar la lease deja dos copias que se sabe que divergen, y la app lo muestra.
- **Un proyecto portable lleva sus dependencias,** no sólo su archivo de proyecto, como ya dice nüün («Proyectos portables requieren incluir sus dependencias, no sólo `clipper-project.json`», `docs/media-architecture.md`). La copia la hace nüün, que verifica.
- **Es la etapa siguiente** (F3a, §10), pero el diseño ya va hacia allá: la carpeta por app y la lease por proyecto de F0 y F1 son lo que lo permite.

### 6.5 Carpeta de intercambio · `nu9ve-exchange-v1` (contrato nuevo)

El centro de F1. Es un formato de archivos, no un servicio:

```text
<intercambio>/
└─ devices/
   ├─ 6f0c…/                 sólo la escribe la M4
   │  ├─ device.json         nombre, clave pública, versión del agente
   │  ├─ volumes.json        avistamientos: volumen, cuándo, montado, espacio libre
   │  ├─ catalog/nove.json   el data_catalog de cada motor vivo, tal cual
   │  ├─ catalog/nuun.json
   │  ├─ leases.json         de qué es dueña esta máquina ahora (§6.4)
   │  ├─ nuun/decisions.ndjson  el diario de decisiones de nüün en esta Mac (§6.4)
   │  ├─ requests.json       lo que esta máquina pide a las demás
   │  └─ objects/sha256/…    lo pequeño que comparte: artefactos, tablas
   └─ 9a1e…/                 sólo la escribe la M1
```

- **Un escritor por carpeta.** Cada máquina escribe sólo `devices/<su id>/`. Así ningún transporte genera conflictos y cualquier transporte sirve.
- **Firmado.** Cada archivo lleva una firma de la clave del dispositivo sobre sus bytes canónicos ([`nu9ve-json-v1`](../contracts/nu9ve-json-v1.md)). Quien lee descarta lo que no firma un dispositivo emparejado (§6.7).
- **Orden por contador, no por reloj.** Cada archivo lleva un número de secuencia del dispositivo que sólo sube. Los relojes de dos máquinas no coinciden. La hora queda sólo para mostrarla.
- **Cifrado,** según lo decidido en la pregunta 4 (§6.9): obligatorio cuando el transporte sale de tus máquinas (iCloud, la nube propia). Entre tus máquinas, por Syncthing o un disco, conviene cifrar también, para no tener dos formatos.
- **Transportes**, cualquiera que copie carpetas:
  - Syncthing (programa aparte, MPL-2.0; no se enlaza, así que su licencia no toca a las apps);
  - iCloud Drive (cuidado: descarga a pedido y puede quitar archivos locales);
  - un disco;
  - más tarde, la nube propia (§6.9);
  - por LAN directa entre agentes, si un día hace falta.
- **Tope por archivo** (por ejemplo 16 MiB) y cuota por carpeta. Los bytes grandes no pasan por aquí (principio 3).
- **Traer bajo pedido.** Es la respuesta a «tener disponibles en la M1 los baches procesados en la M4, o al menos poder traerlos»:
  - Lo pequeño de un proyecto que la persona marca como «disponible en todas mis máquinas» se publica solo, hasta un tope.
  - Lo demás se pide. La M1 anota el pedido en su `requests.json`; el agente de la M4 lo ve y publica el objeto en su propia carpeta.
  - Funciona con cualquier transporte, aunque tarde lo que tarde el transporte, y sigue la regla de un escritor por carpeta.
  - Si lo pedido es grande (las fotos de una tanda), el agente no lo publica: dice en qué disco está, para que nüün lo copie o la persona lleve el disco.
- **Identidad entre máquinas.** Al juntar catálogos de varias máquinas, un dataset es `(dispositivo, owner, id)`. El mismo `nove:ds-12` en la M1 y en la M4 son dos datasets, cada uno con su antigüedad.

#### Lo personal y el análisis de todas tus máquinas

Decidido (el usuario, 2026-11-22, pregunta 3):

- **Por omisión, de un dataset `personal` sólo viaja su forma**, como en `nu9ve mcp` ([data §6](../contracts/nu9ve-data-v1.md#6-privacidad)).
- **Pasa entero sólo si la persona lo decide** para esa colección o dataset: «ver en todas mis máquinas», «subir a la nube» o «respaldar en este disco». La decisión dura hasta que la quite, y cada app muestra qué está compartido.
- **El objetivo:** el usuario trabaja en dos o tres computadoras, y todas sus sesiones (Claude Code, Codex, Claude Desktop) son suyas. Quiere verlas juntas: qué proyectos, en qué máquinas, y analizarlas en conjunto. Para él, todo eso es un solo proyecto, su análisis personal, que más adelante puede dividirse por proyecto.
  - En nove, cada máquina aporta un dataset por colección. nove los muestra juntos bajo una sola clave de proyecto y conserva de qué máquina viene cada uno. **Decidido (pregunta 18):** un valor por omisión común, porque es un flujo que la suite sugiere a todos, y que quede listo para el caso del usuario. La clave es `conversaciones`, con el título «Mis conversaciones» (en inglés, «My conversations»). La clave no se traduce: es un id. El título sí, y la persona lo puede cambiar.
  - Es la primera **clave conocida** de la suite. `nu9ve-data` gana una lista corta de claves reservadas (en una versión menor, porque sólo suma). Así nüün y nove ofrecen la misma clave sin que una app la invente para otra ([data §2](../contracts/nu9ve-data-v1.md#2-clave-de-proyecto-común) pide que la elija la persona: aquí la persona la acepta al compartir por primera vez una colección).
  - Se distingue de la modalidad `conversations`: la clave es el proyecto, la modalidad es el tipo de dato.
  - En los respaldos de nüün, cada colección guarda una carpeta por dispositivo (`<colección>/<device_id>/…`). Así un MEDIA reúne los snapshots de todas las máquinas sin mezclarlos, y nove puede analizar el respaldo, la copia de trabajo o las dos.
- **Por MCP sigue saliendo sólo la forma**, también de lo que llega de otras máquinas: lo que pasa por MCP llega al proveedor del modelo.

Ventaja: F1 se puede probar sin escribir ni una línea de red, con Syncthing entre la M1 y la M4.

### 6.6 Agente de la suite · `nu9ve agent`

Un proceso por máquina y por usuario: el mismo binario `nu9ve` de `nu9ve mcp`, como LaunchAgent en Mac y tarea de inicio de sesión en Windows. Hace lo que ninguna app debería hacer sola:

- crea y guarda la identidad del dispositivo y su clave;
- cuando cambia algo, pide `data_catalog` a los motores vivos del registro y escribe la carpeta de esta máquina;
- vigila qué discos se montan y escribe los avistamientos (lee marcadores, no los escribe);
- lee las carpetas de las otras máquinas y verifica sus firmas;
- más adelante, guarda la sesión de la cuenta y habla con la nube (F2).

**Se registra como un motor más** ([registro](../contracts/nu9ve-registry-v1.md)), con capacidades `devices_v1` y `mcp_tools_v1`. Así `nu9ve mcp` ve sus herramientas sin código especial (§7), y nove puede mostrar «otras máquinas» pidiéndoselas por el mismo protocolo.

Esto vuelve urgente la [decisión 4 del MCP](suite-mcp.md#9-decisiones-abiertas): un agente que corre siempre se instala aparte, no dentro del bundle de una app. Propuesta: un instalador propio pequeño, o nüün como primera app que lo trae, porque es la que vive en la barra de menús y la que conoce los discos (pregunta 9).

### 6.7 Emparejar dispositivos

- La primera vez, cada agente muestra la huella de su clave pública como seis palabras o un QR. La persona confirma en las dos máquinas que coinciden. Es lo que hacen Signal y Syncthing.
- La lista de dispositivos emparejados vive en cada máquina. Quitar uno deja de aceptar sus archivos desde ese momento.
- Con cuenta (F2), emparejar es «este dispositivo está en mi cuenta», y la revocación llega a todos desde Fractos.

### 6.8 Cuenta y registro de dispositivos (F2, Fractos)

Lo que ya propone la web (`docs/domains/integrations/README.md`, «API delegada y MCP autenticado») encaja:

- El escritorio es un **cliente público**: Authorization Code con PKCE en el navegador del sistema (`ASWebAuthenticationSession` en Mac, redirección a loopback en Windows), sin secreto embebido y nunca un WebView para la contraseña ([RFC 8252](https://www.rfc-editor.org/rfc/rfc8252)). Fractos Mac contempla lo mismo como alternativa a su formulario actual, con regreso por `fractos://auth` (`ios-fractos` `docs/SUITE-DESKTOP.md`).
- **Scopes de la suite**, con el formato de los que propone Fractos: `devices:manage`, `sync:read`, `sync:write`, `storage:read`, `storage:write`, `models:invoke`. Los decide Fractos cuando elija su servidor OAuth; el de Supabase, según su documento, no admite scopes propios.
- **Registro del dispositivo:** tras el login, el agente envía su clave pública y su nombre. Cada petición posterior se firma con esa clave ([DPoP, RFC 9449](https://www.rfc-editor.org/rfc/rfc9449)), así que un token robado sin la clave no sirve.
- **Revocar** un dispositivo desde «Conexiones» en los ajustes de Fractos, la pantalla que su documento ya propone (último uso, revocar, sin secretos).
- **Decidido (el usuario, 2026-11-22, pregunta 6): OAuth es la ruta, sin atajo.** La suite se diseña contra el OAuth de Fractos desde el principio. No se inicia sesión con Supabase directo, aunque las apps sean de primera parte. El usuario avanza el servidor OAuth en el repo de Fractos con sus sesiones. Mientras tanto, F0 y F1 no necesitan cuenta, así que nada espera.

#### Lo que la suite le pide al OAuth de Fractos

Para pasarlo a una sesión del repo de la web. Sirve con cualquier motor (Hydra u otro) que cumpla estos puntos:

1. **Descubrimiento:** metadatos del servidor de autorización ([RFC 8414](https://www.rfc-editor.org/rfc/rfc8414)) y del recurso protegido ([RFC 9728](https://www.rfc-editor.org/rfc/rfc9728)), que es lo que pide la autorización de MCP. Un `issuer` estable, por ejemplo `auth.fractos.app`.
2. **Clientes nativos públicos:** Authorization Code con PKCE S256, redirección a loopback y a un esquema propio ([RFC 8252](https://www.rfc-editor.org/rfc/rfc8252)), sin secreto. El escritorio de la suite como cliente de primera parte **registrado**, no de autoservicio.
3. **Scopes propios:** `devices:manage`, `sync:read`, `sync:write`, `storage:read`, `storage:write`, `models:invoke` y, para los logros, `activity:write` (§6.12). Según el documento de auth de la web, el servidor OAuth de Supabase no admite scopes propios: este punto lo descarta.
4. **Tokens ligados al dispositivo:** DPoP ([RFC 9449](https://www.rfc-editor.org/rfc/rfc9449)) con la clave del dispositivo, y refresh tokens que rotan y quedan ligados a esa clave.
5. **Audiencias separadas:** el API de la suite y `/mcp/account` son recursos distintos. Un token para uno no sirve en el otro.
6. **Revocación** ([RFC 7009](https://www.rfc-editor.org/rfc/rfc7009)) de un dispositivo o de una conexión, que se cumple aunque el token no haya vencido, como ya pide la web («permiso efectivo»).
7. **Consentimiento** una vez por dispositivo. Añadir scopes pide consentimiento nuevo.
8. **Datos:** una tabla de dispositivos (principal, nombre, huella de la clave pública, alta, último uso, revocado) y la pantalla «Conexiones» en ajustes.
9. **Opcional:** Device Authorization Grant ([RFC 8628](https://www.rfc-editor.org/rfc/rfc8628)) para una máquina sin navegador a mano.
10. **Antes:** los cuatro cierres de seguridad de §2.2.

### 6.9 Qué guarda la nube

| Qué | Cuándo | Cómo |
| --- | --- | --- |
| Lista de dispositivos y sus claves públicas | F2 | Tablas de Fractos, ligadas al principal |
| La carpeta de intercambio | F2 | Un blob por archivo y dispositivo, **cifrado de punta a punta**: el servidor ve tamaños y fechas, no contenido |
| Artefactos y derivados pequeños | F2–F4 | Objetos por SHA-256, con subida directa por URL firmada (las funciones de Vercel no son para archivos grandes) |
| Respaldos en frío de raw | Sólo si la persona lo pide (F4) | Es caro: una tanda de Atlas son 27 GB de DNG. Es una decisión de costo, no técnica |
| Uso y cobro | F4 | El ledger y los créditos que ya tiene Fractos |

**Decidido (el usuario, 2026-11-22, pregunta 4): cifrado de punta a punta por omisión.** Nadie ve el contenido salvo quien tiene acceso:

- **Quién tiene acceso:** tus dispositivos y, cuando compartas algo, las personas o el colectivo de Fractos a quienes se lo des. La llave del contenido se cifra para la clave de cada uno; el contenido no se vuelve a cifrar.
- **Qué ve el servidor:** tamaños, fechas, a qué cuenta pertenece cada cosa y con quién se compartió. Nada más.
- **La excepción es lo que se publica a propósito,** por ejemplo una capa de baches en el mapa de Fractos. Eso es público, y por eso legible.
- **La contrapartida:** un agente en la nube (el MCP de Fractos) no puede leer lo cifrado. Un agente que quiera tus datos los pide a través de uno de tus dispositivos, con las reglas de `nu9ve mcp`.
- **Si se pierden todos los dispositivos,** hace falta una llave de recuperación. **Decidido (pregunta 19): las dos formas,** sin que Fractos pueda leer:
  - **Llave impresa,** que guarda la persona. Funciona aunque Fractos no exista.
  - **Copia en Fractos, protegida con una passkey.** Fractos guarda la llave del contenido cifrada con un secreto que sale de una passkey de la persona (la extensión PRF de WebAuthn), y esa passkey vive en su llavero o su gestor de contraseñas. Fractos tiene el cifrado pero no la passkey, así que no puede abrirlo, ni puede alguien que entre a su base de datos. Recuperar es iniciar sesión en Fractos y usar la passkey en un dispositivo nuevo.
  - **Lo que no se hace:** guardar en Fractos una llave que se abra sólo con el login. Eso querría decir que Fractos, o quien robe su base de datos, lee todo, y el cifrado de punta a punta dejaría de valer. Ésa es la parte insegura de que «Fractos sepa».
  - **Por verificar:** el soporte de PRF en Safari, Chrome, Windows y los gestores de contraseñas que usa la persona, antes de escribir el contrato.

### 6.10 Trabajo en otra máquina (F3)

- nove (u otra app) pide al agente de otra máquina un **trabajo de una lista cerrada**: «procesa la tanda N de la misión M con la receta R». Nunca un comando libre.
- El canal es directo entre agentes emparejados, por TLS con las claves de dispositivo (en la LAN o sobre Tailscale fuera de casa). La nube sólo avisa y nunca transporta las fotos.
- El resultado vuelve como artefacto (`nu9ve-artifact-v1`) por la carpeta de intercambio.
- Se apoya en `nu9ve-jobs` ([plan, Ola 3](suite-adoption.md#ola-3--con-su-condición)), que todavía no existe.
- Nivel `act` en el MCP, y la máquina que ejecuta decide su propia política (pedir confirmación en pantalla, o permitir siempre ciertas recetas).

### 6.11 Modelos y cobro (F4)

Dos caminos, que no se excluyen:

- **Tu propia clave del proveedor (BYOK)**, guardada por el agente en el llavero. No cuesta nada en la nube y basta para una sola persona.
- **Pasarela con el saldo de la cuenta:** la nube llama al proveedor y descuenta créditos de Fractos. Sólo tiene sentido cuando otras personas usen la suite. Hay que revisar los términos de cada proveedor sobre revender acceso, qué datos se le mandan (lo `personal` nunca) y los impuestos, que Fractos ya trata en sus pagos.

### 6.12 Logros en Fractos

Las herramientas de la suite podrían disparar logros en Fractos: «primer respaldo verificado», «primera misión de dron procesada», «primer video publicado».

- **Qué se manda:** el agente manda un **evento de actividad** con tipo, fecha y dispositivo. Nunca contenido, rutas ni nombres. Es una salida explícita, así que no entra en el cifrado de punta a punta: la persona la activa una vez, por tipo de evento.
- **Por qué no basta con mandarlo:** Fractos ya cerró los desbloqueos que reporta el cliente. Las rutas libres de unlock, progreso y eventos responden 403, y «no hay grants locales» (web, `docs/domains/gamification/logros/sistema-logros-documentacion.md`, contrato del 2026-10-18). Por eso un evento de la suite sólo puede:
  - entrar como **logro cosmético y sin puntos**, como hoy el logo o el Konami; o
  - validarse contra algo que el servidor sí puede comprobar: un artefacto subido, una publicación en Fractos, un dispositivo registrado.
  - Lo que dé puntos, XP o dinero nunca se apoya sólo en lo que dice la máquina de la persona, porque eso se puede falsificar.
- **Cuándo:** con la cuenta (F2), con el scope `activity:write`. El catálogo de logros lo decide Fractos.

## 7. MCP: qué se habilita y cuándo

Todo sale del agente como motor del registro (§6.6), así que el servidor sigue sin conocer apps ([MCP §1](suite-mcp.md#1-qué-es)).

| Herramienta | Nivel | Fase | Qué hace |
| --- | --- | --- | --- |
| `suite_devices` | Leer | F1 | Dispositivos emparejados, último contacto y apps vivas en cada uno |
| `suite_volumes` | Leer | F0–F1 | Discos conocidos, dónde se vieron por última vez, si están montados aquí |
| `suite_where` | Leer | F1 | Dónde está un proyecto, dataset o artefacto: máquina, disco, estado y antigüedad |
| `suite_leases` | Leer | F1 | Qué máquina es dueña de qué ahora |
| `suite_lease_take` | Editar | F1 | Pasar a esta máquina la dueña de algo. Deshacible devolviéndola |
| `suite_pull` | Editar | F1 | Pedir a otra máquina un objeto pequeño (§6.5). Deshacible borrando la copia local |
| `cloud_status` | Leer | F2 | Cuenta, plan, cuota, último envío. Aparece sólo con sesión |
| `suite_artifact_share` | Actuar | F2 | Subir un artefacto a la nube. Lo `personal` nunca |
| `suite_job_submit` | Actuar | F3 | Pedir un trabajo de la lista a otra máquina |

- **Sin cuenta,** las de F0 y F1 funcionan igual. «Capacidades adicionales con la nube activa» quiere decir que las de F2 en adelante aparecen cuando el agente tiene sesión, y desaparecen al cerrarla (`list_changed`, [MCP §4](suite-mcp.md#4-herramientas-de-cada-app)).
- **El MCP de Fractos** (`/mcp` público, y el `/mcp/account` propuesto) es otra puerta, del lado de la nube. Puede listar dispositivos y uso, pero no lo cifrado. No se mezcla con `nu9ve mcp`: uno ve tus máquinas y el otro, tu cuenta.
- Todo texto que llega de otra máquina es `_untrusted`, como el de cualquier motor. Otra máquina tuya puede tener datos con instrucciones escondidas.

## 8. Privacidad y seguridad

| Riesgo | Qué lo contiene |
| --- | --- |
| Un archivo de intercambio falso o modificado | Firma del dispositivo; sólo cuentan los emparejados |
| Un portátil perdido | Revocar el dispositivo (lista local en F1, cuenta en F2); su clave no sale del Secure Enclave |
| Un token de la cuenta robado | DPoP: sin la clave del dispositivo no sirve |
| La nube lee lo sincronizado | Cifrado de punta a punta; el servidor ve tamaños y fechas |
| Lo `personal` llega al proveedor del modelo | La regla de [data §6](../contracts/nu9ve-data-v1.md#6-privacidad) sigue: por MCP sólo su forma, también lo de otras máquinas |
| Un agente pide a otra máquina algo destructivo | Trabajos de una lista cerrada, nivel `act`, política de la máquina que ejecuta |
| Instrucciones escondidas en datos sincronizados | `_untrusted` en todo lo que viene de otra máquina |
| Datos de otras personas en la nube (fotos de la calle, conversaciones) | Fuera del alcance hasta que la suite sea de más de una persona. Entonces: aviso de privacidad, LFPDPPP en México y GDPR si hay usuarios en Europa (pregunta 1) |

## 9. Público y privado: qué va dónde

| Pieza | Repo | Por qué | Licencia, si un día se abre |
| --- | --- | --- | --- |
| `nu9ve-device-v1`, `nu9ve-volume-v1`, `nu9ve-exchange-v1` y la versión menor de `nu9ve-data-v1` | `nu9ve/core` | Son protocolo local: no diferencian ni cobran nada | MIT o Apache-2.0, como el resto ([licencias §3](licensing.md#3-tres-capas)) |
| `nu9ve-volumes` (con el marcador) | `nu9ve/core`, desde nüün | Ya planeado en la Ola 2; esta decisión lo desbloquea | Igual |
| `nu9ve agent` sin nube: identidad, emparejado, carpeta de intercambio, avistamientos | `nu9ve/core` (crates `nu9ve-device`, `nu9ve-exchange`, dentro del binario `nu9ve`) | Sirve sin cuenta. Si un día se abre, la seguridad está en las claves, no en esconder el código | Igual |
| Cliente de la nube: sesión, registro, envío cifrado, cuota | **Repo privado nuevo** (crate `nu9ve-cloud`) | Es negocio y toca la cuenta | Cerrado |
| Servidor: dispositivos, blobs, cuota, cobro, pasarela de modelos | **Web de Fractos** (`packages/api`), bajo su marca | Misma cuenta, mismos pagos, mismo OAuth | Cerrado |
| Contrato HTTP de la nube | Generado en la web, como `fractos-public-v1.openapi.json` | Fractos ya copia su contrato a `ios-fractos` con `sync-contracts.sh`; la suite haría igual | — |

- **Por qué un repo privado nuevo y no carpetas en `nu9ve/core`:** [licencias §6](licensing.md#6-antes-de-hacer-público-nu9vecore) ya avisa que abrir el core exige limpiar su historia. Si lo privado nace en otro repo, no hay que limpiar nada. Nombre por decidir (pregunta 8). Con el prefijo de la organización: `nu9ve/cloud`, o `nu9ve/core-private` si se prefiere que quede claro que es la mitad privada del core.
- **El binario `nu9ve` con y sin nube:** la build con nube incluye `nu9ve-cloud` como feature que la build sin nube no trae. Es la regla de [licencias §5](licensing.md#5-reglas-que-hacen-que-funcione): se puede porque lo público es permisivo.
- **`sync-core.sh` sólo conoce `nu9ve/core`.** Para el repo privado hace falta generalizarlo (otra carpeta `vendor/`, otra lista de rutas, otra búsqueda del checkout), con sus pruebas en `scripts/test-sync-core.sh`. Es un cambio propio, antes de que una app fije el repo privado.
- **Una app que fija el repo privado tiene que ser privada.** Hoy lo son todas. Si SoundBender se abre como open core, recibe la parte de nube ya compilada o no la trae.
- **Dependencias nuevas que harían falta,** todas con licencias permisivas según sus repos (se confirman con `cargo-deny` al añadirlas): `rustls`, `ed25519-dalek`, `age` (cifrado de archivos), `keyring` o `security-framework` (llavero), `mdns-sd` (sólo si hay LAN directa). Del lado del servidor, Ory Hydra (Apache-2.0) si Fractos lo elige.

## 10. Fases

Cada fase cabe en sesiones de su repo, sirve sola y no rompe a nadie.

| Fase | Qué queda | Trabajo, por repo | Condición |
| --- | --- | --- | --- |
| **F0 · Cada cosa sabe quién es** | Un disco se reconoce en cualquier Mac aunque se monte con otro nombre. Atlas y nove encuentran sus raíces por disco. Se puede adoptar un disco en otra Mac | **core:** contratos `nu9ve-device-v1` y `nu9ve-volume-v1` (con Windows en el texto y en los ejemplos, aunque se pruebe después), `roots[].volume_id` en `nu9ve-data`, la carpeta por app en el disco, con ejemplos y validador. **nüün:** marcador, segundo factor, su carpeta en `.nu9ve/apps/`, «adoptar este disco»; mudar `nu9ve-volumes`. **Atlas:** raíces por volumen y su carpeta en los discos de trabajo. **nove:** raíces por volumen, «disco no conectado» y la excepción a su regla de no escribir en sitio. **Clipper:** su carpeta en los discos de trabajo | **Sin bloqueos** (preguntas 10, 11 y 13, decididas el 2026-11-22). Conviene responder la 20 antes del contrato de la carpeta por app |
| **F1 · Mis máquinas se cuentan qué tienen** | En la M1 se ve qué hay en la M4, qué discos tiene y qué hizo Atlas allá, con su antigüedad. Se traen sus baches procesados. Las dos Macs ingresan y sus decisiones se juntan. Las sesiones de agentes de todas las máquinas se ven juntas | **core:** `nu9ve-exchange-v1` (con pedidos e identidad entre máquinas), la nueva regla de lo personal en `nu9ve-data`, `nu9ve agent` (identidad, emparejado, escribir y leer la carpeta, avistamientos, pedidos), herramientas de la §7. **nüün:** diario de decisiones y su unión; respaldos de colecciones por dispositivo. **nüün y Clipper:** un transporte registrable para servir su `data_catalog`. **nove:** «otras máquinas», «disponible en todas mis máquinas» y el proyecto de análisis personal. Transporte: Syncthing | Preguntas 2, 9 y 18; probar antes `nu9ve mcp` con un cliente real |
| **F2 · Cuenta Fractos** | Login con Fractos por OAuth, dispositivos en la cuenta, revocar desde la web, la carpeta de intercambio por la nube, cifrada | **Fractos** (lo lleva el usuario con sus sesiones): los cuatro cierres de auth y el OAuth de la §6.8; tablas de dispositivos, blobs y cuota; pantalla de conexiones; copia de la llave protegida con passkey; eventos de actividad para logros cosméticos. **Repo privado:** `nu9ve-cloud`. **core:** `sync-core.sh` para dos repos | Preguntas 5, 7, 8 y 19; el OAuth de Fractos listo |
| **F3 · Procesar en otra máquina** | **F3a:** llevar un proyecto a otra máquina con su lease, por disco o por red, y seguir ahí (configurar en Clipper en una, exportar en la otra). **F3b:** nove en la M1 lanza una tanda de Atlas en la M4 y recibe el artefacto | **F3a, nüün y Clipper:** copia verificada del proyecto con sus dependencias, y la lease que pasa con ella (§6.4). **F3b, core:** `nu9ve-jobs`, canal entre agentes. **Atlas:** sus etapas como trabajos de lista (ya pedido en nove S9). **Clipper:** export como trabajo, si se quiere | Preguntas 12 y 14 |
| **F4 · Servicios de pago** | Almacenamiento de artefactos y derivados, respaldos opcionales, modelos con saldo, compartir con otras personas o con un colectivo de Fractos | **Fractos:** plan de la suite, medición, pasarela de modelos. **Repo privado:** cliente | Preguntas 1, 15 y 16 |

Para el problema de hoy bastan F0 y F1: dos Macs lado a lado no necesitan nube para dejar de confundirse.

## 11. Qué tener en cuenta

- **El sandbox de macOS.** Una app con sandbox no escribe fuera de su contenedor sin un App Group (mismo Team ID) o sin que la persona elija la carpeta. La carpeta de la suite (`~/Library/Application Support/nu9ve/`) y el socket del agente tienen que poder alcanzarse desde las apps con sandbox. Hay que confirmarlo con lo que ya hace nove para registrarse (pasa `--suite-registry` desde su bundle) antes de fijar dónde viven `device.json` y la carpeta de intercambio. El agente, sin sandbox, puede ser el único que toca esas carpetas.
- **Relojes.** Dos Macs no tienen la misma hora; exFAT y FAT guardan hora local ([A5](audits/fable-audit-2026-11-10.md#a5--nombres-fechas-y-durabilidad-dependen-del-sistema-de-archivos)). Por eso se ordena por contador de dispositivo (§6.5).
- **HFS+ en Windows.** El ESSD no se lee en el PC. El contrato de volumen dice qué hace un sistema que ve un disco pero no puede leer su marcador.
- **iCloud Drive** puede quitar archivos locales para ahorrar espacio y bajarlos tarde. Sirve para la carpeta de intercambio, no para bases de datos ni para nada que se abra en sitio.
- **Costos de la nube.** Almacenamiento y salida de datos (egress) en Supabase o en el proveedor de objetos. Los límites de cuerpo y tiempo de las funciones de Vercel obligan a subir por URL firmada. Hay un repo `presigner` en `~/code/nu9ve` que no se revisó para esta propuesta.
- **Una persona, varios repos.** Cada fase toca de tres a cinco repos. Conviene que cada sesión de app reciba sólo su parte, con la regla de mudar piezas en cambios propios ([AGENTS](../AGENTS.md)).
- **Lo que no se construye:** sincronización con varios escritores (CRDT), un servidor de relevo propio para la LAN (Tailscale ya lo resuelve), un motor OAuth propio (la web ya lo descarta) y criptografía propia.

## 12. Preguntas para el usuario

Las decididas el 2026-11-22 quedan tachadas, con la respuesta del usuario. Ninguna de las abiertas frena F0.

**Alcance**

1. ¿La suite es sólo para ti por un buen tiempo, o pronto habrá otras personas o colectivos de Fractos usándola? Cambia la privacidad, el cobro y si compartir datos procesados con otros entra pronto.
2. Para las dos Macs lado a lado, ¿te basta F0 + F1 (sin nube, con Syncthing o un disco) mientras la cuenta madura? Es la recomendación.

**Datos y privacidad**

3. ~~¿Lo `personal` puede pasar entre tus dispositivos emparejados?~~ **Decidido:** no sale del equipo salvo que la persona lo decida: subirlo a la nube, llevarlo o respaldarlo en un disco, y poder juntar snapshots en el respaldo y en la copia de trabajo. Una vez decidido, quiere ver y analizar juntas las sesiones de todas sus máquinas, como un solo proyecto de análisis personal que más adelante puede dividir por proyecto ([§6.5](#lo-personal-y-el-análisis-de-todas-tus-máquinas)).
4. ~~¿Cifrado de punta a punta o una parte legible por el servidor?~~ **Decidido:** lo más seguro y usable, según el caso; que nadie vea el contenido salvo quien tenga acceso. Cifrado de punta a punta por omisión, acceso para quien lo reciba, y legible sólo lo que se publica a propósito ([§6.9](#69-qué-guarda-la-nube)).

**Cuenta y Fractos**

5. ~~¿La cuenta de la suite es la cuenta de Fractos?~~ **Decidido en parte (con la 19):** sí, es el mismo universo, y las apps son herramientas de creador de Fractos. **Sigue abierto:** ¿los perfiles de nüün (`e64`) se vinculan con el perfil público de Fractos o quedan aparte?
6. ~~¿Esperar el OAuth de Fractos o empezar con sesión de Supabase?~~ **Decidido:** documentar OAuth como prioridad y diseñar la suite para funcionar así. El usuario lo avanza en el repo de Fractos con sus sesiones; lo que la suite le pide está en la [§6.8](#lo-que-la-suite-le-pide-al-oauth-de-fractos).
7. ¿El servidor de la suite vive en el API de Fractos (`packages/api`), como se propone, o en un servicio aparte?
8. ¿Repo privado nuevo para el cliente de la nube? ¿Cómo se llama: `nu9ve/cloud`, `nu9ve/core-private` u otro?

**Máquinas y discos**

9. ¿Quién instala el agente `nu9ve`: un instalador propio, o nüün como primera app que lo trae?
10. ~~¿Marcador `.nu9ve/volume.json`, segundo factor y que sólo nüün lo escriba?~~ **Decidido:** sí, pero en los discos de trabajo también escriben otras apps (nove, Clipper). Queda así: el marcador lo crea la primera app que registra el disco y nadie lo reescribe; el rol lo cambia nüün; cada app escribe en su carpeta; en MEDIA sólo escribe nüün ([§6.2](#62-identidad-de-volumen--nu9ve-volume-v1-contrato-nuevo-cierra-a3s6), [§6.4](#64-el-disco-como-mensajero-y-una-dueña-por-proyecto)).
11. ~~¿Una Mac principal para el catálogo de nüün o dos que ingresan?~~ **Decidido:** las dos ingresan. El usuario lleva MEDIA y un disco de trabajo a la otra Mac, respalda, deja esa Mac lista para un proyecto, vuelve con MEDIA y sigue con otros, todo al día. También quiere ver o traer en la M1 lo procesado en la M4. No hace falta una Mac principal: basta una dueña por cosa, y casi siempre es el disco ([§6.4](#64-el-disco-como-mensajero-y-una-dueña-por-proyecto)).
12. ¿Quieres lanzar trabajo en otra máquina (F3)? ¿Cuáles: tandas de Atlas, ODM, exports de Clipper?
13. ~~¿El PC con Windows entra desde F0?~~ **Decidido:** sí, se considera, y puede haber un tercer PC, pero hoy no está a mano. Los contratos de F0 cubren Windows en su texto y sus ejemplos; las pruebas en Windows esperan a tener la máquina.
14. Fuera de casa, ¿aceptas Tailscale como red entre tus máquinas, o todo pasa por la nube propia?

**Pagos y modelos**

15. Modelos: ¿tu clave del proveedor guardada en el agente (BYOK) para empezar, y la pasarela con saldo sólo cuando haya otras personas?
16. Almacenamiento: ¿la nube guarda sólo artefactos y derivados pequeños, o también respaldos de raw? ¿Con qué tope de gasto?

**Nombres**

17. ¿Los dispositivos se llaman como tú los llamas hoy («M1», «M4», «PC»)?
18. ~~¿Qué clave lleva tu análisis personal de todas las máquinas?~~ **Decidido:** «mis conversaciones» o algo parecido, como valor común por omisión del flujo sugerido, listo para el caso del usuario. Clave `conversaciones`, título «Mis conversaciones» ([§6.5](#lo-personal-y-el-análisis-de-todas-tus-máquinas)).
19. ~~¿Llave impresa o copia en Fractos?~~ **Decidido:** las dos, bien seguras y bien integradas. La copia en Fractos va protegida con una passkey, así que Fractos no puede leerla ([§6.9](#69-qué-guarda-la-nube)). De paso: Fractos es el «citybender», estas apps son sus herramientas de creador, y podrían disparar logros (§6.12).
20. ~~¿Dos apps escriben la misma carpeta?~~ **Decidido:** no. Lo que pasa es la misma app en dos máquinas: configurar en una y exportar o procesar la cola en la otra, llevando el disco (más rápido que la red) o copiando por red. Es la etapa siguiente, pero hacia allá va el diseño ([§6.4](#una-app-en-dos-máquinas), F3a).

## 13. Fuentes leídas

El 2026-11-22: en este repo, `README.md`, `AGENTS.md`, `CHANGELOG.md`, `docs/` (mapa, cómo se comparte, qué se muda, licencias, MCP de la suite, plan y adopción, auditoría A3 y A5) y los contratos de registro, datos y benchmarks. En nüün (`n-creator-utils`, `b14e83b`): `docs/media-architecture.md`, `docs/CONTEXT.md`, `docs/STATUS.md`, `docs/release.md` y `docs/contracts/atlas-drone-v1.md`. En nove (`a03a8e2`): `docs/STATUS.md`, `docs/CONTEXT.md`, `docs/data-hub.md`, `docs/architecture.md`, `docs/decisions.md` y `docs/contracts/data-model-v1.md`. En Atlas (`3734519`): `config/atlas.json`. En la web de Fractos (`nu9ve.xyz`, rama `develop` en `f2b78b0c4`, con cambios sin commitear en sus contratos que no se leyeron): `docs/domains/integrations/README.md`, `docs/domains/auth/architecture.md` y el índice de `docs/domains/payments.md`. En `ios-fractos` (`048b41e`): `docs/SUITE-DESKTOP.md`. No se compiló ni se probó nada.
