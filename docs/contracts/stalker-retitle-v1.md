# Handoff Stalker · retitle de completed · 2026-10-28

Estado vigente: coordinador y recuperación implementados en nüün; Stalker ya
implementa `stalker-coordinated-retitle/1`, probado con su CLI real en fixtures.
El [cierre de publicación](#publicación-sobre-una-copia-nueva--2026-10-29) usa ese
escritor sobre una copia nueva, conservando la anterior. El retitle in-place
mantiene sus guardas. Clipper deps/ACK y `media-release-lock/1` se reutilizan.
Sin instalación ni aceptación física nueva; evidencia en [STATUS](../STATUS.md).
Las instrucciones históricas de implementación de abajo no indican tareas aún
pendientes del productor.

## Recuperación pending adoptada por nüün · 2026-10-28

Stalker ya implementó el productor/UI/CLI ([entrega](../../../../ai/stalker/docs/2026-10-28-coordinated-retitle.md)).
El coordinador ahora acepta status pending con IDs coincidentes, proyecto exacto,
hash anterior esperado, hash nuevo SHA-256 y destino absoluto `.stalker` en el
mismo parent, sin traversal ni symlinks. Revalida archivo/jobs/planes y deps
frescos para **ambas rutas**, incluso si el origen ya fue renombrado. Después
repite el apply existente con el request original, mismo ID y ambos FDs vivos;
Stalker valida/reproduce su journal. Nüün no exige el manifest viejo en su ruta
anterior ni escribe/reconstruye el manifest durante recuperación. El ACK final
pasa la misma comprobación de hash, identidad y cambio exclusivo de project.
Errores, resultado inválido, apply que aún responde pending o blockers conservan
la intención pending; jamás publican éxito parcial. Failed sigue requiriendo
recuperación del productor. Reintentar tras applied reconcilia sin otro apply.

El schema exige path/project/ambos hashes también en pending. Sin journal
verificable no se despacha recuperación. La especificación histórica de abajo
que detenía todo pending queda sustituida por esta sección. No cambia la
adopción de nuevas revisiones en Clipper ni la aceptación física.

Puente repetible: `python3 scripts/test_stalker_retitle_bridge.py` desde nüün,
con checkout/binario debug de Stalker existente. Carga su suite sólo lectura y
sustituye el paso histórico de recuperación manual por reintento del coordinador;
prueba los diez SIGKILL con el productor real y estado temporal, sin editar el
repo Stalker ni compilar/instalar/abrir apps.

## Para retomar

Nüün ya coordina; Stalker implementa `nuun-retitle capabilities|status|apply --json`
con `stalker-coordinated-retitle/1`, CAS de hash crudo y ambos FDs heredados.
Requests/results: [schema](stalker-retitle.schema.json). Fixtures y comando de
prueba: sección final. `archived:false` solo no autoriza; archivo, planes y
referencias/colas desconocidas bloquean. Repetir mismo ID consulta status y
recupera; nunca editar manifest desde nüün ni adoptar revisión en Clipper.
Aceptación siguiente: journal durable del productor + tests de crash/rename;
después adopción Clipper y ciclo físico autorizado, pendientes por separado.

## APIs exactas

Servicio común (`media rpc`, request `{method,params}`):

- `retitle_capture` params `{operation_id,path,session_id,expected_manifest_sha256,project}`.
  UUID de operación, ruta absoluta actual, UUID de sesión, SHA-256 crudo esperado,
  `project:{version:1,name,country}` (strings no vacíos, máximo 120). El nombre
  final lo calcula **Stalker** con naming v1, nunca una ruta arbitraria del cliente.
- `retitle_operations`, sin params: operaciones persistidas, request, estado
  `pending|applied`, fecha y resultado cuando existe. Pending conserva el manifest
  anterior para validar que sólo cambió project. Es interfaz local, con paths.
- `cancel_plan` params `{plan_id}`: cancela exclusivamente un plan revisado que
  aún no inició un job. No descarta journals de jobs ni de retitle.
- `integration_settings` acepta `stalker_executable` absoluto, igual que el
  ejecutable Clipper existente. Ejemplo vía RPC con catálogo **fixture**:
  `{"method":"integration_settings","params":{"stalker_executable":"/tmp/fixture/stalker-session"}}`.

CLI equivalente:

```text
nu9ve media retitle-capture OPERATION_UUID /absolute/session.stalker SESSION_UUID RAW_SHA256 --project '{"version":1,"name":"proyecto","country":"mx"}'
nu9ve media retitle-operations
nu9ve media cancel-plan PLAN_UUID
```

Usar `media --state-dir /tmp/fixture/state ...` en pruebas; nunca el catálogo
normal. Stdout mantiene envelope `{ok,command,data}` / `{ok:false,error:{code,message}}`.
Resultado de retitle es `nuun-retitle-operation/1`, con request normalizado
(UUIDs lowercase, `catalog_id` absoluto agregado por nüün), estado y result.
El mismo ID/payload devuelve el resultado previo; otro payload da conflicto.

## Lo que debe implementar Stalker

Comando nuevo del ejecutable, sin UI, stdin JSON y stdout JSON único (sin
envelope, logs a stderr), exit 0 en respuesta de protocolo, otro en fallo:

```text
stalker-session nuun-retitle capabilities --json   # stdin {}
stalker-session nuun-retitle status --json         # stdin request completo
stalker-session nuun-retitle apply --json          # stdin el mismo request
```

[Schema](stalker-retitle.schema.json):

- capabilities: `{schema:"stalker-capabilities/1",capabilities:["stalker-coordinated-retitle/1"]}`.
- request: `{schema:"stalker-coordinated-retitle/1",operation_id,path,session_id,expected_manifest_sha256,project,catalog_id}`.
- status not_found: `{schema:"stalker-retitle-result/1",operation_id,session_id,status:"not_found"}`.
- aplicado (por apply o status): `{schema:"stalker-retitle-result/1",operation_id,session_id,status:"applied",path,project,previous_manifest_sha256,manifest_sha256}`.
  La ruta nueva tiene el mismo parent que la vieja; UUID normalizado, proyecto
  exacto pedido, hashes crudos. pending/failed se reportan con ese schema/IDs;
  nüün conserva pending y exige recuperación del productor.

Capability significa **todas** estas garantías, no sólo reconocer el comando:

1. Antes de anunciarla, quitar la mutación directa de project/nombre de completed
   detrás de `captures.archived:false`. App/CLI deben delegarla a nüün. Recording,
   finalizing y recovery permanecen bajo exclusión propia de Stalker; recovery
   no debe mutar completed en sitio. Reiniciar procesos viejos obligatoriamente.
2. Usar el mismo catálogo/usuario/host de nüün. No hay autorización entre hosts,
   catálogos independientes ni apps que ignoran el protocolo. Los productores
   no abren otro catálogo para eludir un bloqueo. El propietario debe confirmar
   ausencia de consumidores externos antes de habilitar esta capability en su
   UI; Final Cut/Atlas no quedan cubiertos por el flock de Clipper.
3. `apply` exige **dos FDs heredados**, en `NUUN_RETITLE_LOCK_FDS` (editor,
   catálogo), y `catalog_id` correcto. Validar fstat/identidad contra
   `~/.cache/clipper/.media-access.lock` y `<catalog_id>/writer.lock`; rechazar
   llamada directa sin descriptors o con archivos equivocados. No basta una
   variable/token ni un archivo que diga “autorizado”. No soltar/reabrir/downgradear
   los FDs; representan open file descriptions bloqueadas por el coordinador.
   Nüün conserva además `CLIPPER_MEDIA_RELEASE_TOKEN` del guard existente; no
   inventar otro lease. El productor no necesita invocar relink.
4. Con los FDs vivos, adquirir exclusión propia de sesión y revalidar completed,
   UUID y SHA-256 crudo esperado **dentro** de esa exclusión. Bloquear conflictos,
   colisiones, rutas inseguras, cambio de revisión o versiones desconocidas.
   Cambiar sólo `manifest.project` y el basename del bundle con naming v1;
   no tocar tiempos, tracks, eventos, archivos, membresía ni sessionID. Cerrar y
   sincronizar todas las escrituras antes de publicar éxito.
5. Journal del productor durable antes de mutar, keyed por operation_id + hash
   del request completo. Same ID distinto payload → operation_conflict. Rename
   de carpeta + manifest + registro library.json requieren recuperación: fsync
   archivos y parent dirs, temporales + replace atómico, no alegar transacción
   atómica entre archivos. Stalker es el único escritor del manifest/registro.
6. `status` nunca muta ni adopta una revisión: not_found sólo si se garantiza que
   no empezó ninguna mutación para ese ID. Tras un crash parcial, pending/failed
   hasta recuperar de su journal; tras éxito devuelve el resultado original
   aunque ya no exista la ruta vieja. Un status applied es durable y verificable
   leyendo su ruta nueva. Guardar el request para detectar reutilización de ID.
7. Mantener ambos FDs durante comprobación/mutación/publicación, incluso si muere
   nüün. No daemonizar ni lanzar un writer que sobreviva sin esos descriptors.
   El kernel libera locks al morir el último propietario; no TTL ni “unlock” de
   archivos. Un writer huérfano conserva exclusión hasta terminar; un fallo debe
   dejar journal recuperable. Recovery del productor usa la misma exclusión.

## Secuencia implementada por nüün

1. Toma exclusivo de Clipper (`media-release-lock/1`, capability obligatoria) y
   después writer del catálogo; ambos abarcan checks y subprocess mutador.
   Pasa FDs heredados a Stalker: si muere el padre, el hijo mantiene exclusión.
2. Rechaza cualquier revisión de sesión ya archivada en `sessions` (recibos
   históricos cuentan incluso con disco offline), cualquier job que referencie
   sesión, plan revisado vigente o **cualquier plan de release no completado**
   (conservador). Planes revisados nuevos quedan registrados desde build_plan;
   cancelar uno no borra su documento y apply rechaza ese documento cancelado.
3. Reutiliza deps frescos + guard de referencias antiguas/colas de Clipper.
   Offline, ilegible, cola/version desconocida bloquean. Cualquier referencia
   a la ruta/interior bloquea incluso `in_use:false`. Sin reemplazo/relink en
   retitle: nueva revisión requiere decisión explícita del consumidor.
4. Valida manifest de origen por lectura segura y CAS crudo. Guarda intención
   pending durable **antes** de status/apply. Pending bloquea planning/apply de
   esa sesión y otra operación distinta hasta reconciliar; ausencia de ACK no
   se interpreta como ausencia de mutación.
5. Status not_found permite checks frescos y apply idempotente. Applied recupera
   sin repetir apply. Valida ACK (IDs, proyecto, hash anterior, ruta), lee el
   manifest final y verifica hash nuevo y que sólo cambió project.
6. Publica en una transacción SQLite operación applied + `retitle-result:UUID`
   con digest canónico nuevo. Planes anteriores con esa sesión/digest viejo
   fallan `plan_changed`, incluidos documentos exportados antes del registro
   de planes. Captures vuelve a descubrir el manifest/ruta del productor por
   su invalidación stat normal. No crea/adopta assets nuevos, no modifica
   Clipper ni promete migrar su revisión. Respuesta + retitle_operations son
   el resultado consultable; no hay webhook/daemon.

Interrupción: repetir **mismo request e ID**. Si productor no está disponible,
conservar pending. Si productor devuelve pending/failed o un ACK inconsistente,
recuperar primero en Stalker bajo los locks y volver a consultar. No borrar
intención ni autorizar manualmente con archived:false. No hay rollback por
escribir el manifest desde nüün. Si un consumidor quedó offline entre intent y
reintento, apply sólo ocurre tras checks frescos; status applied sólo reconcilia
un resultado ya durable, sin mutación adicional.

## Errores y aceptación para las dos sesiones siguientes

| Error nüün | Acción |
| --- | --- |
| `stalker_upgrade_required` | Implementar/configurar/reiniciar capability; no cambiar el manifest a mano. |
| `clipper_upgrade_required`, `clipper_busy`, `job_busy` | Actualizar consumidor o esperar al propietario del lock. |
| `retitle_archived` | Usar tags de catálogo; manifest archivado inmutable. |
| `retitle_plan_active`, `plan_cancelled` | Resolver job/release o cancelar plan no iniciado y revisar uno nuevo. |
| `retitle_dependencies` / `clipper_invalid` | Resolver referencias/colas/offline en Clipper; no adoptar revisión nueva automáticamente. |
| `retitle_pending`, `retitle_recovery_required`, `stalker_failed`, `stalker_invalid` | Consultar operaciones y recuperar mismo ID en productor; conservar sesión. |
| `revision_mismatch`, `plan_changed`, `operation_conflict` | No reusar revisión/payload diferente; revisar evidencia. |

Fixtures compartidas en [tests/fixtures/contracts](../../tests/fixtures/contracts):
manifest v3 con reloj y zona, casos de cámaras con/sin offset, handoff v1 legacy
y ejecutable sintético. Este último **no es un escritor Stalker de producción**:
escribe únicamente bajo STALKER_RETITLE_FIXTURE_ROOT temporal explícita y no
implementa su journal/fsync/rename completo. Ejemplo portable generado por el
productor en [examples/handoff.json](examples/handoff.json). Suite:
`python3 -m unittest discover -s tests -p test_media_contracts.py -v`.
JSON Schema se valida con jsonschema ya disponible, sin dependencia runtime nueva;
ese check se omite explícitamente si la herramienta no está instalada.

Sesión Stalker: implementar API + guarantees anteriores, copiar/reutilizar
fixtures y validar schemas; probar completed genérico/proyecto, CAS, rename
collision, versiones 1/2/3 y rechazo de major nuevo, crash antes/después de cada
paso manifest/rename/registry, status idempotente/conflict, exclusión con grabación,
planning, cola y muerte de padre/hijo. Actualizar sólo su fila de adopción común.

Sesión Clipper: consumir campos opcionales sin romper sources antiguo, validar
editable bytes y hash crudo, zona desconocida y mapping explícito; mantener
rechazo de manifest diferente y pedir adopción/reimport explícito. Portable
handoff sigue pendiente de consumidor. Luego acordar ciclo físico pequeño
**autorizado aparte** con ambas apps reiniciadas; fixtures no certifican captura,
sync/escucha, desconexión/remount ni retiro de originales reales.

## Publicación sobre una copia nueva · 2026-10-29

El retitle in-place de este documento conserva sus bloqueos. La operación
separada `publish-revision` crea/verifica una copia nueva en un archivo registrado,
antes de llamar al mismo escritor Stalker y conservar sus FDs/CAS/journal.
Stalker recibe sólo la ruta nueva sin referencias; nunca la copia archivada o
referenciada anterior. Nüün verifica project-only y publica recibos/historial
bajo el mismo asset; el editor adopta explícitamente después.
[Secuencia, CLI, recuperación y límites](media-library-v1.md#publicación-explícita-de-revisión-project-only).
El escritor actual actualiza también su registro library según su protocolo;
esta operación explícita no cambia la configuración de biblioteca de nüün.
No se necesita una capability nueva ni relajar `archived:false` o referencias.
