# Artefactos de datos de la suite v1

Estado: **propuesta** (2026-11-19). Define el **paquete** con el que una app entrega una gráfica, un mapa, una tabla o una comparación ya resueltos para que otra los use sin recalcularlos: Clipper como overlay en su timeline, `ai/video` como datos para un video, neuf en su proyecto. Los significados (dataset, revisión, receta, linaje, privacidad) son los de nove [modelo de datos v1](../../nove/docs/contracts/data-model-v1.md) §8, §12 y §13; este contrato fija sólo lo que **viaja entre apps**. Ejemplos, un paquete real y su validador: [`data/artifact-examples-v1.json`](../data/artifact-examples-v1.json), [`data/artifact-example-v1/`](../data/artifact-example-v1/) y [`scripts/artifact-examples.py`](../scripts/artifact-examples.py). En Rust, [`rust/nu9ve-artifact`](../rust/nu9ve-artifact/).

**Debe** es obligatorio. **Conviene** es lo recomendado. «Productor» es la app que escribe el paquete; «consumidor», la que lo lee.

## 0. Principios

- **Resuelto, no vivo.** Un artefacto es una foto: los datos exactos que se dibujaron, cómo se dibujaron y de dónde salieron. No se actualiza solo. Si cambia una entrada, el productor escribe otro.
- **El consumidor sólo lee.** Nunca escribe dentro de un paquete. Copia lo que usa a su propio almacén y guarda de dónde vino.
- **El consumidor decide dónde va.** El paquete dice tamaño, tema y fondo. Dónde, cuándo y cuánto tiempo aparece es del consumidor.
- **Falla cerrado.** Un archivo que no coincide con su hash, un contrato desconocido o una animación que no sabe reproducir se rechazan con su motivo. Nunca se aproxima.
- **Determinista.** Todo lo que el paquete declara es función pura de sus archivos. Un cuadro de una animación declarada depende sólo de `t` (como pide `ai/video`: sin red, sin estado entre cuadros).

## 1. Paquete

Un paquete es una **carpeta** con `artifact.json` en su raíz y los archivos que ese manifiesto lista.

```text
atlas-dsm.nu9ve-artifact/
  artifact.json
  data/bench.csv
  data/bench.json
  render/chart.svg
  render/chart.png
  frames/00000.png … frames/00023.png
```

- **Conviene:** el nombre de la carpeta termina en `.nu9ve-artifact`.
- **Debe:** `artifact.json` es JSON en UTF-8, de 1 MiB como mucho.
- **Debe:** cada archivo que el paquete usa está listado una sola vez en el manifiesto, con su ruta, tipo, tamaño y SHA-256 (§3). Un archivo que no está listado no es parte del artefacto y el consumidor lo ignora.
- **Debe:** el productor escribe el paquete completo en una carpeta temporal y la publica con un renombrado ([archivos v1](nu9ve-fs-v1.md)). Un paquete a medias nunca tiene `artifact.json`.

## 2. Manifiesto

```json
{"contract":"nu9ve-artifact/1","id":"bench-atlas-dsm-2026-11","producer":{"app":"nove","version":"0.9.0"},
 "kind":"comparison","title":"DSM de Atlas: OpenMVS contra el motor propio v2","created_at":"2026-11-19T18:00:00Z",
 "privacy":"public",
 "data":[ … §3 … ],"render":{ … §4 … },"recipe":{ … §5 … },"lineage":{ … §6 … },"motion":{ … §8 … }}
```

