# Apps con y sin la parte privada

Estado: **propuesta** (2026-11-22), pedida por el usuario el mismo día que se creó `nu9ve/inner-core`. Dice cómo una app usa lo privado sin dejar de compilar y funcionar sola. Ejemplo: que Clipper y su TUI se puedan compilar sin ningún módulo interno, y que la misma app gane funciones cuando sí los tiene. Relacionados: [licencias](licensing.md), [varias máquinas, discos y nube §9](suite-devices.md#9-público-y-privado-qué-va-dónde), [cómo se comparte](suite-sharing.md).

## 1. Tres capas, tres repos

| Capa | Repo | Qué guarda | Puede abrirse |
| --- | --- | --- | --- |
| Público | `nu9ve/core` | Contratos, datos, protocolos locales y piezas genéricas (`nu9ve-json`, `nu9ve-fs`, `nu9ve-engine`, `NuMacShell`, `Nu9veTheme`…) | Sí, algún día ([licencias §6](licensing.md#6-antes-de-hacer-público-nu9vecore)) |
| Privado | `nu9ve/inner-core` | Lo compartido que es negocio o toca la cuenta: cliente de la nube, cuenta y dispositivos, licencias y activación, pasarela de modelos, telemetría, conectores propios | No |
| App | El repo de cada app | Su motor de dominio y su interfaz | Lo decide cada app |

- **Debe:** `nu9ve/core` nunca depende de `nu9ve/inner-core`. Al revés sí: `inner-core` fija su copia del core como cualquier app.
- **Debe:** un repo público nunca lleva copia de `inner-core` ([licencias §5](licensing.md#5-reglas-que-hacen-que-funcione)).
- Las mismas reglas de nombres que el core: todo con prefijo `nu9ve-` (`Nu9ve` en Swift y C#), y nada con el nombre, los tipos o las rutas de una app.

## 2. La regla principal: las apps piden lo privado, no lo enlazan

Casi todo lo privado de la suite es **cuenta, nube y dispositivos**. Eso vive en un solo proceso por máquina, el agente `nu9ve` ([varias máquinas §6.6](suite-devices.md#66-agente-de-la-suite--nu9ve-agent)), que es el único que enlaza `inner-core`.

- Una app le pregunta al agente por el protocolo de siempre ([`nu9ve-engine-v1`](../contracts/nu9ve-engine-v1.md)) y mira sus `capabilities` (`cloud_v1`, `devices_v1`…). Si el agente no está, o está pero sin nube, la app funciona igual y esas funciones no aparecen.
- Así **la app no cambia** entre la build pública y la privada: el código que pide la nube es público, porque es un cliente del protocolo, y lo privado está del otro lado del socket.
- Es lo mismo que ya hace `nu9ve mcp`: si una app no está abierta, sus herramientas no aparecen.

Clipper, con esta regla: su TUI y su CLI compilan y corren sin `inner-core`, sin red y sin cuenta. Si el agente tiene sesión, Clipper ve `cloud_v1` y ofrece, por ejemplo, «subir este artefacto». El TUI no enlaza nada privado.

## 3. Cuando una app sí tiene que enlazar algo privado

Por ejemplo, una función de pago que corre dentro de la app, o un conector que no tiene sentido en el agente. Entonces el código privado va en un **paquete aparte**, que depende de la app pública. La app pública nunca depende de él:

```text
<app>/
├─ …                         la app pública, que compila sola
├─ vendor/nu9ve-core/        copia fijada del core
└─ private/                  sólo en la build privada
   ├─ vendor/nu9ve-inner-core/   copia fijada de inner-core
   └─ el paquete que une los dos (otro binario, o un plugin que la app descubre)
```

| Ecosistema | Build pública | Build privada | Por qué así |
| --- | --- | --- | --- |
| Rust | El workspace de la app no nombra `private/` | `private/Cargo.toml` es otro workspace: un binario que depende del crate público por ruta y de `inner-core` | Cargo lee el manifiesto de una dependencia por ruta aunque sea opcional, así que una ruta que no existe rompe la build pública. Una feature con dependencia opcional no basta |
| Swift | `Package.swift` de la app, sin nada privado | `private/Package.swift` depende del paquete de la app por ruta | SwiftPM tampoco admite dependencias que pueden faltar |
| Python (Clipper, nüün) | Nada que instalar | Un paquete en `private/` que la app descubre al arrancar (entry points o `importlib.util.find_spec`) | Python sí puede descubrir en tiempo de ejecución; si el paquete no está, la función no aparece |
| C# | La solución de la app | Un proyecto en `private/` que referencia al público | Igual que Rust |

- **Debe:** sin `private/`, la app compila, pasa su gate y funciona completa en lo que es suyo. Nada falla por ausencia: simplemente no aparece.
- **Debe:** el gate de una app que tiene las dos builds corre las dos. La pública, **siempre en un checkout sin `private/`**, para que ninguna dependencia escondida pase desapercibida.
- **Debe:** si el repo de la app se abre algún día, `private/` sale antes del repo (a uno propio de esa app) y su historia se limpia, como pide licencias §6.
- **Conviene:** que cada función privada se anuncie también como capacidad (`engine_info.capabilities`) o como plugin registrado, para que la interfaz decida qué mostrar por lo que hay y no por cómo se compiló.

## 4. Apps privadas para siempre

nove, SoundBender, neuf y las que nunca se vayan a abrir pueden fijar `inner-core` directamente en `vendor/nu9ve-inner-core/`, sin carpeta `private/`. Conviene igual que lo de cuenta y nube pase por el agente (§2): así una sola sesión de la cuenta sirve a todas las apps de la máquina, y nada se repite.

## 5. Cómo se fija `inner-core`

Con el mismo mecanismo que el core ([cómo se comparte §4](suite-sharing.md#4-cómo-lo-consume-una-app)): una lista de rutas en `vendor/nu9ve-inner-core.paths` (o `private/vendor/…`), una copia exacta con `SOURCE` y `MANIFEST.sha256`, y `--check` en el gate.

**Hecho el 2026-11-22 (U1).** `scripts/sync-core.sh` maneja los dos repos sin romper a las apps que ya lo usan:

- el repo se reconoce por un archivo en su raíz (`.nu9ve-repo` con su nombre), no por llevar el script;
- `--repo inner-core` elige el repo, con su carpeta en `vendor/`, su lista de rutas y su orden de búsqueda: `NU9VE_INNER_CORE_REPO`, `../inner-core`, `~/code/ai/inner-core`;
- sin `--repo`, todo sigue igual que hoy;
- una copia de `inner-core` no lleva el script: la app la comprueba con el de su copia del core (`vendor/nu9ve-core/scripts/sync-core.sh --repo inner-core --check`), y el `SOURCE` de cada copia dice de qué repo es, así que una copia en la carpeta equivocada falla;
- con pruebas nuevas en `scripts/test-sync-core.sh`.

Una app recibe el script nuevo cuando vuelve a fijar el core. Hasta entonces, su copia vieja sigue funcionando sólo con el core.

## 6. Qué va en `inner-core` y cuándo

La misma regla que el core: una pieza entra cuando dos apps la usan igual o cuando nace genérica. Lo primero que se espera:

| Pieza | Cuándo | De dónde |
| --- | --- | --- |
| `nu9ve-cloud`: sesión de la cuenta, registro del dispositivo con DPoP, envío cifrado de la carpeta de intercambio | F2 de [varias máquinas](suite-devices.md#10-fases), con el OAuth de Fractos listo | Nace genérica |
| Cifrado de punta a punta y recuperación con passkey | F2 | Nace genérica |
| Eventos de actividad para los logros de Fractos | F2 | Nace genérica |
| Licencias y activación de apps de pago | Cuando la primera app se venda (SoundBender, según [licencias §4](licensing.md#4-modelos-para-una-app)) | — |
| Pasarela de modelos con el saldo de la cuenta | F4 | — |

Lo que es protocolo local (identidad de dispositivo y de volumen, carpeta de intercambio, emparejado) **no** va en `inner-core`: es público, en el core.

## 7. Decisiones abiertas

1. Si las builds privadas de las apps que pueden abrirse (Clipper, curado) viven en `private/` dentro de su repo mientras sea privado, o desde ya en un repo aparte por app.
2. Cómo se distribuye el agente con nube: un binario privado firmado que reemplaza al público, o el público más un plugin.
