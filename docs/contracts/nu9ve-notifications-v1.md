# Avisos del sistema de las apps de nu9ve v1

Estado: **propuesta** (2026-11-10), parte de la [auditoría Fable](../audits/fable-audit-2026-11-10.md). Define cómo una app de la suite avisa por el sistema operativo (notificación de Windows, de macOS o de Linux) cuando pasa algo que la persona quiere saber con la ventana cerrada o en segundo plano. La base es **nove en Windows**, que ya lo hace con uso real. Escribir la regla no la da por adoptada: ver [adopción](#adopción).

**Debe** es obligatorio salvo que la app documente por qué no aplica. **Conviene** es lo recomendado.

Leído el 2026-11-10, sin ejecutar nada:

| App | Dónde | Qué toma este contrato |
| --- | --- | --- |
| nove (Windows) | `nove`: `core/src/deliveries.rs`, `docs/contracts/deliveries-v1.md`, `apps/windows/Nove/Notifications.cs`, `apps/windows/Package/AppxManifest.template.xml`, decisiones A16 y A36 | La cola de entregas en el motor, el ayudante que muestra una notificación y sale, la activación que abre el aviso exacto, los límites, el alcance «todo salvo lo que apagues» y la prueba de canal |
| nove (Mac) | `nove`: `apps/macos/Sources/Nove/Markets.swift`, `ViewNotices.swift`, `ViewWatches.swift` | Permiso pedido al habilitar; la notificación la publica la app, sin cola del motor (la Mac va en datos 11; la cola llegó en datos 14) |
| nüün (Mac) | [Model.swift](../../native/MediaMenu/Sources/MediaMenu/Model.swift), [Application.swift](../../native/MediaMenu/Sources/MediaMenu/Application.swift) | Un aviso de «tarjeta detectada» con identificador por tarjeta; el clic abre Ingreso |

## 0. Principios

- **El aviso dentro de la app es la verdad.** El Buzón de nove, el resultado de un trabajo en nüün. La notificación del sistema es una **copia**: puede fallar, llegar tarde o estar bloqueada por el sistema sin que se pierda nada.
- **El motor decide; el sistema muestra.** Qué se avisa, a qué canal, cuándo se reintenta y cuándo caduca lo decide el motor. El cliente nativo sólo presenta la notificación y devuelve el resultado.
- **Un clic lleva al objeto exacto.** Nunca a «la app» en general.
- **La persona controla sin buscar.** Un interruptor por canal, una excepción por elemento y un aviso de prueba.
- **Nada sensible sale de la app.** Una notificación se ve en la pantalla de bloqueo y en el historial del sistema.

## 1. Modelo

```text
Algo pasa → Aviso (durable, en el motor) → Entrega por canal (copia en cola) → Adaptador del sistema
```

- **Debe:** la entrega se encola en la **misma transacción** que el aviso. Si la transacción falla, no hay aviso ni copia; si la copia falla, el aviso sigue.
- **Debe:** entrega **al menos una vez**. Tras un reinicio, lo que estaba «enviando» vuelve a la cola. Un duplicado es aceptable; un aviso perdido, no.
- **Debe:** estados visibles y con motivo. Los de nove sirven tal cual: `queued` → `sending` → `sent` | `failed` | `withheld` (canal apagado, configuración cambiada, límite) | `expired`.
- Canales: el del sistema operativo (`windows_toast`, `macos_notification`, `linux_notification`) es el común. Otros (Telegram en nove) son propios de la app y siguen las mismas reglas, más las de sus credenciales.
- Una app sin motor de larga vida (nüün hoy, con procesos cortos) puede publicar desde el cliente, pero **debe** conservar las reglas de identidad, límites y clic de este contrato, y mover la cola al motor cuando tenga uno.

## 2. Qué se avisa

Cada app declara sus tipos de aviso en un catálogo, como hace con los atajos. Un tipo tiene: identificador estable, texto, urgencia, a qué objeto lleva el clic y qué gesto de sonido usa ([§7](#7-sonido)).

| App | Tipos (hoy o previstos) | Urgencia |
| --- | --- | --- |
| nove | Condición cumplida de un seguimiento web; señal de estrategia de un par; par sin datos; lectura cortada | Normal; **recordatorio** para las señales de estrategia |
| nüün | Tarjeta detectada (existe en Mac); trabajo terminado y verificado; trabajo fallido; la cámara necesita una acción; archivo casi lleno | Normal; recordatorio para un fallo |
| curado | Separación de stems terminada con la app en segundo plano | Normal |
| Fractos | Los decide su servidor; fuera de este contrato | — |

- **Debe:** avisar sólo de lo que la persona no está viendo. Un trabajo que termina con su ventana al frente no manda notificación ([macos-app-v1 §8](macos-app-v1.md#8-sistema)).
- **Debe:** una acción larga avisa **al terminar**, una vez. Nunca por archivo ni por paso.

## 3. Valores por defecto y control

Decisión del usuario para nove (A36, 2026-11-10): «que todos por default se manden a Windows a menos que se configure que no». Se adopta para la suite:

- **Debe:** el canal del sistema, una vez activo, avisa de **todo** por defecto. Cada elemento (un par, un seguimiento, un tipo de aviso) se puede apagar como **excepción**, y cambiar de «todo» a «sólo los elegidos» conserva cada elección.
- **Debe:** Ajustes → **Avisos** con, en este orden: interruptor del canal del sistema, alcance («Todo, salvo lo que desmarque» / «Sólo lo que elija»), sonido del aviso, «Enviar aviso de prueba» y el estado del permiso del sistema con un enlace a su configuración cuando está bloqueado.
- **Debe:** pedir el permiso del sistema al activar el canal o al crear el primer aviso, nunca al abrir la app. En macOS el permiso es un diálogo del sistema; en Windows no hay diálogo y la persona lo apaga en Configuración. Si el sistema lo tiene bloqueado, la app lo dice (`notifications_disabled`) y no reintenta.
- **Debe:** restaurar una biblioteca desde un respaldo deja los canales apagados y retiene lo pendiente. Nadie recibe una ráfaga de avisos viejos.
- **Por decidir en cada app:** si el canal nace encendido. En nove el canal se activa en Ajustes y la migración no lo enciende. Con la decisión de «sonido encendido por defecto» del [sonido de tema](nu9ve-sound-v1.md#7-valor-por-defecto-ajustes-y-formato), lo coherente es que el canal del sistema nazca encendido donde el sistema no pide permiso (Windows) y se ofrezca en el primer uso donde sí lo pide (macOS).

## 4. Contenido

- **Debe:** tres líneas como máximo: título (el nombre del objeto), una línea con el hecho (valor, condición, hora) y, si hace falta, una nota.
- **Debe:** hasta dos botones. El primero siempre es «ver» y lleva al objeto.
- **Debe:** un botón que **hace** algo sólo si es seguro y reversible desde donde está la persona: «Expulsar tarjeta» sí; «Aplicar plan», «Limpiar» o «Liberar» nunca. Es el mismo límite del nivel Mostrar del [MCP](nu9ve-mcp-v1.md#2-cuatro-niveles-de-herramientas).
- **Debe:** nada de tokens, credenciales ni rutas personales completas. El nombre de un archivo o de un proyecto es dato observado: se limpia de caracteres de control y bidi y se acota.
- **Debe:** los argumentos de activación se validan al recibirlos (prefijo conocido, longitud, forma), como `Notifications.AlertFrom` de nove. Un argumento inválido abre la app sin navegar.
- **Conviene:** recordar que el contenido se ve con la pantalla bloqueada. Si un valor es privado para la persona, Ajustes ofrece «ocultar el detalle».
- Idioma: el de la app, del mismo catálogo de textos que la interfaz.

## 5. Identidad, reemplazo, límites y caducidad

| Regla | En nove (Windows) | Para la suite |
| --- | --- | --- |
| Un reintento reemplaza, no duplica | `Tag` y `Group` de la notificación | **Debe.** macOS: el identificador de la petición. Linux: `replaces_id` |
| Límite por hora | 30; el exceso se **retiene** (el Buzón ya lo tiene) | **Debe** tener un límite y decir que se retuvo |
| Caducidad | 12 h: un aviso que no pudo salir no se manda tarde | **Debe** |
| Reintentos | 30 s a 30 min, 6 intentos; los errores permanentes no reintentan | **Debe** distinguir permanente de reintentable |
| Historial | Últimas 1000 entregas terminadas | **Conviene** |
| Plazo del ayudante | 15 s | **Debe** tener plazo: un adaptador colgado no bloquea la cola |

## 6. Urgencia

| Nivel | Qué es | Windows | macOS | Linux |
| --- | --- | --- | --- | --- |
| Normal | Banner que se va solo | Notificación por defecto | Banner | Urgencia normal |
| Recordatorio | Suena y se queda hasta atenderlo | Escenario `Reminder` (nove lo usa para señales de estrategia) | Nivel de interrupción alto; **que se quede en pantalla lo decide la persona** en Ajustes del Sistema (Tiras o Alertas), no la app | Urgencia crítica, sin tiempo de expiración |

- **Debe:** el recordatorio se reserva para lo que pierde valor si no se atiende pronto. Si todo es urgente, la persona apaga el canal.
- **Debe:** respetar «No molestar» y los modos de concentración. La app no los rodea.
- En macOS, cuando un tipo de aviso necesita quedarse en pantalla, Ajustes lo explica y enlaza a la configuración del sistema.

## 7. Sonido

Enlaza con el [sonido de tema](nu9ve-sound-v1.md). Una notificación puede sonar con la app cerrada, así que su sonido es siempre un **archivo**, no el motor en vivo.

- **Debe:** Ajustes → Avisos → **Sonido del aviso**: Del tema (por defecto) · Del sistema · Ninguno.
- «Del tema» usa el archivo del gesto que corresponde al tipo de aviso, en la voz del skin activo al encolar: `discovery` para algo nuevo, `success` y `error` para el fin de un trabajo, `warning` para lo que pide atención. Los archivos salen del paquete de sonidos de la suite ([sonido v1 §4](nu9ve-sound-v1.md#4-cómo-llegan-los-sonidos-a-cada-app)).
- **Debe:** si «Sonidos de interfaz» está silenciado, los avisos también usan «Del sistema» o «Ninguno», lo que la persona haya elegido; silenciar la app no deja sonando sus avisos con la voz del tema.
- Windows: audio del propio paquete en la notificación. macOS: un sonido con nombre dentro del bundle, de pocos segundos. Los formatos y límites se confirman al implementar.
- Hasta que exista el paquete, se usa el sonido del sistema.

## 8. Adaptador por sistema

| | Windows | macOS | Linux |
| --- | --- | --- | --- |
| API | `AppNotificationManager` (Windows App SDK) | `UNUserNotificationCenter` | Portal de notificaciones o `org.freedesktop.Notifications` |
| Quién publica | Un **rol** del ejecutable (`--deliver-toast`), lanzado por el motor, que muestra una notificación y sale | La app o su ayudante dentro del mismo bundle; requiere un bundle real (protegerlo cuando no lo hay) | El host de usuario |
| Clic con la app cerrada | Activación COM declarada en el manifiesto MSIX; la ventana se registra **antes** de leer sus argumentos de arranque | El delegado recibe la respuesta al abrir | Acción por D-Bus |
| Acciones | Botones con argumentos | Categorías y acciones registradas al arrancar | Acciones de la especificación |
| Permiso | Sin diálogo; se lee `Setting` antes de mostrar | Diálogo del sistema al pedir autorización | Sin diálogo |
| Desinstalar | El MSIX retira el registro | Se va con la app | Se va con el paquete |

- **Debe:** el adaptador recibe un **plan** del motor (título, líneas, identificador, argumentos, sonido) y devuelve un resultado con código estable y si es reintentable. No decide nada.
- **Debe:** el plan está acotado en tamaño y versionado; un plan inválido es un error permanente.
- **Conviene** en Mac el mismo patrón que en Windows: un ejecutable con roles ([windows-app-v1 §8](windows-app-v1.md#8-sistema-y-segundo-plano)), para que el motor en segundo plano pueda publicar como la app.

## 9. Validación

- **Debe**, con el motor real en una raíz temporal y un ayudante falso:
  - el aviso y su copia nacen juntos, y una transacción fallida no deja ninguno;
  - un reintento no duplica; el límite retiene y lo dice; lo caducado no sale;
  - una fuente apagada no encola; cambiar de alcance conserva las excepciones;
  - restaurar apaga canales y retiene lo pendiente;
  - un argumento de activación inválido no navega.
- **Debe**, a mano y registrado como tal: un clic real con la app cerrada y con la ventana abierta; el aviso con «No molestar» activo; el contenido en la pantalla de bloqueo; el sistema con notificaciones bloqueadas para la app.
- nove tiene pendiente justo esto último: «clic físico en la notificación» sigue sin registrar en su STATUS.

## Adopción

Leído en el código el 2026-11-10; nadie probó un aviso desde esta sesión.

| Regla | nove (Windows) | nove (Mac) | nüün (Mac) | nüün (Windows) | curado | Fractos Mac |
| --- | --- | --- | --- | --- | --- | --- |
| 1 Cola en el motor | Sí (`deliveries.rs`) | No: publica la app | No: publica la app | No | No | No |
| 2 Catálogo de tipos | Implícito en el motor (web, OTC, prueba) | Implícito | Un tipo: tarjeta detectada. Falta fin de trabajo | No | No | No |
| 3 Por defecto y control | Sí: «todo salvo excepciones» (A36), prueba de canal | Opt-in por vigilancia | Sin ajustes de avisos | No | No | No |
| 4 Contenido | Sí: tres líneas, botón «Ver en el Buzón», argumentos validados | Parcial | Parcial: título y recomendación; sin botones | No | No | No |
| 5 Identidad y límites | Sí: etiqueta y grupo, 30/h, 12 h, reintentos | Identificador por aviso | Identificador por tarjeta; sin límite | No | No | No |
| 6 Urgencia | Sí: recordatorio para señales de estrategia | No | No | No | No | No |
| 7 Sonido | Del sistema | Del sistema (pide `.sound`) | Silencioso (sólo pide `.alert`) | No | No | No |
| 8 Adaptador | Sí: rol `--deliver-toast`, activación COM | En la app | En la app; el clic abre Ingreso | No | No | No |
| 9 Validación | Automática sí; clic físico pendiente | Con ayudante ficticio; aceptación del sistema pendiente | Sin pruebas vistas | — | — | — |

Primeros pasos sugeridos:

- **nüün (Mac):** avisos de trabajo terminado y fallido con la app en segundo plano, con «Ver resultado» y «Expulsar tarjeta»; sección Avisos en Ajustes; sonido.
- **nüün (Windows):** el adaptador copiando el rol de nove con comentario de origen, cuando `plan`/`apply` estén en la ventana ([brief S9](../audits/session-briefs-2026-11-10.md#s9--integración-de-sistema-en-la-ventana-de-windows)).
- **nove (Mac):** llevar el canal del sistema a la cola del motor al alcanzar el esquema actual, para que Mac y Windows compartan límites, alcance y caducidad.
- **nove (Windows):** registrar el clic físico; decidir si el canal nace encendido.
- **curado y Fractos Mac:** sólo cuando tengan algo que avisar en segundo plano.

Brief: [S18](../audits/session-briefs-2026-11-10.md#s18--avisos-del-sistema-en-nüün-para-mac).

## Cambios

Regla nueva o más estricta: añadirla aquí, actualizar la tabla y avisar en cada app afectada. Lo que cambie en la cola de nove se refleja primero en su contrato (`nove`: `docs/contracts/deliveries-v1.md`) y después aquí.
