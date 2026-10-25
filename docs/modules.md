# Mapa de módulos

El mapa de ownership y reglas de cambio está en [development](development.md#dónde-implementar-cada-cambio); el estado vigente en [STATUS](STATUS.md). Esta página conecta el núcleo nuevo con los brazos anteriores.

```text
nu9ve.py
  save → media.save → planner / transfer / catalog
  media → cli / service → ingest, library, recovery, publication, captures
    rpc → una petición JSON; mcp → herramientas de lectura por stdio
  inspect → manager.inventory
  doctor → configuration / constants / herramientas del legado
  legacy-save → manager.file.save_files
  download / split → manager.file → yt-dlp / scdl / demucs

native/MediaMenu
  UI/modelo → MediaMenuCore JSON/Process → media.service
  eventos de volúmenes + FSEvents de captura configurada
```

`media/` no importa constantes globales de destinos del legado. Su registro/configuración viven en el catálogo local. `save all` consulta el mapa histórico sólo para ayudar a identificar tarjetas; no hereda automáticamente raíces como destinos registrados.

`manager/video.py` y `manager/audio.py` siguen atendiendo metadata de los flujos antiguos. `hardware/` conserva drivers y pruebas manuales MIDI; `utils/` experimentos antiguos incompletos; `playground/` scripts manuales con posibles efectos reales/S3. No cargarlos desde la app idle ni eliminarlos sin confirmar usos. `social/` no tiene una implementación conectada.

El editor interno `clipper/` fue retirado. Fuentes de verdad externas: captura Stalker, edición/perfiles/queue Clipper y geografía Atlas. [Índice de contratos](contracts/README.md).
