# Dashboard, launcher y mejoras de producto

Backlog de diseño; no confundir con [implementado](../STATUS.md). Prioridad técnica global: [PLAN](../../PLAN.md). Toda superficie consume `media/service.py`; ni dashboard ni launcher hacen su propio copy/delete.

## Qué debe responder el dashboard

«¿Qué requiere mi atención?», «¿dónde están mis archivos?», «¿qué puedo editar ahora?» y «¿qué puedo liberar?». Archivo verificado, trabajo preparado, publicación y SD lista son estados separados.

| Prioridad | Feature | Datos / criterio de aceptación |
| --- | --- | --- |
| P1 | Centro de acciones | Jobs bloqueados/pausados, disco necesario, razón y siguiente paso. Reanudar sólo un plan consistente; ningún “retry” borra precondiciones. |
| P1 | Pendientes de trabajo | Agrupar por proyecto/perfil, bytes y SSD esperado; copiar desde MEDIA, posponer u omitir. No volver a molestar por omitidos. |
| P1 | Discos y capacidad | Identidad, rol, online/offline, libre/reservado y fecha del snapshot. Dos ubicaciones en un físico no cuentan como redundancia. |
| P1 | Capturas recientes | Stalker completed/review/archivado y SDs candidatas; activos visibles sin tratarlos como completos. Acciones manuales de archivo por selección. |
| P1 | Detalle de job | Fases, archivos/bytes por destino, verificación y cleanup pendiente. Progreso basado en eventos/recibos reales, nunca porcentaje inventado. |
| P2 | Biblioteca por proyecto y fecha | Filtros, búsqueda, cámara/modelo, perfiles, disponibilidad y orden de selección. Previews paginados bajo demanda. |
| P2 | Cobertura de archivo | Miembros requeridos, última verificación, replicas online/offline e incertidumbre. “Recibo existente” no implica hash comprobado hoy. |
| P2 | Entregas/publicación | Master/shorts/thumbnails/copy por variante y canal; pendientes de archivar al montar MEDIA. `subidos` configurable como sugerencia, no prueba universal. |
| P2 | Preparar para Clipper | Selector de assets/recordings, orden y perfil sugerido; entregar contrato o rutas estables. Relocalización sólo con ACK del editor. |
| P2 | Recuperar espacio | Copias de trabajo/derivados/cachés con dependencias y cuota. Mostrar cuánto y por qué; no borrar raws por existir un export. |
| P2 | Historial y procedencia | Captura → job/ubicaciones → edición/entrega → publicación. Decisiones de excepción y omisión auditables. |
| P3 | Colecciones y recomendaciones | Rutas/vuelos Atlas, presets de producción, restauración parcial y propuestas de agrupación. No disparar ML/ODM sólo por montar disco. |

## Launcher y menubar

La app se instala como **n creator utils**, con icono y menús nativos. Grid de miniaturas, selección ninguna/todas/algunas copias para editar, importador compacto y apariencia Sistema/Claro/Oscuro + HSV ya existen. El popup al conectar es opt-in y queda pendiente la prueba de reconexión física. La menubar y la instalación local con ruta fija/rollback ya existen; abrir o reabrir desde el launcher muestra la ventana, con hash de build en ajustes. [Instalación y actualización local](../operations.md#app-de-desarrollo-y-launcher). Aún requiere Python externo y actualizar explícitamente desde código. Próximas mejoras útiles:

- Acción «Respaldar SD» con último proyecto como elección, no asignación silenciosa; atajo y quick actions con nombres claros.
- Onboarding corto para registrar roles de disco, perfil y defaults. Separar «Sólo copiar» de «Sólo MEDIA».
- Badge discreto por pendientes/bloqueos; notificar cambios significativos y finalización. No abrir popups sin destinos ni repetir todos los avisos al reconectar.
- Launch at login y launcher instalable con detección de versiones/bundle duplicado. Definir comportamiento al abrir una segunda instancia y al actualizar con job activo.
- Instalación/actualización que preserve catálogo, registro de volúmenes y preferencias; rollback de binario independiente de la media.
- Panel de salud: runtime empaquetado, schema/capabilities, permisos, herramientas opcionales y comandos de diagnóstico copiables. Errores operativos en lenguaje humano, detalles técnicos desplegables.
- Exportar diagnóstico redactado: versiones, job/plan digest, fases y errores, sin videos, secretos o paths personales por defecto.

Implementar launch-at-login/actualización después de decidir firma/runtime. No crear un LaunchAgent/daemon adicional sólo para mostrar un icono.

## Diseño modular de las superficies

Al crecer, separar el actual `Application.swift` en modelo de operación, vistas de ingreso/biblioteca/jobs/settings y coordinador de eventos/ventanas. Mantener `MediaMenuCore` para protocolo/runtime/eventos y pruebas independientes; el dominio y SQLite permanecen en Python hasta una decisión explícita de migración.

Una futura UI web/otra plataforma necesita el mismo catálogo de operaciones, capacidades y estados; no acceso directo indiscriminado a SQLite ni serializers que descarten campos. Primero medir si el helper one-shot es suficiente. Un servicio persistente se justifica por costo medido o streaming real, no por estética del dashboard.

## Cómo tomar una feature en otra sesión

Ejemplo: «Implementar detalle de job con fases y bytes reales. Leer CONTEXT, STATUS, PLAN y `media/catalog.py`, `service.py`, `MediaMenuCore/Protocol.swift`. No cambiar cleanup ni formato editorial. Añadir campos compatibles de progress y su vista, con prueba de frontera y un smoke test aislado. Actualizar STATUS con evidencia.»

Cada tarea debe declarar: problema concreto, fuente de datos, operación del servicio, estado vacío/offline/error, impacto de contrato, prueba mínima y fuera de alcance. Así pueden avanzar sesiones de UI, launcher y backend sin redefinir las mismas reglas.
