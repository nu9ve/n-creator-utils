# Documentation map

| Need | Read |
| --- | --- |
| Understand the product in a few minutes | [CONTEXT](CONTEXT.md) |
| Know what works today, per area | [STATUS](STATUS.md); history in [status log](releases/status-log-2026-10-25_2026-11-08.md) |
| Pick the next task | [PLAN](../PLAN.md) |
| Install, run, register disks and debug | [Operations](operations.md) |
| Change code or resume another session | [Development](development.md), root [AGENTS](../AGENTS.md) |
| Assess a distribution build | [Release gates](release.md) |
| Understand the three-mode nüün window | [Window flow/limits](features/nuun-window.md) |
| Review the final local artifact / manual acceptance | [2026-10-26 closeout](releases/2026-10-26-local.md) |
| Close the product loop: navigation, project lifecycle, per-type flows, recipes | [Product flows](features/product-flows.md) (proposal) |
| Understand what fills the disks and what is safe to free | [Storage map](features/storage-map.md) |
| Evolve dashboard/launcher | [Feature backlog](features/dashboard-launcher.md) |
| Improve app shell, flows, performance, i18n, platforms | [App review 2026-10](features/app-review-2026-10.md) |
| Change a cross-app boundary | [Contracts index](contracts/README.md) |
| Build or review a native Mac app: title bar, menus, About, documents, distribution | [macOS app contract](contracts/macos-app-v1.md) |
| Shared look across nu9ve apps: color roles, Appearance settings, skins, per-app identity | [Visual theme contract](contracts/nu9ve-theme-v1.md) (proposal) |
| Keyboard-first interaction: shortcut catalog, key badges and modifier layers, context help, ⌘/ panel | [Interaction contract](contracts/nu9ve-interaction-v1.md) (proposal) |
| Build or review a native Windows app sharing an engine with its Mac app | [Windows app contract](contracts/windows-app-v1.md) (proposal) |
| See every nu9ve app, what it is for, its platforms, shared flows/contracts and package candidates | [Suite map](suite.md) |
| Review storage architecture and intended UX | [Architecture](media-architecture.md), [experience](media-experience.md) |
| Understand original requirements and legacy failures | [Audit](media-audit.md), [legacy importing](importing.md), [historical flow review](file-workflows.md) |
| Configure older downloads/stems/importer | [Configuration](configuration.md), [commands](commands.md) |
| Resume the current Sony/two-multicam acceptance | [Expected flow and pending evidence](sony-validation.md#aceptación-acompañada-sony-y-dos-multicams--2026-11-06) |
| Run a Sony hardware trial | [Sony validation](sony-validation.md) |

STATUS is the current snapshot. Architecture/experience contain design beyond shipped code. Historical pages retain their date/scope and link forward. Contracts say separately whether a producer, consumer, transport or schema is implemented.

Update the owner document when behavior changes; keep this index and the compact context short. Add a dated decision note only when a tradeoff needs history, then link it from the owner document. Avoid a second competing “master plan”.
