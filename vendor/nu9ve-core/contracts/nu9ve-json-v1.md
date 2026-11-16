# JSON canónico, digest, ids y sobre de errores v1

Estado: **implementado en nüün** (2026-11-16). Escribe como regla de la suite lo que ya hacen dos implementaciones de nüün: la de Python (`media/common.py`: `canonical`, `digest`, `uid`, `MediaError`) y el crate de Rust `nu9ve-json` (`n-creator-utils/engine/crates/nu9ve-json`). Mientras Python sea el oráculo del motor, «compatible con Python byte a byte» es una restricción; cuando Python se retire, este documento y sus [vectores](../data/json-vectors-v1.json) pasan a ser la especificación.

**Debe** es obligatorio. **Conviene** es lo recomendado.

## 0. Para qué

Un plan de nüün se identifica por el SHA-256 de su JSON canónico, y una persona autoriza ese digest. Si dos implementaciones escriben bytes distintos para el mismo valor, el plan que la persona revisó no es el que se aplica. Por eso el JSON canónico se define por **bytes**, no por «JSON equivalente».

## 1. La regla en una línea

Los bytes canónicos de un valor son exactamente los de CPython:

```python
json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(',', ':'), allow_nan=False).encode('utf-8')
```

Lo que sigue es esa línea escrita para quien no tenga Python.

## 2. Estructura

- **Sin espacios.** `,` entre elementos y `:` entre clave y valor, sin nada alrededor. Sin salto de línea al final.
- **Literales:** `null`, `true`, `false`.
- **Arreglos:** el orden se conserva tal cual.
- **Objetos:** claves ordenadas por **punto de código Unicode**, que es el mismo orden que los bytes UTF-8. **No** es el orden UTF-16: `"￿"` va antes que `"😀"` (U+1F600), aunque en UTF-16 el surrogate `D83D` sea menor. Mayúsculas antes que minúsculas (`"A" < "B" < "_" < "a"`).
- **Claves repetidas** en un texto de entrada: gana la última (`{"a":1,"a":2}` → `{"a":2}`). **Conviene** no producirlas nunca.

## 3. Números

### 3.1 Enteros

Un entero se escribe en decimal, sin signo `+`, sin ceros a la izquierda y sin exponente. Un entero del texto de entrada **debe** seguir siendo entero: `1` es `1`, no `1.0`. El rango que la suite garantiza es el de `i64` más el de `u64` (de −9223372036854775808 a 18446744073709551615). Fuera de él, ver §8.

### 3.2 Decimales

Un número con `.` o exponente es un double IEEE 754 y se escribe como el `repr` de CPython (`float_repr_style == 'short'`):

1. Los **dígitos más cortos** que, leídos de nuevo, dan el mismo double.
2. Sea `p` la posición del punto decimal (`1.5` → `p = 1`; `0.001` → `p = −2`). Si `p ≤ −4` o `p > 16`, **notación científica**: `d.ddde±XX`, con el signo del exponente siempre y al menos dos dígitos (`1e-07`, `1e+16`, `1.5e+300`, `5e-324`). Sin `.` cuando la mantisa tiene un solo dígito.
3. Si no, **notación fija**, y un valor entero lleva `.0` (`1.0`, `100.0`, `1234567890123456.0`).
4. `-0.0` conserva su signo.

| Entrada | Canónico |
| --- | --- |
| `1e2` | `100.0` |
| `0.0001` | `0.0001` |
| `0.00001` | `1e-05` |
| `1e16` | `1e+16` |
| `2E-3` | `0.002` |
| `0.1000000000000000055511151231257827` | `0.1` |
| `1.7976931348623157e308` | `1.7976931348623157e+308` |

`serde_json` solo escribe `1e-7` y `1e16`: `nu9ve-json` tiene su propio escritor de decimales (`python_float_repr`).

### 3.3 No finitos

`NaN`, `Infinity` y `-Infinity` **no** son JSON y **no** tienen forma canónica. Un número que se sale de un double (`1e400`) tampoco: Python lo lee como infinito y luego lo rechaza. Ver §5.

## 4. Cadenas

