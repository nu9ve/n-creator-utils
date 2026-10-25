# Desarrollo y verificación

El clipper local quedó reemplazado por [nu9ve/clipper](https://github.com/nu9ve/clipper). Las pruebas e instrucciones de renderer en esta página describen el código histórico aún presente; no son una prioridad para nuevas funciones aquí. La evolución entre repos se centra en [archivos, paquetes y transferencias](file-workflows.md).

## Preparar el entorno

```sh
python3 -m venv .venv
source .venv/bin/activate
python -m pip install --upgrade pip
python -m pip install -e .
python3 -m unittest discover -s tests -v
python3 -m compileall -q nu9ve.py constants.py configuration.py manager clipper hardware utils
```

La suite usa `unittest` y no necesita pytest ni paquetes Python opcionales. Las pruebas de media real se omiten si faltan FFmpeg/FFprobe; la prueba de texto se omite si FFmpeg no tiene `drawtext`. Lee el reporte de skips para saber qué se verificó realmente. Ejecuta desde la raíz del repo.

No ejecutes `hardware/test.py`, `clipper/openai_clipper.py` ni scripts de `playground/` como parte de pruebas automáticas: son experimentos manuales, pueden usar equipos/cuentas reales y no equivalen a la suite de regresión.

## Qué cubren las pruebas

| Suite | Casos |
| --- | --- |
| `test_configuration.py` | Defaults, home correcto, JSON parcial, precedencia, rutas relativas, no crear destinos, errores de esquema y mapa de volúmenes. |
| `test_import.py` | Naming/colisiones de fotos, fechas, bytes, origen conservado ante fallo/copia truncada, raíz ausente, solapamientos, bundle Insta360 con manifest existente, Sony y archivos de soporte, WAV minúsculo y segunda copia de audio. |
| `test_clipper.py` | Configuración mínima, selección explícita de JSON, recursos relativos, listas de inputs independientes, rangos fraccionarios/largos/inválidos, cadena de textos y detener fallos FFmpeg. |
| `test_commands.py` | Ayuda/diagnóstico sin site-packages, argumentos inválidos, códigos de error, URL como argumento, destinos con espacios, proveedores no implementados, omisión de tokens en errores y ciclo de vida del original para split. |
| `test_media_integration.py` | Video/audio generado localmente con FFmpeg, dos rangos desde fuente con espacios/extensión mayúscula, fuente intacta, previews, texto múltiple real y render completo horizontal/vertical con formatos adicionales. |

La revisión validó los 26 tests en Python 3.9.6 y 3.12.12 en macOS, incluidos los tres tests locales de media sin skips. Todas las escrituras de las pruebas van a directorios temporales. Las rutas de archivo/respaldo del importador se sustituyen por fixtures. Las descargas, Demucs y servicios externos se simulan. El test Sony usa un clip de bytes ficticios y mocks de FFprobe; valida el movimiento/metadata, no decodificación de una cámara real.

El empaquetado normal y editable se construyó con setuptools en Python 3.12 y se instaló sin dependencias en entornos temporales Python 3.9. Se verificaron `nu9ve --help`, `nu9ve clip --help` y `nu9ve doctor` desde fuera del checkout. Esto comprueba el entry point, no sustituye la suite de media ni una matriz de plataformas.

## Cambios sobre importación

Mantén fixtures que representen tanto el archivo principal como sus sidecars, índices y carpetas ignoradas. Antes de probar en hardware:

1. Copia una muestra pequeña de una tarjeta a un directorio temporal y configura destinos de prueba ya existentes.
2. Registra qué nombres, fechas, archivos auxiliares y rutas deberían resultar.
3. Ejecuta el flujo sobre la copia y compara fuente/destinos, incluidos archivos no reconocidos y segunda copia de audio.
4. Para una prueba real de cámara, valida que puede reutilizar la tarjeta y registra el modelo/firmware/filesystem. No generalices desde una fixture a toda la familia de dispositivos.

No existe una garantía actual de checksum, atomicidad o tolerancia a ejecuciones simultáneas. Las pruebas de tamaño no deben describirse como validación de integridad completa.

## Cambios sobre clipper e integraciones

Para filtros, usa media sintética pequeña antes de un video largo. Valida forma, duración y apariencia de la salida. Nuevas combinaciones de logo, fondos, orientación, texto especial y formatos necesitan sus propios casos; no basta con comprobar que se construyó una cadena de argumentos.

Para descargas/ML, separa pruebas simuladas y pruebas optativas con acceso externo. Registra versiones de ejecutables/SDKs, proveedor, formato de salida y comportamiento de errores. Evita credenciales en fixtures, ejemplos, logs y argumentos registrados.

## Compatibilidad de cambios

- Conserva comandos y nombres de carpetas existentes o documenta una migración explícita.
- No elimines scripts “antiguos” basándote sólo en no estar importados: pueden usarse manualmente.
- Mantén defaults existentes; agrega nuevas reglas peligrosas como opciones explícitas y verificables.
- Convierte decisiones de usuario/ambiente en configuración validada, evitando más flags o rutas personales dispersas.
- Al separar módulos, desacopla dominio, filesystem y proveedores con pruebas de comportamiento antes de cambiar salidas.

El inventario y los contratos actuales están en [módulos](modules.md); los trabajos pendientes en [PLAN.md](../PLAN.md).
