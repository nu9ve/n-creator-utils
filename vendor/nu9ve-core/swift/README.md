# Swift

Paquete de Swift (`Package.swift`, paquete `nu9ve-swift`), para las apps de Mac. Una app lo usa por su copia fijada: declara `swift` en `vendor/nu9ve-core.paths`, sincroniza y apunta su proyecto de Xcode (o su `Package.swift`) a `vendor/nu9ve-core/swift` como paquete local.

| Pieza | Qué hace | De dónde | Estado |
| --- | --- | --- | --- |
| `NuMacShell` | `ShellWindow` y `ShellWindowHost` (ventana propia, esquinas cuadradas, bordes de arrastre, zoom que llena la pantalla), `WindowChromeBridge` (barra junto a los botones de ventana, pantalla completa), `TitleBarDragArea` y doble clic según el sistema, `PeriodWindowBar` (barra de época de los tres comunes: franja, × − +, fila, biseles y caja de agarre), `AboutView`, `AppIdentity` con el sello `Nu9veBuildID`/`Nu9veBuildDate`, `HapticPlayer` y `PressureMonitor` | Fractos (`ios-fractos/Packages/FractosKit`, mudado el 2026-11-16); la barra de época, de SoundBender, Kyū y nüün (2026-11-18) | Aquí. Consumidores: Fractos, Kyū y SoundBender; la barra de época, ninguno todavía |
| `Nu9veTheme` | Color sRGB, `readable`, 15 roles, el tipo `ThemePreset` y las medidas de los skins del catálogo, con `ThemeSkin.commons` (Macintosh, Windows 9x y Terminal) y `ThemeSkinPart`. Prueba los 110 casos de `data/theme-vectors-v1.json`. No trae colores de ninguna app: cada una declara su color por omisión y sus presets | nüün (`ThemeColor.swift`) | Aquí. Consumidores: nüün, Kyū y SoundBender |
| `Nu9veEngine` | Cliente de [`nu9ve-engine-v1`](../contracts/nu9ve-engine-v1.md) por líneas: lanza el motor, handshake con `contract` y `protocol`, peticiones concurrentes emparejadas por `id`, plazos, lectura acotada, reinicio con espera creciente, `not_ported` como resultado y leases de sesión ([§6.4](../contracts/nu9ve-engine-v1.md#64-leases-de-sesión)). Su lector de JSON guarda los números como texto. Sólo Foundation y Synchronization | nüün (`LineServer` de `Protocol.swift`), 2026-11-22 | Aquí. Consumidores: ninguno todavía; Neuf, el primero previsto ([abajo](#nu9veengine-cliente-de-nu9ve-engine-v1)) |
| Motor de sonido de interfaz | | Fractos (`FractosSoundV2`) | Según el contrato de sonido |

## macOS mínimo: 15

`Package.swift` pide macOS 15. Es lo más bajo que permiten las APIs del shell, comprobado compilando con un mínimo menor: con macOS 13 o 14 sólo fallan `NSCursor.frameResize(position:directions:)` y `NSCursor.FrameResizePosition`, los cursores de borde y esquina de `ShellWindowResizeOverlay`, que llegaron en macOS 15. Lo demás pide macOS 13 a lo sumo (`URL.host()`, `Task.sleep(for:)`, `NSHostingView.sizingOptions`).

- **Fractos** pide macOS 26 y lo usa sin cambios.
- **nüün, Kyū y SoundBender** subieron su mínimo de macOS 14 a 15 al adoptar `Nu9veTheme`.

## Reglas

- **La identidad es de cada app** ([tema §5 y §7](../contracts/nu9ve-theme-v1.md#juego-de-skins-por-app)). `Nu9veTheme` da lo común: roles, contraste, los tres comunes y los skins del catálogo con sus medidas. El **original** y el **contraste** de cada app, su color por omisión y sus presets los declara la app. Si su original no está en el catálogo, la app le da sus `ThemeSurfaces` y usa `ThemeRoles(dark:base:surfaces:)`, que aplica las mismas reglas de contraste. Así una app que adopta el paquete se sigue viendo igual por omisión, y suma los comunes.
- **No conoce ninguna app.** `scripts/check-boundary.sh` lo comprueba: el código de `swift/` sólo importa frameworks de Apple y sus propios targets, y ningún archivo de código nombra una app de la suite. Lo que es de una app llega como parámetro: `ShellWindowHost(frameAutosavePrefix:)` recibe el nombre con que la app guarda sus ventanas (Fractos pasa `fractos.window`, el de siempre, para no perder los marcos guardados), `AppIdentity` recibe nombre, lema y enlaces, y `AboutView` el logo y los colores.
- **Plataformas.** El código AppKit/SwiftUI de `NuMacShell` va dentro de `#if os(macOS)`. `Nu9veTheme` contiene datos y funciones de color, con Foundation como única dependencia. `NuMacShell` depende de `Nu9veTheme` para los comunes y sus medidas.
- **`.task` en la raíz de una ventana.** Cambiar la barra de título (`usesSquareCorners`, `WindowChromeBridge`) o entrar en pantalla completa hace que AppKit reconstruya la vista de marco: la raíz aparece otra vez y sus `.task` y `.onAppear` vuelven a correr, cancelando los que corrían. Lo que se hace una vez por arranque va fuera de la vista, con una bandera.
- **Gate:** `scripts/check-swift.sh` corre la frontera, `swift test` y un build de Release con los avisos como errores (`NU9VE_STRICT_WARNINGS=1`). No puede ser siempre: Xcode compila los paquetes locales con `-suppress-warnings`, y el compilador rechaza las dos opciones juntas. El CI lo corre en macOS 26 (Swift 6.2).
- **Las apps no corren `swift test` dentro de `vendor/nu9ve-core/swift`:** dejaría `.build/` ahí y `sync-core.sh --check` lo rechaza por archivos fuera del manifiesto. Las pruebas del shell corren aquí; la app prueba que lo usa bien.
- El nombre `NuMacShell` viene de Fractos y se mantiene para no romper los `import`. La regla del prefijo pediría `Nu9veMacShell`; renombrarlo sería un cambio propio, con `-v2` conviviendo.

## `Nu9veEngine`: cliente de nu9ve-engine-v1

Habla con un motor persistente por NDJSON (stdin/stdout), como `clipper-core serve` o `nuun-engine rpc-serve`. No sabe qué motor es: la app pasa el binario, los argumentos y el `protocol` que espera. Qué cumple de cada regla está en la [§12.4 del contrato](../contracts/nu9ve-engine-v1.md#124-cliente-de-swift-nu9veengine).

**API pública.**

| Tipo | Qué es |
| --- | --- |
| `EngineConfiguration` | Binario (ruta absoluta), argumentos, entorno, carpeta, stderr (`.discard` o `.file(URL)`), `expectedProtocol`, plazos (`startupTimeout` 2 s, `requestTimeout` 30 s), topes (1 MiB por petición, 16 MiB por respuesta), espera entre reinicios (`backoff` 10 s y `maximumBackoff` 320 s), `shutdownGrace` y `checksMethods` |
| `EngineClient` | El proceso persistente. `start()` devuelve el `EngineInfo`; `call(_:_:timeout:idempotencyKey:)` devuelve `EngineOutcome<EngineReply>`; `call(_:_:as:)` decodifica `data` con `JSONDecoder`; `openLease(_:_:release:timeout:onEnd:)`; `stop()`; `info` y `processIdentifier` |
| `EngineOutcome<Value>` | `.value(Value)` o `.notPorted(EngineNotPorted)`: `not_ported` no es una excepción |
| `EngineNotPorted` | `reason`, `message` y `answeredByClient` (el método no estaba en `ported` ni `native`: `method_unavailable` sin preguntar) |
| `EngineReply` | `value` (`EngineValue`) y `bytes`, los bytes exactos de `data`; `decode(_:)` |
| `EngineError` | `code`, `message`, `origin` (`.engine`: error del método, como `project_not_found`; `.client`: transporte, §7), `dispatched` (§6.3), `reason`, `retryable`, `retryAfterSeconds` y el objeto `details`. Los códigos del cliente están en `EngineError.Code` |
| `EngineLease` | Una lease en su propio proceso: `acquired`, `isHeld`, `release(timeout:)` (manda el método de liberación y cierra), `close()` (EOF) |
| `EngineInfo` | `contract`, `protocol`, `engine`, `version`, `capabilities`, `ported`, `native`, `fields` y `answers(_:)` |
| `EngineValue`, `EngineObject`, `EngineNumber`, `EngineID` | JSON con números como texto (`1` no es `1.0` y un id de más de 64 bits vuelve igual) y claves en orden. `EngineValue(parsing:)` es estricto (sin `NaN`, BOM ni texto sobrante, 128 niveles); `jsonData` es compacto y sin saltos de línea crudos; `EngineValue(encoding:)` y `decode(_:)` pasan por `Codable` |
| `EngineEnvelope`, `EnvelopeProblem`, `EngineInfoProblem` | El sobre leído según la §3 y por qué no vale, con los mismos motivos que `data/engine-examples-v1.json` |

**Cómo lo usa una app (Neuf con `clipper-core`).** Neuf ya fija `swift` en `vendor/nu9ve-core.paths`; suma `contracts/nu9ve-engine-v1.md` si quiere la regla a mano, corre `vendor/nu9ve-core/scripts/sync-core.sh <commit>` y agrega el producto `Nu9veEngine` del paquete local `vendor/nu9ve-core/swift` a su target. Las vistas son las de M8 en el `APP_SERVICES_CONTRACT.md` de Clipper.

```swift
import Nu9veEngine

// Uno por app: el proceso vive mientras haga falta y se reinicia solo.
let clipper = EngineClient({
    var configuration = EngineConfiguration(
        executable: bundledEngineURL,            // ruta absoluta de clipper-core
        arguments: ["serve"],
        expectedProtocol: "clipper-engine/1")
    configuration.standardError = .file(logsURL.appendingPathComponent("clipper-core.log"))
    return configuration
}())

struct ReviewItem: Decodable, Sendable { /* los campos que usa la vista */ }

/// nil: el motor todavía no lo resuelve (p. ej. un salto manual por preset); la app avisa o usa su respaldo.
func reviewItem(project: URL, item: String) async throws(EngineError) -> ReviewItem? {
    let params: EngineObject = ["project": .string(project.path), "item": .string(item)]
    switch try await clipper.call("review_item", params, as: ReviewItem.self) {
    case .value(let view): return view
    case .notPorted(let missing): log("review_item: not_ported \(missing.reason)"); return nil
    }
}
```

- **Errores.** `origin == .engine` es la respuesta del método (`project_not_found`…), igual que la daría Python. `origin == .client` es transporte: `timeout`, `connection_lost`, `engine_unavailable` (esperando el próximo reinicio, con `retryAfterSeconds`), `incompatible_engine`, `engine_missing`, `invalid_response`.
- **Escrituras** (`item_review_toggle`, `item_transcript_edit`…). Llevan `request_id` y `expected_revision` en `params`, como pide Clipper. Si fallan con `dispatched == true`, no se reenvían: la app pregunta `write_status` con ese `request_id` (§6.3) y, al confirmar, pide un `project_summary` nuevo.
- **Leases de reproducción.** Una por reproductor u original, cada una en su proceso:

  ```swift
  switch try await clipper.openLease("project_output_lease", ["project": .string(project.path)],
                                     release: "playback_lease_release",
                                     onEnd: { lost in /* el proceso murió: los candados se soltaron */ }) {
  case .value(let lease):
      let revision = lease.acquired.value["revision"]   // phase == "ready"
      // … reproducir …
      try await lease.release()                         // phase == "released"
  case .notPorted(let missing):
      // mapa inexistente o alterado: la app avisa
  }
  ```

  Si la app sale sin liberar, el EOF del proceso también suelta los candados.

**Pruebas.** `swift test` corre los ejemplos del contrato (sobres, ids, `engine_info`), los 20 casos de motor contra `Nu9veEngineFixture` (un motor de prueba del paquete, sin producto) y el comportamiento del cliente: concurrencia, plazos, caídas y espera, respuestas de más, violaciones del protocolo, cancelación y leases. Contra el motor real de una app, de sólo lectura, con un archivo que describe la sonda:

```bash
cat > /tmp/clipper-probe.json <<'JSON'
{"command": ["/Users/<tú>/code/ai/clipper/core/target/release/clipper-core", "serve"],
 "protocol": "clipper-engine/1",
 "reads": [
  {"method": "capabilities", "expect_field": "contracts"},
  {"method": "project_summary", "params": {"project": "$FIXTURE", "allow_legacy_seed": true}, "expect_field": "project",
   "fixture": {"clipper-project.json": {"name": "Prueba", "slug": "prueba", "root": "$FIXTURE",
     "settings": {"creative_seed": 1}, "created_at": "2026-11-22T00:00:00Z", "updated_at": "2026-11-22T00:00:00Z"}}}]}
JSON
```

```bash
NU9VE_ENGINE_LIVE_PROBE=/tmp/clipper-probe.json swift test --package-path swift --filter ALiveEngine
```

La prueba escribe cada `fixture` en una carpeta temporal, cambia `$FIXTURE` por su ruta y corre los 20 casos y las lecturas. Sin la variable, se salta.
