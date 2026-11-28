# Documentos

| Necesitas | Lee |
| --- | --- |
| Saber cómo aíslan la QA, cómo se instalan y qué gate corren las apps, y en qué difieren | [Prácticas comunes](practices.md) (guía) |
| Ver cada app de nu9ve, para qué sirve, sus plataformas y lo que se repite | [Mapa de la suite](suite.md) |
| Entender por qué hay un repo común y cómo lo consume una app | [Cómo se comparte](suite-sharing.md) |
| Saber qué falta mudar de n-creator-utils y cuándo | [Qué se muda](suite-split.md) |
| Ver qué cumple cada app de cada contrato | [Adopción por app](adoption.md) |
| Ver qué se extrae después, en qué orden, y cómo empieza a compartir cada app | [Qué se extrae y cómo se adopta](suite-adoption.md) |
| Saber qué controles comparten las apps (fechas, números, listas, textos, estados) y cuáles se extraen | [Controles comunes](suite-ui.md) (propuesta, con preguntas abiertas) |
| Saber qué se estandariza del progreso de las acciones largas, sus estados y el hover por skin | [Acciones, progreso y estados](actions-next.md) (propuesta, desde nüün) |
| Saber cómo se comparten los perfiles entre lenguajes, las páginas dentro de una app en el controlador y los eventos de acción para Stalker y Clipper | [Perfiles, páginas y eventos de acción](profiles-pages-actions-next.md) (propuesta, desde SoundBender, con preguntas para el usuario) |
| Saber qué se comparte del uso de agentes por cuenta y de las vistas por categoría | [Uso de agentes por cuenta](agent-usage-next.md) (decidido el 2026-11-26, vive en nove) |
| Construir una app de Windows sobre la base de la suite | [Windows en la suite](platforms/windows.md) |
| Saber qué repiten nove y nüün en Windows y qué subir aquí | [Windows: lo que se repite](windows-shared-next.md) (propuesta; la cancelación en Windows se decidió el 2026-11-26) |
| Traducir una función del sistema entre macOS, Windows y Linux | [Integración con el sistema](platforms/os-integration.md) (propuesta) |
| Planear Linux | [Linux en la suite](platforms/linux.md) (propuesta) |
| Decidir qué código de audio se comparte | [Audio en la suite](platforms/audio.md) (propuesta) |
| Decidir qué se comparte de Metal y la GPU | [GPU en la suite](platforms/gpu.md) (propuesta) |
| Decidir qué se abre y qué queda privado | [Licencias](licensing.md) (propuesta, sin decidir) |
| Hacer que una app use lo privado (`nu9ve/inner-core`) sin dejar de compilar y funcionar sola | [Apps con y sin la parte privada](suite-modules.md) (propuesta) |
| Trabajar con varias máquinas y discos, y decidir cómo entran la cuenta de Fractos, la nube y el MCP | [Varias máquinas, discos y nube](suite-devices.md) (propuesta, con preguntas abiertas) |
| Decidir cómo sería tu navegador diario y qué capa le pone la suite | [Navegador, versión 2](suite-browser-v2.md) (propuesta); el inventario y los contratos siguen en la [versión 1](suite-browser.md) |
| Leer la auditoría del 2026-11-10 y sus briefs | [Auditoría Fable](audits/fable-audit-2026-11-10.md), [briefs](audits/session-briefs-2026-11-10.md) |

La auditoría y sus briefs son históricos: conservan su fecha y sus rutas de entonces (`nu9ve/suite`, `vendor/suite`).

El análisis del motor en Rust sigue en [n-creator-utils](https://github.com/nu9ve/n-creator-utils/blob/master/docs/platforms/rust-engine.md); su §6 («qué compartir en Rust entre las apps») se muda aquí cuando se parta el documento.
