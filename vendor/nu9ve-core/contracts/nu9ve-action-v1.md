# Acciones largas y su progreso v1

Estado: **propuesta** (2026-11-26). Define el **registro de una acción larga** (copiar, verificar, respaldar, exportar, escanear) y el método común `action_progress` con el que un cliente lo lee. Así la misma vista ([interacción §7](nu9ve-interaction-v1.md#acciones-largas)), el agente o un supervisor muestran el progreso de cualquier motor sin saber de qué app es. Nace de la [propuesta de nüün](../docs/actions-next.md) y recoge lo que ya escriben nüün (`ActionMeter` en Python, `Meter` y `ActionProgress` en Rust) y Clipper (`ProgressPlan`). Ejemplos y vectores: [`data/action-examples-v1.json`](../data/action-examples-v1.json), escritos por la referencia [`scripts/action-examples.py`](../scripts/action-examples.py).

**Debe** es obligatorio. **Conviene** es lo recomendado. «Motor» es el de [`nu9ve-engine-v1`](nu9ve-engine-v1.md); «cliente», quien lanza la acción o la muestra; «registro», el objeto de la §3.

## 0. Para qué y para qué no

- **Para** que el progreso se lea igual en toda la suite: una medida global que nunca retrocede, la etapa, el archivo en mano, un log corto, un latido y un cierre con éxito, aviso o error.
- **No** cambia cómo corre una acción. Sigue fuera del bucle del motor persistente y se cancela como dice [motor §6.2](nu9ve-engine-v1.md#62-progreso-y-cancelación). Este contrato es sólo lo que se lee mientras corre y al terminar.
- **No** lleva eventos dentro del canal (decisión del usuario, 2026-11-16, [motor §13](nu9ve-engine-v1.md#13-decisiones-abiertas)): el cliente sondea.
- **No** calcula la ETA en el motor: la calcula el cliente con las reglas de la §4, que son las de Clipper.
- **Local.** El registro no sale de la máquina: el agente no lo publica en la [carpeta de intercambio](nu9ve-exchange-v1.md) (§6).

## 1. Lanzar y nombrar

- **Debe:** quien lanza la acción le da un `progress_id` en los parámetros del método que la empieza (`{"progress_id": "ingreso-2026-11-26-1"}`). El motor **no** lo inventa: así el cliente puede leer el progreso desde la primera respuesta, aunque la acción corra en otro proceso.
- **Debe:** `progress_id` cumple `^[a-z0-9][a-z0-9._:-]{0,127}$`, el patrón de id de [data §4.1](nu9ve-data-v1.md#41-identidad-y-dueño). Un id inválido es `invalid_params` en el método que lanza.
- **Debe:** un `progress_id` que ya tiene una acción con `state` `running` es `action_running` en el método que lanza (un código de este contrato, que se suma a los de [motor §7](nu9ve-engine-v1.md#7-códigos-de-error)), y la acción que corre sigue. Uno cuyo registro ya terminó se puede reutilizar: la acción nueva reemplaza el registro.
- **Conviene:** un id que diga qué es y sea único en el tiempo (`<acción>-<fecha>-<n>`, o un UUID).

## 2. Método `action_progress`

Un motor que lo sirve lista `action_progress_v1` en `capabilities` de su `engine_info` y en su [registro](nu9ve-registry-v1.md#2-archivo).

```json
{"id":7,"method":"action_progress","params":{"progress_id":"ingreso-2026-11-26-1"}}
```

| Parámetro | Tipo | Qué hace |
| --- | --- | --- |
| `progress_id` | cadena, obligatoria | El id que dio quien lanzó la acción (§1) |

- **Debe:** sin `progress_id`, con otro parámetro o con un tipo o patrón equivocado, responde `invalid_params`.
- **Debe:** es una lectura. No cambia nada, responde dentro del plazo del cliente (5 s en [`nu9ve-ipc`](../rust/nu9ve-ipc/)) y nunca espera a la acción.
- **Debe:** con el id de una acción que existe, `data` es su registro (§3). Con un id que el motor no conoce, `data` es `{"contract": "nu9ve-action/1", "progress_id": "…", "state": "unknown"}`. No es un error: puede ser una acción que todavía no escribió su primer registro o una cuyo registro ya se retiró (§3.5).
- **Debe:** el registro de una acción que terminó se sigue sirviendo **al menos 10 minutos** después de `updated_at`, o hasta que el mismo `progress_id` lance otra acción. Así el cliente que vuelve a esa vista ve el cierre.
- Los métodos propios de antes (nüün `closure_progress` y `storage_progress`, nove `events`) pueden seguir como alias mientras la app migra. Un cliente de la suite sólo llama `action_progress`.

## 3. El registro

```json
{
  "contract": "nu9ve-action/1",
  "progress_id": "ingreso-2026-11-26-1",
  "action": "ingest",
  "label": "Copiando y verificando la tarjeta A7S-03",
  "state": "running",
  "stage": "verifying",
  "phase": {"index": 2, "count": 3, "label": "Verificar en MEDIA502"},
  "overall_done": 1600000000,
  "overall_total": 4000000000,
  "unit": "bytes",
  "files_done": 212,
  "files_total": 540,
  "current": {"name": "C0212.MP4", "from": "A7S-03", "to": "MEDIA502", "action": "verify", "file_done": 52428800, "file_total": 104857600},
  "log": [{"at": "2026-11-26T18:02:11Z", "kind": "copied", "name": "C0211.MP4", "target": "MEDIA502"}],
  "cancellable": true,
  "started_at": "2026-11-26T18:00:00Z",
  "progressed_at": "2026-11-26T18:02:12Z",
  "updated_at": "2026-11-26T18:02:12Z"
}
```

### 3.1 Campos

| Campo | Obligatorio | Qué es |
| --- | --- | --- |
| `contract` | Sí | `nu9ve-action/1` |
| `progress_id` | Sí | El de la petición |
| `action` | Sí | El verbo de la acción, `^[a-z][a-z0-9_]{0,63}$` (`copy`, `backup`, `ingest`, `release`, `export`, `scan`). La app lo traduce a su texto |
| `label` | No | Una frase concreta para la persona, hasta 200 caracteres: «Copiando y verificando finales», no «Procesando». Si falta, el cliente la arma con `action` |
| `state` | Sí | `running`, `complete`, `failed` o `cancelled` (§3.4) |
| `stage` | Sí | Qué hace ahora, del vocabulario de la §3.2 |
| `phase` | No | La fase declarada de la acción: `{index, count, label}`, con `1 ≤ index ≤ count`. Es el «Stage 2/7» de Clipper |
| `step` | No | El trabajo activo dentro de la fase: `{index, count, label, fraction}`; `fraction` de 0 a 1 o `null`. Es el «Step 4/8 · 9 %» de Clipper |
| `overall_done`, `overall_total` | Sí | La medida de **toda** la acción (§3.3). `overall_total` es `null` mientras no se conoce |
| `unit` | Sí | `bytes`, `files`, `items`, `seconds` (tiempo de media) o `work` (pesos sin unidad, los de Clipper) |
| `files_done`, `files_total` | No | Archivos terminados y por hacer, cuando la unidad no es `files` |
| `current` | No | El archivo en mano: `name` (sólo el nombre, sin carpeta), `from`, `to`, `action` y, si se mide, `file_done` y `file_total` en bytes |
| `log` | No | Hasta 60 entradas, la más vieja primero: `{at, kind, name, target}`, más un `text` opcional. Lo escribe el motor; la vista no lo reconstruye |
| `cancellable` | Sí | Si esta acción se puede cancelar ahora (cómo, en [motor §6.2](nu9ve-engine-v1.md#62-progreso-y-cancelación)) |
| `started_at` | Sí | Cuándo empezó |
| `progressed_at` | Sí | La última vez que `overall_done` subió. Al empezar, igual a `started_at` |
| `updated_at` | Sí | La última vez que el motor escribió el registro: el latido (§3.3) |
| `outcome` | Al terminar | §3.4 |
| `error` | Si `failed` | §3.4 |
| `resumable` | Si `failed` o `cancelled` | Si «Reanudar» sigue desde donde quedó |

- **Debe:** los instantes son RFC 3339 en UTC con `Z`, con fracción de segundo opcional, como en [perfiles](nu9ve-profile-v1.md).
- **Debe:** los enteros son de 0 a 2^53 − 1.
- **Debe:** el registro entero, como JSON, ocupa hasta 256 KiB, para que quepa en un marco de 1 MiB con el sobre.
- **Debe:** un campo desconocido se ignora. Así una app suma lo suyo (nüün `released_files`) sin romper a nadie. Si es propio, conviene darle el prefijo de la app (`nuun_released_files`).

### 3.2 Etapas

El vocabulario común, en el orden en que suelen ir:

| `stage` | Qué dice la vista |
| --- | --- |
| `preparing` | Preparando: leer el plan, montar, abrir |
| `listing` | Contando qué hay que hacer |
| `scanning` | Escaneando (un escaneo largo es una acción más, con `unit` `items`) |
| `reviewing` | Revisando lo que ya existe en el destino (hashear para no copiar dos veces) |
| `copying` | Copiando |
| `verifying` | Verificando |
| `rendering` | Renderizando |
| `analyzing` | Analizando |
| `releasing` | Liberando originales |
| `writing` | Escribiendo el resultado o el recibo |
| `finishing` | Cerrando: publicar, limpiar |

- **Debe:** una etapa propia de una app lleva su prefijo, `<app>.<etapa>` (`nuun.metadata_backup_after`, `clipper.compose_canvas`), con `^[a-z][a-z0-9-]{0,31}\.[a-z][a-z0-9_]{0,63}$`. Un cliente que no la conoce muestra «Trabajando» y conserva los números.
- **Debe:** una etapa sin prefijo que no está en la tabla no es válida.
- Una etapa común nueva entra en una versión menor de este contrato.

### 3.3 La medida y el latido

- **Debe:** `overall_done` y `overall_total` miden la acción **completa**, no el paso actual ([interacción §7](nu9ve-interaction-v1.md#acciones-largas)). `overall_done ≤ overall_total` cuando hay total.
- **Debe:** la fracción `overall_done / overall_total` **nunca retrocede** entre dos registros del mismo `progress_id`. Si cambia el total (un archivo que no se esperaba, un reencode), el motor redistribuye lo pendiente sin bajar la fracción: es la regla de `ProgressPlan` de Clipper. Un reintento o una relectura no mueve la barra hacia atrás (`ActionMeter.reach` de nüün).
- **Debe:** `overall_total` `null` (o 0) es una barra indeterminada, pero visible. Lo que se sabe (`files_done`, `current`) se sigue mostrando.
- **Debe:** el 100 % (`overall_done == overall_total`) llega sólo con `state` `complete`. Mientras corre, el motor deja el último tramo para cerrar (validar, publicar).
- **Debe:** mientras `state` es `running`, el motor escribe el registro **al menos cada 10 s**, aunque no avance, sólo para mover `updated_at`. Así el cliente distingue una acción lenta (`progressed_at` viejo, `updated_at` reciente) de un proceso que ya no está (los dos viejos) (§4).
- **Conviene:** escribir no más de 4 veces por segundo. Al terminar un archivo y al cambiar de etapa, siempre.

### 3.4 Al terminar

| `state` | Qué lleva | Qué muestra la vista ([interacción §7](nu9ve-interaction-v1.md#acciones-largas)) |
| --- | --- | --- |
| `complete` | `outcome` con `summary`; `warnings` vacía | Éxito |
| `complete` | `outcome` con `warnings` no vacía | Hecho, con aviso: qué quedó pendiente |
| `failed` | `error {code, message}`; `outcome` con lo que sí quedó hecho, si hubo algo; `resumable` | Error en palabras, qué quedó hecho, y «Reanudar» o «Reintentar» |
| `cancelled` | `outcome` si hubo algo; `resumable` | Cancelado, qué quedó hecho |

- `outcome`: `{summary, warnings, receipt}`. `summary` es una frase (hasta 500 caracteres, «540 archivos copiados y verificados en MEDIA502»). `warnings` es una lista de `{code, message}` (hasta 20); `code` es `^[a-z][a-z0-9_]{0,63}$`, por ejemplo `changed_during_copy` («8 archivos cambiaron durante la copia: entran en el próximo respaldo»). `receipt` es la ruta absoluta del recibo o `null`.
- **Debe:** `error.code` es un código de [motor §7](nu9ve-engine-v1.md#7-códigos-de-error) o uno propio de la app, y `error.message` dice en palabras qué pasó, sin rastros de pila.
- **Debe:** un registro terminado no vuelve a `running`. Una acción cancelada no publica salidas parciales ([motor §6.2](nu9ve-engine-v1.md#62-progreso-y-cancelación)): su `outcome` dice lo que quedó hecho antes de cancelar, no lo que se publicó.
- **Debe:** al terminar, `cancellable` es `false` y `current` se vacía o se quita.

### 3.5 Dónde vive

El registro lo escribe la acción, que corre en otro proceso, y lo lee el motor persistente para responder `action_progress`. Dónde lo guarda es de cada app: nüün, en los metadatos de su catálogo (`action-progress:<id>`); Clipper, en el archivo de progreso de su trabajo.

- **Debe:** escribirlo de forma que una lectura nunca vea un registro a medias: una fila de una base, o un archivo con [`nu9ve-fs`](nu9ve-fs-v1.md) (temporal y renombre).
- **Debe:** si la acción muere sin escribir su cierre, el registro se queda en `running` y el latido envejece. El motor que lo detecta (el proceso de la acción ya no existe) **puede** escribir `failed` con `error.code` `internal_error`; nunca `complete`.
- **Debe:** conservar el registro terminado el tiempo de la §2. **No debe** borrarlo al salir la acción.

## 4. Cómo lo lee un cliente

Lo que hace la vista común con los registros que lee. La referencia es `client_view` en el script de ejemplos; en Swift, `ActionWatch` de [`Nu9veAction`](../swift/README.md), que pasa los mismos ejemplos.

- **Debe:** sondear cada 0,5 a 1 s con la vista en pantalla o compacta, y no más de cada 5 s si sólo se ve el indicador (§5 de interacción).
- **Debe:** mostrar la fracción más alta que ya vio de ese `progress_id`, por si un motor no cumple la §3.3.
- **Debe:** la **ETA** la calcula el cliente con el tiempo transcurrido y la fracción global, suavizada. Empieza tras **5 s y 2 %**. Tras **15 s** sin que suba `progressed_at`, muestra «ETA pendiente» y conserva el tiempo transcurrido; vuelve cuando hay avance. No dice «trabado»: una medición larga tampoco avanza.
- **Debe:** si `updated_at` tiene más de **60 s**, la vista dice «Sin señal de la acción desde hace N s» y ofrece cancelar si `cancellable`. Sigue sondeando: si el latido vuelve, todo sigue como antes.
- **Debe:** `state` `unknown` antes del primer registro se muestra como «Preparando». Si llega `unknown` después de haber visto un registro, la acción ya no está en el motor: la vista dice «La acción terminó; su resultado ya no está disponible» y deja de sondear.
- **Debe:** un registro inválido (ver los inválidos de los ejemplos) no se muestra: la vista conserva el último válido y lo registra para el desarrollo.
- **Debe:** los porcentajes en pantalla truncan, no redondean, y la barra y el texto usan el mismo número (Clipper). Sin total, no hay porcentaje.

## 5. Privacidad

- `current`, `log`, `label` y `outcome` llevan nombres de archivos y de discos. Son personales, como los datos `personal` de [data](nu9ve-data-v1.md#6-privacidad).
- **Debe:** el agente y `nu9ve mcp` no publican el registro fuera de la máquina. Un cliente MCP que lo lea lo recibe marcado como datos, no instrucciones ([MCP §4](nu9ve-mcp-v1.md)).
- **No debe** llevar contraseñas, tokens ni contenido de archivos.

## 6. Ejemplos

[`data/action-examples-v1.json`](../data/action-examples-v1.json) lleva:

- `requests`: peticiones de `action_progress` con su respuesta esperada. `--server` las manda a un motor real como marcos (lo que el motor no conoce responde `unknown`);
- `valid` e `invalid`: registros, y por qué no vale cada inválido;
- `sequences`: lecturas sucesivas de una acción, con el instante en que el cliente lee, y lo que muestra (fracción, ETA, «ETA pendiente», «sin señal», cierre).

```bash
python3 -I scripts/action-examples.py           # vuelve a escribir el archivo
python3 -I scripts/action-examples.py --check   # falla si no es lo que escribe el script
python3 -I scripts/action-examples.py --server <motor> <args>
```

## 7. Cómo llegan las apps

| App | Hoy | Para cumplir |
| --- | --- | --- |
| nüün | `ActionMeter` (Python) y `Meter`/`ActionProgress` (Rust) escriben `overall_done`, `overall_total`, `files_*`, `current`, `log` y `stage` en `action-progress:<id>` y `closure-progress:<id>`; métodos `action_progress`, `closure_progress` y `storage_progress` | Sumar `contract`, `progress_id`, `action`, `state`, `unit`, `cancellable` y los tres instantes con el latido; pasar sus etapas a las comunes o a `nuun.*`; **no borrar el registro al terminar** (hoy `Drop` lo borra) y escribir el cierre; servir el respaldo de conversaciones y la revisión de «Copiar al otro disco» con el mismo registro |
| Clipper | `ProgressPlan`: pesos por etapa, fracción global que no retrocede, Stage y Step, ETA tras 5 s y 2 %, «ETA pending» a los 15 s, watchdog de 120 s; archivo de progreso por trabajo | `unit` `work` con sus pesos, `phase` y `step` con sus Stage y Step, y `action_progress` en `clipper-core serve` leyendo el archivo de progreso. Su watchdog sigue siendo suyo |
| nove | Estado de sus trabajos y `events` con cursor `after` | `action_progress` para sus trabajos largos; `events` sigue para su historial |

## Adopción

Quién cumple qué está en [adopción por app](../docs/adoption.md#nu9ve-action-v1).

## Cambios

Un campo o una etapa común nueva es una versión menor, con su línea en el [CHANGELOG](../CHANGELOG.md). Cambiar el significado de un campo o la regla de la §3.3 es `nu9ve-action/2`.
