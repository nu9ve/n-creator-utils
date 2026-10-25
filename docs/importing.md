# Importador histórico: Importación de media (`save`)

Esta página describe `manager/file.py`, ahora accesible como **`legacy-save`**. Las menciones antiguas a `save` en ejemplos/secciones se refieren a ese motor. Para el nuevo flujo ver [operations](operations.md) y [STATUS](STATUS.md).

## Alcance frente a Stalker y Clipper

El [diseño de media](media-architecture.md) define copiar, verificar destinos y después limpiar la SD en el mismo flujo; el motor nuevo ya existe con los límites de STATUS; este documento describe el importador histórico, que todavía borra originales y soporte Sony sin esas garantías. [Prueba Sony](sony-validation.md).

Este importador organiza archivos de cámaras/grabadoras; no entiende sesiones `.stalker`, proyectos `clipper-project.json` ni paquetes de entrega del editor. Ahora se rechazan fuentes dentro de paquetes Stalker/Clipper o que los contengan, antes de modificar esa fuente; también links/entradas especiales. Todavía no hay adaptador de copia para ellos. Paquetes de entrega, Final Cut y Atlas tampoco deben pasarse a este importador genérico. Además, mover fuentes ya importadas puede romper referencias de Clipper. El contrato y los flujos que necesitamos están documentados en [file-workflows.md](file-workflows.md).

## Contrato actual

`save` organiza un árbol de archivos por categoría, año y mes. Su comportamiento histórico es **mover**, no hacer una copia de sólo lectura. En una operación normal copia con metadata, ajusta fechas, compara tamaños y elimina la fuente. Audio recibe primero una copia secundaria. El flujo genérico también elimina directamente archivos `hprj`, `bmp`, `gyro` y `aae`; estas eliminaciones no tienen copia previa. El bundle Insta360 evita ese clasificador y conserva sus archivos.

Antes de recorrer se exige que existan la fuente y la raíz principal; se rechaza solapamiento entre fuente y raíces de archivo/respaldo, incluyendo rutas resueltas a través de enlaces simbólicos. La raíz secundaria se comprueba al llegar a un archivo de audio. Las subcarpetas de destino se crean según se necesitan. Esto evita crear automáticamente la raíz de un destino ausente, pero no comprueba identidad de volumen, espacio libre ni que el disco esté realmente montado.

No hay transacción de toda la tarjeta. Un fallo puede dejar archivos ya movidos, copias parciales y otros aún pendientes. Una comparación por tamaño no detecta corrupción de igual longitud. No hay checksum, rollback, reanudación idempotente, bloqueo entre procesos ni manifiesto general. Ejecuta una sola importación por destino y valida con una copia temporal antes de usar una tarjeta real.

## Fuentes y detección

```sh
nu9ve save "/ruta/a/tarjeta" mx hermosillo AUTO
nu9ve save "/ruta/a/tarjeta" mx hermosillo SONYA6400
nu9ve save all mx hermosillo
nu9ve save hijack mx hermosillo
```

- `all`: nombres configurados en `sd_src_name` bajo `sd_root_dir`. La detección por estructura puede sustituir el perfil asignado a un volumen.
- Directorio explícito: usa la etiqueta solicitada; detecta el perfil sólo con `AUTO`. Algunos clips Sony/GoPro además ajustan su etiqueta durante el recorrido.
- `hijack`: usa `audio_hijack_dir` y clasifica audio en categorías Hijack.
- `AUTO`: detecta Sony primero, luego Insta360. Si no reconoce la fuente, la omite.

Sony se detecta por `PRIVATE/M4ROOT/CLIP`, `THMBNL`, `PRIVATE/AVCHD/BDMV/STREAM` o `AVF_INFO`. Insta360 por `DCIM/fileinfo_list.list` o presencia de `.insv`, `.lrv`, `.thm`, `.insp` en el árbol. No hay detección equivalente completa para cada modelo Canon, Nikon, DJI o GoPro; sus etiquetas y reglas parciales no deben confundirse con perfiles intercambiables.

Se omiten `.fseventsd`, `.Spotlight-V100`, `.Trashes` y `System Volume Information`. Otras carpetas ocultas no se omiten automáticamente. Los tags de origen/país/ciudad no admiten separadores de ruta ni valores vacíos. El naming genérico conserva otros caracteres; su normalización multiplataforma está pendiente.

## Categorías y rutas

Todas las rutas son relativas a `root_dir`, salvo la copia secundaria de audio. El nombre histórico de la carpeta no garantiza una resolución exacta.

| Archivos/regla | Destino |
| --- | --- |
| JPG, JPEG, PNG, DNG, HEIC, GIF | `images/YYYY/MM` |
| CR2, CR3, NEF, ARW | `rimages/YYYY/MM` |
| Imagen cuya ruta contiene `screenshot` o `screen shot` | `screens/YYYY/MM`, con prioridad sobre RAW |
| MOV, MP4, MPG, MTS | `videos/YYYY/MM` |
| Video con ancho **o** alto mayor de 2000 px | `v4k/YYYY/MM` |
| Video cuya ruta contiene `screenrecording` o `screen recording` | `screens/recordings/YYYY/MM`, después de la regla de resolución |
| Video cuya ruta contiene `mavic2` | `vdron/YYYY/MM`, con prioridad sobre las reglas anteriores |
| Audio WAV/MP3 de hasta 600 segundos | `recordings/YYYY/MM` y segunda copia en `root_recordings_dir/recordings/YYYY/MM` |
| Audio de más de 600 segundos | `recordingslong/YYYY/MM` y segunda copia homóloga |
| Audio de `hijack` | `hijack/YYYY/MM` o `hijacklong/YYYY/MM`, más segunda copia homóloga |
| DAT | `corrupt/YYYY` |
| INSV/LRV/THM fuera del perfil bundle | `360/videos/YYYY/MM` |
| INSP fuera del perfil bundle | `360/images/YYYY/MM` |