Se escriben entre `"` en UTF-8, **sin** `\u` para lo que no es ASCII. Sólo se escapan:

| Carácter | Escape |
| --- | --- |
| `"` | `\"` |
| `\` | `\\` |
| U+000A, U+000D, U+0009, U+0008, U+000C | `\n`, `\r`, `\t`, `\b`, `\f` |
| Los demás por debajo de U+0020 | `\u00XX` con hexadecimal en **minúsculas** (`\u0000`, `\u001f`) |

Todo lo demás va tal cual: `/` (aunque la entrada traiga `\/`), U+007F, U+2028 y U+2029, el BOM U+FEFF, y los caracteres fuera del plano básico como sus cuatro bytes UTF-8 (nunca como par de surrogates). **No** se normaliza Unicode: `"é"` (U+00E9) y `"é"` son cadenas distintas con digests distintos.

## 5. Lo que se rechaza

Una implementación **debe** fallar, al leer o al escribir, antes de producir bytes canónicos para:

- `NaN`, `Infinity`, `-Infinity`, y números que se salen de un double.
- Un surrogate suelto (`"\ud800"`), que no tiene UTF-8.

Python los lee y falla al escribir (`ValueError`, `UnicodeEncodeError`); `serde_json` ya falla al leer, y su `Value` no puede guardar un no finito. Las dos cosas cumplen. **No debe** cambiar el valor por `null` ni por otro número para seguir.

## 6. Digest

El digest de un valor es **SHA-256 de sus bytes canónicos**, en hexadecimal de 64 caracteres en minúsculas. `{"a":1}` → `015abd7f5cc57a2dd94b7590f04ad8084273905ee33ec5cebeae62276a97f862`.

Para bytes que no son un valor canónico (un archivo de esquema, un texto ya serializado), `sha256_hex(bytes)` da el mismo hexadecimal que `hashlib.sha256(data).hexdigest()`. Eso no es un digest de la suite: sólo dice qué bytes se hashearon.

## 7. Ids

| Id | Forma | Python | Rust |
| --- | --- | --- | --- |
| Aleatorio | UUID versión 4, variante RFC 4122, en minúsculas con guiones (`8-4-4-4-12`), del generador del sistema | `str(uuid.uuid4())` | `uid()`: `/dev/urandom`, o `BCryptGenRandom` en Windows |
| Por nombre | UUID versión 5 (SHA-1 del espacio de nombres más el nombre en UTF-8) | `str(uuid.uuid5(ns, name))` | `uuid5(&ns, name)` |
| De contenido | UUID 5 en `NAMESPACE_URL` del **digest** del valor | `str(uuid.uuid5(uuid.NAMESPACE_URL, digest(value)))` | `uuid5(&NAMESPACE_URL, &digest(&value))` |

- El espacio de nombres de la suite es `uuid.NAMESPACE_URL` (`6ba7b811-9dad-11d1-80b4-00c04fd430c8`). Otro espacio sólo cuando el id cuelga de otro id (nüün: una operación dentro de un plan usa el id del plan como espacio).
- SHA-1 sirve aquí **sólo** para ids por nombre, porque así lo define el UUID 5. **No debe** usarse para integridad: para eso está el digest.
- Un id de contenido cambia si cambia un solo byte canónico. Así nüün identifica planes y unidades.

## 8. Límites conocidos

Entradas en las que hoy las implementaciones **no** coinciden. Ningún productor de la suite **debe** escribirlas; están en `limits` de los vectores, y la prueba de Rust falla el día que coincidan, para pasarlas a `accepted`.

| Entrada | Python | `nu9ve-json` hoy | Por qué |
| --- | --- | --- | --- |
| `-0` (entero) | `0` | `-0.0` | `serde_json` lee `-0` como double |
| Enteros fuera de `i64`/`u64` (`18446744073709551616`) | el entero exacto | un double (`1.8446744073709552e+19`) | `serde_json` sin `arbitrary_precision` |
| Un decimal que cae justo entre dos doubles con más de 17 dígitos (`9007199254740993.0`, `12345678901234567.0`) | redondea al par | a veces al otro | `serde_json` sin `float_roundtrip` no siempre redondea bien |

**Con `arbitrary_precision`.** Cargo une las features de `serde_json` en todo el binario, así que una app que las activa (Clipper: `arbitrary_precision` y `preserve_order`) compila `nu9ve-json` con ellas. Medido el 2026-11-16: todos los vectores `accepted` siguen dando los mismos bytes y digests que Python. Cambian dos cosas del lector: el entero `-0` se escribe `0`, como en Python, y `1e400` se lee en vez de rechazarse. Esa app **no debe** pasar a `canonical` ni a `digest` valores leídos de afuera sin rechazar antes los números que no caben en un double.

Lo que sí está garantizado: todo texto que escribe esta regla se relee igual. Los dígitos más cortos de §3.2 nunca caen en un empate, y las pruebas lo comprueban leyendo de vuelta cada salida canónica.

## 9. Sobre de errores

Un error tiene un **código estable** en `snake_case` (`read_only`, `unsafe_path`, `random_unavailable`) y un **mensaje** para personas, en el idioma de la app. El código es lo que el programa compara; el mensaje puede cambiar.

| Python | Rust |
| --- | --- |
| `MediaError(code, message)` | `nu9ve_json::Error { code, message }` |
| `require(condition, code, message)` | `require(condition, code, message)?` |

En el transporte de líneas (`rpc-serve` de nüün), cada respuesta es un objeto JSON en una línea, con el `id` de la petición (o `null`) y una de dos formas:

```json
{"data":…,"id":7,"ok":true}
{"error":{"code":"read_only","message":"No se puede escribir."},"id":null,"ok":false}
```

- **Debe** haber `ok`, y exactamente uno de `data` o `error`.
- `error` tiene `code` y `message`, los dos cadenas. Detalles extra, si alguna vez hacen falta, van en un campo nuevo de `error`, nunca en el mensaje.
- En Rust: `ok_envelope(&id, data)` y `error_envelope(&id, &error)`.
- La línea en el cable no tiene por qué ser canónica. Cuando un sobre se guarda o se firma, sí: se escribe con §1.

Que nove adopte este sobre en su IPC es la ola 3 de la [adopción](../docs/suite-adoption.md); hoy usa `{code, message}` en sus errores, que es la misma forma de `error`.

## 10. Vectores

[`data/json-vectors-v1.json`](../data/json-vectors-v1.json), escrito por [`scripts/json-vectors.py`](../scripts/json-vectors.py) con el oráculo de §1. Cada entrada es **texto JSON**, para que cada lenguaje lo lea con su propio lector: así los vectores prueban también el lector (enteros que siguen siendo enteros, decimales bien redondeados).

| Sección | Qué trae | Qué debe dar una implementación |
| --- | --- | --- |
| `accepted` | `input`, `canonical`, `digest` | Exactamente `canonical` y `digest` |
| `rejected` | `input`, `why` | Un error, al leer o al escribir |
| `limits` | `input`, `canonical` de Python, `why` | Nada garantizado (§8) |
| `uuid5` | `namespace`, `name`, `uuid` | El mismo `uuid` |
| `content_id` | `value`, `digest`, `uuid` | El mismo digest y su UUID 5 en `NAMESPACE_URL` |

Se regeneran aquí con `python3 -I scripts/json-vectors.py` (y `--check` comprueba que estén al día), nunca en una copia fijada. Un vector nuevo es un cambio de este contrato, con su entrada en el CHANGELOG.

## Adopción

| App | Implementación | Prueba los vectores | Nota |
| --- | --- | --- | --- |
| nüün | Python `media/common.py` (oráculo) y Rust `nu9ve-json` | Rust: `engine/crates/nu9ve-json/tests/vectors.rs`, desde su copia fijada | Python además se compara con Rust en `scripts/engine_golden.py` |
| Clipper | Rust `nu9ve-json` desde su copia fijada, para el sobre de `clipper-engine/1`, `sha256_hex` y `uuid4` (desde el 2026-11-16); su JSON propio sigue en `py.rs`/`pyval.rs` | No todavía: el canónico no lo usa | Compila `serde_json` con `arbitrary_precision` y `preserve_order` (§8) |
| nove | Sobre `{code, message}` propio | — | Ola 3 |
