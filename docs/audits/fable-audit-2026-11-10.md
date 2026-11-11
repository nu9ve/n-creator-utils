# Auditoría Fable de la suite · 2026-11-10

Estado: **auditoría de lectura**. No cambia código, decisiones ni estados; propone. Lo que aquí se llama «hallazgo» tiene su evidencia al lado; lo que es una idea lo dice. Las prioridades vigentes siguen en [PLAN](../../PLAN.md) y el estado en [STATUS](../STATUS.md).

Documentos que salen de esta auditoría:

| Documento | Para qué |
| --- | --- |
| Este | Hallazgos priorizados, por repo y por tema |
| [Integración con el sistema operativo](../platforms/os-integration.md) | Tabla de equivalencias macOS ↔ Windows ↔ Linux y las funciones del sistema que la suite todavía no usa |
| [Linux en la suite](../platforms/linux.md) | Cómo llevar motor y apps a Linux y a varias distros |
| [Sonido de tema v1](../contracts/nu9ve-sound-v1.md) (propuesta) | Sonidos de interfaz compartidos, generados por tema, con Fractos como fuente |
| [Avisos del sistema v1](../contracts/nu9ve-notifications-v1.md) (propuesta) | Notificaciones del sistema operativo con nove en Windows como base, y lo que falta en Mac |
| [Cómo se comparte entre repos](../suite-sharing.md) (propuesta) | Qué sería el repo común, cómo lo consume cada app y cómo se coordinan las versiones |
| [Briefs de sesión](session-briefs-2026-11-10.md) | Encargos listos para que otro modelo implemente, uno por sesión |
| `ios-fractos`: `docs/SUITE-DESKTOP.md` | Doc inicial de Fractos: lo aprendido en nove, curado y nüün, la app de Mac y el arranque en Windows |

## 0. Alcance y método