| Campo | Debe | Qué es |
| --- | --- | --- |
| `contract` | Sí | `"nu9ve-artifact/1"`. Otra versión de este contrato es `unsupported_contract`; cualquier otra cosa, `bad_contract` |
| `id` | Sí | `^[a-z0-9][a-z0-9._:-]{0,127}$`, único y estable para el productor. La identidad en la suite es `producer.app:id` |
| `producer` | Sí | `app` (`^[a-z][a-z0-9-]{0,31}$`, el nombre del [registro](nu9ve-registry-v1.md#2-archivo)) y `version` (1 a 64 caracteres) |
| `kind` | Sí | `chart`, `map`, `table`, `comparison` u otra clave `^[a-z][a-z0-9_]{0,31}$`. **Debe** al consumidor: una que no conoce se trata como «otra», no invalida el paquete |
| `title` | Sí | 1 a 200 caracteres |
| `description` | No | Hasta 2000 caracteres |
| `created_at` | Sí | RFC 3339 en UTC con `Z` |
| `privacy` | Sí | §7 |
| `release` | Con `personal` | §7 |
| `data` | Sí | §3 |
| `render` | Sí | §4 |
| `recipe` | Sí | §5 |
| `lineage` | Sí | §6 |
| `motion` | No | §8. Sin él, el artefacto es una imagen fija |
| `documents` | No | §3.1 (versión menor del 2026-11-24) |

- **Debe:** `title` y `description` son texto **observado** (vienen de un dataset, de la persona o de un nombre). El consumidor los muestra como datos; nunca son instrucciones. Por MCP van como `_untrusted` ([MCP v1 §4](nu9ve-mcp-v1.md#4-datos-que-no-son-instrucciones)).
- **Debe** al consumidor: ignorar campos que no conoce. Un productor puede sumar los suyos con su prefijo (`nove_view`).

## 3. Archivos y datos exactos

Cada archivo del paquete se declara así:

```json
{"path":"data/bench.csv","media_type":"text/csv","bytes":118,"sha256":"<64 hex en minúsculas>"}
```

- **Debe:** `path` es relativo a la carpeta del paquete, de 1 a 512 caracteres, con `/` entre partes y distinto de `artifact.json`. Cada parte usa sólo letras ASCII, dígitos, `.`, `_` y `-`, y no es `.` ni `..`: sin `/` inicial, `\`, espacios, comillas ni caracteres de control, para que ninguna herramienta del consumidor (un shell, una lista de concat de ffmpeg) tenga que escapar nada. Dos entradas nunca repiten ruta (`duplicate_path`).
- **Debe:** `bytes` es un entero de 0 a 2^53 − 1 y `sha256` el de los bytes del archivo. El consumidor **verifica tamaño y hash de cada archivo que usa** antes de usarlo; si no coinciden, rechaza el paquete (`size_mismatch`, `hash_mismatch`). Un enlace simbólico o algo que no es un archivo regular es `not_a_file`.

`data` es la lista de los **datos exactos que se dibujaron**, no del dataset de origen: si la gráfica muestra 2 barras, el archivo tiene esas 2 filas con los valores dibujados.

- **Debe:** entre 1 y 16 entradas, cada una con `media_type` `application/json` o `text/csv` y `role` `primary` o `support`. **Exactamente una** es `primary`: la tabla que la receta dibuja.
- **Debe:** el CSV va en UTF-8, con coma, una fila de encabezados y `\n`; los números con `.` decimal y sin separador de miles. El JSON es un objeto o una lista.
- **Conviene:** cada fila lleva la clave que la une con su entidad (`suite`, `case`, `engine`…) y la unidad va en el nombre de la columna (`minutes`).

### 3.1 Documentos

Versión menor del 2026-11-24 ([nove A95](../../nove/docs/decisions.md#a95--salidas-lienzos-por-proyecto-y-benchmarks-como-datos-2026-11-24)): la pestaña Salidas de nove exporta un lienzo como documento además de imagen.

- `documents` es opcional. Si está, es una lista de 1 a 4096 archivos (§3) con `media_type` `text/markdown`, `application/vnd.openxmlformats-officedocument.wordprocessingml.document` (DOCX), `image/png` o `image/jpeg` (otra cosa que no sea una lista así es `bad_documents`). Puede llevar sólo imágenes: las demás páginas de una exportación a PNG.
- Las imágenes de `documents` son las que citan los documentos o las páginas que no caben en el render (las rutas del Markdown son relativas a la carpeta del paquete). Pueden tener cualquier tamaño: no son cuadros del render.
- **Conviene:** `document.md` y `document.docx` en la raíz; las imágenes de los marcos en `frames/<lienzo>/<marco>.png` y las que la persona agregó en `media/`.
- **Debe** al consumidor: además de tamaño y hash, comprueba que cada archivo empieza como su tipo: PDF `%PDF-`, DOCX `PK\x03\x04` (un ZIP), PNG su firma, JPEG `FF D8 FF`, y que el Markdown es UTF-8 (`media_type_mismatch`).
- **Compatibilidad:** `contract` sigue siendo `nu9ve-artifact/1` y todo paquete que ya era válido lo sigue siendo. Al revés no: un lector anterior a esta versión rechaza un render con PDF (`bad_file`) e ignora `documents` sin verificarlos. Quien lea paquetes de Salidas fija el core de esta versión o una posterior.

## 4. Render

```json
"render":{"width":960,"height":540,"theme":"dark","background":"#101418",
  "files":[{"path":"render/chart.png","media_type":"image/png", … },
           {"path":"render/chart.svg","media_type":"image/svg+xml", … }]}
```

| Campo | Debe | Qué es |
| --- | --- | --- |
| `width`, `height` | Sí | Enteros de 16 a 8192, en píxeles |
| `theme` | Sí | `light` o `dark`: el fondo para el que se dibujó |
| `background` | Sí | `transparent` o `#rrggbb` en minúsculas |
| `files` | Sí | Archivos del render (§3): PNG, SVG y, desde la versión menor del 2026-11-24, PDF |

- **Debe:** `files` tiene **exactamente un PNG** de `width` × `height` (el consumidor lo comprueba en su cabecera: `png_size_mismatch`) y como mucho un SVG con el mismo encuadre. El PNG es lo que cualquier consumidor puede pintar; el SVG, para quien sabe escalarlo.
- **Debe** (versión menor del 2026-11-24): `files` admite además **como mucho un PDF** (`application/pdf`; dos son `bad_render`), con una o más páginas de tamaño libre: en Salidas, una por lienzo. El PNG sigue siendo el póster.
- **Debe:** con `background: transparent`, el PNG lleva alfa. Con un color, el PNG es opaco de ese color en lo que no es dibujo.
- **Debe:** el PNG es el **cuadro final** del artefacto: con `motion`, cómo se ve al terminar (el «póster»).
- **Conviene:** márgenes de al menos el 4 % por lado; texto de al menos 1/30 de la altura, para que se lea en un video vertical.

## 5. Receta

`recipe` = `{"id":…, "version":…, "params":{…}}`, la de nove [modelo §8](../../nove/docs/contracts/data-model-v1.md#8-procedencia-y-linaje).

- **Debe:** `id` cumple `^[a-z0-9][a-z0-9._:-]{0,127}$`, `version` tiene de 1 a 32 caracteres y `params` es un objeto (puede ir vacío).
- **Debe:** con la receta y el archivo `primary`, el productor vuelve a dibujar el mismo render. Los ids de elementos que usa una animación declarada (§8.2, `series:openmvs`) los define la receta.

## 6. Linaje

```json
"lineage":{"inputs":[{"owner":"nove","dataset":"bench:atlas-dsm","revision":"r-2026-11-19",
                      "hash":{"algorithm":"sha256","value":"…"},"privacy":"public"}],
           "run":{"id":"run-…","engine":"nove-engine","engine_version":"0.9.0"}}
```

- **Debe:** `inputs` tiene de 1 a 64 entradas. Cada una dice `owner` (app dueña, como en [catálogo §4.1](nu9ve-data-v1.md#41-identidad-y-dueño)), `dataset` (su id), `revision` (1 a 128 caracteres), `hash` (`algorithm` `sha256` y su `value`) y `privacy` (la clase del dataset). La identidad del dataset en la suite es `owner:dataset`.
- **Debe:** `run` dice la corrida que produjo el artefacto: `id`, `engine` y `engine_version`, cada uno de 1 a 128 caracteres.
- **Debe** al consumidor: guardar junto a lo que importó el `producer.app:id`, el SHA-256 de `artifact.json` y la lista de `owner:dataset@revision`. Con eso se sabe de dónde salió un overlay sin abrir el paquete.

## 7. Privacidad

`privacy` es la clase de nove [modelo §13](../../nove/docs/contracts/data-model-v1.md#13-privacidad): `public`, `restricted` o `personal`, de menos a más.

- **Debe:** la clase del artefacto es **al menos** la mayor de sus entradas (`privacy_below_inputs`). Una entrada de clase desconocida cuenta como `personal`.
- **Debe:** lo `personal` no sale del equipo ni entra a un artefacto **sin una acción explícita de la persona que diga qué sale** ([catálogo §6](nu9ve-data-v1.md#6-privacidad)). El productor la registra en `release`:

  ```json
  "release":{"by":"person","at":"2026-11-19T18:02:00Z","what":"Minutos por semana de mis sesiones de Claude Code, sin títulos ni rutas"}
  ```

  `by` es `person`; `at`, RFC 3339 UTC; `what`, de 1 a 2000 caracteres, lo que la persona vio y aceptó. Un artefacto `personal` sin `release` es inválido (`release_missing`).
- **Debe** al consumidor: un artefacto `personal` **no se importa sin una acción explícita de la persona en el consumidor** (una opción que se marca a propósito, nunca un valor por omisión): lo que entra a un proyecto de video acaba publicado. Sin ella, `personal_needs_action`.
- **Debe:** un paquete `personal` vive en la carpeta de la app que lo escribió o donde la persona eligió. Ningún productor lo deja en una carpeta compartida por su cuenta.

## 8. Animación

`motion` dice cómo el artefacto llega a su cuadro final. Hay dos formas; un consumidor que no sabe reproducir una la rechaza (`motion_unsupported`) o usa sólo el póster si la persona lo pide.

### 8.1 Cuadros

```json
"motion":{"kind":"frames","fps":12,"loop":false,"frames":[{"path":"frames/00000.png","media_type":"image/png", … }, …]}
```

- **Debe:** `fps` es un número de 1 a 120 y `frames` una lista de 1 a 18000 PNG (§3), cada uno de `width` × `height` del render, en el orden en que se muestran. La duración es `len(frames) / fps`.
- **Debe:** con `background: transparent`, los cuadros llevan alfa como el póster.
- **Conviene:** el último cuadro es igual al póster.
- **Debe:** `loop` (booleano) dice si se repite. Sin repetir, el consumidor sostiene el último cuadro.

Es lo que Clipper y `ai/video` pueden usar sin saber nada de la receta.

### 8.2 Declarada

```json
"motion":{"kind":"declared","fps":30,"duration_s":2.0,"loop":false,
  "tracks":[{"target":"series:openmvs","property":"progress","keys":[{"t":0,"v":0,"ease":"out"},{"t":1.2,"v":1}]},
            {"target":"label:speedup","property":"opacity","keys":[{"t":1.2,"v":0},{"t":1.6,"v":1,"ease":"in_out"}]}]}
```

Para quien vuelve a dibujar desde los datos exactos (`ai/video` con Remotion, a cualquier resolución).

- **Debe:** `fps` de 1 a 120; `duration_s` mayor que 0 y hasta 600; `tracks` de 1 a 256.
- **Debe:** cada pista tiene `target` (`^[a-z0-9][a-z0-9._:-]{0,63}$`, un elemento que la receta define), `property` y `keys` (de 1 a 1024). `property` es una de:
  - `progress`: de 0 a 1, cuánto del elemento está dibujado (una serie que crece, una barra que se llena);
  - `opacity`: de 0 a 1;
  - `scale`: de 0 a 16, sobre su centro;
  - `value`: número que el elemento muestra (un contador), sin límites.
- **Debe:** cada clave tiene `t` (segundos, de 0 a `duration_s`, **estrictamente creciente** en la pista) y `v`; `ease` (`linear`, `in`, `out`, `in_out`, `step`; por omisión `linear`) dice cómo se llega **desde la clave anterior**. Antes de la primera clave vale su `v`; después de la última, la última. `in` = t², `out` = 1 − (1 − t)², `in_out` = 3t² − 2t³, `step` = salta al llegar.
- **Debe:** un elemento sin pista está en su estado final. Un `target` que el renderer no conoce se rechaza; no se ignora.
- **Debe:** en `t = duration_s` el dibujo es el póster.

## 9. Consumidores

- **Debe:** abre el paquete en sólo lectura, verifica el manifiesto (§2 a §8) y el tamaño y hash de cada archivo que va a usar. Con el PNG, también su tamaño.
- **Debe:** copia lo que usa a su almacén (Clipper: su biblioteca de assets por contenido) y no vuelve a leer el paquete para exportar. Borrar o mover el paquete no rompe lo importado.
- **Debe:** ofrece el `title` como texto sugerido (un pie, un rótulo) marcado como dato, y lo mismo con las unidades de los datos.
- **Conviene:** muestra `producer`, `created_at` y el linaje al lado de lo importado.

## 10. Ejemplos y validador

- [`data/artifact-examples-v1.json`](../data/artifact-examples-v1.json): manifiestos válidos (fijo, cuadros, declarada, `personal` con `release`) e inválidos, cada uno con el nombre del primer problema; y casos de consumo (hash cambiado, PNG de otro tamaño, `personal` sin la acción del consumidor).
- [`data/artifact-example-v1/atlas-dsm.nu9ve-artifact/`](../data/artifact-example-v1/atlas-dsm.nu9ve-artifact/): un paquete real con datos, SVG, PNG y 24 cuadros.
- `python3 -I scripts/artifact-examples.py --check` comprueba los dos contra la implementación de referencia; `--package <carpeta>` valida un paquete cualquiera y `--package <carpeta> --allow-personal` como un consumidor con la acción de la persona.
- [`data/artifact-example-v1/canvas-outputs.nu9ve-artifact/`](../data/artifact-example-v1/canvas-outputs.nu9ve-artifact/): un lienzo de Salidas con PDF en el render y `documents` (Markdown, DOCX y dos PNG), de la versión menor; sus casos de consumo llevan `package`.
- `rust/nu9ve-artifact`: `manifest_problem` y `open_package` con los mismos resultados. `Package::pdf` y `Package::documents` dan los archivos nuevos y `content_ok` la comprobación de firma.

## 11. Adopción

Quién produce y quién consume está en [adopción por app](../docs/adoption.md#nu9ve-artifact-v1).

## 12. Decisiones abiertas

1. **Fuentes en el SVG.** Un SVG con texto depende de las fuentes de quien lo abre. Hoy el PNG es la verdad visual; falta decidir si el SVG lleva el texto en trazos.
2. **Video dentro del paquete.** No se admite: los cuadros PNG o la animación declarada bastan para que `ai/video` y Clipper hagan su video. Si un productor necesita entregar video ya hecho, sería una versión menor con su `media_type`.
