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
| Build a Windows app on the suite base; Windows lessons from nove, curado and nüün; nüün's Windows maqueta | [Windows in the suite](platforms/windows.md), [native/windows](../native/windows/README.md) |
| Add or review an app's MCP server: tiers (read, show, edit, act), per-session permission, untrusted data, per-app status | [MCP contract](contracts/nu9ve-mcp-v1.md) (proposal) |
| Decide whether to move the media engine to Rust: measurements, gains, risks, migration path, shared crates | [Rust engine analysis](platforms/rust-engine.md) |
| See every nu9ve app, what it is for, its platforms, shared flows/contracts and package candidates | [Suite map](suite.md) |
| Read the 2026-11-10 suite audit: prioritized findings on OS practices, standards, engine, cross-repo drift | [Fable audit](audits/fable-audit-2026-11-10.md) (read-only audit, proposes) |
| Hand an audit finding to another session as a self-contained task | [Session briefs](audits/session-briefs-2026-11-10.md) |
| Translate a system feature between macOS, Windows and Linux; OS features the suite does not use yet; naming/path convention; `nu9ve://` links | [OS integration](platforms/os-integration.md) (proposal) |
| Plan Linux: engine backend, UI decision, packaging and distro matrix | [Linux in the suite](platforms/linux.md) (proposal) |
| Decide where shared code and contracts live, how each app consumes them and how versions are coordinated | [Sharing across repos](suite-sharing.md) (proposal) |
| OS notifications for any suite app: engine outbox, exact-object click, defaults, limits, urgency, sound; nove on Windows as the base | [Notifications contract](contracts/nu9ve-notifications-v1.md) (proposal) |
| Shared interface sounds per skin, sourced from Fractos' v2 sound engine | [Theme sound contract](contracts/nu9ve-sound-v1.md) (proposal) |
| Review storage architecture and intended UX | [Architecture](media-architecture.md), [experience](media-experience.md) |
| Understand original requirements and legacy failures | [Audit](media-audit.md), [legacy importing](importing.md), [historical flow review](file-workflows.md) |
| Configure older downloads/stems/importer | [Configuration](configuration.md), [commands](commands.md) |
| Resume the current Sony/two-multicam acceptance | [Expected flow and pending evidence](sony-validation.md#aceptación-acompañada-sony-y-dos-multicams--2026-11-06) |
| Run a Sony hardware trial | [Sony validation](sony-validation.md) |

STATUS is the current snapshot. Architecture/experience contain design beyond shipped code. Historical pages retain their date/scope and link forward. Contracts say separately whether a producer, consumer, transport or schema is implemented.

Update the owner document when behavior changes; keep this index and the compact context short. Add a dated decision note only when a tradeoff needs history, then link it from the owner document. Avoid a second competing “master plan”.
