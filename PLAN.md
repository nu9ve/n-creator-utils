# Evolution plan

Current implementation/evidence lives in [STATUS](docs/STATUS.md); product invariants in [CONTEXT](docs/CONTEXT.md). Architecture and UX remain the design references, not a completed-feature list. This plan is the single prioritized backlog.

## General direction

1. Ship a trustworthy macOS ingest vertical slice: plan → verify required copies → clean SD, with archive-only exception and deferred work.
2. Make catalog, recovery, Stalker and editor handoffs dependable without moving old terabytes.
3. Grow the dashboard and launcher on the same service boundary.
4. Expand automation/API/MCP capabilities and platform adapters only after the data-loss boundary is validated.

## Now · next sessions (2026-10-25)

| When | Task |
| --- | --- |
| Done | Sony cycle closed 2026-10-25 (backup → camera format → `confirm-cleanup`). Camera format stays the Sony cleanup; see [sony-validation](docs/sony-validation.md). Next card types: H6, DJI, GoPro. |
| Next | Clipper session, then Stalker session: [prompts](docs/contracts/next-sessions.md). Edit `debloateando-mi-tele` in Clipper. |
| Tonight | Run `media coverage --work ESSD --archive MEDIA502` (read-only, hours). Review its report before any deletion. |
| Tomorrow | (3) Archive what is only on ESSD to MEDIA via verified jobs. (4) «Liberar trabajo» per project: delete ESSD copies only when verified on MEDIA. (5) Periodic MEDIA rehash + catalog export to MEDIA. Policy: [backup/work](docs/features/replication-policy.md). |
| First new card | Measure the parallel copy speed against the 829 s / 6.35 GB baseline. |

## P0 · close the current implementation

| Task | Acceptance / owner |
| --- | --- |
| Review plan validation, source cleanup races, receipt durability and replay | Independent review done 2026-10-25; durability, source re-read and metadata fixes applied. Remaining: authenticated/recorded plan digests, job owner/lease. `media/{planner,transfer,catalog}.py`. |
| Re-run the final snapshot and build a clean package | Full Python suite, Swift tests, current wheel contents, actual handoff schema/semantics and JSON CLI roundtrip. Record exact evidence in STATUS; no inherited green counts. |
| Harden recovery and the location graph | All relevant `asset_jobs`/locations can resolve; recovered work/ownership can reconcile without source SD; later human metadata has a durable backup/export strategy. `catalog/library/recovery`. |
| Physical acceptance | Sony procedure, H6 required SSD1/confirmed exception, DJI drone versus 360, ExFAT/HFS+ and ESSD reconnects using a small explicit trial. [Protocol](docs/sony-validation.md). |
| Freeze a release candidate | Align versions, bundle a verified relocatable runtime, package/sign/notarize and test Gatekeeper on a clean user. [Release gates](docs/release.md). |

Do not start by polishing more panels or testing full personal cards. Local fixture coverage precedes hardware; a hardware pass does not replace fault injection.

## P1 · complete daily workflows

| Feature | Done when |
| --- | --- |
| Device/model registry and capture timestamps | Metadata evidence + user-confirmed model/category persist. DJI ambiguity resolved per file/unit, without global brand or residual-directory guesses. Timezone/precision/fallback explicit. |
| Native onboarding and first-run disk setup | User selects registered archive/work roles and sees identity, free/reserved space and capabilities. No absent-disk fallback. |
| Stalker operational closeout | Real recording/finalizing/recovery/multi-session events, event loss/remount and CPU/IO contention validated; producer finalization/revision agreement documented. |
| Work delivery and restoration UX | Pending/skipped/ready clear by project/selection, preview before large jobs, offline requests retained, order preserved. |
| Clipper handoff/relink adoption | Consumer accepts the library contract and ACKs complete session/voice/tracks/queue updates. No direct JSON edits here. Coordinated in Clipper's repo. |
| Work release and single-archive safeguards | Per [policy](docs/features/replication-policy.md) (MEDIA = backup, ESSD = work, decided 2026-10-25): coverage audit, archive ESSD-only material, explicit per-project release, MEDIA rehash and catalog export. A second replica stays a future option. |
| Stalker/Clipper naming adoption | Stalker names sessions per [naming v1](docs/contracts/media-naming-v1.md) at finalization; Clipper library picker keeps capture order and resolves bundles by manifest/IDs. Coordinated in their repos. |
| Deliveries and published archive | Bundle chosen variants/thumbnails/copy; explicit per-channel publication; `subidos` convention only a configurable suggestion. Idempotent verified archive into final collections. |

## P2 · dashboard, launcher and maintainability

Detailed, prioritized ideas: [dashboard/launcher backlog](docs/features/dashboard-launcher.md).

- Action center for blocked jobs, missing destinations, work pending and final deliveries pending archive.
- Storage/coverage view for logical versus allocated bytes, online/offline snapshots, replication and space reservation.
- Compact launcher/menubar commands with last-project choice, launch at login, real notifications and runtime health.
- Diagnostics bundle with redaction, version inventory, job events and copyable retry command; log/cache quotas.
- Imports/search for historical archives without bulk rename. Profiles/aliases and saved collections reusable by other surfaces.
- Downloads/stems as isolated derived-asset jobs; do not grow the media worker into another editor.

## P3 · broader automation and platforms

Read-only MCP stdio already exists. Mutations need reviewed-plan IDs/digests, clear client approval/cancel semantics and common error/progress contracts; do not expose arbitrary shell/filesystem tools.

Windows/Linux require their own volume identity/events/safe IO/locking, path and filesystem compatibility, packaging and hardware acceptance. The current native UI is macOS-only. Future HTTP/multi-user catalogs are not prerequisites for a local dashboard.

## Parallel sessions / task ownership

Use [development handoff](docs/development.md#cerrar-y-retomar-una-sesión). Choose one vertical feature/module, state files in scope, contract changes and validation. Separate UI work from transfer integrity review; shared protocol/catalog changes need coordination. Respect current staged/unstaged work. Update STATUS and this plan when a priority actually changes.
