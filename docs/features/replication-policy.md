# Política de respaldo y disco de trabajo

**Decisión del usuario, 2026-10-25:** MEDIA502 (HDD) es el respaldo; ESSD es el disco de trabajo. No hay segunda copia de archivo por ahora. Al terminar de editar un proyecto se eliminan sus copias de ESSD para seguir teniendo espacio; eso es el flujo normal, no una excepción. Prioridades en [PLAN](../../PLAN.md).

## Reglas que se derivan

1. **Nada se elimina de ESSD si no está verificado en MEDIA.** Para material ingresado por el motor, la prueba es el recibo de archivo más una comprobación rápida (stamp, o rehash a elección) de la copia en MEDIA en el momento de liberar. Para material viejo o creado en ESSD (ediciones, finales, `0000_acomodar`, bibliotecas de Final Cut) la prueba es la [auditoría de cobertura](#auditoría-de-cobertura): contenido idéntico (SHA-256) en MEDIA.
2. **Lo que existe sólo en ESSD se archiva primero:** job verificado ESSD → MEDIA con recibos, sin limpieza automática del origen. Los finales van a `002_finales`.
3. **Liberar trabajo es una acción explícita por proyecto:** muestra bytes, qué queda sólo en MEDIA y qué no está cubierto, y nunca la dispara un evento.
4. **Única réplica, aceptada conscientemente:** después de liberar, MEDIA es la única copia de esos raws. Mitigaciones que no requieren otro disco:
   - rehash periódico de MEDIA, con `recover-catalog` en un catálogo temporal; ya probado: 8.2 GB en 70 s;
   - export del catálogo SQLite a `.ncreator/catalog-backups/` en MEDIA.

## Auditoría de cobertura

`nu9ve media coverage --work ESSD --archive MEDIA502 [--path CARPETA] [--no-hash]` ([coverage.py](../../media/coverage.py)). Es de sólo lectura en ambos discos.

- Indexa MEDIA por tamaño y recorre ESSD (o una subcarpeta).
- Un archivo de ESSD con recibo de archivo en el catálogo cuenta como cubierto sin rehash.
- Si no hay recibo pero sí candidatos del mismo tamaño en MEDIA, compara SHA-256, aunque los nombres difieran (el importador viejo renombraba).
- `--no-hash` da sólo coincidencias probables por tamaño, para una primera pasada rápida.

Clasifica cada archivo como `in_archive`, `size_match` u `only_work`, con resumen por carpeta. El informe JSON queda en `<catálogo>/reports/`.

Por volumen (ESSD tenía 434 GB al 2026-10-25) la corrida completa con hash toma horas: lanzarla de noche. **Aún no se ha corrido sobre los discos reales.**

## Opciones futuras (no decididas)

Si más adelante hay otro disco o nube fría: job `replicate` verificado MEDIA → segundo destino, cobertura por asset y rehash programado. Nube fría sólo para finales y catálogo (~40 GB) sería la opción más barata sin disco nuevo.
