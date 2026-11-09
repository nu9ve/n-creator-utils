# Estado vigente

Actualizado: 2026-11-08, America/Hermosillo. **No es una release de producción.** Esta página dice qué funciona hoy, con qué evidencia y qué falta; la bitácora por entrega hasta el 8 nov está en [releases/status-log-2026-10-25_2026-11-08](releases/status-log-2026-10-25_2026-11-08.md). Prioridades en [PLAN](../PLAN.md), gates de distribución en [release](release.md).

Regla para mantenerla: al cerrar una entrega se **actualiza la fila de su área** (estado, evidencia, pendiente) y se agrega, como mucho, una entrada corta en «Entregas recientes». Cuando esa sección pase de ~5 entradas, las más viejas se mueven a la bitácora de `releases/`.

## Build y pruebas

| Qué | Valor |
| --- | --- |
| App instalada | `~/Applications/nuun.app`, build **`a350af7f16af`** (8 nov; anterior `555fe99c6c8d` en `app-backups/e86ce22c.app`). Firma ad-hoc, Python externo. Backups en `~/Library/Application Support/n-creator-utils/app-backups/` |
| Suites | Python **362 OK**; Swift **25/25**; localización 858 claves sin faltantes ni sobrantes (8 nov) |
| Instalar | `scripts/install_media_app.py`; se niega si la app está abierta (correcto: no se cierra por el usuario) |

## Por área