- **Leído en esta máquina (Windows 11):** `n-creator-utils` (docs, contratos, `engine/`, `native/windows`, tamaños de `native/MediaMenu`), `nove` (docs, plan, estado, manifiestos, estructura), `curado` (docs, manifiestos, estructura) e `ios-fractos` (docs, paquete `FractosKit`, app de Mac, los dos motores de sonido, proyecto Xcode).
- **Dos pasadas sobre Fractos el mismo día.** La primera leyó `origin/main` del 28 de septiembre, sólo iOS. El usuario subió después la versión con la app de Mac (`f1a7bcb`) y la segunda pasada corrigió lo que cambia: [A2](#a2--trabajo-que-existe-en-una-sola-máquina), [C8](#c8--tres-vocabularios-de-tema-y-dos-de-sonido), [C9](#c9--los-contratos-de-la-suite-viven-en-el-repo-de-una-app), la [sección de Fractos](#fractos-ios-fractos), el contrato de sonido y el documento de Fractos.
- **No ejecutado:** ninguna prueba, compilación ni app. Sólo lectura, `git` de consulta y búsquedas. Ningún número de pruebas de este documento es mío: vienen de los STATUS de cada repo.
- **No disponible aquí:** Stalker, Clipper, neuf, kyu, SoundShop, Atlas y la web de Fractos (`nu9ve.xyz`). Lo que digo de ellos sale de [suite.md](../suite.md) y de los contratos.
- **Sin revisar:** el código de la app de Mac línea por línea, el motor de Python, y la calidad interna del código de nove y curado. Esta auditoría mira estándares, prácticas de sistema operativo, coherencia entre repos y huecos; no es una revisión de código.

## 1. Lo que entendí de la intención

1. **Una suite, no apps sueltas.** Cada app es «nueve» en un idioma, con identidad propia sobre un lenguaje común (tema, teclado, MCP, comportamiento por SO).
2. **Motor común en Rust, interfaz nativa por sistema.** La lógica vive una sola vez; Swift/AppKit en Mac y C#/WinUI 3 en Windows presentan y piden. curado es el caso Tauri. nüün está a mitad del cambio: Python es el oráculo y Rust lo alcanza método por método.
3. **La evidencia manda.** Compilar no es probar; una fixture no es una tarjeta real; lo no visto se anota como no visto. Es la práctica más valiosa de los cuatro repos y hay que protegerla.
4. **Nada irreversible sin plan revisado.** Plan → digest → revisión humana → copia verificada → limpieza. El agente (MCP) prepara y muestra, nunca aplica lo destructivo.
5. **Replicar primero, extraer después.** Una pieza se vuelve paquete cuando hay tres copias iguales.

## 2. Lo que está bien y conviene no tocar

| Práctica | Dónde se ve | Por qué importa |
| --- | --- | --- |
| Estados con evidencia y «sin probar» explícito | STATUS de los cuatro repos; `ios-fractos` `docs/README.md` (estados permitidos) | Evita la deriva de «hecho» a «compiló» |
| Raíz de QA aislada por variable (`NOVE_QA_ROOT`, `NUUN_QA_ROOT`) | [windows-app-v1 §10](../contracts/windows-app-v1.md#10-validación) | Las pruebas nunca tocan la biblioteca personal |
| Pruebas de contrato contra el motor real | `Nove.Tests`, `RustEngineContractTests.cs` | Los DTOs no se validan contra mocks |
| Oráculo y diferencial (JSON canónico byte a byte) | [engine/README](../../engine/README.md) | La forma correcta de reescribir lógica de seguridad de datos |
| El cromo sigue al sistema; el contenido lleva el skin | [windows-app-v1 §0](../contracts/windows-app-v1.md#0-principio-dos-apps-nativas-un-producto) | Se siente nativo sin perder identidad |
| Un ejecutable con roles y alias de un solo rol | nove `Program.cs`, `nove-mcp.exe` | Menos binarios que firmar y validar |
| Cerrar ≠ pausar ≠ detener ≠ desactivar | nove A15 | Nunca un motor invisible |
| MCP por niveles, permiso por sesión, `_untrusted` | [nu9ve-mcp-v1](../contracts/nu9ve-mcp-v1.md) | El modelo no puede contestar lo irreversible |
| Dependencias fijadas y anotadas con su razón | `engine/Cargo.toml`, curado AGENTS | Reproducible sin red |
| Lecciones de Windows escritas con causa y regla | [windows.md §10](../platforms/windows.md#10-aprendizajes-con-evidencia) | Ya ahorra tiempo entre apps |
| Reglas de capas comprobadas por script | `ios-fractos`: `scripts/verify-packages.sh` | «Sólo este módulo importa aquello» deja de ser texto |
| Contrato ajeno como copia fijada con origen y hashes | `ios-fractos`: `Contracts/`, `scripts/sync-contracts.sh` | Las pruebas corren sin el repo hermano y un cambio llega como diff |
| Puerto medido contra una referencia, con versión de receta | `ios-fractos`: `FractosSoundV2`, `web-reference.json` | El «dorado» de nüün aplicado a audio |

## 3. Hallazgos priorizados

Severidad: **A** integridad de datos o pérdida de trabajo · **B** bloquea distribuir o portar · **C** deriva entre repos · **D** oportunidad. El orden dentro de cada letra es el orden sugerido.

| ID | Hallazgo | Evidencia | Propuesta corta |
| --- | --- | --- | --- |
| [A1](#a1--la-relectura-de-verificación-no-evita-la-caché-fuera-de-macos) | En Windows (y Linux) la relectura SHA-256 puede salir de la caché del sistema, no del disco | `engine/crates/nu9ve-volumes/src/fs.rs` `uncached` | Lectura sin búfer antes de la primera corrida física |
| [A2](#a2--trabajo-que-existe-en-una-sola-máquina) | Hay apps sin remoto (kyu, neuf, SoundShop, Atlas). Fractos Mac ya se subió | suite.md; `git log` de `ios-fractos` | Subir; es la misma regla que la app aplica a la media |
| [A3](#a3--la-identidad-de-un-disco-no-viaja-entre-equipos-ni-entre-sistemas) | La identidad de volumen es por equipo en Windows y distinta en Mac | [engine/README](../../engine/README.md#identidad-de-volumen-en-windows-diseño-nuevo) | Identidad portable escrita en el disco, con contrato |
| [A4](#a4--liberar-a-la-papelera-no-se-traduce-entre-sistemas) | «Mover a la Papelera del volumen» no existe igual en Windows ni en Linux | [music-flows](../features/music-flows.md), STATUS (M1, DNG) | Cuarentena propia del motor, decidida por el usuario |
| [A5](#a5--nombres-fechas-y-durabilidad-dependen-del-sistema-de-archivos) | Nombres, `mtime` y durabilidad cambian con el sistema de archivos y el SO | `fs.rs` (comentarios), engine/README (NFC, `TZ`) | Matriz de sistemas de archivos con fixtures |
| [B1](#b1--ninguna-app-tiene-firma-de-distribución-ni-actualización) | Nada está firmado para distribuir y ninguna app se actualiza sola | release de nüün, nove y curado | Decidir canal por SO |
| [B2](#b2--no-hay-ci-y-el-diferencial-nunca-ha-corrido) | No hay CI en ningún repo; el dorado Python↔Rust nunca se ejecutó | `git ls-files`; engine/README | Fixtures doradas en git y una matriz mínima |
| [B3](#b3--finales-de-línea-sin-fijar) | Sin `.gitattributes` en nüün, nove y fractos | `git ls-files --eol` | Fijar LF |
| [B4](#b4--herramientas-sin-fijar) | Sin `rust-toolchain.toml` ni `global.json` | árbol de los repos | Fijar versiones |
| [B5](#b5--linux-sin-diseño) | Linux no tiene diseño de interfaz, de volúmenes ni de paquete | [suite.md](../suite.md#plataformas) | [linux.md](../platforms/linux.md) |
| [B6](#b6--el-motor-decide-por-cfgwindows) | El binario del motor separa «Windows» de «lo demás» | `nuun-engine/src/main.rs` | Backends por SO detrás del mismo rasgo |
| [C1](#c1--dos-sobres-y-dos-transportes) | nove y nüün hablan protocolos distintos | [windows.md §7](../platforms/windows.md#7-motor-y-contratos) | Elegir el sobre antes de extraer `nu9ve-ipc` |
| [C2](#c2--identificadores-carpetas-y-variables-sin-convención) | Identificadores, carpetas de datos y variables sin patrón | manifiestos y código | Convención para lo nuevo |
| [C3](#c3--versiones-desalineadas) | Cada pieza de nüün dice una versión distinta | `pyproject.toml`, `mcp.py`, props | Una fuente de versión |
| [C4](#c4--el-nombre-interno-ya-vive-en-tres-bases-de-código) | `MediaMenu`, `ncreator-media`, `n-creator-utils`, `nu9ve`, `nuun` | PLAN (prioridad baja) | Subir la prioridad del renombrado |
| [C5](#c5--documentos-de-entrada-desactualizados) | El README dice «la app para macOS»; windows.md dice que nüün no lee discos | [README](../../README.md), [windows.md](../platforms/windows.md) | Una pasada corta |
| [C6](#c6--enlaces-entre-repos-con-rutas-de-una-mac) | Unas 65 líneas enlazan con rutas de una Mac concreta | conteo por archivo abajo | Pasada mecánica |
| [C7](#c7--textos-duplicados-por-plataforma) | Textos de interfaz y de errores duplicados por plataforma | Swift 858 claves; C# con cadenas en línea | Catálogo único, del motor para los códigos |
| [C8](#c8--tres-vocabularios-de-tema-y-dos-de-sonido) | Tres vocabularios de tema y dos motores de sonido | tema §5/§12; `ios-fractos`: `DESIGN_SYSTEM.md`, `AUDIO_ENGINE.md` | Tabla de nombres en el tema; [sonido v1](../contracts/nu9ve-sound-v1.md) con el motor v2 |
| [C9](#c9--los-contratos-de-la-suite-viven-en-el-repo-de-una-app) | Cuatro decisiones esperan la misma: dónde vive lo compartido | PLAN P3, tema §10, rust-engine §6, MCP §8 | Decidir el repo de la suite |
| [C10](#c10--higiene-de-los-documentos-de-estado) | Los STATUS crecen contra su propia regla | STATUS de nüün y nove | Tabla por área; historia a `releases/` |
| D | Funciones del sistema sin usar, eficiencia, configuración, inmersión | — | [§6](#6-oportunidades) y [os-integration](../platforms/os-integration.md) |

### A1 · La relectura de verificación no evita la caché fuera de macOS

`fs::uncached` sólo hace algo en macOS (`F_NOCACHE`); en Windows y Linux devuelve el archivo tal cual, y su comentario lo dice: «nothing elsewhere, as Python» ([fs.rs](../../engine/crates/nu9ve-volumes/src/fs.rs)). `transfer.rs` la usa en la lectura del origen, en la relectura de cada `.ncpartial`, al adoptar un destino existente y en la relectura previa a limpiar.

- **Qué significa:** en Windows, releer un archivo recién escrito lo sirve la caché de archivos de Windows con mucha probabilidad. El SHA-256 coincide porque compara memoria con memoria. `FlushFileBuffers` sí manda los datos al dispositivo, pero la relectura no prueba que el dispositivo los devuelva. La garantía «verificado» de [rust-engine §3](../platforms/rust-engine.md#3-rendimiento-dónde-se-va-el-tiempo) («esa segunda lectura es la garantía») es más débil en Windows que en Mac.
- **Por qué ahora:** STATUS dice que `apply` no ha copiado una tarjeta real en Windows. Conviene corregirlo antes de esa corrida, no después.
- **Propuesta:** en Windows, abrir el identificador de verificación con `FILE_FLAG_NO_BUFFERING` (búfer y tamaños de lectura alineados al sector; el final del archivo se lee con un bloque alineado y se recorta). En Linux, `O_DIRECT` con la misma alineación o `posix_fadvise(POSIX_FADV_DONTNEED)` tras `fsync`. Si un sistema de archivos lo rechaza, se registra en el recibo como `verify_cache: "os"` en vez de fingir. Añadir la fila al [gate de límite de pérdida de datos](../release.md).
- **Lo mismo aplica** a `coverage`, a las liberaciones y a cualquier rehash futuro.
- Brief: [S1](session-briefs-2026-11-10.md#s1--verificación-sin-caché-en-windows-y-linux).

### A2 · Trabajo que existe en una sola máquina

| Repo | Situación | Fuente |
| --- | --- | --- |
| `ios-fractos` | **Resuelto el mismo día.** Por la mañana `origin/main` era `da4899d` del 2026-09-28, sólo iOS: cinco semanas de trabajo (monorepo, app de Mac, `FractosKit`, motor de sonido v2) vivían en una sola Mac. El usuario lo subió y hoy `main` es `f1a7bcb` | `git log` |
| `nove` | En este PC el árbol está limpio y al día con `origin/main`. La cabecera de su STATUS todavía dice «0.12.0.45–49 sin commit»: es un dato viejo, no trabajo en riesgo | `git status -sb`; nove `docs/STATUS.md` |
| `curado` | `docs/SUITE-WINDOWS.md` sin seguimiento desde el 9 nov | `git status` |
| kyu, neuf, SoundShop, Atlas | «local, sin remoto» | [suite.md](../suite.md#las-apps) |

El producto principal de la suite es «una sola copia no es un respaldo». El código fuente merece la misma regla. El caso de Fractos muestra el costo: la primera versión de esta auditoría, del contrato de sonido y del documento de Fractos se escribió sin poder leer la referencia expresiva de la suite, y hubo que rehacer las tres. Queda pendiente lo que no tiene remoto.

### A3 · La identidad de un disco no viaja entre equipos ni entre sistemas

- En Windows, `volume_uuid` es la ruta GUID del volumen, **propia de cada PC**; en Mac es `VolumeUUID`, que viaja con el disco. El mismo MEDIA502 (exFAT, pensado para intercambio en [media-architecture](../media-architecture.md)) tiene dos identidades y dos catálogos.
- El catálogo es local por diseño y «multi-host» es una entrega aparte ([release](../release.md)). Pero al llevar nüün a Windows, el caso «el mismo archivo conectado a mi Mac y a mi PC» deja de ser hipotético.
- **Propuesta (necesita decisión, es un límite de seguridad):** un marcador que el motor escribe al registrar un destino, junto a los recibos (`.ncreator/volume.json`: UUID propio, fecha, etiqueta al registrar). La identidad efectiva sería marcador **y** un segundo factor del sistema (serial del sistema de archivos, tamaño), para que un clon bit a bit no pase por el original. Un contrato `volume-identity-v1` diría qué pasa con clones, formateos y discos sin marcador.
- **Segunda parte:** el respaldo del catálogo que ya se escribe en el archivo (`VACUUM INTO`) es la base natural de «adoptar este archivo en otro equipo» en modo de sólo lectura. Hoy no hay flujo para eso.
- Brief: [S6](session-briefs-2026-11-10.md#s6--identidad-de-volumen-portable).

### A4 · Liberar «a la Papelera» no se traduce entre sistemas

M1, la liberación de DNG y varias limpiezas mueven a la Papelera del volumen con diario. En Mac eso es `.Trashes/<uid>` dentro del mismo volumen: un renombrado, barato y recuperable.

| Sistema | Qué hay | Problema |
| --- | --- | --- |
| Windows | `$RECYCLE.BIN` por volumen en discos fijos (también externos); los medios de clase extraíble (lectores SD, muchas memorias USB) no tienen papelera | La misma operación es recuperable en un disco y definitiva en otro |
| Linux | Especificación de FreeDesktop: `$XDG_DATA_HOME/Trash` o `.Trash-$uid` en la raíz del volumen, si se puede escribir | Depende del escritorio y del montaje |

**Propuesta:** que el motor tenga su propia cuarentena en el mismo volumen (`_nuun/released/<job>/…`, renombrado sin reemplazar, con su diario) y que Discos muestre «en cuarentena: N GB · vaciar…». Es igual en los tres sistemas, sigue siendo un renombrado atómico dentro del mismo sistema de archivos, y el espacio se libera en un segundo paso explícito. La alternativa es conservar la papelera del sistema donde exista y declarar la capacidad por volumen. La elección cambia qué ve el usuario al liberar: **es suya**. Brief: [S7](session-briefs-2026-11-10.md#s7--liberación-sin-depender-de-la-papelera-del-sistema).

### A5 · Nombres, fechas y durabilidad dependen del sistema de archivos

Cosas que hoy pasan en Mac por construcción y que en Windows o Linux hay que probar. Todas afectan nombres de destino, digest del plan o reanudación:

| Tema | Riesgo | Evidencia o estado |
| --- | --- | --- |
| Normalización Unicode | HFS+ guarda NFD, Windows y la mayoría NFC: un proyecto «Culiacán» puede ser dos carpetas | engine/README: «Colisiones de nombre sin NFC» |
| Caracteres y nombres reservados | `: ? * < > \|`, punto o espacio final, `CON`, `NUL`, `COM1`… son válidos en Mac e inválidos en Windows. Un proyecto nombrado en Mac puede no poder crearse en el mismo disco desde Windows | La regla de `slug` existe; falta probarla contra la lista de Windows |
| Mayúsculas | APFS/HFS+ suelen no distinguirlas, ext4 sí | Colisiones sólo en Linux |
| `mtime` en FAT/exFAT | FAT32 guarda hora local con resolución de 2 s; exFAT 10 ms y desfase UTC opcional. Mac y Windows pueden leer horas distintas de la misma tarjeta | Los nombres sin hora de cámara usan `mtime` en la zona del sistema; el dorado exige `TZ` fijo |
| Durabilidad del renombrado | exFAT/FAT no tienen diario; el `flush` de carpeta falla y se ignora | Comentario de `sync_dir` en `fs.rs` |
| Lectura cruzada | Windows no lee HFS+ ni APFS; Mac no escribe NTFS; Linux lee HFS+ con diario sólo en lectura | El ESSD (HFS+) no es intercambiable; MEDIA502 (exFAT) sí |
| Rutas largas | Resuelto con `\\?\` | engine/README |
| Antivirus | Defender analiza cada archivo escrito; miles de archivos chicos lo notan | Medido en compilaciones por curado; sin medir en copias |

**Propuesta:** una tabla de sistemas de archivos como contrato corto (qué garantiza el motor en cada uno y qué registra cuando no puede) y fixtures que la prueben: nombres NFD/NFC, reservados, `mtime` con resolución gruesa. En exFAT, tratar un `mtime` sin hora de cámara como evidencia débil (ya se pide confirmación en casos parecidos). Al reanudar un job sobre exFAT, rehash de lo renombrado después del último punto seguro (confirmar si ya ocurre).

### B1 · Ninguna app tiene firma de distribución ni actualización

Estado: nüün ad-hoc en Mac y MSIX sin firmar en Windows; nove MSIX con certificado de desarrollo (confiarlo pide administrador); curado NSIS/MSI sin firmar. Ninguna tiene actualizador. Para el caso real más cercano (nove en el PC del papá del usuario) esto ya estorba: cada versión nueva es un paquete instalado a mano.

| SO | Opciones a decidir | Nota |
| --- | --- | --- |
| Windows | (a) Microsoft Store: el MSIX lo firma la Store y la actualización viene incluida. (b) Firma en la nube de Microsoft (Trusted Signing): verificar elegibilidad por país y tipo de cuenta antes de planear sobre ella. (c) Certificado OV/EV. Con (b) o (c), archivo `.appinstaller` para que el MSIX se actualice solo | El Setup con identidad sigue sirviendo en los tres casos |
| macOS | Developer ID + notarización (ya es gate); actualización con Sparkle o con un actualizador propio mínimo | [macos-app-v1 §9](../contracts/macos-app-v1.md#9-bundle-y-distribución) pide declararlo |
| Tauri (curado) | El plugin de actualización de Tauri con su par de claves, o Store | Firma de Windows aparte |
| Linux | Repositorio firmado con GPG o Flathub | [linux.md](../platforms/linux.md#5-paquetes-y-compatibilidad-entre-distros) |

Es una decisión con costo (cuentas de desarrollador). Mientras tanto, lo honesto es lo que ya se hace: llamarlo build de desarrollo.

### B2 · No hay CI y el diferencial nunca ha corrido

- STATUS de nüün dice «CI (`.gitlab-ci.yml`) existe pero no se ha ejecutado en GitLab». En este checkout **no existe** ese archivo (`git ls-files` no lo lista) y el remoto es GitHub. curado lo comprobó por su lado («no CI workflow exists»). nove no tiene. Fractos tiene `scripts/verify-ios.sh` sin configuración de runner a la vista.
- La consecuencia concreta está en el motor: las pruebas diferenciales contra Python esperan `scripts/engine_golden.py` en la Mac. Hoy hay 105 pruebas de Rust portadas a mano y **ninguna comparación ejecutada** contra el oráculo. La paridad es una afirmación razonable, no un hecho.
- La Mac no corre las pruebas de Windows y el PC no tiene Python: cada máquina valida la mitad.

**Propuesta, en dos pasos baratos:**
1. Generar las respuestas doradas en la Mac y **guardarlas en git** (`tests/fixtures/engine-golden/`; son fixtures sintéticas). Así las pruebas ignoradas dejan de depender de tener Python y corren en el PC y en CI.
2. GitHub Actions con tres trabajos (macOS, Windows, Ubuntu): `fmt`, `clippy -D warnings`, `cargo test`; Python y Swift en macOS; C# en Windows. Sin secretos, sin firma, sin discos. Un archivo de workflow no prueba ejecución: se registra la primera corrida verde en STATUS.

Briefs: [S2](session-briefs-2026-11-10.md#s2--dorado-en-la-mac-y-fixtures-en-git), [S4](session-briefs-2026-11-10.md#s4--ci-mínima-en-tres-sistemas).

### B3 · Finales de línea sin fijar

`core.autocrlf=true` en los cuatro checkouts de este PC. Sólo curado tiene `.gitattributes`. En nüün hay 176 archivos con CRLF en el árbol de trabajo y 2 mezclados; en `ios-fractos` hay 19 con CRLF **dentro del índice**. Riesgos: fixtures doradas que se comparan byte a byte, scripts de shell que fallan en Mac, diffs ruidosos. curado ya pagó esta lección ([windows.md §10](../platforms/windows.md#10-aprendizajes-con-evidencia)). Propuesta: `* text=auto eol=lf` más las excepciones binarias, en los tres repos que faltan.

### B4 · Herramientas sin fijar

- Ningún repo tiene `rust-toolchain.toml`. nüün y nove declaran `rust-version = "1.89"` (mínimo), curado ni eso (`edition = "2021"`), y este PC compila con 1.99.1. Un `clippy -D warnings` verde hoy puede fallar con el siguiente compilador.
- No hay `global.json` para .NET en nüün ni en nove.
- Propuesta: `rust-toolchain.toml` con el canal exacto en cada workspace, `global.json` con `rollForward` acotado, y anotar las versiones de Swift, Bun y Python en un solo lugar por repo. Unificar `edition`/`rust-version` de curado con los otros dos cuando su mantenedor lo decida (su regla es no actualizar por rutina).

### B5 · Linux sin diseño

Hay frases sueltas (XDG, «por interfaces equivalentes») y las notas de compilación de curado. Falta decidir la interfaz, el backend de volúmenes, el paquete y la matriz de distros. Está en [linux.md](../platforms/linux.md), con una ruta por fases que empieza por lo casi gratis: compilar y probar el motor en Linux en CI.

### B6 · El motor decide por `cfg(windows)`

`nuun-engine/src/main.rs` tiene 18 condiciones `cfg(windows)` / `cfg(not(windows))`: `volumes`, `source_cards`, `register_volume`, `plan` y `apply` existen en Windows y responden `not_ported` en lo demás. Es coherente con «Windows primero», pero deja dos deudas:

1. El backend de volúmenes de **macOS en Rust** no existe (sigue en Python con `diskutil`). La fase 5 («Mac cambia de motor») depende de él y es el trabajo más delicado que queda, porque es el que corre con los discos reales del usuario.
2. La forma «Windows contra el resto» invita a que Linux se cuele por la rama de «el resto» sin backend propio.

Propuesta: mantener el rasgo de backend que ya usan las fixtures y registrar tres implementaciones reales (Windows hecho; macOS con DiskArbitration/`statfs`; Linux con UDisks2), con `capabilities` diciendo qué puede cada una. El binario deja de preguntar por el SO y pregunta por la capacidad.

### C1 · Dos sobres y dos transportes

| | nove | nüün |
| --- | --- | --- |
| Transporte | Named pipe / socket Unix, frames de hasta 1 MiB, ACL y PID verificados | Línea JSON por stdin/stdout, un proceso por mutación |
| Sobre | `{"result": …}` / `{"error": {code, message}}` | `{"ok": true, "data"}` / `{"ok": false, "error": {code, message}}` |
| Vida del motor | Larga, con host de fondo | Lector persistente + procesos cortos |

El «sobre común» está en el PLAN (P1 paso 4) como diseño. Se vuelve urgente por otro lado: en cuanto nüün quiera reaccionar a una tarjeta con la ventana cerrada (bandeja, popup al insertar en Windows) necesita un motor de larga vida, y la única implementación probada es la de nove. Decidir el sobre **antes** de copiar `pipe.rs` evita una segunda variante. Recomendación: un `protocol` con versión en el saludo (`engine_info` ya existe en nüün; nove tiene la guarda de versión de datos), el objeto de error idéntico (ya lo es) y un solo nombre para el éxito. Brief: [S8](session-briefs-2026-11-10.md#s8--sobre-de-protocolo-y-versiones).

### C2 · Identificadores, carpetas y variables sin convención

| Cosa | nüün | nove | curado |
| --- | --- | --- | --- |
| Identificador | `app.nu9ve.media` | `xyz.nu9ve.nove…` (plist del agente) | `studio.curado.player` |
| Datos en Windows | `%LOCALAPPDATA%\n-creator-utils\media` (catálogo) **y** `%LOCALAPPDATA%\nuun` (preferencias) | `%LOCALAPPDATA%\Nove` | `%APPDATA%\studio.curado.player` (Roaming) |
| Datos en Mac | `~/Library/Application Support/n-creator-utils/` | — | `~/Library/Application Support/studio.curado.player/` |
| Variables | `NU9VE_MEDIA_HOME`, `NUUN_QA_ROOT`, `NUUN_ENGINE`, `NUUN_ENGINE_BIN`, `NUUN_PYTHON`, y una aparición de `NU9VE_QA_ROOT` | `NOVE_QA_ROOT`, `NOVE_QA_*` | `CURADO_TEST_PYTHON` |

- nüün reparte su estado en dos raíces con dos nombres. Una persona que quiera respaldar o borrar «los datos de nüün» tiene que saber que la mitad se llama `n-creator-utils`.
- La biblioteca SQLite de curado vive en Roaming: en un equipo con perfil móvil se sincroniza con WAL incluido. Es el valor por defecto de Tauri, no una decisión.
- `NU9VE_QA_ROOT` aparece una vez junto a 11 `NUUN_QA_ROOT`: revisar si es un resto.
- Fractos Mac usa `app.fractos.macos`, variables `FRACTOS_MAC_QA*` (un interruptor más una cuenta de QA, no una raíz) y ya escribe en una carpeta común de la suite: `~/Library/Application Support/nu9ve/app-backups`. Esa carpeta `nu9ve` compartida es la que la convención propone para lo común.

**Propuesta:** una convención para lo **nuevo** (Fractos de escritorio, Linux) y una tabla de equivalencias para lo existente. Renombrar carpetas de datos ya creadas sólo con migración y respaldo, y sólo si el usuario lo quiere: los identificadores actuales preservan datos y eso vale más que la simetría. Detalle en [os-integration §3](../platforms/os-integration.md#3-convención-propuesta-para-nombres-y-rutas).

### C3 · Versiones desalineadas

nüün: `pyproject.toml` 0.1.0, app de Mac 0.2.0 (según sus pruebas), Windows 0.1.0, motor de Rust 0.1.0, MCP `ncreator-media` 0.2.0. nove: `Cargo.toml` 0.11.0 con paquete Windows 0.12.0.49 y datos 26. «Versiones alineadas» ya es gate de release. Propuesta: una fuente por repo, y que `engine_info` (nüün) y `nove_status` (nove) digan app, motor, protocolo y esquema; Acerca de muestra los cuatro, como ya hace nove.

### C4 · El nombre interno ya vive en tres bases de código

El renombrado `MediaMenu` → `Nuun` está en el PLAN como prioridad baja. Desde entonces nacieron `native/windows` (ya dice `Nuun`) y `engine/` (`nuun-*`). Quedan con el nombre viejo el paquete Swift, el MCP (`ncreator-media`), el CLI (`nu9ve media`), la carpeta de datos y el propio repo. Cada semana que pasa hay más scripts y documentos que tocar. Sugerencia: hacerlo cuando la app de Mac cambie de motor, en el mismo ciclo, porque ese cambio ya obliga a tocar empaquetado y rutas.

### C5 · Documentos de entrada desactualizados

- [README](../../README.md): «nüün es la app para macOS de este repositorio»; no menciona `engine/` ni `native/windows`; «Empezar» sólo cubre Python.
- [suite.md](../suite.md): fecha de lectura 2026-11-07, antes de la maqueta de Windows y del motor en Rust (la fila de nüün sí se actualizó). La fila de Fractos se corrigió en la segunda pasada de esta auditoría con lo leído en su repo.
- [windows.md §1](../platforms/windows.md#1-qué-hay-hoy) aún dice «nüün en Windows **no** lee discos todavía», y STATUS dice que ya lee, planea y aplica con fixtures.

### C6 · Enlaces entre repos con rutas de una Mac

La regla ya está escrita dos veces ([windows.md §10](../platforms/windows.md#10-aprendizajes-con-evidencia), nove `docs/suite-alignment-windows.md` §5) y no se aplicó. Líneas con `../../../../ai/…` o `/Users/nu9ve/…` (unas pocas son citas del propio problema, no enlaces):

| Repo | Archivos | Líneas |
| --- | --- | --- |
| n-creator-utils | `contracts/README.md` 12, `next-sessions.md` 9, `media-library-v1.md` 3, bitácora 3, y otros 9 archivos | 40 |
| nove | `references.md` 20, y otros 3 | 24 |
| curado | `STATUS.md` | 1 |

Pasada mecánica: «`repo`: `ruta`» en texto, y enlace relativo sólo entre repos hermanos.

### C7 · Textos duplicados por plataforma

- La app de Mac tiene 858 claves localizadas (es/en) con un comprobador. La de Windows escribe las cadenas en español dentro del C# (`"Ajustes"`, `"Respaldar y liberar origen"`); el inglés está pendiente.
- Los textos de los códigos de error viven en cada cliente (`App.Core`), y el motor de Rust emite mensajes técnicos en inglés (`"Not a regular file"`). Son tres fuentes para el mismo error (Swift, C#, MCP).

**Propuesta:** (1) el **motor** es dueño del catálogo `código → texto y qué hacer` en es/en, lo sirve por un método (`messages`) y lo usan la app de Mac, la de Windows y el MCP; (2) un catálogo de interfaz único (JSON o `.xcstrings` como fuente) del que se generan los recursos de Swift y de C#, con la misma prueba de claves faltantes en ambos. No hace falta una dependencia: es un script como `check_localization.py`. Brief: [S15](session-briefs-2026-11-10.md#s15--un-solo-catálogo-de-textos).

### C8 · Tres vocabularios de tema y dos de sonido

| | Nombres |
| --- | --- |
| Skins de la suite ([tema §5](../contracts/nu9ve-theme-v1.md#5-catálogo-de-skins)) | `glass`, `minimal`, `dither`, `pixel`, `gothic`, `y2k`, más `classicMac`, `win9x`, `terminal` |
| Materiales de Fractos (`ThemeMaterial`) | `glass`, `classicMac`, `win9x`, `terminal`, `neon`, `gothic`, `parchment`, `ledger`, `chrome` |
| Temas de Fractos (web, iOS, Mac y voces de sonido) | `light`, `dark`, `cyber`, `matrix`, `pixel`, `sangre`, `win99`, `bitcoin`, `y2k`, `nora94` |

Fractos ya tiene escrita su mitad de la tabla (`ios-fractos`: `docs/DESIGN_SYSTEM.md`, «Materiales»): Pixel → `classicMac`, Win99 → `win9x`, Matrix → `terminal`, Sangre → `gothic`, Cyber → `neon`, Nora94 → `parchment`, Bitcoin → `ledger`, Y2K → `chrome`. Lo que falta está del lado de la suite:

- El material que Fractos llama `chrome` es el skin `y2k` del contrato.
- `pixel`, `dither` y `minimal` existen en el contrato (vienen de curado) y no en Fractos; `pixel` del contrato no es el tema Pixel de Fractos, que usa `classicMac`.
- El contrato dice que `neon`, `parchment` y `ledger` son «sólo de Fractos», correcto, pero no nombra `chrome`.

**Sonido.** Hay dos motores en Fractos: el v1 (Sound Lab, 11 acciones, el de iOS, documentado en `AUDIO_ENGINE.md`) y el v2 (26 gestos, el de Mac, documentado en `DESIGN_SYSTEM.md`). El tema §12 cita el v2 (`FractosSoundV2`, `SoundCues`, `HapticPlayer`) y su vocabulario de señales coincide con el de `FractosInteractionCue`. Lo que no coincidía: el tema §12 pedía sonido apagado por defecto y la app de Mac de Fractos arranca con sonido y háptica encendidos. **El usuario decidió el mismo día: encendido por defecto en toda la suite, con una opción clara para silenciar**; el tema §12 y el contrato de sonido ya lo dicen. Sigue pendiente que los documentos de Fractos digan cuál de sus dos motores es el vigente, y que alguien confirme el contrato contra el motor de la web, que es el canónico y no está en este PC.

La propuesta completa está en [sonido de tema v1](../contracts/nu9ve-sound-v1.md), ya con el v2 como fuente. La tabla de nombres debería añadirse al contrato de tema.

### C9 · Los contratos de la suite viven en el repo de una app

Hay cuatro pendientes que son la misma pregunta:

- PLAN P3 Windows, paso 1: dónde viven `Nu9ve.Core`/`Nu9ve.WinUI`.
- Tema §10: dónde vive `NuTheme`/`NuMacShell`.
- Motor en Rust §6: dónde viven `nu9ve-json`, `nu9ve-volumes`, `nu9ve-ipc`, `nu9ve-mcp`.
- MCP §8: `nu9ve-mcp` «cuando otra app lo necesite».

Y los cinco contratos de suite, los vectores de tema y `scripts/windows` están en `n-creator-utils`, que es el repo de nüün.

| Opción | A favor | En contra |
| --- | --- | --- |
| Seguir en `n-creator-utils` | Cero trabajo hoy | Las demás apps dependen del repo de una app; las copias siguen divergiendo; nove pospone su tema por esto |
| **Repo `nu9ve/suite`** (recomendada) | Un lugar para contratos, vectores, crates `nu9ve-*`, `Nu9ve.Core`/`WinUI`, `NuMacShell`, scripts de QA y el paquete de sonido | Un repo más; hay que fijar cómo se consume |
| Monorepo de todo | Cambios atómicos | Contradice «cada app en su repo» y mezcla licencias y ritmos |

Dos hechos de Fractos que pesan en esta decisión:

- **`NuMacShell` ya existe como pieza genérica** (ventana propia, barra, arrastre y doble clic, Acerca de, identidad de build, háptica; unas 965 líneas con pruebas, sin importar nada de Fractos), pero vive dentro del paquete `FractosKit` en `ios-fractos`. nüün, SoundShop y neuf tienen copias. Es la primera pieza Swift que se mudaría.
- **Fractos ya resolvió cómo consumir un contrato ajeno:** una copia fijada (`Contracts/`) con el commit de origen y un manifiesto de hashes, y un script con `--check` (integridad) y `--strict` (falla si el origen avanzó). Es más ligero que un submódulo y sirve tal cual para que cada app tenga los contratos de la suite, los vectores de tema y la referencia de sonido sin depender de la disposición de carpetas.

**Decidido por el usuario el 2026-11-11: repo `nu9ve/suite` con copia fijada.** Era la recomendación afinada tras su pregunta («¿todos dependerían de tener ese repo y de manejar versiones?»): un repo `nu9ve/suite` donde lo compartido se edita, y en cada app una **copia fijada** de lo que usa (documentos, datos y también el código de Rust, Swift y C#), con commit de origen, hashes y un script de sincronización. Ninguna app necesita el repo de la suite para compilar, no hay credenciales ni red de por medio, una sesión que sólo ve el repo de la app puede leer lo compartido, y cada app sube de versión cuando quiere. Está desarrollado en [cómo se comparte entre repos](../suite-sharing.md), con la comparación contra las dependencias de paquete y la migración por pasos. «Replicar primero» sigue valiendo para lo que aún cambia; lo que ya nació con nombre `nu9ve-*` y tiene dos copias (identidad de volumen: curado y nüün) puede mudarse ya. Es decisión del usuario; mientras no se tome, las cuatro colas siguen paradas. Brief: [S5](session-briefs-2026-11-10.md#s5--decidir-y-crear-el-repo-de-la-suite).

### C10 · Higiene de los documentos de estado

- STATUS de nüün: «Entregas recientes» tiene 8 entradas contra su regla de ~5; la fila «Motor en Rust» es un párrafo de más de 300 palabras en una celda.
- STATUS de nove: 169 líneas, la mayoría bitácora por build; su «Siguiente acción mínima» habla de 0.12.0.27 con la cabecera en 0.12.0.49. CONTEXT tiene tres capas de «estado anterior» dentro de «Entrada de siguiente sesión».
- Un modelo que retoma lee estos archivos primero: su tamaño es costo de cada sesión y los párrafos viejos son instrucciones contradictorias.

Propuesta: en nüün, mover las entradas viejas a `releases/` y partir la fila del motor en «portado / evidencia / falta» con enlaces a [engine/README](../../engine/README.md). En nove, adoptar el formato de tabla por área de nüün y dejar la bitácora en `docs/qa`. Es trabajo de cada repo; aquí sólo se señala.

## 4. Por repositorio

### nüün (n-creator-utils)

- **Lo más sólido:** las garantías de transferencia (parcial → verificar → renombrar sin reemplazar → diario → limpieza con relectura), la receta como datos y los contratos con Stalker, Clipper y Atlas.
- **Lo más frágil hoy:** la migración misma. Hay tres implementaciones parciales del mismo producto (Python completo en Mac, Rust parcial en Windows, dos interfaces), con la paridad sin medir ([B2](#b2--no-hay-ci-y-el-diferencial-nunca-ha-corrido)) y la garantía de verificación más débil en la plataforma nueva ([A1](#a1--la-relectura-de-verificación-no-evita-la-caché-fuera-de-macos)).
- **Orden que sugiero para el motor:** A1 → dorado en git (B2) → corrida física en Windows → backend de volúmenes de macOS en Rust → cambio de motor en Mac por método. No abrir Stalker/`delivery`/cierre en Rust hasta que la base de tarjetas tenga diferencial verde y una corrida física.
- **Regla de migración a vigilar:** «cada método tiene un dueño». M1 y M2 (`collection_release.py`, `living.py`) nacieron en Python el 9 nov, el mismo día en que se adoptó Rust. Es correcto según la regla, pero agranda lo que falta portar; conviene decidir pronto si las funciones nuevas de música esperan al motor de Rust.
- **Ventana de Windows:** falta llevar `plan`/`apply` a la interfaz, la cancelación desde el cliente y todo lo de sistema (progreso en la barra de tareas, mantener despierto el equipo, llegada de tarjetas, aviso al terminar). Ver [os-integration](../platforms/os-integration.md#5-las-diez-con-mejor-relación-valor--costo).
- **MCP:** las lecturas que el MCP necesita ya están portadas a Rust. Un `nuun-engine mcp` (Leer + Mostrar) copiando el servidor de nove sería la segunda copia y cerraría cuatro filas de adopción de [nu9ve-mcp-v1](../contracts/nu9ve-mcp-v1.md#adopción). Brief: [S11](session-briefs-2026-11-10.md#s11--mcp-de-nüün-en-rust).

### nove

- **Es la referencia de Windows** y lo demuestra con uso real: mediciones de 10–12 h, host de fondo, extensión, MCP con cambios deshacibles.
- **Brechas frente a la suite** (ya listadas en su `docs/suite-alignment-windows.md`): roles de color derivados, catálogo de atajos, acciones largas, instalador con identidad. Siguen pendientes y siguen siendo correctas. La más barata y visible: el orden de Ctrl+4/Ctrl+5.
- **Mac rezagada:** el plan original era Rust → Mac → Windows; hoy Windows va 15 versiones de datos por delante (Mac en datos 11, Windows en 26) y `cargo test` no se ha corrido en Mac con el esquema actual. El MCP responde «sólo Windows» en Unix. Cuando se retome la Mac, el primer paso es compilar y pasar las pruebas del motor ahí; CI lo haría solo ([B2](#b2--no-hay-ci-y-el-diferencial-nunca-ha-corrido)).
- **Riesgo operativo medido y abierto:** la pestaña del broker crece ~500 MB/h en Chrome; falta el A/B con y sin lector. No es un tema de estándares, pero es lo que más afecta el uso diario.
- **Piezas que la suite necesita de nove:** `pipe.rs`, `mcp.rs` + `agent.rs`, el host de bandeja Win32, la petición de energía (A30) y `os.rs`. Son los candidatos naturales a `nu9ve-ipc`, `nu9ve-mcp` y `nu9ve-os`.
- **Dos librerías de tiempo** (`jiff` y `chrono`): ya señalado en [rust-engine §6](../platforms/rust-engine.md#6-qué-compartir-en-rust-entre-las-apps); nüün eligió `chrono` más un crate propio.

### curado

- **El más cuidadoso con sus invariantes** (callback sin asignaciones, sin WebAudio como sustituto, sin pruebas visuales sin permiso) y el más completo en teclado y tema.
- **Windows:** compila, pasa pruebas y empaqueta; **no se ha escuchado**. La lista de diez verificaciones físicas de su `docs/WINDOWS.md` es el siguiente paso real y sólo lo puede hacer una persona.
- **Brechas frente a la suite:** `forced-colors` para alto contraste, skins comunes, instalador con marca (config de NSIS), `CURADO_QA_ROOT`. Todas están en su `docs/SUITE-WINDOWS.md`, que sigue sin commit.
- **Datos en Roaming** ([C2](#c2--identificadores-carpetas-y-variables-sin-convención)).
- **Integración con nüün** (su ROADMAP, «Pieces»): nüün le dice dónde está un archivo y curado le dice de qué depende para que no se libere. En Windows ambos son Rust: la identidad de volumen compartida ([A3](#a3--la-identidad-de-un-disco-no-viaja-entre-equipos-ni-entre-sistemas)) es el primer ladrillo.
- **Sonido de interfaz:** curado es la app donde la regla «nunca por la salida de mezcla o de cue» más importa. Ver [sonido v1 §5](../contracts/nu9ve-sound-v1.md#5-reproducción-por-plataforma).
- **Linux:** es la app que antes puede llegar (Tauri ya empaqueta `deb`/`rpm`/AppImage). Lo que falta es probarla.

### Fractos (ios-fractos)

Leído en `f1a7bcb` (2026-11-10). Detalle, brechas y rutas para Windows en `ios-fractos`: `docs/SUITE-DESKTOP.md`.

- **Qué es hoy:** un monorepo Swift con la app de iOS (270 archivos, ~99 mil líneas), una app de Mac (`Apps/macOS/FractosMac`, 25 archivos, ~6 800 líneas, `app.fractos.macos` 0.1.0) y el paquete `FractosKit` con 13 targets. La Mac tiene login, Trabajo, Calendario, Mapa, Textos, Grupos y Perfil contra el API, `implemented-local`, probada en modo QA y sin cuenta real.
- **Es el repo con mejores prácticas de estructura de la suite**, y varias no están en los demás: reglas de capas comprobadas por script (`verify-packages.sh`), contrato ajeno como copia fijada con hashes, entorno como pareja completa con prueba, sello de build compartido (`Nu9veBuildID`), almacén de sesión que distingue la firma ad hoc, «qué correr y cuándo» con su costo, y estados que sólo expresan evidencia.
- **Diferencia de fondo:** no tiene motor local; la autoridad es el servidor. «Motor en Rust + interfaz nativa» no se traduce tal cual: el oráculo es el contrato con sus fixtures, y la lógica que no debe duplicarse ya está aislada en targets sin interfaz.
- **Sonido:** dos motores ([C8](#c8--tres-vocabularios-de-tema-y-dos-de-sonido)). El v2 es un puerto del motor de la web con referencia medida, calibración y versión de receta: el mismo patrón del «dorado» de nüün, aplicado a audio, y lo que hace viable repartir sonido a la suite.
- **Brechas de la Mac frente a los contratos** (leídas en el código, no vistas):
  - **Teclado:** sin catálogo; atajos en `AppCommands.swift` y sueltos en pantallas. ⌘1 y ⌘2 son filtros de Trabajo y las secciones van en ⌥⌘1…, cuando el contrato reserva ⌘1…⌘9 para las vistas. ⇧⌘Q cierra la sesión de Fractos y es el atajo de «Cerrar sesión» de macOS.
  - **Idioma:** la interfaz de Mac es sólo español; no hay catálogo de textos en `Apps/macOS`.
  - **Tema:** contraste probado sólo en los temas por defecto; preferencias en claves propias, no en el formato común; controles de Win99 a la derecha (excepción anotada).
  - **Sonido:** encendido por defecto; sin volumen; no calla cuando otra app graba ni lee la preferencia del sistema.
  - **Distribución:** ad hoc, sin actualizador, icono de iOS (ya en su plan como P3).
- **Higiene:** sin `.gitattributes`, sin CI, contrato pendiente de sincronizar con la web (sus pruebas de contrato fallan hasta entonces, según su `MONOREPO.md`), y `AUDIO_ENGINE.md` no menciona el motor v2.
- **Activos heredados:** 23 archivos de audio en el bundle de iOS sin licencia documentada. Son del motor v1; el v2 no usa archivos, así que la suite no los hereda.

## 5. Estado de los estándares de la suite

| Contrato | Estado declarado | Lo que le falta para ser estándar de verdad |
| --- | --- | --- |
| [macos-app-v1](../contracts/macos-app-v1.md) | En adopción (SoundShop, nüün, Fractos) | Fractos lleva su propia tabla en `ios-fractos`: `docs/MONOREPO.md`; se resumió en el contrato el 2026-11-10. nove Mac, curado y kyu sin revisar |
| [windows-app-v1](../contracts/windows-app-v1.md) | Propuesta | Promoverlo: tres apps lo siguen. Añadir progreso en la barra de tareas, energía, llegada de dispositivos y enlaces `nu9ve://` ([os-integration](../platforms/os-integration.md)); corregir el enlace a nove de la cabecera |
| [nu9ve-theme-v1](../contracts/nu9ve-theme-v1.md) | Propuesta | Tabla de correspondencia de nombres ([C8](#c8--tres-vocabularios-de-tema-y-dos-de-sonido)); regla «apps de datos» que pidió nove; vectores leídos por Swift y TypeScript |
| [nu9ve-interaction-v1](../contracts/nu9ve-interaction-v1.md) | Propuesta | La app de Mac de nüün no tiene catálogo y la de Windows sí: la referencia está al revés que la plataforma principal |
| [nu9ve-mcp-v1](../contracts/nu9ve-mcp-v1.md) | Propuesta | Segunda implementación en Rust (nüün); servidor de nove en Mac |
| Par de Linux | No existe | [linux.md](../platforms/linux.md) como base de un `linux-app-v1` cuando haya una app |
| Sonido | §12 del tema; implementado en Fractos Mac (motor v2) y, con otro motor, en iOS | [nu9ve-sound-v1](../contracts/nu9ve-sound-v1.md); decidir el valor por defecto |
| Avisos del sistema | No existía como estándar. nove en Windows lo tiene completo (cola en el motor, clic al aviso exacto, límites, «todo salvo excepciones»); en Mac, nove y nüün publican desde la app y nüün sólo avisa de una tarjeta detectada | [nu9ve-notifications-v1](../contracts/nu9ve-notifications-v1.md) |
| Protocolo común | Diseño en PLAN | [C1](#c1--dos-sobres-y-dos-transportes) |
| Identidad de volumen | Por app | [A3](#a3--la-identidad-de-un-disco-no-viaja-entre-equipos-ni-entre-sistemas) |
| Nombres, rutas y variables | No existe | [C2](#c2--identificadores-carpetas-y-variables-sin-convención) |

Una observación general: los contratos son buenos y casi todos dicen «propuesta». Lo que falta no es escribir más reglas, sino **cerrar el circuito**: cada regla con una prueba o un script que la compruebe (como ya pasa con los vectores de tema y el catálogo de atajos), y cada tabla de adopción revisada cuando la app cambia. Un contrato cuya adopción dice «sin revisar» en seis columnas todavía es un documento, no un estándar.

## 6. Oportunidades

El detalle por sistema operativo está en [os-integration](../platforms/os-integration.md). Aquí, lo que cruza apps.

### Eficiencia

| Idea | Dónde | Nota |
| --- | --- | --- |
| Medir cada fase antes de optimizar | nüün | Ya está en [rust-engine §8](../platforms/rust-engine.md#8-qué-se-puede-mejorar-ya-sin-cambiar-de-lenguaje); sigue sin hacerse |
| Reservar el tamaño del destino antes de escribir (`SetFileInformationByHandle`, `F_PREALLOCATE`, `fallocate`) | motor | Detecta «disco lleno» por archivo al empezar y reduce fragmentación en exFAT. Medir |
| Tamaño de bloque por bus (USB 3, SD, NVMe) | motor | Hoy es uno; medir 1–8 MiB |
| Medir el costo del antivirus en miles de archivos chicos | Windows | No se puede apagar desde la app; se documenta y se mide |
| Llegada de tarjetas por evento, no por sondeo | Windows, Linux | `WM_DEVICECHANGE` / UDisks2; ver os-integration |
| Motor de larga vida sólo cuando haga falta | nüün | Con Rust, un proceso por mutación arranca en milisegundos; el host largo se justifica por la bandeja y el progreso, no por velocidad |
| Una sola sentada de QA física por SO para todas las apps | suite | Narrador, 150 %, alto contraste y suspensión están pendientes en nove, nüün y curado a la vez. Brief [S14](session-briefs-2026-11-10.md#s14--pasada-física-de-accesibilidad-en-windows) |

### Usabilidad

- **Primer arranque de nüün.** La app calla si no hay destinos registrados (correcto), pero entonces el primer uso depende de saber que hay que registrar un archivo y un disco de trabajo. Un recorrido de dos pasos con los discos externos detectados, que explique por qué el interno no cuenta.
- **Terminar sin abrir la ventana.** Aviso al completar con dos acciones: «Expulsar tarjeta» y «Ver resultado». Cierra el flujo SD → respaldo → tarjeta libre con un clic.
- **Errores con «qué hacer».** curado lo hace con «sin salida de audio». El catálogo de códigos del motor ([C7](#c7--textos-duplicados-por-plataforma)) debería tener siempre dos campos: qué pasó y qué hacer.
- **Diagnóstico copiable.** nove lo tiene como gate (G10). Patrón de suite: Acerca de → «Copiar diagnóstico» con versiones de app, motor, protocolo, esquema, SO y los últimos códigos de error, sin rutas ni nombres.
- **Salud del disco.** Para una herramienta de respaldo, mostrar el estado SMART de un archivo («el sistema reporta fallos en este disco») vale más que cualquier animación. `diskutil info` en Mac, contadores de fiabilidad de almacenamiento en Windows, UDisks2 en Linux.

### Configurabilidad

- **Un paquete de ajustes portable.** Receta, perfiles creativos, preferencias de tema y atajos reasignados en un archivo versionado (`nuun-settings-v1.json`) que se exporta e importa. Sirve para llevar la configuración de la Mac al PC, que hoy es a mano.
- **Seguir el tema de nu9ve** (tema §4, fase 2): un archivo común por usuario (`nu9ve/theme.json`) con skin, color y sonido, que cada app sigue sólo si el usuario lo activa. Con cuatro apps en el mismo equipo empieza a tener sentido.
- **Recetas editables** (PLAN P1 paso 5): ya previsto. El lenguaje cerrado de condiciones es la decisión correcta.

### Inmersión

- **Sonido por tema compartido:** [nu9ve-sound-v1](../contracts/nu9ve-sound-v1.md).
- **El sistema también lleva el skin:** imagen del aviso, icono de la lista de saltos y estado del icono de bandeja dibujados con la paleta activa (nunca el acento en el icono de la barra de menús de Mac, que es plantilla).
- **Progreso fuera de la ventana:** barra de tareas en Windows, Dock en Mac y progreso del archivo en Finder (`NSProgress` publicado). La copia se siente parte del sistema.
- **Setup con identidad** para nove y curado, con la base de `Nuun.Setup` (ya propuesto en sus docs).

### Integraciones

| Integración | Qué da | Requisito |
| --- | --- | --- |
| Enlaces `nu9ve://<app>/<tipo>/<id>` | `show_in_app` del MCP, clic en un aviso y saltos entre apps (nüün → Clipper, nüün → kyu) con el mismo mecanismo en los tres SO | Registrar el esquema por app; sólo abre y muestra, nunca aplica |
| nüün ↔ curado | Ubicación de archivos y dependencias para no liberar lo que un set usa | `music-library-v1` y la identidad de [A3](#a3--la-identidad-de-un-disco-no-viaja-entre-equipos-ni-entre-sistemas) |
| MCP en todas | Un agente que lee nüün, nove y curado con las mismas reglas | `nu9ve-mcp` tras la segunda copia |
| Suite → Fractos | Un respaldo verificado o un cierre de proyecto como entrada de trabajo en Fractos (opt-in) | Contrato con la API de Fractos (`/api/v1`), nunca Supabase directo; es idea, no pedido |
| nüün en una Raspberry Pi | El motor sin interfaz como nodo de archivo (segunda réplica) | Motor estático para Linux arm64 ([linux.md](../platforms/linux.md)); conecta con la visión de IoT del README |

## 7. Orden sugerido

No reemplaza al [PLAN](../../PLAN.md). Es el orden en que estos hallazgos convendría entrar a él.

1. **Antes de la primera copia real en Windows:** A1; dorado en git (B2).
2. **Esta semana, sin diseño:** subir lo que está en una sola máquina (A2), `.gitattributes` (B3), toolchains (B4), README y suite.md (C5), enlaces (C6).
3. **Decisiones del usuario que destraban colas:** repo de la suite (C9), canal de firma y actualización (B1), cuarentena o papelera (A4), identidad portable (A3), nombres de skins (C8). Ya decidido el 2026-11-10: sonido encendido por defecto con «Silenciar» a un paso.
4. **Después de decidir:** CI (B2), sobre común (C1), catálogo de textos (C7), integración de sistema en la ventana de Windows, MCP en Rust.
5. **En paralelo, con una persona:** escuchar curado en Windows, pasada de accesibilidad, corrida física de nüün.
6. **Más adelante:** Linux por fases, sonido compartido, Fractos en Windows.

## 8. Lo que esta auditoría no vio

- Ninguna app corriendo. Nada de lo visual está evaluado.
- La app de Mac de nüün compilada, el motor de Python en ejecución y las apps que no están en este PC.
- El comportamiento real de la caché de Windows en `apply`: A1 sale de leer el código y de cómo funciona la caché de archivos, no de una medición. La corrección trae su propia prueba.
- Elegibilidad y precios de firma: cambian; verificarlos al decidir.
- Detalles de API de sistema citados en [os-integration](../platforms/os-integration.md) y [linux.md](../platforms/linux.md): son la dirección correcta, pero cada una se confirma con la documentación del sistema al implementarla.
