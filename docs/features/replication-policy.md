# Política de respaldo y disco de trabajo

**Decisión del usuario, 2026-10-25:** MEDIA502 (HDD) es el respaldo; ESSD es el disco de trabajo. No hay segunda copia de archivo por ahora. Al terminar de editar un proyecto se eliminan sus copias de ESSD para seguir teniendo espacio; eso es el flujo normal, no una excepción. Prioridades en [PLAN](../../PLAN.md).

## Reglas que se derivan

1. **Fuentes originales y finales no se eliminan de ESSD sin respaldo verificado en MEDIA.** **Excepción decidida el 2026-11-04:** los intermedios identificados se descartan sin copia bajo cierre revisado y confirmado; la primera entrega sólo toma `renders/` de proyectos conocidos. Lo desconocido se conserva y Final Cut queda fuera. [Contrato de cierre](../contracts/project-closeout-v1.md). Los planes de liberación implementados exigen rehash actual de MEDIA y del origen, también para material con recibos. Para material viejo o creado en ESSD (ediciones, finales, `0000_acomodar`, bibliotecas de Final Cut) la prueba es la [auditoría de cobertura](#auditoría-de-cobertura): contenido idéntico (SHA-256) en MEDIA.
2. **Fuentes y finales que existen sólo en ESSD se archivan primero:** job verificado ESSD → MEDIA con recibos. Los finales van a `002_finales`; el cierre puede encadenar copia y liberación explícita. Los intermedios descartables no se archivan.
3. **Liberar trabajo es una acción explícita por proyecto:** muestra bytes, qué queda sólo en MEDIA y qué no está cubierto, y nunca la dispara un evento.
4. **Única réplica, aceptada conscientemente:** después de liberar, MEDIA es la única copia de esos raws. Mitigaciones que no requieren otro disco:
   - rehash periódico de MEDIA, con `recover-catalog` en un catálogo temporal; ya probado: 8.2 GB en 70 s;
   - export del catálogo SQLite a `.ncreator/catalog-backups/` en MEDIA.

## Auditoría de cobertura

`nu9ve media coverage --work ESSD --archive MEDIA502 [--path CARPETA] [--no-hash]` ([coverage.py](../../media/coverage.py)). Es de sólo lectura en ambos discos.

- Indexa MEDIA por tamaño y recorre ESSD (o una subcarpeta).
- La auditoría con hash comprueba los bytes actuales, también para archivos con recibos previos; el recibo solo no prueba presencia/integridad actual.
- Si no hay recibo pero sí candidatos del mismo tamaño en MEDIA, compara SHA-256, aunque los nombres difieran (el importador viejo renombraba).
- `--no-hash` da sólo coincidencias probables por tamaño, para una primera pasada rápida.

Clasifica cada archivo como `in_archive`, `size_match` u `only_work`, con resumen por carpeta. El informe JSON queda en `<catálogo>/reports/`.

Por volumen (ESSD tenía 434 GB al 2026-10-25) la corrida completa con hash toma horas: lanzarla de noche. **Aún no se ha corrido sobre los discos reales.** La app ofrece la auditoría y planes explícitos de liberación de raws por proyecto; [flujo, comprobaciones y límites](../operations.md#bandeja-stalker-proyectos-y-liberar-espacio). El cierre añade descarte explícito de renders; editables Clipper y Final Cut se conservan.

## Opciones futuras (no decididas)

Confirmado el 2026-10-30: una copia de respaldo basta por ahora; la redundancia no es prioridad.

Próximamente, para quien tenga varios discos: segundo destino configurable con job `replicate` verificado MEDIA → segundo destino, cobertura por asset y rehash programado.

La nube podría ofrecerse como producto de paga para redundancia y otras funciones; sólo finales y catálogo (~40 GB) sería el punto de partida más barato. Es una idea de producto, sin diseño ni compromiso.
