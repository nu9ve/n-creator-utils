# Política de réplicas del archivo (propuesta)

Estado: **propuesta para decidir**, nada implementado más allá de lo existente. Prioridad en [PLAN](../../PLAN.md). Se relaciona con «Liberar trabajo» en [media-experience](../media-experience.md).

## El problema

Hoy un ingreso deja MEDIA502 (archivo) + ESSD (trabajo): dos discos físicos. ESSD está al 94 %, y liberar copias de trabajo de proyectos publicados es parte del flujo. En cuanto se libera un proyecto, **MEDIA502 queda como copia única** de sus raws. Un recibo dice que los bytes se leyeron bien el día del ingreso, no que sigan sanos hoy. Un disco USB de 4.5 TB es un punto único de falla: caída, firmware, borrado accidental o robo.

## Regla propuesta

1. **Retiro de trabajo exige dos réplicas de archivo independientes.** Antes de proponer borrar copias de trabajo de un proyecto, cada asset debe tener ≥2 ubicaciones de rol archivo en discos físicos distintos (o MEDIA + nube), con verificación vigente. Sin eso, la UI lo muestra como «cobertura: 1 réplica» y no ofrece liberar.
2. **Segunda réplica por job de replicación verificado**, mismo motor: origen MEDIA, destino registrado con rol `archive`, recibos propios, idempotente y reanudable. Nunca una sincronización ciega tipo `rsync --delete`.
3. **Scrub periódico.** Rehash de una muestra o de todo lo que tenga más de N meses sin verificar, usando la ruta de `recover-catalog` (ya probada: 8.2 GB en 70 s desde MEDIA502). Registrar `last_verified_at` por copia; una diferencia marca la réplica como dañada y propone repararla desde la otra.
4. **Catálogo**: los recibos ya viven en cada archivo (`.ncreator/receipts`). Añadir export periódico del SQLite a `.ncreator/catalog-backups/` en cada disco de archivo, para conservar decisiones humanas que los recibos no tienen.

## Opciones de segunda réplica (decidir una)

| Opción | Pros | Contras |
| --- | --- | --- |
| A. Segundo disco rotatorio (p. ej. MEDIA503) guardado fuera de casa | Barato, rápido, sin servicio externo; restauración local. | Disciplina manual de rotación; mismo riesgo si ambos están juntos. |
| B. Nube fría (Backblaze B2, S3 Glacier Deep Archive) para raws de proyectos publicados | Fuera de sitio automático; no depende de rotar discos. | Costo mensual y de egreso; subir TBs toma días; requiere credenciales y cifrado del lado cliente. |
| C. A ahora + B sólo para proyectos publicados/finales | Equilibra costo y riesgo; los finales pesan menos. | Dos mecanismos que mantener. |

Recomendación: **C**, empezando por A. El motor ya sabe copiar y verificar entre discos, y B se suma después como otro tipo de destino, sin cambiar la regla 1.

## Criterios de aceptación cuando se implemente

- Job `replicate` (MEDIA → segundo archivo) con plan revisado, recibos y cobertura por asset en el catálogo.
- La UI y el CLI muestran cobertura (1/2 réplicas, última verificación) por proyecto. «Liberar trabajo» bloqueado con <2.
- `scrub` programable sin daemon nuevo (lanzado desde la app al montar ambos discos, o manual), con presupuesto de IO y cancelable.
- Pruebas en fixtures: réplica corrupta detectada, reparación desde la sana, nunca borrar la única copia sana.
