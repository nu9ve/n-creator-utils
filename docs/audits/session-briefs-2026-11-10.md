# Briefs de sesión · auditoría 2026-11-10

Encargos listos para entregar a otro modelo, **uno por sesión**. Salen de la [auditoría Fable](fable-audit-2026-11-10.md) y no son otra lista de prioridades: el orden vigente es el del [PLAN](../../PLAN.md). Un brief entra al PLAN cuando el usuario lo elige.

## Cómo usarlos

1. Copia el bloque del brief como primer mensaje de la sesión, en el repo indicado.
2. Cada brief es autosuficiente: dice qué leer, qué tocar, cuándo está hecho y qué no hacer.
3. Las reglas de [AGENTS](../../AGENTS.md) valen siempre y no se repiten en cada bloque: leer CONTEXT → STATUS → PLAN; conservar el trabajo existente; sin commits, push ni mensajes a otras sesiones sin autorización; fixtures temporales, nunca discos personales; registrar evidencia real en STATUS; no declarar nada «listo» por compilar.
4. Los marcados **(decisión)** no se implementan hasta que el usuario elija. Su primer entregable es la decisión escrita.
5. Los marcados **(Mac)** o **(Windows)** necesitan esa máquina. Los marcados **(persona)** necesitan a alguien frente al equipo.