| Área | Estado | Evidencia física | Pendiente |
| --- | --- | --- | --- |
| **Ingreso de cámara** (Entrada → Flujo → Resultado) | En uso | Varios ingresos reales a MEDIA502 + ESSD desde la app | Popup al insertar: detección/latencia incierta. Desconexión y reanudación a mitad de copia sin probar en físico |
| **Sony α6400** | En uso. Retira clips verificados y conserva índices; al reconectar, nüün observa el formato y cierra el trabajo solo | 7 nov: cuatro trabajos cerrados solos al reconectar; 8 nov: Baches (`060b585d`, el `FileNotFoundError` del 6 nov) cerrado por observación. La fase `checking_card` reintenta la lectura que fallaba | Sólo confirmar en el próximo ingreso normal que el retiro sobrevive al remontar sin formatear. Nada que diagnosticar ahora |
| **Stalker** | En uso: bandeja, bundles completos, liberación local con relink/ACK de Clipper | Ingresos reales completos | Casos `recording`/`finalizing`/`recovery` reales; [pedidos](contracts/README.md#pendientes-para-n-creator-utils) |
| **Dron / Atlas** | En uso de punta a punta: dispositivo por EXIF, registro de misiones, misión o no, trabajo JPG sin duplicados, descartes, `units.ndjson`, A11, A14, DNG a pedido | 8 nov: SD `MAVIC200` respaldada, verificada y vaciada (job `cc8bd128`); ESSD mayo–junio respaldado (`77bdcb73`); DNG `dng-d9549817` | Formatear la SD desde el dron. Reclasificar `pending_export` → misión cuando el export llega tarde. Confirmar un dispositivo nuevo desde la app. UI del plan de DNG. Atlas: leer `_missions/` y `units.ndjson`. [Contrato](contracts/atlas-drone-v1.md#estado-del-lado-nüün--2026-11-08) |
| **Cierre de proyecto** | En uso: finales, publicados, regenerables de Clipper por su `cleanup-plan`, transcripciones a MEDIA | 8 nov: cierre conjunto de Terra aplicado (`8fe6e578`) | Seis verticales de Terra por publicar; aviso opcional por verticales viejos. [Contrato](contracts/project-closeout-v1.md) |
| **Biblioteca, Discos, cobertura** | Implementado en sólo lectura + recuperación/entrega desde archivo | Entrega desde MEDIA502 a ESSD (6 nov) | `coverage --no-hash` ESSD contra MEDIA502; restaurar sin SD; rehash periódico |
| **Carpetas vivas** (música, samples, `0000_acomodar`) | Sin implementar | — | Diseño en [product-flows](features/product-flows.md#carpetas-vivas-música-y-samples); siguiente entrega candidata |
| **Fotos / Kyū, H6, GoPro** | Detecta y planea con fixtures | Ninguna física | Aceptación por dispositivo |
| **Recetas y Flujos** | La receta `nu9ve-default` describe familias de tarjeta (Stalker, Sony, H6, GoPro, Insta360, DJI, Canon) con su recomendación, tags de modelo, dispositivos por EXIF, categorías y flujos de misión. RPC/CLI `recipe` de sólo lectura; Flujos muestra «Lo que reconoce» desde ahí | Lectura del catálogo real con el motor instalado | Modelo por serial en vez de un tag por familia; editar la receta desde la app; generar los bloques de cada flujo desde la receta |
| **Distribución** | Sólo instalación local de desarrollo | — | Runtime empaquetado, Developer ID, notarización. [Gates](release.md) |

## Límites conocidos (vigentes)

1. El digest del plan no está autenticado: `apply` confía en verificaciones hechas al planear. Los jobs no tienen owner/lease, así que uno matado parece activo.
2. Los snapshots SQLite guardan decisiones humanas, pero no hay retención automática ni restauración completa desde la app. La recuperación por recibos no es un respaldo completo del catálogo.
3. Clipper: relink/ACK adoptados por CLI/TUI; faltan leases de consumidor y aceptación cruzada amplia. Final Cut queda fuera.
4. Sin firma Developer ID ni runtime portable. Windows/Linux no validados.
5. CI (`.gitlab-ci.yml`) existe pero no se ha ejecutado en GitLab.

Esta página no autoriza operaciones sobre media real. Las pruebas automáticas usan fixtures temporales; cada corrida física es un plan revisado que el usuario pide.

## Entregas recientes

### Detección de tarjetas como datos de receta · 2026-11-08

Build `a350af7f16af`, instalada con la app cerrada; firma estricta, `media/` idéntico al checkout y `recipe` responde con el motor instalado. Las firmas de tarjeta y sus recomendaciones salen de `cards` en la receta, con un lenguaje de condiciones cerrado (`top`, `top_prefix`, `top_suffix`, `in`, `dcim_prefix`, `dcim_suffix`, `dcim_regex`); una condición desconocida se rechaza (`invalid_recipe`). Los tags de modelo (`ILCE-6400` → `SONYA6400`) salen de `models`/`family_source`. Una receta guardada sin estas secciones hereda las de la plantilla. Las guardas de seguridad (índices Sony, limpieza sólo tras verificar) siguen en código: la detección Sony del planner se unificó en `layout.sony_card_path`. Flujos: «Dron y misiones» describe el flujo real y la sección «Lo que reconoce» lee la receta. Pruebas nuevas: `tests/test_recipe_cards.py` (6), incluida una cámara nueva agregada sólo como dato. Mismos resultados en todas las pruebas previas de detección y naming.

### Tarjeta para ver, respaldo desde el SSD, filtros y A14 · 2026-11-08

Build `555fe99c6c8d`. Tocar una tarjeta sólo muestra su contenido y «Preparar respaldo» abre el Flujo. Con la fuente dentro de un disco de trabajo, la app elige «sólo respaldo» (`source_on_work_disk`). Hay filtros de Fecha y Sesión. Con limpieza, un plan con fotos `pending_export` se rechaza (`mission_export_missing`). Las validaciones ya no releen EXIF (`photos=False`). Falta la aceptación desde la app.

### DNG del ESSD (A16) y A11 · 2026-11-08

Muestra de 128 DNG en trabajo; 999 liberados tras rehash contra MEDIA502. Mayo–junio del ESSD respaldado en MEDIA502 (534 pares). Detalle en la [bitácora](releases/status-log-2026-10-25_2026-11-08.md#dng-del-essd-muestra-conservada-y-el-resto-a-la-papelera-a16--2026-11-08).
