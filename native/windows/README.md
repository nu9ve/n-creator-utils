# nüün para Windows (maqueta)

Cliente WinUI 3 de nüün y la primera base reutilizable de la suite para Windows. **Es una maqueta:** muestra el shell, los cinco skins y las pantallas principales con datos ficticios. No lee ni escribe discos, porque el motor (`media/service.py`) todavía no tiene adaptador de volúmenes para Windows. Reglas: [windows-app-v1](../../docs/contracts/windows-app-v1.md) · guía y aprendizajes: [Windows en la suite](../../docs/platforms/windows.md) · estado: [STATUS](../../docs/STATUS.md).

| Proyecto | Contenido | Depende de |
| --- | --- | --- |
| `Nu9ve.Core` | Base de la suite sin UI: `ThemeRoles` (port de `ThemeColor.swift`), skins y sus medidas, `SurfaceChrome`, preferencias en el formato común, reglas de atajos, encaje de ventana, treemap, argumentos y formas de instalador | — |
| `Nu9ve.WinUI` | Base WinUI de la suite: `ThemeService`, `NuSurface`/`NuButton`/`NuToggle`, `SkinCaption`, `WindowChrome`, `Storage`, constructores `UI.*` | `Nu9ve.Core`, Windows App SDK |
| `Nuun.Core` | Lo propio de nüün: transporte JSON con `media rpc-serve`, DTOs, maqueta (`Fixtures/*.json` con la forma real de `snapshot` y `recipe`), catálogo de atajos, modos de ventana | `Nu9ve.Core` |
| `Nuun` | La ventana: Inicio, Ingreso (Entrada → Flujo → Resultado), Flujos, Biblioteca, Ajustes, Acerca de | todo lo anterior |
| `Nuun.Setup` | `NuunSetup.exe`: instalador con identidad (forma de SD, skins, `PackageManager`, `--quiet`) | `Nu9ve.WinUI` |
| `Nuun.Core.Tests` | 58 pruebas: matriz de contraste, contraste por superficie, vectores compartidos, preferencias, atajos, ventana, transporte, maqueta, instalador, contrato con el motor de Rust | `Nuun.Core` |
| `Package` | Plantilla del manifiesto MSIX (`app.nu9ve.media`, la misma identidad que en Mac) | — |

Mismo stack y versiones que nove (`.NET 10`, Windows App SDK 1.8.260921001, xUnit 2.9.3); no se añadió ningún paquete nuevo.

## Compilar, probar y abrir

```powershell
dotnet test native\windows\Nuun.Core.Tests
dotnet build native\windows\Nuun -c Debug
$env:NUUN_QA_ROOT = "$env:TEMP\nuun-qa\lib"; native\windows\Nuun\bin\Debug\net10.0-windows10.0.22000.0\win-x64\Nuun.exe
```

Con `NUUN_QA_ROOT` las preferencias de la ventana van bajo esa raíz y cada ejecución es una instancia aparte. Sin ella, Debug guarda en `%LOCALAPPDATA%\nuun\Desarrollo`. Argumentos: `--page ingest|flows|library|settings|about`, `--skin glass|classicMac|win9x|terminal|gothic` (sólo con raíz de QA), `--compact` y `--maqueta` (datos ficticios aunque el motor de Rust esté junto a la app; sin ella, la ventana muestra los datos reales de este PC y no simula respaldos).

Motor real (sólo lectura de `snapshot` y `recipe`, si hay Python): `NUUN_PYTHON` y `NUUN_ENGINE` (ruta a `nu9ve.py`). Planear y aplicar responden `not_on_windows`. Motor de Rust: si `engine\target\debug\nuun-engine.exe` existe al compilar, se copia junto a la app y responde lo ya portado (hoy `recipe`); el resto sigue en maqueta. Ver [engine](../../engine/README.md).

Regenerar los vectores de tema compartidos, a propósito: `$env:NU9VE_WRITE_THEME_VECTORS = "1"; dotnet test native\windows\Nuun.Core.Tests`.

## QA visual

```powershell
powershell -File scripts\windows\qa-skins.ps1 -Out "$env:TEMP\nuun-qa\shots"
powershell -File scripts\windows\qa-skins.ps1 -Skins win9x -Pages ingest -Compact
```

Cada skin × página se captura en una raíz temporal que se borra al terminar. `scripts\windows\ui-invoke.ps1 -ProcessId <pid> -List` recorre el árbol de UI Automation (el de Narrador) y `-Name "Revisar plan"` invoca un control. Las capturas sirven para revisar a mano: no reemplazan las del README ([capturas del producto](../../docs/development.md#capturas-del-producto)).

## Paquete y Setup

```powershell
powershell -File scripts\windows\package-msix.ps1 -NoSign
native\windows\Nuun.Setup\bin\Debug\net10.0-windows10.0.22000.0\win-x64\NuunSetup.exe
```

`-NoSign` publica, arma el MSIX y valida el manifiesto con `makeappx`, sin tocar certificados. `-Sign` crea, si falta, el certificado de desarrollo en `CurrentUser\My` y firma. Confiar en ese certificado requiere un paso aparte como administrador, que no está automatizado aquí. Sin `--package`, el Setup es una demostración rotulada: `--package dist\windows\nuun-<v>-x64.msix` instala de verdad y `--quiet` lo hace sin ventana (códigos 0/1/2/3).

Instala tú, desde el Explorador o tu terminal: una app agente empaquetada como MSIX virtualiza lo que sus procesos hijos escriben en `AppData` (lección de curado).

## Límites

Sin motor en Windows, sin firma ni instalación probada, sin recordar el marco por modo. `TextBox` y `ComboBox` conservan el estilo del sistema dentro de los skins de época. Las fuentes OFL de los skins siguen pendientes, igual que los sonidos de tema, los badges con capa Ctrl/Shift y el inglés. Alto contraste, escalas 125/150/200 % y Narrador escuchado no se revisaron a mano.
