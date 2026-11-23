# GPU en la suite nu9ve: Metal, shaders y render común

Estado: **propuesta** del 2026-11-19, pedida por el usuario en la sesión de artefactos de datos (S8). No hay nada construido y no se movió código. El inventario se leyó en sólo lectura ese día, sobre Clipper `0e3056f2`, `ai/video` `8a5214d`, SoundBender `4045b8c`, Atlas `7c96b94`, nüün `b14e83b`, y sobre Kyū, curado, neuf y nove en su estado de esa fecha. [Mapa de la suite](../suite.md) · [Qué se extrae](../suite-adoption.md) · [Audio en la suite](audio.md) · [Artefactos de datos v1](../../contracts/nu9ve-artifact-v1.md) · [Licencias](../licensing.md).

Responde a esta pregunta: ¿qué partes de Metal y de la GPU se repiten entre apps y podrían ser una pieza común en `nu9ve/core`, como ya lo son los crates `nu9ve-*` de Rust?

## 1. Respuesta corta

- **Sí hay repetición,** pero casi toda está en la **infraestructura**, no en los efectos. Cinco apps de Mac abren su propio `MTLDevice`, compilan MSL desde texto, guardan pipelines, pasan `CVPixelBuffer` a textura y leen cuadros de vuelta. Eso cumple la regla del [AGENTS](../../AGENTS.md) (dos apps que lo usan igual) y puede ser la primera pieza.
- **Los efectos de video** se repiten entre Clipper y `ai/video`, pero en dos lenguajes: MSL en Clipper y WGSL en `ai/video`. Además, el aura de `ai/video` se copió a mano del cristal de Clipper. Antes de compartirlos hace falta elegir un idioma fuente (§4).
- **Lo que nació genérico** y todavía no se repite: reproyectar un equirectangular 360 a una vista, componer con alfa a un tiempo `t` y evaluar la animación declarada de un artefacto. Son funciones puras de píxeles, parámetros y `t`.
- **No conviene un motor de render común.** Clipper exporta con AVFoundation y Core Image, `ai/video` con Remotion y `vgpu`, SoundBender dibuja una escena interactiva con MetalKit y Atlas procesa en CPU (con MLX en plan). Igual que en el audio, lo que se comparte son primitivas con vectores y cada app conserva su motor ([audio §1](audio.md#1-respuesta-corta)).
- **Para Windows:** WGSL como fuente única de los kernels puros, traducido a MSL para los hosts de Swift y servido por `wgpu` en Rust cuando una app de Windows lo pida. Lo que toca frameworks de Apple sigue en Swift (§4).

## 2. Inventario

| App | GPU hoy | Lenguaje | Dónde |
| --- | --- | --- | --- |
| Clipper | 7 kernels de cómputo en MSL (CRT, ruido analógico, glitch de paleta, chromatic dreams en tres pasadas, signal glitch, efectos de marca, cristal de intro). Export `timeline-metal`: AVAssetReader → `MTLTexture` → kernel → `CVPixelBuffer` → AVAssetWriter. Overlays con alfa compuestos con Core Image (`composited(over:)`), clips alfa en ProRes 4444 y tres lectores de video por tiempo (`GeneratedIntroTile` con bucle, `OverlayClipReader`, `MotionVideoFrames`). `ClipperMotion` pinta escenas en Core Graphics, con BGRA premultiplicado. Además, 12 shaders `.hook` de libplacebo para el camino de FFmpeg | Swift, MSL; GLSL de libplacebo | `native/clipper-macos-render/Sources/ClipperMacOSRender/{main.swift,OverlayClips.swift,Shaders/}`, `Sources/ClipperMotion/`, `assets/shaders/`, `docs/2026-shaders-libplacebo-and-metal.md` |
| `ai/video` | 6 shaders WGSL con `vgpu` 0.3.1 (WebGPU en Node sobre Metal): aura, intervención digital, mineral, post del mineral, estudio y visor con raymarching. Placas PNG por cuadro, destinos `rgba16float` (HDR) y `rgba8unorm`, y lectura de vuelta para Remotion. Ambiente 360: el shader muestrea un equirectangular con yaw y ganancia | WGSL, JS | `src/vgpu/shaders/`, `scripts/fractos-studio.mjs`, `scripts/render-vgpu-sequence.mjs`, `docs/vgpu-integration.md` |
| `ai/video` · 360 | Cosido del X3 (dos ojos de pez a equirectangular) **en CPU**: lee la calibración del trailer de Insta360, calcula tablas de remapeo (modelo de Mei con distorsión), funde la costura y sólo usa FFmpeg para decodificar. La orientación del IMU rota el equirectangular en JS (`rotateRGB`, `rotateHalfRGBA`), con línea de tiempo en ping-pong | Python (NumPy, OpenCV), JS | `scripts/native-360/x3.py`, `orientation.py`, `scripts/environment/{video,orientation,hdri,lighting}.mjs`, `docs/native-x3.md` |
| SoundBender | `Scene.metal` (445 líneas): cuerpos por banda, cajas, líneas, partículas, campos y bloom HDR con composición. `Spectrum.metal`: el espectrograma como textura y como superficie 3D. Escena espacial interactiva en MetalKit | Swift, MSL | `Sources/SoundShop/{SceneRenderer,SceneBasicRenderer,SpectrumView}.swift`, `Resources/*.metal`, `docs/plans/spatial-scene.md` |
| Kyū | Revelado RAW: Core Image + `CIRAWFilter` + un kernel MSL fusionado, en lineal Rec.2020, compilado en tiempo de ejecución porque Xcode 26 separa el Metal Toolchain. Caché de pipelines por dispositivo y visor en `CAMetalLayer`. **Plan:** etapa 3 con `wgpu` y pruebas de paridad contra Metal | Swift, MSL | `apps/macos/Sources/KyuRender/{DevelopKernel,RenderContext}.swift`, `docs/STATUS.md` |
| curado | Ventana de visuales en un proceso aparte con MTKView (336 líneas), a 20 Hz y con telemetría reducida. `wgpu` está pendiente para Windows | Swift | `src-tauri/native/CuradoVisuals.swift`, ADR-006 en `docs/DECISIONS.md` |
| neuf | Vista previa con descriptor de shader Metal | Swift | `apps/Neuf/Neuf/PreviewRuntimeViews.swift` |
| Atlas | Mosaico, DSM y ortofotos **en CPU** (OpenCV, rasterio). ODM y OpenMVS corren como procesos externos. **Plan:** barrido del DSM en GPU con MLX o PyTorch MPS (M6d) y kernels Metal propios sólo como último recurso (M17) | Python | `atlas_vision/`, `docs/processing-profiles.md` §4, `config/engines.json` |
| nüün | Sin GPU. Clasifica `dji-drone`, `dji-360` e `insta360` (INSV, INSP) al ingresar y tiene el plan físico de INSV en Rust. No lee píxeles | Swift, Python, Rust | `native/MediaMenu/Sources/MediaMenu/{Model,ImportViews}.swift`, `docs/contracts/atlas-drone-v1.md` |
| nove | Decidió Swift Charts (y Chart3D) y **no** un motor propio sobre Metal o `wgpu` | Swift, Rust | `docs/chart-renderers.md` |

Repeticiones de hoy:

- **Arranque de Metal** (dispositivo, cola, MSL compilado en tiempo de ejecución, caché de pipelines, despacho 2D): Clipper (seis veces dentro del mismo `main.swift`), Kyū, SoundBender, curado y neuf.
- **`CVPixelBuffer` ↔ `MTLTexture` y lectura de vuelta a PNG:** Clipper y Kyū; en WebGPU, `ai/video`.
- **La misma familia visual en dos lenguajes:** el aura de `ai/video` toma la geometría de facetas, la dispersión cromática y el ritmo de ataque y caída del cristal de Clipper (`docs/vgpu-integration.md`).
- **Grano, viñeta y aberración cromática:** Clipper (ruido analógico, CRT, chromatic dreams), Kyū (viñeta y grano del revelado) y `ai/video` (post del mineral).
- **HDR, bloom y tono:** SoundBender (`sceneBright`, `sceneComposite`) y `ai/video` (`rgba16float` y post).
- **Cuadros por tiempo con bucle:** el `truncatingRemainder` de Clipper y el ping-pong de `ai/video`.

## 3. Candidatos

Una fila por candidato. «Regla» dice si cumple la del AGENTS: **dos** (dos apps lo usan igual), **genérico** (nació genérico) o **no todavía**.

| # | Candidato | Dónde vive hoy | Quién lo usaría | Regla | Forma propuesta |
| --- | --- | --- | --- | --- | --- |
| G1 | **Arranque de Metal:** dispositivo y cola, MSL desde texto con caché de pipelines por fuente, despacho 2D con su rejilla, textura desde `CVPixelBuffer` (`CVMetalTextureCache`), lectura de vuelta a BGRA o PNG | Clipper `main.swift`, Kyū `DevelopKernel.swift`, SoundBender, curado, neuf | Las cinco | **Dos**, de sobra | `swift/Sources/Nu9veMetal`: un paquete pequeño sin efectos. Nombra la fuente del kernel y la app le pasa la suya |
| G2 | **Lectura de equirectangulares 360 a cuadros:** reproyectar a una vista rectilínea (yaw, pitch, roll, campo de visión), rotar el equirectangular entero por una orientación del IMU y muestrear un mapa de ambiente | `ai/video`: `orientation.mjs` (CPU) y el muestreo del ambiente en `fractos-mineral-studio.wgsl` | `ai/video` hoy. Clipper, si mete tomas 360 en un timeline. SoundBender, para su fondo de VR (fase 5). nüün, para miniaturas de INSV | **Genérico** (matemática pura) | Kernel WGSL con vectores de imagen y tolerancia (§4), más su referencia en CPU. El cosido del X3 (dos ojos de pez) **no** entra: depende del aparato y queda en `ai/video`. Sí puede entrar después un kernel genérico de «remapeo por tabla», que es su paso caro |
| G3 | **Imágenes de dron a cuadros:** recorrer con la cámara una ortofoto o una foto nadir grande (paneo y zoom por una ruta), leer por teselas de una pirámide sin cargar la imagen entera y superponer las capas de Atlas | Nada lo hace hoy. Atlas produce ortofotos, teselas y DSM en CPU; el paquete de ejemplo del core (`atlas-dsm`) ya trae 24 cuadros | Atlas como productor. Clipper y `ai/video` como consumidores | **No todavía.** Hay un solo productor y ningún lector | Primero por el contrato: Atlas exporta `motion: frames` de `nu9ve-artifact-v1` y no hace falta GPU común. Si aparece un segundo productor, una **ruta de cámara declarada** sobre una imagen grande sería una versión menor del contrato (P3). La GPU de Atlas (DSM con MLX) es de su dominio y se queda allá |
| G4 | **Efectos de video comunes:** kernels deterministas de la forma `(fuente, destino, {tamaño, t, progreso, intensidad, semilla})` → grano o ruido analógico, CRT, aberración cromática, glitch de paleta, viñeta y la familia cristal/aura | Clipper (7 MSL y 12 `.hook`), `ai/video` (WGSL), Kyū (viñeta y grano dentro del revelado) | Clipper y `ai/video` | **Dos** para grano, aberración y cristal/aura. El resto sólo lo tiene Clipper | Una carpeta de kernels WGSL con un manifiesto (parámetros, rangos, `t`), sus imágenes doradas y su MSL generado (§4). Se mudan uno por uno y sólo los que ya estén en dos apps. Kyū conserva su revelado: es su color, no un efecto |
| G5 | **Composición con alfa:** `sourceOver` premultiplicado, en un espacio de color declarado (sRGB o lineal), con clips por intervalo de tiempo, bucle o ping-pong y la regla de qué cuadro corresponde a `t` | Clipper: Core Image `composited(over:)`, `OverlayClipReader`, `GeneratedIntroTile`, BGRA premultiplicado de `ClipperMotion` y ProRes 4444. `ai/video`: composición de Remotion y ping-pong del ambiente | Clipper y `ai/video`, y cualquiera que consuma cuadros de un artefacto | **Genérico.** La matemática es trivial; lo que vale es la regla | Una **regla escrita con vectores** antes que código: alfa premultiplicado o recto en cada frontera, espacio de color en el que se compone, cuadro de `t` con redondeo explícito, bucle y ping-pong. Su lugar natural es junto a `nu9ve-artifact-v1` §8 (P2). El kernel `over` en WGSL sería la parte más chica |
| G6 | **Evaluador de la animación declarada** de un artefacto: dado `motion.tracks` y `t`, el valor de cada pista con sus curvas (`linear`, `in`, `out`, `in_out`, `step`) | No existe. El contrato lo define (§8.2) y Clipper responde `motion_unsupported` | `ai/video` (quien redibuja) y Clipper (si deja de rechazarla) | **Genérico** | Es CPU, no GPU: una función `motion_at(t)` en `rust/nu9ve-artifact` y en la referencia `scripts/artifact-examples.py`, con vectores. Es lo que conecta el render común con los artefactos (§5) |
| G7 | **HDR, bloom y tono:** extraer los brillos, desenfocar en pirámide y componer con una curva de tono | SoundBender (`Scene.metal`), `ai/video` (`rgba16float` y post del mineral) | SoundBender y `ai/video` | **Dos**, pero con fines distintos (escena interactiva frente a cuadro de video) | Esperar. Se comparte cuando los dos acepten la misma curva y los mismos parámetros; mientras tanto, cada uno el suyo |
| G8 | **Visualizaciones de datos y espaciales:** cuerpos por banda, partículas, campos, espectrograma como superficie, visuales que reaccionan al audio | SoundBender (escena espacial, espectro), curado (`CuradoVisuals`) | SoundBender y curado | **No todavía.** Ambas dibujan audio, pero sin compartir modelo ni pasadas | Se quedan en cada app. Lo común entre ellas es G1 y quizá G7. Una visualización que deba salir a video se exporta como artefacto (`frames`), no como renderer compartido. nove ya decidió no construir un motor de gráficas |
| G9 | **Lectura y escritura de video** (decodificar a textura y codificar desde textura) | Clipper (AVFoundation), `ai/video` y Atlas (FFmpeg como proceso) | Todas las que exportan video | **No.** Depende de la plataforma | Se queda en cada app. Primero Clipper une sus tres lectores por tiempo. La regla de G5 dice qué cuadro corresponde a `t`; la implementación es de cada plataforma |

Fuera de esta lista: el revelado RAW de Kyū, el DSM de Atlas y el cosido del X3. Cada uno es de su app.

## 4. Metal y Swift frente a shaders portables

| Opción | A favor | En contra |
| --- | --- | --- |
| **A · Sólo Metal y Swift** (`swift/Sources/Nu9veMetal` con kernels MSL) | Se integra sin copias con `CVPixelBuffer`, IOSurface, Core Image y AVFoundation, que es donde viven Clipper y Kyū. Es lo que ya escriben cinco apps | Sólo Mac. `ai/video` ya está en WGSL y tendría que reescribir. El paso de Kyū a Windows (etapa 3) y el de SoundBender (fase 4) quedan sin nada |
| **B · Todo en `wgpu` y Rust** (`rust/nu9ve-gpu` con WGSL) | Un solo código para Metal, DX12 y Vulkan. WGSL ya es el idioma de `ai/video`. Kyū y curado ya nombran `wgpu` para Windows | Pasar por Rust para algo que vive en AVFoundation agrega una frontera (FFI y probablemente copias de textura). Ninguna app de Mac lo pidió. Hay que sostener `wgpu` con la MSRV 1.89 de Clipper |
| **C · WGSL como fuente única, con un host por plataforma** | Los kernels puros (G2, G4, G5) se escriben una vez en WGSL. `naga` los traduce a MSL al sincronizar, y el MSL generado se versiona junto al WGSL para que los hosts de Swift lo compilen en tiempo de ejecución, como ya hace Kyū. En Windows o sin interfaz, `wgpu` corre el mismo WGSL. `ai/video` lo usa sin traducir | La traducción de `naga` tiene bordes (tamaño de grupo, muestreo, `f16`) y hacen falta imágenes doradas con tolerancia en los dos backends. Son dos hosts que mantener, aunque el de Swift ya existe en cada app |

**Propuesta: C**, en dos capas:

- **`Nu9veMetal` en Swift** para la infraestructura que toca Apple (G1): no lleva efectos y no hay que traducir nada.
- **Kernels puros en WGSL** con su manifiesto, su MSL generado y sus vectores. Tendrían una carpeta propia, por ejemplo `gpu/` o `shaders/`; el nombre se decide (P1). El host de `wgpu` en Rust (`rust/nu9ve-gpu`) llega sólo cuando una app de Windows lo necesite, previsiblemente Kyū en su etapa 3.

Igual que con el audio, **los números deben coincidir:** cada kernel trae imágenes de entrada y de salida con una tolerancia por canal, y cada backend (MSL desde Swift, `wgpu`, `vgpu`) las pasa. Es la «paridad contra Metal» que Kyū ya planea.

## 5. Encaje con `nu9ve-artifact-v1`

Un artefacto con cuadros o con animación declarada es la entrada natural de un render común, porque ya fija lo difícil: tamaño, fondo transparente o no, cuadros con alfa y el póster como último cuadro, todo determinista por `t` ([§8](../../contracts/nu9ve-artifact-v1.md#8-animación)).

- **`motion: frames`:** el consumidor sólo necesita G5 (qué cuadro toca en `t` y cómo se compone con alfa). Clipper ya lo hace: codifica a ProRes 4444 y compone con Core Image. Escribir la regla evita que `ai/video` la decida distinto.
- **`motion: declared`:** necesita G6 para los valores y un redibujo desde la receta, que es de quien produce o de `ai/video`. G6 vive en `nu9ve-artifact` y no en la GPU.
- **Dron y 360:** Atlas (y, después, `ai/video` con un ambiente) exportan cuadros como artefacto. La GPU común aparece cuando dos apps necesiten producir esos cuadros desde la misma fuente (G2, G3).

## 6. Licencias

Reglas del core: nada copyleft en `rust/` (`cargo-deny`) y se puede vender cerrado ([licencias](../licensing.md)).

| Componente | Licencia | Qué significa |
| --- | --- | --- |
| Metal, Core Image, AVFoundation, MetalKit | Frameworks del sistema | Sin obligaciones de distribución |
| `wgpu`, `naga` | MIT o Apache-2.0 | Aptos; entran a `deny.toml` sin excepción. Falta confirmar su MSRV contra 1.89 antes de fijarlos |
| `vgpu` (npm, 0.3.1) | MIT | Apto. Es de `ai/video` y no entra al core |
| `telemetry-parser` (estructura del trailer del X3) | MIT o Apache-2.0 | Apto, y ya lleva su aviso en `scripts/native-360/` |
| OpenCV 4.12 o posterior, NumPy | Apache-2.0, BSD | Aptos. Son de `ai/video` y Atlas |
| MLX, PyTorch (plan de Atlas) | MIT, BSD | Aptos |
| MoltenVK (libplacebo en Mac) | Apache-2.0 | Apto |
| libplacebo y FFmpeg | LGPL-2.1 o posterior (FFmpeg puede ser GPL según cómo se compile) | Clipper los usa como **proceso externo**. No se enlazan ni entran al core |
| ODM, OpenMVS | AGPL-3.0 | Atlas los corre como proceso (Docker). Nunca se copian ni se enlazan |
| Gyroflow | GPL-3.0 | No se copia nada de su código para la estabilización 360 |
| Remotion | Licencia propia: gratis para personas y equipos chicos, de pago para empresas más grandes | Es de `ai/video`. Hay que revisarla si la suite se vuelve empresa |
| **Shaders de terceros** | Shadertoy usa por omisión CC BY-NC-SA 3.0, que **prohíbe el uso comercial** | Clipper documenta `chromatic_dreams_3` como port de un shader de Shadertoy (`docs/2026-shaders-libplacebo-and-metal.md`). Antes de mudar un efecto al core hay que confirmar su origen y su licencia, o reescribirlo limpio (P4). Ningún shader de la suite lleva hoy una cabecera de origen |

Propuesta: cada kernel que entra al core lleva en su manifiesto `origin` (propio, o la URL y la licencia de su fuente). Un kernel sin origen claro no entra.

## 7. Orden propuesto

1. **G6, el evaluador de `motion: declared`,** en `rust/nu9ve-artifact` y en la referencia de Python, con vectores. Es pequeño, no necesita GPU y destraba que Clipper deje de responder `motion_unsupported`. Es un cambio propio del contrato de artefactos: versión menor, sin romper nada.
2. **G5, la regla de composición y de tiempo,** escrita con vectores (premultiplicado, espacio de color, cuadro de `t`, bucle y ping-pong), junto a artefactos §8 o como contrato propio (P2). Clipper y `ai/video` comprueban que la cumplen.
3. **G1, `Nu9veMetal` en `swift/`,** desde Kyū (el más limpio, con caché de pipelines y MSL en tiempo de ejecución) y con Clipper como segundo usuario, que de paso une sus seis arranques. SoundBender, curado y neuf lo adoptan cuando toquen su render.
4. **La carpeta de kernels WGSL con un primer kernel:** el grano o la aberración cromática, que ya están en Clipper y en `ai/video`. Trae manifiesto, origen, MSL generado con `naga`, imágenes doradas y la prueba en Swift y en `vgpu`. Con eso se valida la opción C de §4 antes de mudar más.
5. **G2, la reproyección 360,** cuando un segundo usuario (Clipper o SoundBender) lo pida. Antes, `ai/video` puede pasar su `rotateRGB` de CPU al kernel.
6. **G4, el resto de los efectos,** uno por uno y sólo si ya están en dos apps y tienen su origen confirmado.
7. **Espera su condición:** G3 (otro lector de imágenes de dron), G7 (que SoundBender y `ai/video` acepten la misma curva), G8 (no se extrae) y `rust/nu9ve-gpu` con `wgpu` (la primera app de Windows que lo use).

Cada paso es su propio cambio aquí, con su prueba, su línea en el CHANGELOG y la copia fijada en la app de origen ([AGENTS](../../AGENTS.md)).

## 8. Preguntas para el usuario

- **P1. ¿WGSL como fuente única** de los kernels puros, con MSL generado para Swift (opción C)? ¿O Metal sólo en Swift (A) hasta que haya una app de Windows con GPU? ¿Y cómo se llama la carpeta: `gpu/`, `shaders/` o dentro de `swift/`?
- **P2. ¿La regla de composición y de tiempo (G5)** va como sección nueva de `nu9ve-artifact-v1` o como contrato propio (`nu9ve-frames-v1`), que también usarían los clips alfa de Clipper que no vienen de un artefacto?
- **P3. ¿Una ruta de cámara declarada sobre una imagen grande** (paneo y zoom sobre una ortofoto) entra como versión menor de artefactos, o Atlas entrega siempre cuadros ya hechos?
- **P4. ¿Qué se hace con los efectos de origen dudoso** (`chromatic_dreams_3` y los que sean ports): se busca su licencia, se reescriben limpios o se quedan sólo en Clipper?
- **P5. ¿Qué versión mínima de macOS tiene `Nu9veMetal`?** `NuMacShell` pide macOS 15, Clipper declara 14 y curado 12.
- **P6. ¿`ai/video` entra a la suite** con `sync-core.sh` y una copia fijada? Hoy no sincroniza el core, y G2, G4 y G5 lo tienen como primer usuario.
