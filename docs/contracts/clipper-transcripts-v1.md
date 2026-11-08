# Transcripciones de Clipper en MEDIA · v1 · propuesta · 2026-11-07

Decisión del usuario (7 nov): las transcripciones que Clipper va sacando se conservan a largo plazo, **ligadas a los videos ya respaldados**, en un área de datos por programa dentro de MEDIA. La caché regenerable no se respalda.

Reparto, por las fronteras vigentes:
- **Clipper** es dueño del contenido y decide qué transcripción es la vigente. Sólo la **exporta**; nunca escribe en MEDIA.
- **nüün** es el único que escribe en MEDIA: copia, verifica por SHA-256, deja recibo y la liga al asset del catálogo.

## 1. Lo que expone Clipper

```sh
clipper library transcripts-export <project_root> --json
```

Sólo lectura. Responde un documento `clipper-transcript-export/1`:

```json
{
  "schema": "clipper-transcript-export/1",
  "project_root": "/Volumes/ESSD/001_proyectos/e64/terra-6-1-extra/clipper",
  "project_state": "active | retired",
  "generated_at": "2026-11-07T23:40:00Z",
  "entries": [
    {
      "path": "/Volumes/ESSD/.../clipper/transcripts/26OCT27-040446_SONYA6400_23-11_MX-TERRA-6-1-EXTRA[].transcript.json",
      "format": "transcript.json | srt | vtt | ass | provider-response.json",
      "bytes": 123456,
      "sha256": "…64 hex…",
      "stamp": {"device": 0, "inode": 0, "size": 0, "mtime_ns": 0, "ctime_ns": 0},
      "source": {
        "path": "/Volumes/ESSD/001_proyectos/e64_raw/terra-6-1-extra/26OCT27-040446_SONYA6400_23-11_MX-TERRA-6-1-EXTRA[].MP4",
        "asset_id": "uuid de nüün si la fuente llegó por `sources`/handoff, o null",
        "member_sha256": "sha256 del miembro de nüün si se conoce, o null",
        "clipper_fingerprint": "huella de transcript_store"
      }
    }
  ],
  "digest": "sha256 canónico del documento sin este campo"
}
```

- Lista sólo las transcripciones vigentes de ese proyecto, no la caché global ni variantes descartadas.
- `source.path` es la ruta que Clipper transcribió. `asset_id` y `member_sha256` van cuando la fuente llegó desde nüün (`sources`), y si no, `null`.
- **Precisiones de la implementación de Clipper (2026-11-07):**
  - «Vigente» es el `transcript_path` actual de cada fuente del proyecto, más sus hermanos `.provider-response.json`, `.srt`, `.vtt` y `.ass` cuando existen. No entran otras transcripciones de `transcripts/` ni `transcripts/derived/` (las de cortes, que se regeneran desde la edición).
  - `source.path` es la ruta actual del item en el proyecto, la misma que usa Clipper al transcribir o tras un relink. Campo aditivo: `source.source_id`, el id del item en Clipper.
  - `member_sha256` sale de los miembros que guardó el handoff (`editable_members`, `media_members` o `members`, comparando `local_path` o la ruta dentro del bundle). En un asset de un solo archivo sin miembros, sale de su `sha256`.
  - `clipper_fingerprint` es `null` si la fuente está offline; la entrada se lista igual.
  - `generated_at` entra en el digest, así que dos exports del mismo contenido tienen digests distintos. Para comparar contenido se usan `sha256` y `stamp` de cada entrada.
  - Funciona igual con un proyecto retirado (`clipper-retired-project.json`). Storage / Cleanup protege todo `transcripts/` y nunca lo lista en `cleanup-plan`.
- Clipper no garantiza que la fuente esté respaldada: eso lo comprueba nüün.

## 2. Lo que hace nüün

**Área en MEDIA:** `004_datos/<programa>/<tipo>/AAAA/MM/<nombre del clip>/<archivo original>`. Para Clipper:
`004_datos/clipper/transcripts/AAAA/MM/<nombre del clip archivado>/<archivo>`. `AAAA/MM` y el nombre del clip son los del miembro ya archivado en `001_raw`, igual que los sidecars de `001_raw/metadata`. Así la transcripción queda junto al nombre del video respaldado aunque la carpeta de edición desaparezca.

**Ligar al asset:**
1. por `asset_id` y `member_sha256` si vienen;
2. si no, por `source.path`, comparándolo con los destinos de trabajo registrados en los recibos;
3. una fuente que no corresponda a un miembro respaldado no se copia, y la revisión lo dice.

**Copia:** se usa el mismo Transfer verificado (`.ncpartial`, SHA-256 y renombre sin reemplazar), con recibo `nu9ve-data-receipt/1` que guarda el asset, el miembro, el formato y el digest del export. Un archivo idéntico ya presente se adopta. Uno distinto en el mismo destino bloquea, salvo que sea una versión nueva: se guarda junto al anterior con sufijo de fecha, sin reemplazarlo.

**Cuándo:**
- en el cierre de proyecto: la revisión muestra «Transcripciones a respaldar» y el cierre no termina sin verificarlas;
- y a pedido desde el proyecto («Respaldar transcripciones»).

El ingreso de la SD no espera transcripciones.

**Nada se borra en este flujo.** Los originales siguen en `clipper/`. Su limpieza, si algún día procede, la decide Clipper con su lista de regenerables, y la transcripción no está en ella.

## 3. Estado

| Parte | Estado |
| --- | --- |
| Clipper `transcripts-export` | Implementado sin commit (7 nov), con pruebas en carpetas temporales |
| nüün área `004_datos`, copia y recibo | Pendiente, este repo |
| nüün revisión de cierre y acción por proyecto | Pendiente, este repo |
| Nombre del área en [media-naming-v1](media-naming-v1.md) | Se registra al implementar |

Las pruebas de las dos partes usan fixtures y carpetas temporales; nunca ESSD ni MEDIA502. La aceptación física es una prueba aparte con un proyecto pequeño.