Las extensiones se reconocen sin distinguir mayúsculas/minúsculas. `.dat` se clasifica por extensión: no hay diagnóstico real de corrupción. El enrutamiento de drone depende del texto de la **ruta**, no sólo de la etiqueta `MAVIC2`. DNG actualmente sigue la ruta de imágenes normales. Los formatos no reconocidos permanecen en la fuente; no hay conversión/transcodificación durante `save`.

## Nombres y fechas

Formato general para imágenes:

```text
YYMONDD-HHMMSS_SOURCE_COUNTRY-CITY[].EXT
```

En video se añade duración `MM-SS` después de la fuente. Audio conserva la convención `SOURCE` seguida del identificador de micrófono y duración. Las colisiones reciben `-02`, `-03`, etc., antes de la extensión; eso conserva ambos archivos pero no deduplica por contenido.

Prioridad de fecha para video:

1. `CreationDate` del XML Sony `C0001M01.XML`, si existe y se puede leer.
2. `creation_time` del contenedor leído con `ffprobe`; si tiene zona, se convierte a la zona local de la máquina.
3. `mtime` del archivo.

Imágenes y audio usan `mtime`; no se extrae EXIF de las fotos. El XML Sony conserva su hora/zona como viene de cámara, por lo que sus reglas no equivalen a la conversión del contenedor. El naming usa abreviaturas de mes basadas parcialmente en locale y aún no hay política explícita de zona horaria.

GoPro tiene una corrección histórica fija si reconoce ciertos prefijos y año anterior a 2018: suma desplazamientos de día/mes/minutos/año definidos en `update_file_time()`. No es una corrección genérica ni contempla todos los calendarios; está pendiente convertirla en configuración por perfil.

`shutil.copy2` conserva atributos cuando el filesystem lo permite. `set_file_dates` utiliza metadata de creación de video si existe, o fechas de filesystem; en macOS intenta establecer creation date con `SetFile` si está disponible. No se garantiza preservar todos los atributos extendidos, permisos o fechas entre filesystems distintos.

## Sony a6400 / XAVC S / AVCHD

```text
TARJETA/
  DCIM/100MSDCF/
  PRIVATE/M4ROOT/CLIP/C0001.MP4
  PRIVATE/M4ROOT/CLIP/C0001M01.XML
  PRIVATE/M4ROOT/THMBNL/
  PRIVATE/AVCHD/BDMV/STREAM/
```

- Los MP4, fotos JPG/ARW y MTS se mueven sin convertirlos.
- Los sidecars `C0001M01.XML` se copian a `metadata/sony/YYYY/MM` con el nombre base del clip renombrado y se elimina su original después de copiar.
- El XML aporta fecha, codec, fps, resolución y modelo cuando están presentes; no se añade resolución al nombre.
- Después de mover media, se archivan archivos de soporte de Sony bajo `metadata/sony/support/YYYY/MM/SESION/`, preservando rutas internas y agregando un manifiesto.
- Las rutas de soporte incluyen `AVF_INFO`, carpetas `THMBNL`, `SUB`, `GENERAL`, `MEDIAPRO.XML`, `STATUS.BIN` y los índices/listas `INDEX.BDM`, `MOVIEOBJ.BDM`, `PLAYLIST`, `CLIPINF` de AVCHD. La lista exacta está en `SONY_SUPPORT_PATHS`.
- Las carpetas base quedan en la tarjeta; por ejemplo `PRIVATE/SONY/SONYCARD.IND` no está incluido en el archivo de soporte.

La cámara puede mostrar `Image Database File error` porque la computadora movió media/índices. Tras verificar el archivo, reconstruye la base de datos o formatea desde la cámara para limpiar la tarjeta. **Hoy no hay opción copy-only.** Una nota anterior de `PLAN.md` decía lo contrario; describía una intención, no la implementación.

Los movimientos de video, XML y soporte no son atómicos entre sí. Si falla un XML después de mover el video, el video puede estar ya en el destino y el XML en la tarjeta. Verifica ambas ubicaciones antes de reintentar.

## Insta360: bundle rehidratable

Con perfil `INSTA360` se conserva la estructura original completa, nombres incluidos:

```text
root_dir/insta360/YYYY/MM/DDMM-HHMMSS_COUNTRY-CITY_INSTA360/
  DCIM/...
  MISC/...
  manifest.json
```

La fecha de sesión usa el `mtime` más antiguo de los archivos de extensiones Insta360. El recorrido mueve **todos** los archivos de la fuente salvo carpetas de sistema ignoradas; usa una raíz dedicada a la tarjeta. Si no encuentra media Insta360, no crea un bundle. El manifiesto registra fuente, ruta relativa, destino, tamaño, extensión y fechas de importación/media; no incluye checksum.

Si ya había un `manifest.json` entre los archivos originales, se conserva y el manifiesto de importación recibe `manifest-02.json` o el siguiente nombre disponible. La generación del manifiesto ocurre al final: un fallo intermedio puede dejar un bundle incompleto sin registro final.

Los `.INSV` contienen el video; `.LRV` son proxies y `.THM` thumbnails. Mantener nombres y carpetas ayuda a conservar relaciones. Para reconstruir una tarjeta, copia las carpetas internas como `DCIM` y `MISC` a su raíz, no el contenedor de sesión. La aceptación por una app/cámara concreta debe comprobarse; conservar la estructura no garantiza esa compatibilidad.