| # | Brief | Repo | Dónde | Depende de |
| --- | --- | --- | --- | --- |
| [S1](#s1--verificación-sin-caché-en-windows-y-linux) | Verificación sin caché | n-creator-utils | Windows | — |
| [S2](#s2--dorado-en-la-mac-y-fixtures-en-git) | Dorado y fixtures en git | n-creator-utils | Mac | — |
| [S3](#s3--higiene-de-repos) | Higiene de repos | los cuatro | cualquiera | autorización de commits |
| [S4](#s4--ci-mínima-en-tres-sistemas) | CI mínima | los cuatro | cualquiera | S2, S3 |
| [S5](#s5--decidir-y-crear-el-repo-de-la-suite) | Repo de la suite (decidido: copia fijada) | nuevo | cualquiera | — |
| [S6](#s6--identidad-de-volumen-portable) | Identidad de volumen portable **(decisión)** | n-creator-utils | Windows + Mac | — |
| [S7](#s7--liberación-sin-depender-de-la-papelera-del-sistema) | Cuarentena del motor **(decisión)** | n-creator-utils | cualquiera | — |
| [S8](#s8--sobre-de-protocolo-y-versiones) | Sobre de protocolo y versiones **(decisión)** | n-creator-utils, nove | cualquiera | — |
| [S9](#s9--integración-de-sistema-en-la-ventana-de-windows) | Integración de sistema en Windows | n-creator-utils | Windows | `plan`/`apply` en la ventana |
| [S10](#s10--paquete-de-sonidos-de-tema) | Paquete de sonidos | ios-fractos, n-creator-utils | Mac, luego Windows | S19 |
| [S11](#s11--mcp-de-nüün-en-rust) | MCP de nüün en Rust | n-creator-utils | Windows | — |
| [S12](#s12--motor-en-linux-fase-l0) | Motor en Linux, L0 | n-creator-utils | cualquiera | S4 |
| [S13](#s13--fractos-cerrar-las-brechas-de-la-mac-frente-a-los-contratos) | Fractos Mac: brechas frente a los contratos | ios-fractos | Mac | — |
| [S17](#s17--motor-de-sonido-en-rust) | Motor de sonido en Rust | repo de la suite o n-creator-utils | cualquiera | S10, S5 |
| [S18](#s18--avisos-del-sistema-en-nüün-para-mac) | Avisos del sistema en nüün para Mac | n-creator-utils | Mac | — |
| [S19](#s19--confirmar-el-contrato-de-sonido-contra-la-web) | Confirmar el sonido contra la web y exportar el paquete | ios-fractos + nu9ve.xyz | Mac | — |
| [S14](#s14--pasada-física-de-accesibilidad-en-windows) | Pasada de accesibilidad **(persona)** | nove, nüün, curado | Windows | — |
| [S15](#s15--un-solo-catálogo-de-textos) | Catálogo único de textos | n-creator-utils | cualquiera | — |
| [S16](#s16--backend-de-volúmenes-de-macos-en-rust) | Volúmenes de macOS en Rust | n-creator-utils | Mac | S2 |

---

## S1 · Verificación sin caché en Windows y Linux

```text
Repo: n-creator-utils (Windows). Lee AGENTS.md, docs/CONTEXT.md, docs/STATUS.md, y luego
docs/audits/fable-audit-2026-11-10.md (hallazgo A1), engine/README.md y
engine/crates/nu9ve-volumes/src/fs.rs.

Problema: `fs::uncached` sólo hace algo en macOS (F_NOCACHE). En Windows y Linux, la relectura
SHA-256 que da la garantía «verificado» puede salir de la caché de archivos del sistema y no
del dispositivo. La usan transfer.rs (origen, relectura del .ncpartial, adopción de un destino
existente, relectura antes de limpiar) y la usará coverage.

Objetivo: que la lectura de verificación no pase por la caché del sistema en Windows y Linux,
sin cambiar el protocolo, los códigos de error ni el digest.

Alcance:
- nu9ve-volumes::fs: una apertura de lectura para verificación. Windows: FILE_FLAG_NO_BUFFERING
  más no seguir enlaces, con búfer y tamaños alineados al sector del volumen; el final del
  archivo se lee con un bloque alineado y se recorta. Linux: O_DIRECT alineado, o
  posix_fadvise(DONTNEED) tras fsync si O_DIRECT no se admite.
- Si el sistema de archivos rechaza el modo sin búfer, seguir con lectura normal y dejarlo
  registrado en el recibo y en el job (por ejemplo verify_cache: "os" | "bypassed"). No fingir.
- transfer.rs: usar esa apertura sólo donde se verifica un destino o se relee un original
  antes de borrarlo. La lectura de copia del origen puede seguir como está.
- Pruebas: tamaños 0, 1, sector-1, sector, sector+1 y varios MiB; contenido idéntico al de la
  lectura normal; corrupción del mismo tamaño detectada; rutas largas \\?\. No se puede probar
  la caché en sí: dilo.
- Medir en este PC la diferencia de tiempo con un archivo de 1 GiB en un disco local y anotarla.

Hecho cuando: cargo fmt --check, clippy -D warnings y cargo test verdes; las pruebas portadas de
test_media_engine siguen pasando; STATUS (fila Motor en Rust) dice qué cambió, la medición y que
sigue sin corrida física; docs/release.md nombra la lectura sin caché en el gate de pérdida de datos.

No hacer: cambiar SHA-256, tocar media/ (Python), ni hacer una corrida con una tarjeta real.
Si Python necesita lo mismo en una plataforma que no es Mac, anótalo como pendiente; hoy Python
sólo corre en Mac.
```

## S2 · Dorado en la Mac y fixtures en git

```text
Repo: n-creator-utils (Mac, con Python). Lee AGENTS.md, CONTEXT, STATUS, engine/README.md
(sección «Dorado en la Mac») y docs/audits/fable-audit-2026-11-10.md (B2).

Problema: las pruebas diferenciales Rust↔Python nunca se han ejecutado. El PC no tiene Python y
la Mac no había corrido el script. La paridad del motor de Rust es una suposición.

Objetivo: ejecutar el dorado, corregir lo que difiera y dejar las respuestas doradas en git
para que el diferencial corra en cualquier máquina y en CI sin Python.

Pasos:
1. python3 scripts/engine_golden.py
2. cd engine && TZ=America/Hermosillo TMPDIR="$(cd "$TMPDIR" && pwd -P)" cargo test --offline -- --ignored
3. Correr la prueba nueva de Python tests/test_devices.py
   (test_discover_keeps_each_units_device_and_camera_time_apart) y la suite de Python completa.
4. Por cada diferencia: decidir cuál motor tiene razón leyendo media/ (Python es el oráculo salvo
   error demostrado, como el de discover del 9 nov). Corregir, repetir.
5. Revisar que tests/fixtures/engine-golden/ no contenga rutas ni nombres personales (son
   fixtures sintéticas) y proponer al usuario añadirlas a git. Añadir .gitattributes para que se
   guarden sin conversión de finales de línea.
6. Quitar #[ignore] de las diferenciales que ya no necesitan Python, o dejarlas condicionadas a
   que exista el archivo dorado, diciendo por qué se omiten si falta.
7. Cruce pendiente: abrir con Python un catálogo creado por Rust (register_volume + un apply de
   fixture) y al revés. Registrar el resultado.

Hecho cuando: las diferenciales pasan en la Mac; el resultado (cuántos casos, qué se corrigió y
en qué motor) está en STATUS y en engine/README; las fixtures están listas para commit con la
aprobación del usuario.

No hacer: usar MEDIA, ESSD ni una SD; cambiar el formato del JSON canónico para «hacer pasar»
una prueba; commits sin pedirlos.
```

## S3 · Higiene de repos

```text
Repos: n-creator-utils, nove, curado, ios-fractos (uno por vez; pregunta antes de cada commit).
Lee docs/audits/fable-audit-2026-11-10.md en n-creator-utils: A2, B3, B4, C5, C6.

Objetivo: quitar las fuentes de ruido y de riesgo que no necesitan diseño.

Por repo:
1. .gitattributes con `* text=auto eol=lf` y las excepciones binarias (png, ico, icns, sqlite,
   wav, mp3, fuentes). curado ya lo tiene: úsalo de modelo. No renormalices sin avisar: muestra
   primero cuántos archivos cambiarían (git add --renormalize . en una rama).
2. rust-toolchain.toml con el canal exacto que hoy pasa las pruebas; global.json para .NET en
   nüün y nove. Anota la versión de Swift, Bun y Python donde aplique.
3. Enlaces entre repos: reemplazar rutas `../../../../ai/...` y `/Users/nu9ve/...` por
   «`repo`: `ruta`» en texto. No toques los que citan el problema como ejemplo ni los prompts
   históricos.
4. n-creator-utils: README (hoy dice «la app para macOS»; añadir Windows en maqueta con motor
   de Rust parcial y cómo compilar engine/), docs/suite.md (fila de Fractos y fecha de lectura),
   docs/platforms/windows.md §1 (ya lee discos por el motor de Rust), STATUS límite 5 (el
   archivo .gitlab-ci.yml no existe en el repo).
5. nove: cabecera de STATUS (dice «sin commit» y el árbol está limpio) y «Siguiente acción
   mínima» (habla de 0.12.0.27).
6. curado: docs/SUITE-WINDOWS.md está sin seguimiento; preguntar si se añade.
7. Lista para el usuario, sin ejecutarla: qué repos no tienen remoto (kyu, neuf, SoundShop,
   Atlas).

Hecho cuando: cada repo tiene un diff pequeño y revisable; los enlaces internos siguen
resolviendo (en ios-fractos, bash scripts/verify-docs.sh); nada se subió sin permiso.

No hacer: cambiar código, renombrar carpetas de datos, actualizar dependencias, ni mezclar esta
pasada con otra tarea.
```

## S4 · CI mínima en tres sistemas

```text
Repo: n-creator-utils primero; luego nove y curado con el mismo patrón.
Lee docs/audits/fable-audit-2026-11-10.md (B2) y docs/development.md (validación).
Requiere S2 (fixtures doradas en git) y S3 (finales de línea y toolchain fijados).

Objetivo: un workflow de GitHub Actions que corra lo que hoy sólo se corre a mano, en macOS,
Windows y Ubuntu. Sin secretos, sin firma, sin discos reales.

Trabajos para n-creator-utils:
- macOS: python3 -m unittest discover -s tests; swift test --package-path native/MediaMenu;
  cd engine && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test;
  generar el dorado y comparar con el guardado (falla si Python y las fixtures difieren).
- Windows: lo mismo para engine/; dotnet test de native/windows (las de contrato con el
  binario recién compilado).
- Ubuntu: fmt, clippy y test de engine/. Las pruebas que necesitan el backend de Windows se
  omiten diciendo por qué.
- Cachear cargo y NuGet. Las dependencias se descargan en CI; anota que --offline deja de
  aplicar ahí.

Hecho cuando: la primera corrida verde (o roja con causa entendida) queda registrada en STATUS
con su enlace. Un archivo de workflow sin ejecutar no cuenta.

No hacer: publicar artefactos, firmar, ni añadir pruebas que usen la red o el HOME real.
Revisa que las pruebas de Python aíslen Path.home (hubo un caso que leyó el registro real de
Clipper).
```

## S5 · Decidir y crear el repo de la suite

```text
(decisión) Repo: n-creator-utils para confirmar la decisión; el repo nuevo sólo si se aprueba.
Lee docs/suite-sharing.md completo (es la propuesta), docs/audits/fable-audit-2026-11-10.md
(C9), docs/suite.md («Lo que ya se repite») y, en ios-fractos, Contracts/README.md y
scripts/sync-contracts.sh (el mecanismo que se generaliza).

Ya decidido por el usuario (2026-11-11): se crea nu9ve/suite y cada app lleva copia fijada.
Antes de tocar nada, confirma lo que falta de suite-sharing.md §8: nombre y privacidad del
repo; si NuMacShell sale de ios-fractos; idioma de los documentos.

Sigue suite-sharing.md §7, un paso por sesión. Esta sesión hace
los pasos 1 y 2:
1. Crear el repo con contracts/ (los contratos de suite: macos-app, windows-app, nu9ve-theme,
   nu9ve-interaction, nu9ve-mcp, nu9ve-sound, nu9ve-notifications), docs/ (suite.md,
   platforms/, audits/), data/theme-vectors-v1.json y CHANGELOG.md. Mover con historia
   mínima; los contratos de media se quedan en n-creator-utils.
2. scripts/sync-suite en sh y en PowerShell, generalizando sync-contracts.sh: una lista de
   rutas que la app declara, SOURCE con repo, commit y fecha, MANIFEST.sha256, y los modos
   --check, --strict, <commit> y --from <carpeta> (copia sucia que el gate rechaza).
   Pruebas del script: copia íntegra pasa; un archivo editado falla; un archivo no declarado
   falla; origen adelantado avisa con --check y falla con --strict.
3. En n-creator-utils: vendor/suite/ con su primera copia, y docs/ con un índice que apunta a
   ella en lugar de los archivos movidos. Corregir los enlaces internos.

Hecho cuando: sync-suite --check pasa en n-creator-utils; sus enlaces resuelven; el repo de la
suite tiene su primera corrida de CI o, si aún no hay CI, queda anotado. nove, curado e
ios-fractos no se tocan en esta sesión: cada uno adopta en la suya.

No hacer: mover código (rust/, windows/, swift/) en la misma sesión que los documentos;
mover piezas con una sola copia que siguen cambiando (pipe, MCP y host de bandeja de nove);
editar una copia fijada; subir nada sin que el usuario lo pida.
```

## S6 · Identidad de volumen portable

```text
(decisión) Repo: n-creator-utils. Lee la auditoría (A3), engine/README.md («Identidad de volumen
en Windows»), media/volumes.py, docs/release.md (límites operativos) y
docs/features/replication-policy.md.

Problema: en Windows la identidad de un destino es la ruta GUID del volumen, propia de cada PC;
en Mac es VolumeUUID. El mismo disco exFAT es dos discos para dos catálogos.

Entregable 1: contrato docs/contracts/volume-identity-v1.md (propuesta) que responda:
- Marcador escrito al registrar un destino (.ncreator/volume.json): campos, versión, quién lo
  escribe y cuándo se relee.
- Identidad efectiva = marcador + segundo factor del sistema (serial del sistema de archivos,
  tamaño). Qué pasa con: clon bit a bit, formateo, marcador borrado, marcador copiado a otro
  disco, disco de sólo lectura, tarjetas de cámara (nunca se les escribe un marcador).
- Cómo conviven las identidades actuales (mode: "windows" y la de Mac) con la nueva sin volver
  a registrar todo.
- «Adoptar un archivo en otro equipo» en sólo lectura, a partir del respaldo del catálogo que
  ya vive en el archivo.
Presenta dos o tres opciones con su riesgo y espera la elección del usuario.

Entregable 2 (tras decidir): implementación con fixtures en Rust (y en Python si Mac sigue en
Python para volúmenes), con pruebas de cada caso de la lista.

Hecho cuando: el contrato está enlazado desde docs/contracts/README.md y la fila de adopción
dice qué motor lo cumple.

No hacer: debilitar «fallar cerrado» cuando la identidad no está disponible; escribir en una
tarjeta de cámara; probar con MEDIA o ESSD.
```

## S7 · Liberación sin depender de la Papelera del sistema

```text
(decisión) Repo: n-creator-utils. Lee la auditoría (A4), docs/features/music-flows.md (M1),
media/collection_release.py, docs/operations.md (liberar espacio) y
docs/platforms/os-integration.md (fila Papelera).

Problema: liberar mueve a la Papelera del volumen, que sólo existe así en Mac. En Windows los
medios extraíbles no tienen papelera y en Linux depende del escritorio.

Entregable 1: propuesta corta con dos opciones para que el usuario elija:
(a) Cuarentena del motor en el mismo volumen (_nuun/released/<job>/, renombrado sin reemplazar,
    diario propio, «vaciar» como segundo paso explícito y visible en Discos).
(b) Papelera del sistema donde exista, declarada como capacidad por volumen, y negarse donde no.
Incluye: qué ve la persona, cuándo se recupera el espacio, cómo se deshace, qué pasa con
cuarentenas viejas y cómo migra lo ya movido a .Trashes.

Entregable 2 (tras decidir): implementación en el motor dueño del método (hoy Python para
liberaciones), con las pruebas de fallo que ya existen para liberar, más: corte a mitad del
renombrado, vaciar con un archivo en uso, volumen desconectado.

Hecho cuando: el contrato o la página dueña (operations / music-flows) describe la semántica
nueva y STATUS dice en qué plataformas está probada.

No hacer: borrar definitivamente como paso implícito; cambiar el comportamiento en Mac sin que
el usuario lo pida.
```

## S8 · Sobre de protocolo y versiones

```text
(decisión) Repos: n-creator-utils (contrato), después nove. Lee la auditoría (C1, C3),
docs/platforms/windows.md §7, engine/README.md, y en nove docs/contracts/engine-v0.md y
docs/suite-alignment-windows.md §4.

Problema: nove responde {"result"} / {"error"} por pipe con frames; nüün responde
{"ok","data"} / {"ok":false,"error"} por líneas en stdin/stdout. nüün necesitará un motor de
larga vida (bandeja, tarjeta con la ventana cerrada) y la única implementación probada es la de
nove.

Entregable 1: docs/contracts/nu9ve-engine-protocol-v1.md (propuesta) con:
- Un sobre (elige y justifica), el objeto de error común (ya coincide) y cómo los clientes
  aceptan ambos durante la transición.
- Saludo con versiones: app, motor, protocolo, esquema de datos. nüün ya tiene engine_info;
  nove tiene la guarda de versión de datos.
- Transportes permitidos: stdio por líneas (procesos cortos) y pipe/socket con frames (motor de
  larga vida), con las reglas de seguridad de nove (ACL del usuario, primera instancia, PID).
- Progreso y cancelación de acciones largas como parte del protocolo (hoy nüün no tiene
  cancelación desde el cliente en Rust).
- Catálogo de códigos de error compartidos (not_ported, read_only, job_busy…).

Entregable 2 (tras decidir): una fuente única de versión en n-creator-utils (pyproject, app de
Mac, Directory.Build.props, engine/Cargo.toml, serverInfo del MCP) y Acerca de mostrando app,
motor, protocolo y esquema.

Hecho cuando: el contrato está en el índice y cada app tiene su fila de adopción.

No hacer: cambiar el protocolo de nove en esta sesión; copiar pipe.rs a nüün antes de decidir.
```

## S9 · Integración de sistema en la ventana de Windows

```text
Repo: n-creator-utils (Windows). Requiere que plan/apply ya estén en la ventana de Windows.
Lee docs/platforms/os-integration.md (§2 y §5), docs/contracts/windows-app-v1.md §8,
docs/contracts/nu9ve-interaction-v1.md (acciones largas) y, en nove, docs/decisions.md A15 y A30.

Objetivo: las cinco primeras filas de os-integration §5, en este orden, cada una cerrada antes
de la siguiente:
1. No dormir durante apply: capacidad del motor (petición de energía de Windows; en Mac la
   actividad equivalente), tomada al empezar y soltada al terminar, fallar o cancelar. Copia el
   patrón de nove (core/src/os.rs) con comentario de origen.
2. Progreso en la barra de tareas (ITaskbarList3) alimentado por overall_done/overall_total;
   estado de pausa y de error.
3. eject_card en Rust para Windows (bloquear, desmontar, expulsar; nunca un destino registrado;
   bloqueado mientras un job use la tarjeta), con el backend de fixture.
4. Aviso al terminar con «Expulsar tarjeta» y «Ver resultado» (AppNotification con activación).
5. Llegada y salida de volúmenes por evento en lugar de pedir source_cards al refrescar.

Cada paso: prueba de la regla pura, recorrido por UI Automation en raíz de QA y, lo que no se
pueda automatizar (suspender de verdad, expulsar una memoria real), anotado como pendiente
físico.

Hecho cuando: STATUS (fila Windows) lista las cinco con su evidencia y sus pendientes;
windows-app-v1 §8 tiene las filas nuevas.

No hacer: AutoPlay ni arranque con Windows en esta sesión (son opt-in que persisten fuera de la
app y merecen la suya); un proceso residente; llamar APIs de disco desde C#.
```

## S10 · Paquete de sonidos de tema

```text
Repos: ios-fractos (Mac) para generar; n-creator-utils (Windows) para consumir.
El valor por defecto ya está decidido: encendido, con «Silenciar» a un paso (contrato §7).
La Parte A la reemplaza S19 si esa sesión ya exportó el paquete desde el motor de la web.
Lee docs/contracts/nu9ve-sound-v1.md completo y, en ios-fractos, docs/DESIGN_SYSTEM.md
(Sonido y Háptica), Packages/FractosKit/Sources/FractosSoundV2 (FractosSoundEngine.swift,
FractosSoundV2Player.swift, SoundLevels.swift), Apps/macOS/FractosMac/SoundCues.swift y
Sources/FractosDesign/FractosCue.swift. La fuente es el motor v2; no uses el v1
(FractosSound, AUDIO_ENGINE.md), que es el de iOS.

Parte A (ios-fractos, Mac): exportador del paquete.
- Un ejecutable o prueba del paquete que llame a FractosSoundEngine.render (ya normalizado) y
  escriba, por voz y gesto: 3 variantes; para tab y nav, además direction −1 y +1; para
  expand y collapse, la duración por defecto. WAV estéreo a 48 kHz.
- manifest.json: versión de receta (recipeVersion), commit, y por archivo la sonoridad y el
  pico verdadero medidos con FractosSoundMetrics, más su SHA-256.
- Prueba: cada archivo cae en el objetivo de su gesto (FractosSoundEngine.target) con la
  tolerancia que ya usa normalizedCellsLandOnTheirTargets.
- No uses generate-sound-reference.ts --wav como paquete: son los renders de Chrome sin
  normalizar.
- Añade una línea a AUDIO_ENGINE.md y al README del paquete diciendo que el v2 es el vigente.

Parte B (n-creator-utils, Windows): reproductor y señales.
- Copia fijada del paquete con su origen y hashes (como Contracts/ en ios-fractos), sólo con
  las voces de los cinco skins de nüün y los gestos que la app usa.
- Señales en los controles con skin de Nu9ve.WinUI (NuButton, NuToggle, navegación), con el
  mismo vocabulario que FractosInteractionCue; los controles no conocen el audio.
- Reproductor: precarga de lo frecuente, los límites de la fuente (redisparo por gesto,
  máximo de voces, fatiga con piso 0.62 en una ventana de 0.28 s), paneo según el lugar en la
  ventana, rotación de variantes, volumen, y cierre del audio tras 20 s en silencio.
- Tablas señal → gesto y skin → voz como datos, con una prueba de que ninguna señal queda sin
  decidir.
- Ajustes → Apariencia: Sonidos de interfaz (encendido por defecto) y Volumen; migrar
  app.interfaceSounds. «Silenciar» a un paso en el menú de la barra, con atajo del catálogo, y
  la línea de primera vez («Los sonidos de la interfaz están encendidos · Silenciar · Ajustes»).
- Reglas de silencio del contrato §6 con prueba.
- Medir la latencia percibida y anotarla. Si no baja de ~30 ms, decirlo y no encenderlo.

Hecho cuando: los cinco skins de nüün suenan en Windows con la opción encendida y callan con
ella apagada; la tabla de adopción del contrato lo refleja; queda registrado lo que sólo se
puede juzgar escuchando.

No hacer: portar el sintetizador en esta sesión (es S17); añadir una dependencia de audio sin
anotarla; sonar durante un trabajo; cambiar recetas o niveles.
```

## S11 · MCP de nüün en Rust

```text
Repo: n-creator-utils (Windows). Lee docs/contracts/nu9ve-mcp-v1.md completo (sobre todo la
tabla de adopción), media/mcp.py, engine/README.md y, en nove, core/src/mcp.rs, docs/mcp.md y
core/tests/mcp.rs.

Objetivo: `nuun-engine mcp` con los niveles Leer y Mostrar, como segunda copia del servidor de
nove (copiar con comentario de origen; no extraer todavía).

Alcance:
- serverInfo «nuun» con la versión de la app; instructions en español.
- Herramientas de lectura sobre lo ya portado: estado (nuun_status), biblioteca, recurso,
  jobs, job, volúmenes, almacenamiento, receta.
- Campos _untrusted para nombres de archivo, proyecto, etiquetas de volumen y metadatos EXIF,
  limpios de caracteres de control y bidi, y acotados.
- Rutas relativas al volumen o al proyecto.
- show_in_app sólo si existe el mecanismo para llevar la ventana a un objeto; si no, déjalo
  fuera y anótalo (ver os-integration §4).
- Nunca copiar, limpiar ni liberar. Sin --allow-actions en esta entrega.
- Pruebas con el motor real en raíz temporal, las de la sección 6 del contrato.

Hecho cuando: un cliente real (Claude Code) lista y llama una herramienta contra una raíz de
QA; docs/mcp.md existe; la tabla de adopción se actualiza. El MCP de Python queda como está
para Mac hasta que el motor cambie allí.

No hacer: abrir el catálogo en modo escritura; exponer rutas absolutas del usuario; registrar
el servidor contra la biblioteca personal sin que el usuario lo pida.
```

## S12 · Motor en Linux, fase L0

```text
Repo: n-creator-utils. Lee docs/platforms/linux.md (§2 y §6) y la auditoría (B5, B6).
Requiere el workflow de S4.

Objetivo: que engine/ compile y pase sus pruebas en Ubuntu, y que el binario diga con
honestidad qué puede hacer ahí.

Alcance:
- Corregir lo que no compile o falle en Linux (ramas cfg(unix), rutas temporales, hora local).
- nuun-engine: sustituir las decisiones por cfg(windows) / cfg(not(windows)) por una consulta
  al backend de volúmenes; en Linux, sin backend real todavía, cada método responde not_ported
  con el motivo y engine_info/capabilities lo listan.
- rename_new en Linux con renameat2(RENAME_NOREPLACE) y la comprobación actual como respaldo.
- Nada de UDisks2 todavía: eso es L1.

Hecho cuando: el trabajo de Ubuntu está verde y STATUS dice «compila y pasa N pruebas en
Linux; sin backend de volúmenes; sin corrida física».

No hacer: añadir dependencias; prometer soporte de Linux en README o suite.md.
```

## S13 · Fractos: cerrar las brechas de la Mac frente a los contratos

```text
Repo: ios-fractos (Mac). Lee AGENTS.md, docs/MONOREPO.md, docs/DESIGN_SYSTEM.md y
docs/SUITE-DESKTOP.md (§6 y §9 son la lista de trabajo), y en n-creator-utils:
docs/contracts/macos-app-v1.md, nu9ve-theme-v1.md, nu9ve-interaction-v1.md y nu9ve-sound-v1.md.
Sigue «qué correr y cuándo» de docs/DEVELOPMENT.md: prueba puntual, build-apps.sh y, si tocas
Packages/, verify-packages.sh. No corras verify-ios.sh.

Cada punto es independiente; ciérralos en este orden y detente donde el usuario diga.

1. Catálogo de atajos. Una tabla única (id, título, grupo, teclas, alcance) de la que salgan
   AppCommands, los atajos sueltos de las pantallas (⌘S en Perfil y Textos, flechas en Trabajo
   y Calendario) y un panel ⌘/. Prueba: sin duplicados en el mismo alcance y sin teclas del
   sistema. Esa prueba debe atrapar ⇧⌘Q (Cerrar sesión de macOS).
2. Vistas principales. Hoy ⌘1 y ⌘2 son filtros de Trabajo y las secciones van en ⌥⌘1…; el
   contrato de interacción reserva ⌘1…⌘9 para las vistas. Propón el cambio al usuario antes de
   hacerlo y corrige DESIGN_SYSTEM.md, que dice «⌘1…⌘6».
3. Sonido: el valor por defecto (encendido) se queda: el usuario lo decidió para toda la suite
   el 2026-11-10. Falta lo que esa decisión exige: «Silenciar» a un paso (menú de la app y
   atajo), la línea de primera vez, volumen en Ajustes y las reglas de silencio (no sonar
   mientras otra app graba o reproduce; leer la preferencia de efectos de sonido del sistema).
4. Documentos: una línea en AUDIO_ENGINE.md y en Packages/FractosKit/README.md que diga que
   FractosSoundV2 es el motor vigente y FractosSound el de iOS hasta migrar.
5. Contraste: extender FractosDesignTests de los temas por defecto a los diez temas y a las
   mezclas de paleta y material que Ajustes permite. Lo que no pase se anota, no se maquilla.
6. Idioma: catálogo de textos para la app de Mac (español e inglés), empezando por menús,
   Ajustes y Acerca de.
7. .gitattributes con LF y excepciones binarias (pregunta antes de renormalizar).
8. Al terminar: actualizar la tabla de adopción de docs/MONOREPO.md, borrar de
   docs/SUITE-DESKTOP.md §6 y §9 lo cerrado, y avisar qué filas cambian en las tablas de
   adopción de los contratos de n-creator-utils.

Hecho cuando: cada punto cerrado tiene su prueba puntual y su fila actualizada;
bash scripts/verify-docs.sh pasa.

No hacer: cambiar la arquitectura API-first; mezclar esto con una extracción de paquete;
declarar verified-device; tocar recetas o niveles del sonido.
```

## S14 · Pasada física de accesibilidad en Windows

```text
(persona) Repos: nove, n-creator-utils, curado, en este PC. Una sola sentada de una o dos horas
con el usuario frente al equipo. El modelo prepara y registra; la persona mira y escucha.

Preparación (modelo): una lista de comprobación por app a partir de lo que cada STATUS marca
«sin probar»: escala 125/150/200 %, tamaño de texto del sistema, alto contraste (los cuatro
temas de Windows), tema claro y oscuro, Narrador en el recorrido principal, sólo teclado,
suspender y reanudar con un trabajo activo, cierre a la bandeja (nove), y la lista de diez de
curado en docs/WINDOWS.md. Cada app con su raíz de QA; nunca la biblioteca personal salvo que
el usuario lo decida para nove.

Durante: una fila por comprobación con visto / fallo / no aplica y una nota. Capturas sólo con
datos de ejemplo.

Después: actualizar el STATUS de cada repo con lo observado, mover a «pendiente» lo que falló
con su causa probable, y las tablas de adopción de windows-app-v1.

No hacer: arreglar durante la sentada; lanzar curado o inspeccionarlo visualmente sin la
instrucción explícita de su mantenedor (su AGENTS lo prohíbe por defecto).
```

## S15 · Un solo catálogo de textos

```text
Repo: n-creator-utils. Lee la auditoría (C7), scripts/check_localization.py, cómo carga textos
native/MediaMenu (858 claves) y cómo los escribe native/windows (cadenas en el C#).

Objetivo:
1. Catálogo de códigos del motor: un archivo de datos (es, en) con, por código, «qué pasó» y
   «qué hacer». El motor lo sirve por un método de lectura; la app de Mac, la de Windows y el
   MCP lo usan. Un código sin entrada falla una prueba.
2. Catálogo de interfaz: una fuente (elige entre .xcstrings o JSON y justifica) de la que se
   generan los recursos de Swift y de C#, con la misma comprobación de claves faltantes y
   sobrantes en ambos. Empieza por las pantallas que existen en las dos plataformas.
3. Inglés en la ventana de Windows.

Hecho cuando: el comprobador corre en ambas plataformas; la barra de título se probó en los
dos idiomas al estrecharse (tema §9); STATUS lo registra.

No hacer: añadir un framework de localización; traducir identificadores estables (skin, modo).
```

## S16 · Backend de volúmenes de macOS en Rust

```text
Repo: n-creator-utils (Mac). Requiere S2. Lee engine/README.md, media/volumes.py, la auditoría
(B6) y docs/platforms/rust-engine.md §7 (fase 5).

Problema: en Rust, volumes, source_cards, register_volume, plan y apply sólo existen en
Windows. La app de Mac no puede cambiar de motor sin un backend de volúmenes de macOS, y es el
código que tocará los discos reales del usuario.

Objetivo: el backend de macOS detrás del mismo rasgo que usan Windows y las fixtures, con la
identidad que ya usa Python (VolumeUUID, DiskUUID, almacenes físicos, externo, imagen de disco,
st_ino de la raíz), de modo que un destino registrado por Python aparezca en línea en Rust sin
volver a registrarlo.

Alcance: identidad y montajes (DiskArbitration o diskutil con salida estructurada; elige y
justifica), espacio, expulsión; diferencial contra Python sobre los mismos volúmenes de fixture
y sobre el volumen del sistema. Sólo lectura en esta entrega: volumes, source_cards, snapshot.

Hecho cuando: las diferenciales de esos métodos pasan en la Mac; la app de Mac puede elegir
Rust para esos tres métodos detrás de BackendConfiguration, con Python como respaldo; STATUS
dice exactamente qué métodos responde cada motor en Mac.

No hacer: plan ni apply en Mac con Rust en esta sesión; cambiar la identidad de Python; tocar
MEDIA o ESSD más que para leer su identidad si el usuario lo pide expresamente.
```

## S17 · Motor de sonido en Rust

```text
Repo: el de la suite si ya existe (S5); si no, n-creator-utils/engine/crates como nu9ve-sound.
Requiere S10 hecho (ya hay una app que suena con archivos) y una segunda razón: otra app que
no es de Apple lo quiere, o hace falta dirección, duración o fuerza por gesto.
Lee docs/contracts/nu9ve-sound-v1.md §4 y, en ios-fractos, docs/DESIGN_SYSTEM.md (Sonido),
todo Packages/FractosKit/Sources/FractosSoundV2, Tests/FractosSoundV2Tests y
scripts/generate-sound-reference.ts.

Objetivo: un crate que sintetice lo mismo que FractosSoundV2 (10 temas × 26 gestos, variantes,
hint de dirección y duración, controles de intensidad, brillo, cola y afinación) y entregue
PCM estéreo a 48 kHz. Sólo el motor; el reproductor de cada plataforma queda fuera.

Método, el mismo que usó el puerto de Swift:
- Copia fijada de web-reference.json (con su recipeVersion y commit) como vector compartido.
- Portar en el orden de los archivos: SoundDSP, SoundInstruments, SoundThemes, SoundGestures,
  SoundLevels, FractosSoundEngine. Lo único de Apple es Accelerate: sustituir por código
  propio o por un crate de FFT anotado con su razón.
- Pruebas equivalentes a FractosSoundV2Tests: la referencia coincide con la receta portada;
  todo tema toca todo gesto; una variante es determinista; las variantes siguen más allá de
  cinco; una pestaña se mueve hacia su destino; un panel dura lo que su animación; las celdas
  coinciden con la web dentro de las tolerancias de Swift; las celdas normalizadas caen en su
  objetivo.
- Comparar además contra el paquete de S10 (el motor de Swift) celda por celda.
- La tabla de niveles se copia de SoundLevels.swift; si el puerto no cae en el objetivo con
  esos números, hay un error de puerto: no se recalibra para taparlo.

Hecho cuando: todas las celdas pasan; el crate declara su recipeVersion y una prueba falla si
no coincide con la referencia; queda escrito qué se escuchó y qué no.

No hacer: cambiar una receta; añadir el reproductor; usar el motor v1; darlo por bueno sin que
alguien lo escuche junto al de Mac.
```

## S18 · Avisos del sistema en nüün para Mac

```text
Repo: n-creator-utils (Mac). Lee AGENTS.md, CONTEXT, STATUS, docs/contracts/
nu9ve-notifications-v1.md completo, docs/contracts/macos-app-v1.md §8 y
docs/contracts/nu9ve-interaction-v1.md (acciones largas). Código de partida:
native/MediaMenu/Sources/MediaMenu/Model.swift (el aviso de «tarjeta detectada») y
Application.swift (delegado de UNUserNotificationCenter). Referencia de comportamiento:
en nove, docs/contracts/deliveries-v1.md y apps/windows/Nove/Notifications.cs.

Hoy nüün en Mac sólo avisa cuando detecta una tarjeta: sin sonido, sin botones, sin ajustes,
y el clic siempre abre Ingreso. No avisa cuando un trabajo largo termina.

Objetivo: los avisos de nüün en Mac según el contrato, publicados por la app (nüün no tiene
motor de larga vida todavía; el contrato §1 lo permite).

Alcance:
1. Catálogo de tipos de aviso (id, texto es/en, urgencia, destino del clic, gesto de sonido):
   tarjeta detectada; trabajo terminado y verificado; trabajo fallido; la cámara necesita una
   acción. Una prueba falla si un tipo no tiene destino.
2. Avisar al terminar un trabajo sólo si su ventana no está al frente. Una vez por trabajo,
   con identificador por job para que un reintento reemplace.
3. Categorías y acciones: «Ver resultado» siempre; «Expulsar tarjeta» en trabajo terminado,
   por el método del motor que ya existe en Mac y con sus mismas guardas. Nunca una acción que
   aplique, limpie o libere.
4. El clic lleva al objeto exacto (el job, la tarjeta), validando el identificador recibido.
5. Ajustes → Avisos: interruptor, tipos apagables como excepción, «Enviar aviso de prueba»,
   estado del permiso con enlace a Ajustes del Sistema, y Sonido del aviso (Del tema, Del
   sistema, Ninguno). Mientras no exista el paquete de sonidos, «Del tema» usa el del sistema
   y lo dice.
6. Pedir el permiso al activar o al primer aviso, con .alert y .sound.
7. Proteger todo cuando no hay bundle real (pruebas, QA), como ya se hace.

Hecho cuando: pruebas del catálogo y de «no avisar con la ventana al frente»; recorrido en
modo QA; localización sin claves faltantes; STATUS dice qué se probó a mano (clic con la app
cerrada, No molestar, pantalla de bloqueo) y qué no; la tabla de adopción del contrato se
actualiza.

No hacer: mover la cola al motor en esta sesión; avisar por archivo o por paso; tocar la
ventana de Windows (eso es S9).
```

## S19 · Confirmar el contrato de sonido contra la web

```text
Repos: ios-fractos y nu9ve.xyz, en la Mac (el motor canónico está en la web y no existe en el
PC de Windows). Lee, en n-creator-utils, docs/contracts/nu9ve-sound-v1.md completo y
docs/suite-sharing.md §6; en ios-fractos, docs/DESIGN_SYSTEM.md (Sonido), docs/UPDATING.md,
scripts/generate-sound-reference.ts y scripts/calibrate-sound-engine.sh; en nu9ve.xyz,
packages/lib/src/audio/engine (themes.ts, levels.ts) y packages/lib/src/audio/index.ts.

El contrato de sonido se escribió leyendo sólo el puerto de Swift. Esta sesión lo confronta
con el motor real.

Parte 1 · Confirmar (sólo lectura y medición):
- Lista de gestos y de temas; UI_SOUND_EVENTS frente a la tabla del contrato §2.
- Límites del reproductor (redisparo, voces por gesto, fatiga) frente al contrato §1.
- FRACTOS_SOUND_RECIPE_VERSION y FRACTOS_LEVEL_TARGETS.
- Anota cada diferencia en el contrato; no cambies recetas.

Parte 2 · Niveles. DESIGN_SYSTEM.md afirma que la tabla de niveles de la web se midió con un
compresor que arranca hundido y que en vivo los gestos cortos suenan hasta 12 dB sobre su
objetivo. Mídelo (el mismo analizador, con y sin el compresor) y presenta el resultado al
usuario con una pregunta clara: ¿se corrige la tabla de la web con el método nativo, o se
queda? No la cambies sin su respuesta: afecta cómo suena la web en producción.

Parte 3 · Paquete para la suite. Con el motor que el usuario elija como origen:
- Web: generate-sound-reference.ts --wav y, después, llevar cada archivo a su objetivo de
  sonoridad midiéndolo.
- Swift: FractosSoundEngine.render, ya calibrado.
Por voz y gesto, 3 variantes; para tab y nav, además direction −1 y +1; estéreo 48 kHz.
manifest.json con versión de receta, commit del motor de origen y, por archivo, sonoridad,
pico verdadero y SHA-256. Incluye los gestos que usan los avisos del sistema (discovery,
success, error, warning) en un formato que macOS y Windows acepten para notificaciones.

Hecho cuando: el contrato dice «confirmado contra la web en <commit>» o lista sus
diferencias; la pregunta de niveles está respondida o planteada; el paquete existe con su
manifiesto y alguien escuchó una muestra de cada voz.

No hacer: editar el motor de la web o el de Swift; mezclar esto con adopción en una app.
```
