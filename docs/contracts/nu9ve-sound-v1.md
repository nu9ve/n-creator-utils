# Sonido de tema de las apps de nu9ve v1

Estado: **propuesta** (2026-11-10), parte de la [auditoría Fable](../audits/fable-audit-2026-11-10.md). Desarrolla el [§12 del tema visual](nu9ve-theme-v1.md#12-sonido-y-háptica-de-tema): cómo una app de la suite suena a su skin usando **una sola fuente de sonidos**, la de Fractos, en vez de que cada app invente la suya. Fuera de Fractos nada de esto está implementado, y escribirlo no lo da por adoptado.

**Debe** es obligatorio salvo que la app documente por qué no aplica. **Conviene** es lo recomendado.

Leído el 2026-11-10 en `ios-fractos` (`main`, `f1a7bcb`), sin compilar ni escuchar nada: `docs/DESIGN_SYSTEM.md` (Sonido y Háptica), `docs/AUDIO_ENGINE.md`, `Packages/FractosKit/Sources/FractosSoundV2`, `Sources/FractosDesign/FractosCue.swift`, `Sources/NuMacShell/HapticPlayer.swift`, `Apps/macOS/FractosMac/SoundCues.swift`, `scripts/generate-sound-reference.ts` y `scripts/calibrate-sound-engine.sh`.

## 0. Principios

Se conservan los del tema §12; aquí sólo se resumen.

- **Nunca es la única señal** ni suena encima del trabajo (grabación, reproducción, mezcla, video a pantalla completa).
- **El tema decide el material; el gesto decide la forma.** Una voz por tema, un gesto por acción.
- **Fractos es la fuente.** La receta canónica es la del motor v2 de la web; las demás apps consumen, no rediseñan.
- **Los componentes no conocen el audio.** Un control avisa qué pasó (una señal) y la app decide qué suena y qué se siente. Es el diseño de Fractos (`\.fractosCue` → `SoundCues`) y es lo que permite adoptar sonido sin tocar las vistas.
- **Local y breve.** Sin red, y sin audio abierto mientras nada suena.

## 1. La fuente: el motor v2

Fractos tiene dos motores. La suite usa el segundo.

| | v1 · Sound Lab | **v2 · la fuente de la suite** |
| --- | --- | --- |
| Receta en la web (`nu9ve.xyz`) | `apps/fractos/lib/services/soundLabAudio.ts` | `packages/lib/src/audio/engine` (`themes.ts`, `levels.ts`); eventos en `packages/lib/src/audio/index.ts` (`UI_SOUND_EVENTS`) |
| Puerto nativo (`ios-fractos`) | `FractosSound`, usado por iOS | `FractosSoundV2`, usado por la app de Mac |
| Gestos | 11 | 26 (abajo) |
| Formato | Mono 44.1 kHz | Estéreo 48 kHz |
| Archivos de audio | 23 heredados en el bundle de iOS, sin licencia documentada | Ninguno: todo se sintetiza |
| Método de paridad | Manifiesto de 110 combinaciones | Referencia medida (`web-reference.json`), calibración y versión de receta |

Lo que el v2 ofrece:

| Hecho | Valor |
| --- | --- |
| Voces | 10: `light`, `dark`, `matrix`, `cyber`, `sangre`, `win99`, `pixel`, `bitcoin`, `y2k`, `nora94` |
| Gestos | `hover`, `focus`, `select`, `tab`, `nav`, `toggleOn`, `toggleOff`, `expand`, `collapse`, `popupOpen`, `popupClose`, `drop`, `send`, `open`, `close`, `enter`, `back`, `undo`, `warning`, `success`, `error`, `discovery`, `shift`, `portal`, `achievement`, `startup` |
| Niveles | Sonoridad y pico verdadero máximo por gesto (`FRACTOS_LEVEL_TARGETS`): de `hover` (−37, −20 dB) a `achievement` (−14, −3 dB). Lo frecuente es lo más bajo |
| Expresión | `direction` (una pestaña suena hacia donde va), lugar en la ventana (paneo), `durationMs` (un panel dura lo que su animación), intensidad por presión, variantes sin fin con semilla propia |
| Reproductor | Límite de redisparo por gesto (18–55 ms), máximo de voces por gesto, fatiga (una racha de repeticiones baja hasta 0.62), voz robada con desvanecido de 12 ms, limitador, y el motor duerme tras 20 s en silencio |
| Versión | `FRACTOS_SOUND_RECIPE_VERSION` (hoy 3). El gate de Fractos falla si el puerto y la web no coinciden |
| Referencia | 520 celdas (tema × gesto × 2 variantes) con sonoridad, pico verdadero, duración, centroide, correlación y energía por bandas, medidas del motor de la web |

- **Debe:** una app de la suite nombra la versión de receta que reproduce.
- El v1 queda para iOS hasta que migre. Ninguna app nueva lo adopta.

**Lo que esta página no pudo leer.** La receta canónica y más probada es la de la **web** (TypeScript), y ese repositorio no está en la máquina donde se escribió este contrato. Todo lo que aquí se dice del v2 sale del puerto de Swift y de los documentos de Fractos. Una sesión en la Mac, con `nu9ve.xyz` delante, debe confirmar la lista de gestos, los límites del reproductor y los niveles contra `packages/lib/src/audio/engine` antes de que este contrato deje de ser propuesta ([brief S19](../audits/session-briefs-2026-11-10.md#s19--confirmar-el-contrato-de-sonido-contra-la-web)).

**Niveles: una pregunta abierta para esa sesión.** Según `ios-fractos`: `docs/DESIGN_SYSTEM.md`, la tabla de niveles de la web se midió con un compresor que arranca hundido, así que en vivo los gestos cortos suenan hasta 12 dB por encima de su objetivo; el puerto nativo se calibró sin ese compresor. Si es así, la web y la Mac no suenan al mismo volumen relativo. Decidir cuál es el correcto y, si es el nativo, llevar la corrección a la web: es la única forma de que «todos usan los mismos sonidos» sea literal.

## 2. Señales y gestos

La tabla es la de Fractos ([origen](#9-implementación-compartida)); aquí se fija para toda la suite. La columna «señal» es lo que reporta un componente; la «gesto», lo que suena.

| Señal del componente | Dónde | Gesto |
| --- | --- | --- |
| `hover` | El puntero llega a un botón | `hover` |
| `press` | Un botón baja | `select` |
| `select` | Una opción entre varias: tile, filtro, fila | `select`, más leve |
| `step(direction)` | Flechas en una lista | `nav` |
| `tab(direction)` | Pestañas y cambio de vista, también por atajo | `tab` |
| `toggleOn`, `toggleOff` | Interruptor, casilla | `toggleOn`, `toggleOff` |
| `focus` | Un campo toma el foco por acción del usuario | `focus` |
| `expand`, `collapse` | Panel plegable, con la duración de su animación | `expand`, `collapse` |
| `popupOpen`, `popupClose` | Menús y popovers, salvo la barra de menús | `popupOpen`, `popupClose` |
| `link` | Un enlace que sale de la app | `enter` |
| `brand` | Interacción deliberada con la marca | `discovery` |
| `spaceEntered` | Otro material o skin | `shift` |
| `worldEntered` | Otro tema | `portal` |
| Ventana | Ampliar y restaurar, minimizar, cerrar, pantalla completa | `expand`/`collapse`, `collapse`, `close`, `enter`/`back` |
| Formulario | Enviar, entrar, fallo, no se puede enviar | `send`, `discovery`, `error`, `warning` |

Extensiones para apps de trabajo (nüün, nove, curado), **propuestas** y por confirmar con la fuente:

| Situación | Gesto | Nota |
| --- | --- | --- |
| Un trabajo largo termina bien | `success` | Sólo al terminar; nunca por archivo |
| Un trabajo largo falla | `error` | |
| Algo necesita atención sin ser un fallo | `warning` | Existe en el v2 |
| Llegó algo nuevo (tarjeta reconocida, aviso en el Buzón) | `discovery` | |
| Deshacer | `undo` | Incluido «Deshacer» de un cambio de agente |
| Soltar o asentar (arrastrar y soltar, segundo tope de Force Touch) | `drop` | |
| Un hito raro (cerrar un proyecto) | `achievement` | Raro por diseño |
| Arranque | `startup` | Sólo en el Setup con identidad o si el usuario lo pide |

- **Debe:** un gesto produce una sola señal. Un control que ya suena por su resultado no suena al pulsarse (la selección actual, las pestañas de navegación, los botones de ventana).
- **Debe:** una acción por atajo suena igual que por clic.
- **Debe:** las acciones largas ([interacción §7](nu9ve-interaction-v1.md#acciones-largas)) suenan al terminar, nunca durante el trabajo.

## 3. Skins y voces

Fractos ya empareja cada tema con un material (`ios-fractos`: `docs/DESIGN_SYSTEM.md`, «Materiales»). Esa tabla decide la voz de los skins que la suite comparte con Fractos.

| Skin de la suite | Material en Fractos | Tema y voz | Estado |
| --- | --- | --- | --- |
| Cristal (`glass`), claro / oscuro | `glass` | `light` / `dark` | Definido por Fractos |
| Macintosh (`classicMac`) | `classicMac` | `pixel` | Definido por Fractos |
| Windows 9x (`win9x`) | `win9x` | `win99` | Definido por Fractos |
| Terminal (`terminal`) | `terminal` | `matrix` | Definido por Fractos |
| Gótico (`gothic`) | `gothic` | `sangre` | Definido por Fractos |
| Cromo (`y2k`) | `chrome` | `y2k` | Definido; el material se llama distinto en cada lado |
| — | `neon`, `parchment`, `ledger` | `cyber`, `nora94`, `bitcoin` | Sólo en Fractos |
| Pixel (`pixel`), Grano (`dither`), Mínimo (`minimal`) | No existen | Por decidir | Propuesta: `pixel` para Pixel; `light`/`dark` más bajo para Mínimo; Grano por decidir escuchando |
| Originales por definir (nove «Puerto», kyu washi, neuf) | No existen | Por decidir o voz nueva | Una voz nueva se diseña en la web y entra a la receta |

- **Debe:** la voz la da el skin. No hay un selector de voz aparte.
- **Debe:** si el usuario cambia de skin, el cambio suena `shift` en la voz nueva.
- La diferencia de nombres (`y2k` ↔ `chrome`, y los tres skins de la suite que Fractos no tiene) debe resolverse en el [tema §5](nu9ve-theme-v1.md#5-catálogo-de-skins) ([auditoría C8](../audits/fable-audit-2026-11-10.md#c8--tres-vocabularios-de-tema-y-dos-de-sonido)).

## 4. Cómo llegan los sonidos a cada app

| Camino | Qué es | A favor | En contra |
| --- | --- | --- | --- |
| **Motor Swift** | Usar `FractosSoundV2` y su reproductor | Ya existe y está calibrado; expresión completa | Sólo Apple. Vive dentro del paquete de Fractos |
| **A. Paquete de archivos** | El motor nativo exporta cada voz, gesto, variante y dirección a archivos; la app los reproduce | Sin DSP nuevo; cualquier lenguaje reproduce un archivo; se puede escuchar y auditar | Pierde lo que Fractos llama «lo que un archivo fijo no podría»: dirección continua, duración atada a la animación, fuerza por presión y variantes sin fin |
| **B. Puerto a Rust (`nu9ve-sound`)** | El motor en un crate para Windows, Linux y Tauri | Expresión completa en todo sistema; una implementación para todo lo que no es Apple | Tercera implementación de la receta (web, Swift, Rust). Unas 4 000 líneas de DSP que portar |
| C. Cada app porta lo suyo | — | — | Rechazado |

**Recomendación por plataforma:**

- **Mac (nüün, SoundShop, nove, neuf, kyu):** el motor Swift. Es la misma decisión pendiente que `NuMacShell`: copiarlo con comentario de origen o mudarlo al repo común ([auditoría C9](../audits/fable-audit-2026-11-10.md#c9--los-contratos-de-la-suite-viven-en-el-repo-de-una-app)). `FractosSoundV2` sólo depende de `FractosCore` para registrar.
- **Windows, primera adopción (nüün):** camino **A**. Sirve para resolver lo que no es DSP: señales en los controles, Ajustes, reglas de silencio, latencia y reproducción. Lo que el reproductor sí conserva con archivos: el lugar (paneo al reproducir), los límites de redisparo, el máximo de voces y la fatiga.
- **Después:** camino **B**, cuando una segunda app que no es de Apple lo adopte o cuando la dirección y la duración importen. El método para medirlo ya existe en Fractos: la misma referencia y las mismas tolerancias que usó el puerto de Swift. Lo único de Apple en el motor es `Accelerate`.

Para el camino A:

- **Qué se exporta:** por voz y gesto, 3 variantes; para `tab` y `nav`, además las direcciones −1 y +1; para `expand` y `collapse`, la duración por defecto. Estéreo a 48 kHz.
- **De qué motor sale.** El usuario quiere que toda la suite use los mismos sonidos, y el motor real y más probado es el de la web. Hay dos orígenes posibles y la sesión de la Mac elige uno ([S19](../audits/session-briefs-2026-11-10.md#s19--confirmar-el-contrato-de-sonido-contra-la-web)):
  - **La web** (canónico): `ios-fractos`: `scripts/generate-sound-reference.ts --wav` ya renderiza cada celda con el motor de la web en Chrome, pero **antes de normalizar** y con dos rarezas de Chrome que el puerto nativo evita. Para servir de paquete, cada archivo se lleva después a su objetivo de sonoridad (`FRACTOS_LEVEL_TARGETS`) midiéndolo, sin depender de ninguna de las dos tablas de niveles.
  - **El puerto de Swift:** `FractosSoundEngine.render` ya entrega el sonido calibrado y sin rarezas. Es el mismo sonido dentro de las tolerancias de sus pruebas, no bit a bit.
- **Manifiesto:** versión de receta, commit del motor de origen, y por archivo la sonoridad y el pico medidos más su hash.
- También lo usan las apps de Mac para los **avisos del sistema**: una notificación suena con la app cerrada y necesita un archivo ([avisos §7](nu9ve-notifications-v1.md#7-sonido)).
- **Tamaño:** una app incluye sólo las voces de sus skins y los gestos que usa. Si pesa, se comprime sin pérdida.
- **Dónde vive:** copia fijada en cada app, con su origen y sus hashes, como hace Fractos con `Contracts/`.

## 5. Reproducción por plataforma

| Plataforma | Cómo | Evitar |
| --- | --- | --- |
| macOS | `FractosSoundV2Player`: mezclador propio de 24 voces sobre `AVAudioEngine`, con la siguiente toma de cada gesto ya renderizada | Crear un nodo por evento |
| Windows (WinUI) | API de baja latencia con búferes precargados (XAudio2 o un grafo de audio en mínima latencia) | `MediaPlayer` por evento: arranca tarde y se nota |
| Tauri (curado) | Desde el lado Rust, en un flujo aparte hacia la salida **predeterminada del sistema** | La salida de mezcla o de cue, y Web Audio como motor (invariante 3 de curado). Decidir con su mantenedor |
| Linux | PipeWire o libcanberra con rol de **evento** | Abrir el dispositivo ALSA directamente |

- **Debe:** lo que suena justo después de un clic ya está listo. Fractos precarga `hover`, `select`, `tab`, `nav`, `toggleOn`, `toggleOff`, `popupOpen` y `popupClose`.
- **Debe:** los límites del reproductor son los de la fuente (redisparo, voces por gesto, fatiga), no inventados por app.
- **Debe:** sin audio abierto en reposo. Si mantenerlo abierto es la única forma de cumplir la latencia, se mide y se anota.
- **Conviene:** latencia percibida menor a ~30 ms para `select` y `hover`. Si no se alcanza, es mejor no sonar.

## 6. Cuándo calla

- **Debe** callar: mientras Stalker o cualquier captura de audio del sistema graba; mientras curado o SoundShop reproducen o graban; con video a pantalla completa; con la app en segundo plano, salvo `success` o `error` de un trabajo largo que termina.
- **Debe** respetar al sistema: «Reproducir efectos de sonido de la interfaz» en macOS; el esquema de sonidos y el silencio del sistema en Windows; «No molestar» y modos de concentración. Si el sistema los tiene apagados, Ajustes lo explica.
- **Conviene** bajar o apagar `hover` con un lector de pantalla activo: VoiceOver y Narrador ya tienen sus sonidos.
- Para saber que **otra** app de la suite graba no hay hoy un canal. Mientras no exista, cada app aplica la regla sobre sí misma. Una señal compartida necesita dueño y es trabajo futuro.
- La app de Mac de Fractos, que es la referencia, hoy sólo calla en modo QA: estas reglas también son un pendiente suyo.

## 7. Valor por defecto, ajustes y formato

**Decisión del usuario (2026-11-10): encendido por defecto, con una opción clara para silenciar.** Vale para toda la suite y reemplaza el «apagado por defecto» que tenía el tema §12. La app de Mac de Fractos ya se comporta así.

Encendido por defecto obliga a que silenciar sea inmediato y a que el sonido nunca estorbe:

- **Debe:** un control de **Silenciar** a un paso, sin entrar a Ajustes: en el menú de la app en Mac, en el menú de la barra o de la bandeja en Windows, y con un atajo del catálogo. Dice su estado («Sonidos: silenciados»).
- **Debe:** la primera vez que la app suena, una línea discreta y descartable: «Los sonidos de la interfaz están encendidos · Silenciar · Ajustes». Una sola vez por instalación.
- **Debe:** silenciar es durable, sobrevive a reinstalar y también calla el sonido de tema de los avisos del sistema ([avisos §7](nu9ve-notifications-v1.md#7-sonido)).
- **Debe:** las reglas de [§6](#6-cuándo-calla) pesan más que el valor por defecto: con el sistema en silencio, con sus efectos de sonido apagados o mientras se graba, la app calla aunque esté «encendida».
- **Debe:** el volumen por defecto es moderado y lo frecuente es lo más bajo, como ya fija la receta (`hover` a −37 dB de sonoridad).
- El Setup con identidad es la excepción: sigue arrancando en silencio, porque todavía no hay preferencia de nadie ([windows-app-v1 §9](windows-app-v1.md#instalador-con-identidad)).

Ajustes → Apariencia, después de Tipografía:

| Opción | Valores | Por defecto |
| --- | --- | --- |
| Sonidos de interfaz | Encendido / Silenciado | Encendido |
| Volumen | 0–100 % | 60 % |
| Háptica | Encendido / Apagado; sólo si el equipo la tiene | Encendido |

Se añade al formato del [tema §4](nu9ve-theme-v1.md#4-lo-que-el-usuario-configura), validado campo por campo:

```json
{
  "sound": { "enabled": true, "volume": 0.6, "haptics": true }
}
```

- **Claves que ya existen:** `Nu9ve.Core` (`ThemePreferences.cs`, nüün Windows) guarda `app.interfaceSounds` sin que nada suene; Fractos Mac guarda `fractos.sound.enabled.v1` y `fractos.haptics.enabled.v1` en `UserDefaults` y no tiene volumen. Al adoptar el contrato, cada app migra su clave leyéndola una vez.
- La preferencia sobrevive a reinstalar, como el resto del tema.

## 8. Háptica

Fractos responde con sonido y tacto a la vez, para que se lean como un solo evento físico (`HapticPlayer` y `PressureMonitor` en `NuMacShell`, que no depende de Fractos).

- **Debe:** lo frecuente es lo más leve. Un tick al pasar por un control o dar un paso; un notch al pulsar un botón; dos toques para interruptor y pestaña con dirección; un golpe sólo para resultados y llegadas; nada para el foco y para cerrar un menú.
- **Debe:** sólo donde el equipo la reproduce (trackpad Force Touch en Mac) y con su propio interruptor.
- En Windows y Linux no hay equivalente general; no se simula.

## 9. Implementación compartida

Los **datos** (receta, referencia, paquete) se comparten desde el primer día; lo que se replica es el reproductor de cada plataforma.

| Pieza | Dueño | Dónde |
| --- | --- | --- |
| Receta y eventos | Fractos, web | `nu9ve.xyz`: `packages/lib/src/audio/engine`, `packages/lib/src/audio/index.ts` |
| Motor y reproductor Swift | Fractos | `ios-fractos`: `Packages/FractosKit/Sources/FractosSoundV2` |
| Señales de componente | Fractos | `FractosDesign` `FractosCue.swift` |
| Puente señal → gesto y tacto | Fractos | `Apps/macOS/FractosMac/SoundCues.swift` |
| Háptica | Fractos | `NuMacShell` `HapticPlayer.swift` |
| Referencia medida y calibración | Fractos | `scripts/generate-sound-reference.ts`, `scripts/calibrate-sound-engine.sh`, `Tests/FractosSoundV2Tests` |
| Exportador del paquete | Fractos | Por escribir |
| Tablas de §2 y §3 | Este contrato | Aquí |
| Reproductor C# | Primera app de Windows que lo adopte | `Nu9ve.WinUI` |
| Motor en Rust | Cuando se justifique | Candidato a `nu9ve-sound` |

## 10. Validación

- **Debe**, automático:
  - la app declara su versión de receta, y un paquete nombra el commit y los hashes de donde salió;
  - cada archivo de un paquete cae en el objetivo de sonoridad y bajo el pico máximo de su gesto;
  - la tabla de §2 no deja una señal de la app sin decidir (o la marca «sin sonido» a propósito);
  - los límites del reproductor y las reglas de silencio de §6 tienen prueba.
- **Debe**, para un puerto del motor: las celdas de la referencia de Fractos con sus tolerancias, como `FractosSoundV2Tests`.
- **Debe**, manual y registrado como tal: escuchar cada skin con bocinas y con audífonos; latencia percibida; que no suene al grabar con Stalker; comportamiento con el sistema en silencio. Fractos lo dice así: las medidas cuidan la jerarquía; el carácter lo decide el oído.

## Adopción

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

## Cambios

Regla nueva o más estricta: añadirla aquí, actualizar la tabla y avisar en cada app afectada. Cambiar una voz de §3 la cambia en todas las apps que tienen ese skin. Un cambio de receta en la web sube `FRACTOS_SOUND_RECIPE_VERSION` y obliga a regenerar todo paquete.
