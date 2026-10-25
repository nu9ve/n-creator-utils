# Desarrollo, modularidad y continuidad

Lee [CONTEXT](CONTEXT.md) y [STATUS](STATUS.md) antes de cambiar código. [PLAN](../PLAN.md) fija prioridades; esta página describe cómo trabajar. No volver a auditar todos los repos para una mejora pequeña.

## Dónde implementar cada cambio

| Responsabilidad | Archivos | Regla |
| --- | --- | --- |
| Invariantes, rutas relativas, hashes y IO | [common](../media/common.py) | Sin dependencias de UI ni globals del importador histórico. |
| Clasificación, asignaciones, preview y política efectiva | [planner](../media/planner.py) | La misma planificación alimenta CLI y app; no copiar reglas de rutas en Swift. |
| Montajes, identidad, espacio y capacidades | [volumes](../media/volumes.py) | Adaptadores por plataforma; nombres de disco no son identidad. |
| Copia, verificación, limpieza y replay | [transfer](../media/transfer.py) | Única ruta nueva de escritura/limpieza de fuentes. Pruebas de fallo obligatorias. |
| Persistencia y decisiones humanas | [catalog](../media/catalog.py) | IDs estables, migraciones y un writer; catálogo local, recibos portables. |
| Resolución, restore, pendientes y handoff | [library](../media/library.py), [recovery](../media/recovery.py) | Mantener procedencia y relaciones; no editar proyectos ajenos. |
| Capturas y publicaciones | [captures](../media/captures.py), [publication](../media/publication.py) | Finalización/publicación explícitas; no inferirlas de un MOV o export existente. |
| Boundary de operaciones | [service](../media/service.py), [cli](../media/cli.py), [save](../media/save.py), [MCP](../media/mcp.py) | JSON estructurado; stdout no mezcla logs. MCP actual sólo lectura. |
| App nativa | [main.swift](../native/MediaMenu/Sources/MediaMenu/main.swift) | UI/eventos; no reimplementar transferencias. El modelo y las vistas aún deben separarse al crecer. |
| Puente/runtime/eventos nativos | [Protocol](../native/MediaMenu/Sources/MediaMenuCore/Protocol.swift), [CaptureWatcher](../native/MediaMenu/Sources/MediaMenuCore/CaptureWatcher.swift) | Preservar enteros de 64 bits y errores; cero workers periódicos en idle. |
| Empaquetado | [builder](../scripts/build_media_app.py), [pyproject](../pyproject.toml) | Artefacto nuevo por build; distinguir runtime externo de bundle distribuible. |

`manager/file.py` conserva legacy-save/download/split. No hace falta reescribirlo para agregar una vista al dashboard. Futuras extracciones de download/stems deben entregar assets/provenance por el servicio, sin acoplarse a Swift.

## Validación proporcionada al cambio

```sh
python3 -m unittest discover -s tests -p test_media_engine.py -v
python3 -m unittest discover -s tests -v
python3 -m compileall -q media manager nu9ve.py scripts
swift test --package-path native/MediaMenu
git diff --check
```

No ejecutar todo repetidamente cuando ya pasó y no cambió el comportamiento. Docs: consistencia y enlaces. UI: compilar y revisar el flujo afectado con estado aislado. Cambios de integridad: primero fixture/fallo dirigido, luego suite completa de límites. El estado exacto del último pase está en [STATUS](STATUS.md), no en un conteo duplicado aquí.

`tests/test_media_engine.py` incluye `FixtureVolumes`: simula identidades y fallos sin registrar discos reales. Hay hard exits en subprocess, corrupción del mismo tamaño y recuperación de limpieza. Las pruebas antiguas siguen cubriendo el importador legado y los adaptadores externos simulados. Una prueba de bytes Sony no sustituye la prueba de cámara.

No ejecutar `hardware/test.py` o playground como suite automática. Para fixtures locales con el backend real hay registro explícito `--allow-local`; eso no demuestra redundancia ni convierte carpetas del mismo disco en discos distintos.

## Evolucionar un contrato

1. Cambiar el owner del dominio y su entrada en [contracts](contracts/README.md).
2. Versionar cambios incompatibles; conservar campos desconocidos compatibles. Añadir fixture/replay del comportamiento, no sólo del serializer.
3. El servicio valida, CLI transporta y Swift presenta. No generar otra verdad de paths, estado de copia o timeline editorial.
4. Campos numéricos como `mtime_ns` deben seguir exactos al cruzar Swift/Python. JSON válido no equivale a plan intacto.
5. Registrar adopción por consumidor. Exportar handoff aquí no significa que Clipper ya lo lea.

## Cerrar y retomar una sesión

Handoff recomendado, en pocas líneas:

```text
Objetivo / tarea de PLAN:
Estado: implementado | en curso | bloqueado | validación pendiente
Archivos/módulos tocados:
Decisiones e invariantes afectadas:
Comandos realmente ejecutados y resultado:
Pendientes/riesgos concretos:
Siguiente acción mínima:
Enlaces a STATUS, contrato o nota de decisión:
```

En otra sesión: leer CONTEXT → STATUS → tarea de PLAN → módulos relevantes. Revisar `git status` antes de editar; este checkpoint tiene cambios anteriores staged y trabajo nuevo sin seguimiento. No asumir un checkout limpio ni restagear todo.

Para trabajo simultáneo, definir superficie y archivos antes de empezar. UI/dashboard y revisión del motor pueden avanzar en sesiones distintas si el protocolo se mantiene; cambios en `service`, catálogo o schemas requieren acordar el boundary. Los agentes de Stalker/Clipper/Atlas revisan sus propios contratos; no enviarles mensajes ni editar sus repos sin autorización del usuario.

## Mantener documentación viva

- CONTEXT: objetivo, invariantes, mapa breve; cambiar sólo cuando cambia el producto.
- STATUS: hechos, evidencia, artefactos y límites de la última entrega; actualizar al cerrar sesión.
- PLAN: prioridades y aceptación, sin repetir toda la historia.
- Owner docs/contratos: semántica y decisiones durables; enlazar desde el índice.
- Notas históricas: fecha y scope visibles; no presentarlas como implementación vigente.

Una nueva feature no necesita otra guía general completa. Añadir sección al owner o una nota corta enlazada suele bastar.
