# n-creator-utils

Ingreso, respaldo verificado, catálogo y preparación de media para trabajar. CLI Python y app nativa de barra de menú en macOS. Stalker graba, [Clipper](https://github.com/nu9ve/clipper) edita y Atlas procesa imágenes de dron; este repo conecta almacenamiento y flujos, sin mantener otro editor.

**Estado: implementación de desarrollo; todavía no es una release de producción.** Hay motor, catálogo y app funcionales con pruebas, pero quedan validación física, hardening e instalación/distribución. La evidencia exacta y los límites están en [STATUS](docs/STATUS.md).

## Para continuar en otra sesión

1. [Contexto compacto](docs/CONTEXT.md): producto, flujo real e invariantes.
2. [Estado actual](docs/STATUS.md): qué existe, qué se probó y qué no.
3. [Plan priorizado](PLAN.md): siguiente trabajo y criterios de aceptación.

[Índice completo](docs/README.md) · [Desarrollo modular](docs/development.md) · [Dashboard/launcher](docs/features/dashboard-launcher.md) · [Contratos entre apps](docs/contracts/README.md).

## Flujo principal

**SD → plan revisado → copias requeridas → lectura SHA-256 de destinos → recibos → limpiar SD.** El objetivo es dejarla libre para el siguiente proyecto. Sony devuelve «acción en cámara pendiente» hasta validar un procedimiento compatible con sus índices.

Normalmente: MEDIA + ESSD para video, MEDIA + SSD1 para audio. Si MEDIA está y falta el SSD, se puede confirmar sólo archivo por operación y dejar trabajo pendiente u omitido. Después se copia desde MEDIA, sin necesitar la SD original. Sin destinos registrados online, insertar SD no provoca popup ni escaneo profundo automático.

Proyecto, perfil, país y ciudad son campos distintos. SDs mixtas admiten asignación por unidad; pares y bundles se preservan. DJI requiere categoría explícita cuando es ambiguo. Layout (plan esquema 2, [layout.py](media/layout.py)): nombres históricos `26OCT24-194650_SONYA6400_08-24_MX-PROYECTO[].MP4` desde el XML Sony/ffprobe/mtime. MEDIA por tipo/año/mes (`videos/`, `v4k/`, `recordings/`, `images/`, `stalker/`; XML en `metadata/sony/`, índices y miniaturas en `metadata/sony/support/`). Trabajo plano en `e64_raw/<proyecto>/` sólo con clips y bundles Stalker.

## Empezar

Python 3.9+. El motor de copia usa biblioteca estándar; las utilidades de video/download/stems pueden necesitar programas adicionales.

```sh
python3 -m venv .venv
source .venv/bin/activate
python -m pip install -e .
nu9ve media --help
```

Registrar discos elegidos, revisar y ejecutar un plan: [guía operativa](docs/operations.md). Ejemplos:

```sh
nu9ve media volumes register MEDIA502 /Volumes/MEDIA502 --role archive --prefix 001_raw
nu9ve media volumes register ESSD /Volumes/ESSD --role work --prefix 001_proyectos
nu9ve save /Volumes/TARJETA --archive MEDIA502 --work ESSD --project intro --dry-run
nu9ve save /Volumes/TARJETA --archive MEDIA502 --work ESSD --project intro
```

`save` usa planificación verificada y admite el atajo `save all mx PROYECTO`. Para sólo respaldo, sin copia de trabajo ni pendiente: añadir `--no-work` y confirmar la operación; [ejemplos](docs/operations.md#sólo-respaldo-sin-archivos-de-trabajo). El importador anterior quedó como **`legacy-save`**, conserva sus movimientos/borrados y no tiene las garantías nuevas. `nu9ve clip` sólo informa el editor sucesor; el módulo histórico fue retirado. No hay migración automática de JSON antiguos de Clipper.

## Superficies disponibles

| Área | Entrada / alcance |
| --- | --- |
| Observación | `inspect DIRECTORIO`, sin transferencias; `doctor` sigue describiendo configuración/herramientas del legado. |
| Ingreso seguro | `save` humano; `media plan/apply`, `jobs`, `resume` y JSON RPC. |
| Biblioteca | `media library/resolve/restore/handoff`, pendientes y entrega al SSD, recuperación desde recibos. |
| Publicación | Estado local por variante/canal; no sube archivos ni automatiza todavía `subidos`. |
| App macOS | Menubar, ingreso/asignación/preview, jobs, biblioteca, discos/runtime y observación configurada de Stalker. |
| Agentes | `media mcp`: servidor stdio de sólo lectura, sin puerto HTTP ni herramientas destructivas. |
| Otros brazos | `download`, `split`, prototipos hardware/Bash conservados. Aún no comparten todo el nuevo ciclo de assets. |

Stalker se archiva completo y finalizado; no se limpia su única biblioteca local automáticamente. La relocalización integral de Clipper y las integraciones Atlas/Final Cut siguen pendientes de adopción en sus repos.

## Desarrollo y distribución

```sh
python3 -m unittest discover -s tests -v
swift test --package-path native/MediaMenu
```

[Desarrollo y handoff](docs/development.md) · [Build/debug/instalación](docs/operations.md) · [Gates de release](docs/release.md).

Las pruebas automatizadas nunca usan MEDIA/ESSD/SDs personales. Los bundles de `dist/` son snapshots ad-hoc de desarrollo, no un instalador final. CI: [.gitlab-ci.yml](.gitlab-ci.yml). Revisar `git status` antes de continuar; el estado exacto está en [STATUS](docs/STATUS.md).

La app macOS se presenta como **nüün**, instalada como `nuun.app`. Usa una ventana con modos compacto/selección/completo, carga automática y miniaturas bajo demanda, bandeja, perfiles y ajustes por áreas. [Flujo y límites](docs/features/nuun-window.md), [instalación](docs/operations.md#app-de-desarrollo-y-launcher), [evidencia](docs/STATUS.md).
