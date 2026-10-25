# Implementation status

Updated: 2026-10-25, America/Hermosillo. Snapshot taken at the user's request to stop visual testing and document continuity. Code exists in the working tree; it is **not a production release**. [Next priorities](../PLAN.md), [release gates](release.md).

## Implemented

| Capability | Owner | Current limit |
| --- | --- | --- |
| Safe `save`, explicit legacy command, project/profile/country/city | [save](../media/save.py), [entry](../nu9ve.py) | New flag syntax; old positional flow is `legacy-save`. New layout is not legacy naming parity. |
| Stable plan/digest, paired-file assignment, actual destination preview, required disks and space preflight | [planner](../media/planner.py) | Simple stem/sidecar pairing; DJI category explicit; date routing uses file mtime; no full EXIF/device registry. |
| Volume identity + physical-store overlap checks | [volumes](../media/volumes.py) | macOS diskutil/df; fixtures simulate devices. Exact cloned filesystem identity remains a limitation; other OS backends are incomplete. |
| Copy in bounded blocks, destination read-back SHA-256, per-file reattempts, mutation lock and journal | [transfer](../media/transfer.py), [IO](../media/common.py) | No multi-GB hardware benchmark or power-loss guarantee for every filesystem. No rollback promise. |
| Cleanup after all required receipts; interruption recovery | [transfer](../media/transfer.py) | Generic file cleanup only. Sony/Stalker handled conservatively; system directories are excluded. Not validated in a physical camera. |
| Explicit archive-only + pending/skipped work | [planner](../media/planner.py), [library](../media/library.py) | Registered expected work target required for pending. Later delivery is explicitly selected, not a silent bulk copy. |
| SQLite catalog, stable project IDs/rename aliases, asset/project associations | [catalog](../media/catalog.py) | Shared-asset resolution still primarily uses the initial job. Broader relink/location policy needs review. |
| Restore, work delivery, handoff export and archive receipt recovery | [library](../media/library.py), [recovery](../media/recovery.py) | Recovery restores receipt-time metadata, not every later local decision. Recovered work-location/ownership reconciliation needs more coverage. |
| Per-variant/channel publication metadata | [publication](../media/publication.py) | Explicit registration only; does not upload or automatically archive `subidos`. |
| Completed Stalker discovery and incremental index | [captures](../media/captures.py), [planner](../media/planner.py) | Completed manifest revisions tracked; no producer lock/lease or cross-app relink ACK. |
| Swift menubar, native window, assignment controls, plan review, cancel/resume, library and settings | [app](../native/MediaMenu/Sources/MediaMenu/main.swift) | Initial functional UI, not final UX/accessibility/performance signoff. |
| Native mount/wake events and configured Stalker FSEvents | [app](../native/MediaMenu/Sources/MediaMenu/main.swift), [watcher](../native/MediaMenu/Sources/MediaMenuCore/CaptureWatcher.swift) | Initial implementation; sustained real capture/idle tests and event-loss scenarios remain. No periodic idle scan. |
| Shared one-request JSON service; read-only MCP stdio | [service](../media/service.py), [MCP](../media/mcp.py) | MCP has no destructive tools or HTTP server. No installed external client integration yet. |
| App packaging helper | [builder](../scripts/build_media_app.py) | Development external runtime or supplied relocatable runtime; no provisioned release runtime/Developer ID/notarization. |

## Validation evidence

- Python: previous full suite **59 passed**. After adding recovery/capture cases, full suite ran **63 tests: 62 passed, 1 failed**. Failure was Python 3.9 incompatibility in `Path.stat(follow_symlinks=...)` in capture discovery. It was changed to `os.stat(...)`; **that failing test then passed individually**. No full-suite rerun after this final fix or documentation checkpoint.
- Failure fixtures include same-size corruption, missing/replaced/overlapping disks, insufficient space, symlinks, concurrent writer, changed source, partial cleanup and cancel/resume. Three real child-process hard exits exercise recovery during copy, after receipts and immediately after unlink.
- Native: latest recorded build completed; **5 Swift tests passed**, including a real Python RPC call with isolated state and exact 64-bit nanosecond roundtrip. Swift tools 6.3.3, target macOS 14+.
- UI smoke test: a development .app, isolated catalog and two tiny local fixture files. Inspected paired files, showed SSD absent, requested archive-only confirmation, applied verified copy with source retained and displayed work pending. No real SD cleanup. Latest FSEvents/routing refinements were compiled but not re-tested visually.
- Earlier wheel/schema/link checks belonged to the earlier inventory-only snapshot. They are **not** evidence that the latest full package/handoff is release-validated. Rebuild/validate current outputs before release.

## Artifacts and local test context

| Item | Meaning |
| --- | --- |
| `native/MediaMenu/.build/.../debug/MediaMenu` | Most recently compiled native source. Ignored build output. |
| `dist/Nu9veMedia-dev.app`, `dist/Nu9veMedia-review.app` | Ad-hoc development snapshots. Existing outputs are deliberately not overwritten by the builder. Neither is notarized/current-release quality. |
| `/private/tmp/ncreator-media-ui-20261025` | Isolated UI catalog. Never confuse with the normal user catalog. |
| `/private/tmp/ncreator-media-ui-fixture-20261025` | Tiny test source/archive/work folders; no personal media. |

The review app was opened using the isolated `NU9VE_MEDIA_HOME`; its development runtime was pointed at the checkout's `nu9ve.py`. A running app may therefore use newer Python than its bundled source snapshot. Rebuild a fresh artifact for reproducible review; do not distribute this session's test bundle.

## Release blockers / known gaps

1. Independent review of the data-loss boundaries and a fresh integrated test/package pass on the final snapshot. Physical SD/camera/ExFAT disconnect trials remain necessary.
2. Sony cleanup acceptance and samples/rules for the actual DJI models; full metadata dates and device persistence are not implemented.
3. Catalog backup of later human decisions, secondary-job location resolution, recovery/work ownership reconciliation and active-consumer leases need hardening. Receipt recovery is valuable but is not a complete backup policy for local SQLite.
4. Clipper consumer/relink is not implemented in its repo. Handoff remains a draft; no guarantee of queue/source relocation. Atlas/FCP integrations are design only.
5. No automatic archive of published deliveries or complete dashboard/launcher/onboarding. No runtime provisioning, notarization, Gatekeeper signoff or reproducible installer. Python package/native app versioning must be aligned before release.
6. Windows/Linux product support is not validated. Current secure IO/locking and native UI are macOS/Unix-oriented.

Do not use this status to authorize live media operations. Registration and backup runs must use the user's chosen disks and reviewed plans; automated tests stay in fixtures. No changes were made to the other repositories or sent to other chats.
