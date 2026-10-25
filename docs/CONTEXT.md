# Compact project context

Snapshot: 2026-10-25, America/Hermosillo. Start here, then [STATUS](STATUS.md) and [PLAN](../PLAN.md). This is a handoff, not a claim of release readiness.

## Product and user workflow

n-creator-utils manages creative media: SD ingest, verified archive, working copies, discovery/catalog and retrieval. macOS is first. Stalker captures; Clipper edits/syncs/exports; Final Cut finishes manual edits; Atlas consumes drone images. Downloads/stems remain separate utility arms; the old embedded Clipper was removed.

Typical video flow: SD → MEDIA archive + ESSD working raw → Clipper → Final Cut → finals → published → MEDIA finals. Audio normally needs MEDIA + SSD1. Projects/profiles can be mixed on one card; project/country/city/profile are independent fields. Preserve selection order, sidecars and complete Stalker bundles.

Existing disk layout should remain useful: ESSD `001_proyectos/e64_raw`, `001_proyectos/e64`, `002_finales`; MEDIA502 `001_raw`, `002_finales`, `003_curado`. Do not reorganize old TBs to introduce the catalog. The new layout preserves source-relative paths under per-job/per-unit containers; legacy normalized naming is not automatic parity.

## Product decisions to preserve

- `save` should finish with the SD free for new projects. Copy → verify all effective required destinations → journaled cleanup. Copy-only is optional/diagnostic, not the intended default for a card.
- If MEDIA exists but ESSD/SSD1 is absent: explicitly confirm archive-only for that operation; record work pending or deliberately skipped. Later copy from archive, without the old SD. Do not silently make this exception the profile default.
- No registered archive/work destination online: SD insertion is silent, with no deep automatic scan. Manual inspect is still allowed.
- A completed backup, work readiness, publication and source readiness are different states. Never infer two replicas from two folders on one physical disk.
- Sony currently ends `camera_action_required`; no proven camera-safe automated index cleanup. DJI drone/360/action ambiguity requires an explicit category. No persistent device/model registry yet.
- Stalker must be completed; originals/raw microphone/events stay together. Work with active recording must not copy an incomplete session. Archive does not rewrite Clipper references.
- No silent media staging on the internal drive. Local catalog/journal metadata is expected; local media destinations require explicit registration/choice.

## Code map

```text
nu9ve.py → media/save.py (human save) / media/cli.py (JSON CLI/RPC/MCP)
media/service.py → planner + volumes + transfer + catalog + library
media/common.py → safe tree-relative IO, hashes, errors, JSON
native/MediaMenu → Swift UI / Process JSON bridge / FSEvents
manager/file.py → legacy-save + existing downloads/stems
```

Use [modules](modules.md), [operations](operations.md) and [development](development.md) for details. Contracts are indexed [here](contracts/README.md); avoid new direct writes to an editor's JSON.

## Current working state

Implementation exists in the checkout, including new untracked source directories; earlier work is also staged. No commit/release has been made. `.vscode/` was pre-existing and is unrelated. No personal SD was cleaned and no MEDIA/ESSD was used as a test destination.

The user requested stopping visual exploration and documenting this checkpoint. The next session should start with release blockers/review, not more dashboard polish or an unconditional live backup. Exact tests, artifacts and known limitations are in [STATUS](STATUS.md).
