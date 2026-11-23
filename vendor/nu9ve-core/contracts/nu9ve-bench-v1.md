# Registro de benchmarks v1

Estado: **propuesta** (2026-11-21). Define cómo una app de la suite guarda lo que mide, para que las corridas se puedan comparar entre motores (Python ↔ Rust), commits, máquinas y apps, y para que nove las muestre como datos. Nace de nove [data-hub §9b](../../nove/docs/data-hub.md#9b-benchmarks-como-datos-pedido-del-usuario-2026-11-19-propuesta) (su S10) y de lo que ya miden Atlas (`atlas_vision/engines/bench.py`), Clipper (`scripts/tui_flow_bench.py`), nüün, curado y SoundBender (dorados). Su primer usuario es la M0 del [cierre de la migración de Clipper](../../clipper/docs/2026-11-21-rust-migration-closure.md#benchmark-de-referencia).

Ejemplos: [`data/bench-examples-v1.json`](../data/bench-examples-v1.json). Referencia en Python: [`scripts/nu9ve_bench.py`](../scripts/nu9ve_bench.py). En Rust: [`rust/nu9ve-bench`](../rust/nu9ve-bench/).

**Debe** es obligatorio. **Conviene** es lo recomendado. Una **corrida** es la medición de un caso con un sujeto, con sus repeticiones. El **registro** es el archivo donde se agregan las corridas.

## 0. Para qué y para qué no

- **Para** guardar mediciones comparables: tiempo, CPU, memoria, disco, rendimiento y calidad frente a una referencia, con lo necesario para saber qué se midió, con qué y dónde.
- **No** es un formato de resultados de pruebas: un dorado que falla sigue siendo una prueba que falla. El registro puede guardar *cuánto* difiere, como calidad (§6).
- **No** decide qué es «rápido». Las comparaciones (§8) dan números; los umbrales son de cada app.
- **No** guarda datos de la persona (§9).

## 1. El registro

- **Debe:** UTF-8 sin BOM, NDJSON: un objeto JSON por línea, terminado en `\n`. Como mucho **64 KiB** por línea.
- **Debe:** sólo se agrega. Una corrida escrita no se reescribe ni se borra; una corrección es otra corrida (§7).
- **Debe** al escribir: abrir el registro en modo de agregar (creándolo si falta), tomar el candado exclusivo sobre ese mismo archivo ([`nu9ve-fs` §4](nu9ve-fs-v1.md#4-candados-y-sonda); `flock` en macOS y Linux), escribir la línea entera con su `\n` en una sola escritura y cerrarlo, que suelta el candado. La línea se valida (§2 a §7) **antes** de escribirla: una corrida inválida no llega al registro.
- **Debe** al leer:
  - una última línea sin `\n` está a medio escribir y se ignora sin error;
  - una línea vacía se ignora;
  - una línea inválida se reporta con su número y su motivo y se salta; no invalida las demás;
  - dos corridas con el mismo `run_id` son la misma: se queda la primera.
- **Conviene:** un registro por suite y por máquina, en una carpeta que Git ignora (`bench/` o la que diga la app). El resumen para personas va aparte, en los documentos de la app.

## 2. Una corrida

```json
{
  "contract": "nu9ve-bench/1",
  "run_id": "3f6c0a7e9b1d4c2a8e5f0b6d7c9a1e3f",
  "at": "2026-11-21T18:04:05Z",
  "suite": "clipper-engine",
  "case": "reads/project-summary",
  "category": "read",
  "subject": {"app": "clipper", "engine": "rust", "version": "0.9.0", "commit": "c856924a", "dirty": false},
  "machine": {"model": "MacBookAir10,1", "chip": "Apple M1", "cpu_cores": 8, "memory_bytes": 17179869184, "os": "macOS 26.1", "arch": "aarch64"},
  "inputs": [{"name": "fixture/project-a", "sha256": "…64 hex…", "bytes": 48213}],
  "params": {"mode": "process"},
  "cache": "warm",
  "repeats": 5,
  "warmup": 1,
  "status": "ok",
  "metrics": {
    "wall_ms": {"samples": [41.2, 40.8, 41.0, 43.9, 40.9], "median": 41.0, "p95": 43.9, "min": 40.8, "max": 43.9},
    "cpu_ms": {"samples": [39.0, 38.7, 38.9, 41.2, 38.8], "median": 38.9, "p95": 41.2, "min": 38.7, "max": 41.2},
    "peak_memory": {"bytes": 23592960, "measure": "rss", "scope": "process"}
  },
  "quality": [{"metric": "response_bytes_equal", "value": true, "reference": "python", "limit": true, "op": "eq", "pass": true}],
  "privacy": "restricted",
  "source": {"kind": "measured", "tool": "nu9ve-bench 0.1.0"}
}
```

| Campo | Debe | Qué es |
| --- | --- | --- |
| `contract` | Sí | `"nu9ve-bench/1"` |
| `run_id` | Sí | 32 dígitos hexadecimales en minúscula. Aleatorio al medir (`nu9ve_json::uid`); derivado del origen al importar (§7) |
| `at` | Sí | Cuándo empezó la corrida: RFC 3339 en UTC con `Z`, o una fecha civil `AAAA-MM-DD` cuando sólo se sabe el día (corridas importadas) |
| `suite` | Sí | `^[a-z][a-z0-9-]{0,63}$`. Conviene que empiece con la app: `clipper-engine`, `atlas-pipeline`, `soundbender-goldens` |
| `case` | Sí | `^[a-z0-9][a-z0-9._-]*(/[a-z0-9][a-z0-9._-]*)*$`, como mucho 128 caracteres. Estable entre corridas: es lo que se compara |
| `category` | Sí | Como `suite`. Agrupa casos para las vistas: `read`, `write`, `export`, `poses`, `dsm` |
| `subject` | Sí | §3 |
| `machine` | Sí | §4 |
| `inputs` | Sí | §5. Puede ir vacío |
| `params` | No | Objeto con los parámetros que cambian el resultado (hilos, resolución, preset). Sigue §9 |
| `cache` | Sí | `cold`, `warm` o `unknown` |
| `repeats` | Sí | Repeticiones medidas, entero ≥ 0. Es el largo de cada `samples` |
| `warmup` | Sí | Repeticiones descartadas antes de medir, entero ≥ 0 |
| `status` | Sí | §2.1 |
| `metrics` | Según `status` | §5 |
| `quality` | No | §6 |
| `error` | Si `status` es `failed` | `{"code", "message"}` como en [`nu9ve-engine-v1`](nu9ve-engine-v1.md); `message` sigue §9 |
| `privacy` | Sí | `public`, `restricted` o `personal`, la clase de [`nu9ve-data-v1` §6](nu9ve-data-v1.md#6-privacidad) |
| `source` | Sí | §7 |
| `note` | No | Texto corto para personas, como mucho 500 caracteres. Sigue §9 |

- **Debe** al lector: ignorar campos de primer nivel que no conoce; tratar como inválida una corrida a la que le falta uno de la tabla o lo trae de otro tipo.
- **Debe:** todos los números son finitos (nunca `NaN` ni infinitos) y los enteros caben en 2^53 − 1.

### 2.1 Estados

| `status` | Cuándo | `metrics` |
| --- | --- | --- |
| `ok` | Se midió | Obligatorio, con `wall_ms` |
| `failed` | El caso falló al medirse | Opcional (lo medido antes del fallo); `error` obligatorio |
| `not_ported` | El motor respondió `not_ported` ([motor §5](nu9ve-engine-v1.md#5-not_ported)): la ruta no existe todavía | Ausente; `repeats` es 0 |
| `skipped` | No se corrió (falta el disco, falta Docker) | Ausente; `repeats` es 0 |

## 3. Sujeto: qué se midió

| Campo | Debe | Qué es |
| --- | --- | --- |
| `app` | Sí | El nombre corto de la app, como en el [registro de motores](nu9ve-registry-v1.md#2-archivo) (`^[a-z][a-z0-9-]{0,31}$`) |
| `engine` | Sí | Qué implementación: `python`, `rust`, `swift`, `ffmpeg`, `odm-docker`… Mismo patrón que `app`. Es lo que distingue los dos lados de una migración |
| `version` | Sí | Cadena o `null` si no se sabe |
| `commit` | Sí | Hash abreviado o completo en hexadecimal (7 a 40 dígitos), o `null` |
| `dirty` | Sí | `true` si el checkout tenía cambios sin commit, `false`, o `null` si no se sabe |

- **Debe:** una corrida con `dirty: true` no sirve de línea base; las comparaciones (§8) la marcan.

## 4. Máquina

| Campo | Debe | Qué es |
| --- | --- | --- |
| `model` | Sí | `hw.model` en macOS (`MacBookAir10,1`), o `null` |
| `chip` | Sí | `machdep.cpu.brand_string` en macOS (`Apple M1`), o `null` |
| `cpu_cores` | Sí | Núcleos lógicos, o `null` |
| `memory_bytes` | Sí | Memoria física, o `null` |
| `os` | Sí | Sistema y versión (`macOS 26.1`, `Windows 11 26100`), o `null` |
| `arch` | Sí | `aarch64` o `x86_64`, o `null` |

- **Debe:** sin nombre de equipo, usuario ni número de serie (§9).
- La **clave de máquina** es `model`, `chip` y `memory_bytes` juntos. Dos corridas con la misma clave se comparan directamente; con otra clave, la comparación lo dice (§8).

## 5. Entradas y métricas

**Entradas.** Cada una, `{"name", "sha256", "bytes"}`:

- `name`: nombre estable y genérico de la entrada (`fixture/project-a`, `golden/brush-pluck`), con el patrón de `case`. Nunca una ruta de la persona (§9).
- `sha256`: 64 hexadecimales en minúscula, del contenido. Para una carpeta, el de su manifiesto canónico ([`nu9ve-json-v1`](nu9ve-json-v1.md)) o el que defina la app; la app lo documenta.
- `bytes`: entero ≥ 0, o `null`.

Dos corridas con las mismas entradas (los mismos `sha256`) midieron lo mismo.

**Métricas.** Todas opcionales salvo `wall_ms` en `ok`.

| Métrica | Forma | Qué es |
| --- | --- | --- |
| `wall_ms` | Distribución | Tiempo real de cada repetición, en milisegundos |
| `cpu_ms` | Distribución | Tiempo de CPU (usuario + sistema) de cada repetición, en milisegundos |
| `startup_ms` | Distribución | Arranque del proceso hasta que acepta trabajo, si se mide aparte |
| `peak_memory` | `{"bytes", "measure", "scope"}` | Pico de memoria de toda la corrida. `measure`: `rss`, `footprint` (macOS, lo que muestra el Monitor de Actividad) o `unknown`. `scope`: `process` (sólo el proceso medido) o `tree` (con sus hijos) |
| `temp_disk_bytes` | `{"bytes"}` | Pico de disco temporal |
| `throughput` | `{"value", "unit"}` | Rendimiento: `value` > 0 y `unit` corta (`frames/s`, `MB/s`, `photos/min`) |

**Distribución.** `{"samples", "median", "p95", "min", "max"}`:

- `samples`: los valores de cada repetición, en orden, todos ≥ 0, con largo igual a `repeats` (y `repeats` ≥ 1 cuando hay una distribución).
- `min` y `max`: el menor y el mayor.
- `median`: con un número impar de muestras, la del medio de las ordenadas; con uno par, `(a + b) / 2` de las dos del medio, calculado en `f64`.
- `p95`: rango más cercano: la muestra ordenada en la posición `k = (95 · n + 99) div 100` (el techo de `0,95 · n` en enteros, sin coma flotante), contando desde 1. Con menos de 20 muestras, es el máximo.
- **Debe:** los cuatro resúmenes son los que salen de `samples` con esas reglas, exactos. El lector los recalcula y una corrida que no coincide es inválida.

## 6. Calidad

`quality` es una lista. Cada elemento:

| Campo | Debe | Qué es |
| --- | --- | --- |
| `metric` | Sí | Nombre, 1 a 128 caracteres imprimibles. Estable entre corridas |
| `value` | Sí | Número finito o booleano |
| `reference` | Sí | Contra qué se compara: el `engine` de otra corrida (`python`), un dorado (`golden`) o un nombre corto, con el patrón de `case`; o `null` si es una medida absoluta |
| `limit` | No | El umbral, del mismo tipo que `value` |
| `op` | Si hay `limit` | `le` (`value ≤ limit`), `ge` (`value ≥ limit`) o `eq` (`value == limit`) |
| `pass` | Sí | Booleano, o `null` si no hay criterio (una medida sin umbral) |

- **Debe:** con `limit`, `pass` es exactamente el resultado de aplicar `op`. Sin `limit`, `pass` lo pone el productor (una revisión a ojo) o es `null`.
- **Conviene:** «los mismos bits» como `{"metric": "bytes_equal", "value": true, "limit": true, "op": "eq"}`, y una diferencia numérica como el máximo absoluto con su tolerancia y `le`.

## 7. Origen

`source`:

| `kind` | Campos | Cuándo |
| --- | --- | --- |
| `measured` | `tool`: qué midió (`nu9ve-bench 0.1.0`, `nu9ve_bench.py 1`, o el arnés de la app) | La corrida se midió así |
| `imported` | `origin`: de qué formato y archivo, relativo al repo de origen (`atlas config/bench.json#baselines/0`) | Traducida de un formato anterior (§10) |

- **Debe** al importar: `run_id` sale de los primeros 32 hexadecimales del SHA-256 de `origin`, para que importar dos veces lo mismo no duplique (§1).
- **Debe** al importar: lo que el formato anterior no sabe queda `null` o ausente; nunca se inventa. Una unidad que se convierte (GB a bytes) usa la del formato de origen (Atlas: 2^30) y lo dice su traductor.

## 8. Comparar

Lo que hacen las vistas, para que todas cuenten igual:

- **Comparables:** dos corridas `ok` con la misma `suite`, el mismo `case` y las mismas entradas.
- **Aceleración** de `b` frente a `a`: `mediana(wall_ms de a) / mediana(wall_ms de b)`. Mayor que 1 es que `b` es más rápida.
- **Debe:** la comparación dice, junto al número, si las dos corridas tienen otra clave de máquina, otro `cache`, otros `params` o alguna `dirty`. No se esconde: se marca.
- **Debe:** la línea base de una migración es una corrida `ok`, no `dirty`, de la ruta anterior (`engine` viejo), con las mismas entradas que la nueva.

## 9. Privacidad

- **Debe:** sin rutas de la persona, nombres de proyectos, títulos ni contenido. Lo que identifica una entrada es su `sha256` y un nombre genérico.
- **Debe:** ninguna cadena de `inputs[].name`, `params`, `note` ni `error.message` empieza por `/` o `~`, ni lleva una ruta de Windows (`C:\`) ni `\\`. El lector la rechaza (`personal_path`).
- **Debe:** `privacy` es `public` sólo si todas las entradas lo son (fixtures del repo, señales generadas). Con entradas de la persona, `restricted` como mínimo, aunque sólo vayan sus hashes. Si no se sabe, `personal`.
- El registro viaja como los datasets de [`nu9ve-data-v1` §6](nu9ve-data-v1.md#6-privacidad): local, y fuera del equipo sólo con una acción explícita.

## 10. Formatos anteriores

Se traducen al registro con traductores de sólo lectura, sin cambiar el repo de origen:

| Formato | Traductor | Notas |
| --- | --- | --- |
| Atlas: `derived/bench/<stage>/<dataset>/*.json` y `config/bench.json#baselines` | [`scripts/bench-from-atlas.py`](../scripts/bench-from-atlas.py) | Una corrida por archivo de corrida y por línea base; `case` es `<stage>/<dataset>` y el dataset va en `params`, sin `inputs` (Atlas no guarda hashes). `time_s` a `wall_ms` con una muestra. Memoria: GB de 2^30 a bytes, con el `memory_measure` de Atlas y `scope: tree`, o `unknown`. Disco: GB de 10^9 a bytes. `quality` se aplana a una métrica por hoja numérica o booleana, con `reference: null` y sin umbral. `git` `abc1234+dirty` da `commit` y `dirty` |
| Clipper: `scripts/tui_flow_bench.py`, `docs/benchmarks/*.json` | Por escribir en la M0 de Clipper | La M0 escribe ya en este registro |
| Dorados de nüün, curado y SoundBender | Por escribir | Su calidad es `bytes_equal` o el máximo absoluto con tolerancia |

## 11. Cómo llega a nove

nove lee los registros como fuente «Benchmarks» (su S10): una suite es un dataset `table`, con el tiempo de cada corrida como eje. Se registra con su catálogo ([`nu9ve-data-v1`](nu9ve-data-v1.md)) y respeta `privacy`. Las gráficas salen como [`nu9ve-artifact-v1`](nu9ve-artifact-v1.md).

## 12. Ejemplos

[`data/bench-examples-v1.json`](../data/bench-examples-v1.json):

- corridas válidas e inválidas, cada una con su motivo;
- distribuciones con su mediana y su p95 esperados;
- un registro con una línea a medio escribir, una inválida y un `run_id` repetido, con lo que debe leer el lector;
- comparaciones con su aceleración y sus marcas;
- la traducción de un fixture de Atlas.

`python3 -I scripts/bench-examples.py --check` comprueba el archivo contra la referencia de `scripts/nu9ve_bench.py`.

## 13. Adopción

Quién adoptó qué está en [adopción por app](../docs/adoption.md#nu9ve-bench-v1).

## 14. Decisiones abiertas

1. **Memoria en Windows.** v1 escribe `peak_memory` sólo en macOS y Linux. En Windows sería `PeakWorkingSetSize` (`rss`) o `PeakPagefileUsage`; lo decide la primera app que mida allí.
2. **Árbol de procesos.** `scope: tree` lo mide Atlas muestreando `ps` cada 250 ms. El crate y el ayudante de Python miden `process` con `wait4`, que es exacto pero no ve a los nietos. Si Clipper necesita el árbol (FFmpeg dentro del motor), se suma un muestreador.
