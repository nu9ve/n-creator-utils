#if DEBUG
import AppKit
import SwiftUI
import MediaMenuCore

/// Debug-only visual QA without screen-recording permission: `--qa-snapshots DIR` renders each
/// route/step with fixture data to PNG; `--qa-fullscreen` logs whether the toolbar stays visible.
@MainActor enum QASnapshots {
    static func run(_ delegate: AppDelegate) -> Bool {
        let arguments = CommandLine.arguments
        if let index = arguments.firstIndex(of: "--qa-closeout"), index + 1 < arguments.count {
            Task { await projectCloseout(delegate, URL(fileURLWithPath: arguments[index + 1])) }
            return true
        }
        if let index = arguments.firstIndex(of: "--qa-snapshots"), index + 1 < arguments.count {
            Task { await snapshots(delegate, URL(fileURLWithPath: arguments[index + 1])) }
            return true
        }
        if let index = arguments.firstIndex(of: "--qa-ingest"), index + 1 < arguments.count {
            Task { await ingest(delegate, URL(fileURLWithPath: arguments[index + 1])) }
            return true
        }
        if let index = arguments.firstIndex(of: "--qa-disks"), index + 1 < arguments.count {
            Task { await disks(delegate, URL(fileURLWithPath: arguments[index + 1])) }
            return true
        }
        if arguments.contains("--qa-titlebar") {
            Task { await titleBar(delegate) }
            return true
        }
        if let index = arguments.firstIndex(of: "--qa-finals"), index + 1 < arguments.count {
            Task { await finalsRoute(delegate, URL(fileURLWithPath: arguments[index + 1])) }
            return true
        }
        if let index = arguments.firstIndex(of: "--qa-projects"), index + 1 < arguments.count {
            Task { await projects(delegate, URL(fileURLWithPath: arguments[index + 1])) }
            return true
        }
        if arguments.contains("--qa-fullscreen") {
            Task { await fullscreen(delegate) }
            return true
        }
        return false
    }

    static func fixture(_ model: Model, media: URL) {
        model.captureTask?.cancel()  // a slow first read must not replace the fixture
        model.volumes = [
            .object(["id": .string("a"), "name": .string("MEDIA502"), "role": .string("archive"), "online": .bool(true), "free": .number(1.8e12), "root": .string("/Volumes/MEDIA502"), "prefix": .string("001_raw")]),
            .object(["id": .string("w"), "name": .string("ESSD"), "role": .string("work"), "online": .bool(true), "free": .number(4.2e11), "root": .string("/Volumes/ESSD"), "prefix": .string("001_proyectos")]),
            .object(["id": .string("s"), "name": .string("SSD1"), "role": .string("work"), "online": .bool(false), "free": .null, "root": .string("/Volumes/SSD1"), "prefix": .string("001_proyectos")])]
        model.archive = "a"; model.work = "w"
        model.sourceCards = [
            .object(["path": .string(media.path), "name": .string("A7IV_64"), "hint": .object(["family": .string("sony"), "recommendation": .string("sony_format_in_camera"), "used_bytes": .number(38e9), "capacity_bytes": .number(64e9), "last_project": .string("mercado-norte")])]),
            .object(["path": .string("/Volumes/DJI"), "name": .string("MINI4"), "hint": .object(["family": .string("dji"), "recommendation": .string("dji_choose_category"), "used_bytes": .number(12e9), "capacity_bytes": .number(128e9)])])]
        model.cardSuggestions = model.sourceCards.map { $0["path"].string }
        model.captureRoots = ["/Users/demo/Stalker"]
        let stages = ["draft", "in_progress", "in_progress", "backed_up", "backed_up", "finished", "backed_up"]
        model.projectIndex = ["mercado-norte", "taller-luz", "entrevista-ana", "clip-intro", "viaje-sonora", "podcast-07", "demo"].enumerated().map { index, title in
            .object(["key": .string(index == 0 ? "draft:" + title : "p\(index)"), "project_id": .string(index == 0 ? "" : "p\(index)"), "project": .string(title), "profile": .string(index == 0 ? "" : "e64"),
                     "stage": .string(stages[index]), "state": .string(index == 5 ? "finished" : "active"), "assets": .integer(index == 0 ? 0 : Int64(index * 7)), "pending_sessions": .integer(index < 2 ? 3 : 0),
                     "work_pending": .integer(index == 2 ? 4 : 0), "work_ready": .integer(index > 0 ? 6 : 0), "work_skipped": .integer(index > 2 ? 12 : 0), "jobs": .integer(Int64(index)), "open_jobs": .integer(0),
                     "kinds": .object(index == 0 ? [:] : ["stalker_session": .integer(2), "camera_bundle": .integer(Int64(index * 2)), "file": .integer(Int64(index * 5))]), "cameras": .array([.string("ILCE-6400")]), "places": .array([.string("mx/hermosillo")]),
                     "aliases": .array(index == 3 ? [.string("clip-viejo")] : []), "preview": .object([:]), "updated_at": .string("2026-10-2\(9 - index)T18:00:00.000000Z")]) }
        model.pendingCaptures = (0..<3).map { index in
            .object(["path": .string("/Users/demo/Stalker/Stalker_2026-11-01_1\(index)-20-08.stalker"), "root": .string("/Users/demo/Stalker"), "session_id": .string("00000000-0000-4000-8000-00000000000\(index)"), "status": .string("completed"), "archived": .bool(false),
                     "created_at_local": .string("2026-11-01T1\(index):20-07:00"), "duration_seconds": .number(125), "track_count": .integer(3), "preview_path": .string("screen.mov"),
                     "tag": index == 0 ? .object(["name": .string("mercado-norte"), "country": .string("mx")]) : .object([:]), "project_hint": .object([:])]) }
        model.lastTagged = "mercado-norte"
        model.finals = .object(["archive": .string("MEDIA502"), "groups": .array([.object(["work": .string("ESSD"), "profile": .string("e64"), "folder": .string("/Volumes/ESSD/002_finales/e64"), "destination": .string("MEDIA502/002_finales/e64"), "published_folder": .string("03_subidos"), "subfolders": .array([.string("02_shorts"), .string("02_thumbnails"), .string("03_subidos")]), "total": .integer(74), "published_total": .integer(3), "pending": .integer(3), "pending_bytes": .number(2.4e9), "waiting": .integer(42), "waiting_bytes": .number(1.09e10), "in_archive": .integer(29), "in_archive_bytes": .number(8e9), "files": .array([.object(["path": .string("/Volumes/ESSD/002_finales/e64/02_shorts/corto_1.mov"), "name": .string("02_shorts/corto_1.mov"), "bytes": .number(8e8)])])])])])
        model.finalsCheckedAt = Date()
        model.profiles = [.object(["id": .string("e64"), "name": .string("erikiado64"), "clipper_profile": .string("erikiado64"), "raw_dir": .string("e64_raw")]), .object(["id": .string("9logs"), "name": .string("9logs"), "raw_dir": .string("9logs_raw")])]
        model.assets = (0..<14).map { index in .object(["id": .string("asset\(index)"), "project": .string(["mercado-norte", "taller-luz", "entrevista-ana"][index % 3]), "profile": .string("e64"), "kind": .string(index % 4 == 0 ? "camera_bundle" : "file"), "camera": .string(index % 2 == 0 ? "ILCE-6400" : "dji-drone"), "country": .string("mx"), "city": .string("hermosillo"), "paths": .array([.string("a"), .string("b")]), "work_delivery_status": .string(["pending", "ready", "skipped"][index % 3])]) }
        model.jobs = [
            .object(["id": .string("job-1"), "state": .string("copying"), "updated_at": .string("2026-10-29T10:12:00"), "cleanup_profile": .string("files")]),
            .object(["id": .string("job-2"), "state": .string("camera_action_required"), "updated_at": .string("2026-10-28T18:40:00.123456Z"), "cleanup_profile": .string("sony_clips_then_format"), "source_kind": .string("card")]),
            .object(["id": .string("job-3"), "state": .string("completed"), "updated_at": .string("2026-10-27T09:02:00"), "cleanup_profile": .string("files")])]
        let files = (try? FileManager.default.contentsOfDirectory(atPath: media.path))?.sorted() ?? []
        model.source = media.path
        model.units = files.enumerated().map { index, name in
            var unit = DraftUnit(id: name, paths: [name])
            unit.category = name.lowercased().hasSuffix(".mov") || name.lowercased().hasSuffix(".mp4") ? "video" : "image"
            unit.detectedCamera = index == 1 ? "dji" : "ILCE-6400"
            unit.bytes = Double(120_000_000 + index * 9_000_000)
            unit.copyForEditing = index % 2 == 0
            if index == 2 { unit.project = "taller-luz" }
            return unit
        }
        model.loadedSourceKey = model.sourceKey
        model.gridSelection = Set(files.prefix(2))
        model.project = "mercado-norte"
    }

    /// Invented folders and sizes, for the look only: the real reading goes through the engine.
    static func diskFixture(_ model: Model) {
        model.storageFixture = true
        func folder(_ name: String, _ gb: Double, _ safety: [String: Double], kind: String = "", opaque: Bool = false) -> JSONValue {
            .object(["path": .string(name), "name": .string(name), "files": .integer(Int64(gb * 40)), "allocated": .number(gb * 1e9), "bytes": .number(gb * 1e9), "kind": .string(kind), "opaque": .bool(opaque),
                     "kinds": .object(["video": .number(gb * 0.8e9), "audio": .number(gb * 0.2e9)]), "safety": .object(safety.mapValues { .number($0 * 1e9) })])
        }
        model.storageTargets = [
            .object(["id": .string("w"), "name": .string("ESSD"), "role": .string("work"), "root": .string("/Volumes/ESSD"), "online": .bool(true), "indexed": .bool(true), "observed": .bool(false), "indexed_at": .string("2026-10-30T10:00:00.000000Z"), "files": .integer(80061), "allocated": .number(445e9), "fs_total": .number(465e9), "fs_free": .number(20e9), "uncounted": .number(2e9), "unreadable": .integer(2), "samples": .array([.string(".Spotlight-V100")]),
                     "safety": .object(["only_here": .number(130e9), "probable": .number(60e9), "unknown": .number(150e9), "verified": .number(85e9), "derived": .number(20e9)]),
                     "kinds": .object(["video": .number(310e9), "image": .number(40e9), "audio": .number(35e9), "editable": .number(40e9), "other": .number(20e9)])]),
            .object(["id": .string("a"), "name": .string("MEDIA502"), "role": .string("archive"), "root": .string("/Volumes/MEDIA502"), "online": .bool(false), "indexed": .bool(true), "observed": .bool(false), "fs_total": .number(5e12), "fs_free": .number(2.7e12)]),
            .object(["id": .string("new"), "name": .string("Música"), "role": .string("observed"), "root": .string("/Users/demo/Music"), "online": .bool(true), "indexed": .bool(false), "observed": .bool(true), "fs_total": .number(1e12), "fs_free": .number(12e9)])]
        model.storageTargetID = "w"; model.storagePath = ""; model.storageSelection = "acomodar"
        model.storageTree = .object(["name": .string("ESSD"), "allocated": .number(445e9), "rest": .number(4e9), "safety": model.storageTargets[0]["safety"], "files": .array([
            .object(["path": .string("entrevista.mov"), "name": .string("entrevista.mov"), "allocated": .number(12e9), "kind": .string("video"), "state": .string("verified"), "copy": .object(["volume": .string("MEDIA502"), "path": .string("001_raw/videos/entrevista.mov"), "origin": .string("computed")])])]),
            "folders": .array([folder("proyectos", 230, ["only_here": 80, "verified": 85, "probable": 45, "derived": 20]), folder("acomodar", 140, ["unknown": 140]),
                               folder("finales", 43, ["only_here": 28, "probable": 15]), folder("recuerdos", 14, ["only_here": 14]), folder("edicion.fcpbundle", 2, ["only_here": 2], kind: "editable", opaque: true)])])
    }

    static func render<V: View>(_ view: V, size: NSSize, to url: URL) async {
        let window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.borderless], backing: .buffered, defer: false)
        let host = NSHostingView(rootView: view.modifier(CreatorAppearance()).frame(width: size.width, height: size.height))
        host.frame = NSRect(origin: .zero, size: size)
        window.contentView = host
        for _ in 0..<12 { host.layoutSubtreeIfNeeded(); try? await Task.sleep(for: .milliseconds(150)) }
        guard let bitmap = host.bitmapImageRepForCachingDisplay(in: host.bounds) else { return }
        host.cacheDisplay(in: host.bounds, to: bitmap)
        try? bitmap.representation(using: .png, properties: [:])?.write(to: url)
    }

    /// Seeded disposable catalog + fixture-volume RPC; exercises the same model actions as the installed UI.
    static func projectCloseout(_ delegate: AppDelegate, _ folder: URL) async {
        let model = delegate.model
        func wait(_ label: String, _ condition: @MainActor () -> Bool) async -> Bool {
            for _ in 0..<300 {
                if condition() { return true }
                try? await Task.sleep(for: .milliseconds(100))
            }
            print("QA closeout FAILED \(label): \(model.error)"); return false
        }
        try? await Task.sleep(for: .seconds(1))
        await model.refresh()
        guard await wait("projects", { !model.projectIndex.isEmpty }) else { NSApp.terminate(nil); return }
        guard let project = model.projectIndex.first(where: { $0["project"].string == "demo-cierre" }) else { print("QA closeout FAILED missing fixture"); NSApp.terminate(nil); return }
        model.openProjectClosure(project)
        guard await wait("candidates", { !model.busy && model.closureCandidates.count == 3 }) else { NSApp.terminate(nil); return }
        model.showClosure = false
        await render(ProjectClosureSheet(model: model), size: NSSize(width: 820, height: 680), to: folder.appendingPathComponent("closeout-choose.png"))
        model.saveProjectClosure(required: ["horizontal", "vertical", "thumbnail"], noMore: true, published: true,
                                 finals: model.closureCandidates, profileDefault: false, review: true)
        guard await wait("review", { !model.busy && model.closurePlan != nil }) else { NSApp.terminate(nil); return }
        print("QA closeout review entries=\(model.closurePlan!["entries"].array.count) copies=\(model.closurePlan!["closure_copies"].array.count) blockers=\(model.closurePlan!["consumers"]["blockers"].array)")
        await render(ProjectClosureSheet(model: model), size: NSSize(width: 820, height: 680), to: folder.appendingPathComponent("closeout-review.png"))
        model.applyProjectClosure()
        guard await wait("apply", { !model.busy && model.closurePlan?["state"].string == "completed" }) else { NSApp.terminate(nil); return }
        await model.refresh()
        await render(ProjectClosureSheet(model: model), size: NSSize(width: 820, height: 680), to: folder.appendingPathComponent("closeout-done.png"))
        let editor = folder.appendingPathComponent("work/001_proyectos/e64/demo-cierre")
        let kept = FileManager.default.fileExists(atPath: editor.appendingPathComponent("clipper/clipper-project.json").path)
        let removed = !FileManager.default.fileExists(atPath: editor.appendingPathComponent("renders/intermediate.mov").path)
        print("QA closeout \(kept && removed ? "OK" : "FAILED") editableKept=\(kept) rendersRemoved=\(removed) reminders=\(model.projectClosures.filter { $0["needs_release"].bool }.count)")
        NSApp.terminate(nil)
    }

    static func snapshots(_ delegate: AppDelegate, _ folder: URL) async {
        let model = delegate.model
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        try? await Task.sleep(for: .seconds(2))  // let the isolated snapshot refresh settle first
        fixture(model, media: folder.appendingPathComponent("media"))
        let full = NSSize(width: 1180, height: 860), selection = NSSize(width: 780, height: 820), compact = NSSize(width: 440, height: 700)
        model.presentation = .full
        model.route = .home
        await render(MainView(model: model), size: full, to: folder.appendingPathComponent("route-home.png"))
        await render(InboxView(model: model), size: NSSize(width: 1180, height: 1500), to: folder.appendingPathComponent("route-home-full.png"))
        await render(VStack { ForEach(model.projectIndex.prefix(4), id: \.["key"].string) { ProjectRow(model: model, item: $0, expanded: .constant(true)) } }.padding(20).frame(maxHeight: .infinity, alignment: .top), size: NSSize(width: 900, height: 500), to: folder.appendingPathComponent("project-details.png"))
        model.route = .flows
        for flow in model.productFlows {
            model.flowSelection = flow.id
            await render(MainView(model: model), size: full, to: folder.appendingPathComponent("flows-\(flow.id).png"))
        }
        model.route = .library
        for section in LibrarySection.allCases where section != .disks {
            model.librarySection = section
            await render(MainView(model: model), size: full, to: folder.appendingPathComponent("library-\(section.rawValue).png"))
        }
        model.librarySection = .disks
        diskFixture(model)
        await render(MainView(model: model), size: full, to: folder.appendingPathComponent("library-disks.png"))
        model.storageTargetID = "new"; model.storageTree = nil
        await render(MainView(model: model), size: full, to: folder.appendingPathComponent("library-disks-empty.png"))
        model.route = .ingest
        for step in IngestStep.allCases where step != .run {
            model.ingestStep = step
            await render(MainView(model: model), size: full, to: folder.appendingPathComponent("ingest-\(step.rawValue)-\(step).png"))
        }
        model.ingestStep = .run; model.activeScope = .ingest; model.activeJobID = "job-1"
        model.jobProgress = .object(["state": .string("copying"), "progress": .object(["phase": .string("copying"), "copied_bytes": .number(21e9), "bytes_total": .number(38e9), "bytes_per_second": .number(180e6), "files_done": .integer(120), "files_total": .integer(212),
            "destinations": .array([.object(["name": .string("MEDIA502"), "files_done": .integer(120), "files_total": .integer(212)]), .object(["name": .string("ESSD"), "files_done": .integer(96), "files_total": .integer(110)])])])])
        await render(MainView(model: model), size: full, to: folder.appendingPathComponent("ingest-4-run.png"))
        model.activeScope = nil
        model.presentation = .compact
        for step in [IngestStep.source, .destinations] {
            model.ingestStep = step
            await render(WindowRoot(model: model), size: compact, to: folder.appendingPathComponent("compact-\(step).png"))
        }
        model.presentation = .selection; model.ingestStep = .content
        await render(WindowRoot(model: model), size: selection, to: folder.appendingPathComponent("selection-content.png"))
        model.presentation = .full
        for width in [1400.0, 1180.0, 900.0, 700.0, 430.0] {
            model.presentation = width < 600 ? .compact : .full
            await render(WindowTopBar(model: model), size: NSSize(width: width, height: 30), to: folder.appendingPathComponent("topbar-\(Int(width)).png"))
        }
        await render(SettingsView(model: model), size: NSSize(width: 760, height: 600), to: folder.appendingPathComponent("settings.png"))
        UserDefaults.standard.set("appearance", forKey: "settingsSection")
        await render(SettingsView(model: model), size: NSSize(width: 760, height: 600), to: folder.appendingPathComponent("settings-appearance.png"))
        UserDefaults.standard.removeObject(forKey: "settingsSection")
        // README image: compact window, fixture data only (no real libraries, sessions or paths).
        model.captureRoots = ["/Users/demo/Stalker"]; model.pendingCaptures = []; model.captureSessions = []; model.finals = nil
        if model.units.count > 1 { model.units[1].camera = "dji-drone" }
        model.gridSelection = []; model.error = ""; model.route = .ingest; model.presentation = .compact; model.ingestStep = .source
        await render(ReadmeFrame(model: model), size: NSSize(width: 440, height: 730), to: folder.appendingPathComponent("readme-compact.png"))
        // Empty states: nothing connected, nothing backed up.
        model.storageTargets = []; model.storageTargetID = ""; model.storageTree = nil; model.assets = []; model.jobs = []; model.projectIndex = []
        model.sourceCards = []; model.cardSuggestions = []; model.pendingCaptures = []; model.captureRoots = []; model.source = ""; model.units = []
        model.presentation = .full; model.route = .home
        await render(MainView(model: model), size: full, to: folder.appendingPathComponent("empty-home.png"))
        model.route = .library
        for section in LibrarySection.allCases {
            model.librarySection = section
            await render(MainView(model: model), size: full, to: folder.appendingPathComponent("empty-\(section.rawValue).png"))
        }
        model.route = .ingest; model.ingestStep = .content
        await render(MainView(model: model), size: full, to: folder.appendingPathComponent("empty-content.png"))
        print("QA snapshots written to \(folder.path)")
        NSApp.terminate(nil)
    }

    /// Real engine and real Model paths on whatever `NU9VE_MEDIA_HOME` registers: scans each online disk, read-only.
    static func disks(_ delegate: AppDelegate, _ folder: URL) async {
        let model = delegate.model
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        model.python = "/usr/bin/python3"; model.script = ProcessInfo.processInfo.environment["NUUN_QA_SCRIPT"] ?? ""
        model.configure()
        await model.refresh()
        model.route = .library; model.librarySection = .disks; model.presentation = .full
        await model.loadStorage()
        let size = NSSize(width: 1180, height: 860)
        for target in model.storageTargets where target["online"].bool {
            model.selectStorageTarget(target["id"].string, reload: false)
            model.scanStorage()
            for _ in 0..<1200 where model.storageRunning { try? await Task.sleep(for: .milliseconds(250)) }
            print("QA disks \(target["name"].string) error=\(model.storageError) folders=\(model.storageTree?["folders"].array.count ?? -1)")
            let slug = target["name"].string
            await render(MainView(model: model), size: size, to: folder.appendingPathComponent("disks-\(slug)-safety.png"))
            if let first = model.storageTree?["folders"].array.first {
                model.openStorageFolder(first["path"].string)
                try? await Task.sleep(for: .seconds(1))
                await render(MainView(model: model), size: size, to: folder.appendingPathComponent("disks-\(slug)-folder.png"))
            }
        }
        NSApp.terminate(nil)
    }

    /// Real engine, isolated state, local fixture folders: source → review → run → result.
    static func ingest(_ delegate: AppDelegate, _ folder: URL) async {
        let model = delegate.model
        func wait(_ label: String, _ condition: @MainActor () -> Bool) async -> Bool {
            for _ in 0..<300 { if condition() { return true }; try? await Task.sleep(for: .milliseconds(100)) }
            print("QA ingest timeout: \(label) error=\(model.error) message=\(model.message)"); return false
        }
        do {
            try? await Task.sleep(for: .seconds(1))
            for (name, role) in [("QA-MEDIA", "archive"), ("QA-ESSD", "work")] {
                let root = folder.appendingPathComponent(name)
                try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
                _ = try await model.client!.call("register_volume", ["root": .string(root.path), "name": .string(name), "role": .string(role), "allow_local": .bool(true)])
            }
            await model.refresh()
            model.archive = model.volumes.first { $0["role"].string == "archive" }?["id"].string ?? ""
            model.work = model.volumes.first { $0["role"].string == "work" }?["id"].string ?? ""
            print("QA ingest volumes=\(model.volumes.count) online=\(model.volumes.filter { $0["online"].bool }.count)")
            model.route = .ingest
            model.prepareSource(folder.appendingPathComponent("card").path, kind: "card")
            model.cleanup = false; model.copyChoice = "none"; model.project = "qa-flujo"
            print("QA ingest blockers=\(model.planBlockers)")
            model.goToStep(.content)
            guard await wait("discover", { model.contentLoaded && !model.discoveryLoading }) else { NSApp.terminate(nil); return }
            print("QA ingest units=\(model.units.count)")
            model.gridSelection = [model.units[0].id]
            model.updateTargets { $0.project = "qa-otro" }
            model.reviewPlan()
            // Archive-only is confirmed explicitly per operation, as the sheet does.
            guard await wait("confirmation", { model.archiveOnlyPrompt }) else { NSApp.terminate(nil); return }
            model.archiveOnlyPrompt = false
            model.makePlan(archiveOnly: true)
            guard await wait("plan", { model.plan != nil && !model.busy }) else { NSApp.terminate(nil); return }
            print("QA ingest step=\(model.ingestStep) files=\(model.plan?["files"].array.count ?? 0) projects=\(Set(model.plan?["units"].array.map { $0["project"].string } ?? []).sorted())")
            model.apply()
            guard await wait("apply", { model.lastResult != nil && !model.busy }) else { NSApp.terminate(nil); return }
            print("QA ingest result=\(model.lastResult?["state"].string ?? "") label=\(model.jobLabel(model.lastResult!)) step=\(model.ingestStep) error=\(model.error)")
            await model.refresh()
            print("QA ingest library=\(model.assets.count) jobs=\(model.jobs.count) pendingWork=\(model.assets.filter { $0["work_delivery_status"].string == "pending" }.count)")
        } catch { print("QA ingest failed: \(error)") }
        NSApp.terminate(nil)
    }

    /// Real window and AppKit dispatch: what a double click on an empty part of the bar reaches, and what it does.
    static func titleBar(_ delegate: AppDelegate) async {
        delegate.show()
        try? await Task.sleep(for: .seconds(1))
        guard let window = delegate.window, let frameView = window.contentView?.superview else { NSApp.terminate(nil); return }
        func doubleClick(at point: NSPoint) async {
            for (type, count) in [(NSEvent.EventType.leftMouseDown, 1), (.leftMouseUp, 1), (.leftMouseDown, 2), (.leftMouseUp, 2)] where count == 2 {
                if let event = NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: count, pressure: 1) { window.sendEvent(event) }
            }
            try? await Task.sleep(for: .seconds(1))
        }
        let row = window.frame.height - 15
        for (label, x) in [("empty", 170.0), ("right", window.frame.width * 0.62)] {
            let point = NSPoint(x: x, y: row)
            let start = window.frame
            print("QA titlebar \(label) hit=\(frameView.hitTest(point).map { String(describing: type(of: $0)) } ?? "nil")")
            await doubleClick(at: point)
            let zoomed = window.frame
            await doubleClick(at: NSPoint(x: x, y: window.frame.height - 15))
            print("QA titlebar \(label) start=\(Int(start.width))x\(Int(start.height)) afterFirst=\(Int(zoomed.width))x\(Int(zoomed.height)) afterSecond=\(Int(window.frame.width))x\(Int(window.frame.height))")
        }
        // Full click sequences along the row, as real hands make them: a tab switches, empty space zooms, nothing does both.
        func send(_ type: NSEvent.EventType, _ x: CGFloat, _ count: Int) {
            if let event = NSEvent.mouseEvent(with: type, location: NSPoint(x: x, y: window.frame.height - 15), modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: count, pressure: 1) { window.sendEvent(event) }
        }
        for x in stride(from: CGFloat(120), to: window.frame.width - 60, by: 50) {
            delegate.model.route = .home
            let before = window.frame
            send(.leftMouseDown, x, 1); send(.leftMouseUp, x, 1)
            try? await Task.sleep(for: .milliseconds(150))
            let single = delegate.model.route
            send(.leftMouseDown, x, 2); send(.leftMouseUp, x, 2)
            try? await Task.sleep(for: .milliseconds(700))
            let zoomed = window.frame != before
            print("QA titlebar x=\(Int(x)) single=\(single) route=\(delegate.model.route) zoomed=\(zoomed)")
            if zoomed { window.zoom(nil); try? await Task.sleep(for: .milliseconds(500)) }
        }
        NSApp.terminate(nil)
    }

    /// Real engine, isolated state, invented files in temporary folders: detect → plan → review → copy and verify → settled.
    static func finalsRoute(_ delegate: AppDelegate, _ folder: URL) async {
        let model = delegate.model
        func wait(_ label: String, _ condition: @MainActor () -> Bool) async -> Bool {
            for _ in 0..<300 { if condition() { return true }; try? await Task.sleep(for: .milliseconds(100)) }
            print("QA finals FAILED \(label) error=\(model.error) message=\(model.message)"); return false
        }
        do {
            try? await Task.sleep(for: .seconds(1))
            for (name, role) in [("QA-MEDIA", "archive"), ("QA-ESSD", "work")] {
                let root = folder.appendingPathComponent(name)
                try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
                _ = try await model.client!.call("register_volume", ["root": .string(root.path), "name": .string(name), "role": .string(role), "allow_local": .bool(true)])
            }
            let finals = folder.appendingPathComponent("QA-ESSD/002_finales/e64")
            for (path, size) in [("03_subidos/26-10-27-master.mov", 300), ("03_subidos/shorts/corto.mov", 200), ("02_shorts/en-uso.mov", 400)] {
                let file = finals.appendingPathComponent(path)
                try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
                try Data(count: size).write(to: file)
            }
            await model.refresh()
            model.archive = model.volumes.first { $0["role"].string == "archive" }?["id"].string ?? ""
            model.work = model.volumes.first { $0["role"].string == "work" }?["id"].string ?? ""
            model.checkFinals(force: true)
            guard await wait("status", { !model.finalsChecking && !(model.finals?["groups"].array.isEmpty ?? true) }) else { NSApp.terminate(nil); return }
            let group = model.finals!["groups"].array[0]
            print("QA finals status folder=\(group["published_folder"].string) due=\(group["pending"].double) waiting=\(group["waiting"].double)")
            model.planFinals(group)
            guard await wait("same disk refused", { !model.busy && !model.error.isEmpty }) else { NSApp.terminate(nil); return }
            print("QA finals moveOnOneDisk error=\(model.error) copiedAnything=\(FileManager.default.fileExists(atPath: folder.appendingPathComponent("QA-MEDIA/002_finales").path))")
            model.error = ""; model.finalsKeepSource = true
            model.planFinals(group)
            guard await wait("plan", { model.plan != nil && !model.busy }) else { NSApp.terminate(nil); return }
            print("QA finals plan step=\(model.ingestStep) files=\(model.plan!["files"].array.count) cleanup=\(model.plan!["cleanup"].bool) kind=\(model.plan!["source_kind"].string)")
            print("QA finals routes=\(model.plan!["preview"].array.map { String($0["archive_path"].string.split(separator: "/").suffix(4).joined(separator: "/")) })")
            await render(MainView(model: model), size: NSSize(width: 1180, height: 860), to: folder.appendingPathComponent("finals-review.png"))
            // Reviewing again has to stay on the finals route.
            model.plan = nil; model.reviewedPlan = nil; model.reviewPlan()
            guard await wait("replan", { model.plan != nil && !model.busy }) else { NSApp.terminate(nil); return }
            print("QA finals replan layout=\(model.plan!["layout"].object.count)")
            model.apply()
            guard await wait("apply", { model.lastResult != nil && !model.busy }) else { NSApp.terminate(nil); return }
            print("QA finals result=\(model.lastResult?["state"].string ?? "") label=\(model.jobLabel(model.lastResult!)) error=\(model.error)")
            let target = folder.appendingPathComponent("QA-MEDIA/002_finales/e64")
            print("QA finals copied master=\(FileManager.default.fileExists(atPath: target.appendingPathComponent("2026/26-10-27-master.mov").path)) short=\(FileManager.default.fileExists(atPath: target.appendingPathComponent("shorts/corto.mov").path)) waitingCopied=\(FileManager.default.fileExists(atPath: target.appendingPathComponent("02_shorts/en-uso.mov").path)) sourceKept=\(FileManager.default.fileExists(atPath: finals.appendingPathComponent("03_subidos/shorts/corto.mov").path))")
            await render(MainView(model: model), size: NSSize(width: 1180, height: 860), to: folder.appendingPathComponent("finals-done.png"))
            model.checkFinals(force: true)
            guard await wait("settled", { !model.finalsChecking && model.finals!["groups"].array[0]["pending"].double == 0 }) else { NSApp.terminate(nil); return }
            await model.refresh()
            print("QA finals settled inArchive=\(model.finals!["groups"].array[0]["in_archive"].double) waiting=\(model.finals!["groups"].array[0]["waiting"].double) picks=\(model.recentProjects.map { $0["project"].string }) projects=\(model.projectIndex.map { $0["project"].string + ":" + $0["stage"].string })")
            UserDefaults.standard.removeObject(forKey: "finalsKeepSource")
            print("QA finals OK")
        } catch { print("QA finals failed: \(error)") }
        NSApp.terminate(nil)
    }

    /// Real engine, isolated state, invented bundles: assign → reuse → rename → bump → finish → dismiss.
    static func projects(_ delegate: AppDelegate, _ folder: URL) async {
        let model = delegate.model
        func wait(_ label: String, _ condition: @MainActor () -> Bool) async -> Bool {
            for _ in 0..<150 { if condition() { return true }; try? await Task.sleep(for: .milliseconds(100)) }
            print("QA projects FAILED \(label) error=\(model.error) captureError=\(model.captureError)"); return false
        }
        func titles() -> [String] { model.recentProjects.map { $0["project"].string } }
        do {
            let root = folder.appendingPathComponent("Stalker")
            for index in 0..<3 {
                let bundle = root.appendingPathComponent("Stalker_\(index).stalker")
                try FileManager.default.createDirectory(at: bundle, withIntermediateDirectories: true)
                let manifest = ["producer": "stalker", "schemaVersion": 3, "sessionID": UUID().uuidString, "status": "completed", "createdAt": "2026-11-01T1\(index):00:00Z", "tracks": []] as [String: Any]
                try JSONSerialization.data(withJSONObject: manifest).write(to: bundle.appendingPathComponent("manifest.json"))
            }
            try? await Task.sleep(for: .seconds(1))
            model.captureRoot = root.path
            await model.refresh()
            guard await wait("sessions", { model.pendingSessions(in: root.path) == 3 }) else { NSApp.terminate(nil); return }
            let first = model.pendingCaptures.first { $0["root"].string == root.path }!
            model.tagCaptures([first], project: "Mi Proyecto", country: "mx")
            guard await wait("draft in quick picks", { titles().first == "Mi-Proyecto" && model.lastTagged == "Mi-Proyecto" }) else { NSApp.terminate(nil); return }
            print("QA projects saved tag=\(model.pendingCaptures.first { $0["path"] == first["path"] }?["tag"]["name"].string ?? "") picks=\(titles())")
            let rest = model.pendingCaptures.filter { $0["root"].string == root.path && $0["tag"]["name"].string.isEmpty }
            model.tagCaptures(rest, project: model.lastTagged, country: "mx")
            guard await wait("bulk reuse", { model.projectIndex.first { $0["project"].string == "Mi-Proyecto" }?["pending_sessions"].double == 3 }) else { NSApp.terminate(nil); return }
            let draft = model.projectIndex.first { $0["project"].string == "Mi-Proyecto" }!
            print("QA projects bulk stage=\(draft["stage"].string) pending=\(draft["pending_sessions"].double)")
            model.renameProject(draft, to: "Otro Nombre")
            guard await wait("rename", { !model.busy && titles().contains("Otro-Nombre") && !titles().contains("Mi-Proyecto") }) else { NSApp.terminate(nil); return }
            print("QA projects renamed tags=\(Set(model.pendingCaptures.filter { $0["root"].string == root.path }.map { $0["tag"]["name"].string })) lastTagged=\(model.lastTagged)")
            model.tagCaptures([first], project: "segundo", country: "mx")
            guard await wait("second", { titles().first == "segundo" }) else { NSApp.terminate(nil); return }
            let renamed = model.projectIndex.first { $0["project"].string == "Otro-Nombre" }!
            model.bumpProject(renamed)
            guard await wait("bump", { titles().first == "Otro-Nombre" }) else { NSApp.terminate(nil); return }
            try? await Task.sleep(for: .seconds(1))
            print("QA projects bumped picks=\(titles())")
            let source = folder.appendingPathComponent("card")
            try FileManager.default.createDirectory(at: source, withIntermediateDirectories: true)
            print("QA projects kinds stalker=\(Model.holdsStalkerSessions(root)) bundle=\(Model.holdsStalkerSessions(root.appendingPathComponent("Stalker_0.stalker"))) folder=\(Model.holdsStalkerSessions(source))")
            // The real bundle-to-Trash path on an invented session; only this invented bundle is ever touched.
            let name = "QA-discard-" + UUID().uuidString + ".stalker"
            let doomed = root.appendingPathComponent(name)
            try FileManager.default.createDirectory(at: doomed, withIntermediateDirectories: true)
            let manifest: [String: Any] = ["producer": "stalker", "schemaVersion": 3, "sessionID": UUID().uuidString, "status": "completed", "createdAt": "2026-11-01T13:00:00Z", "tracks": []]
            try JSONSerialization.data(withJSONObject: manifest).write(to: doomed.appendingPathComponent("manifest.json"))
            model.captureChanged([])
            guard await wait("discard candidate", { model.pendingCaptures.contains { $0["path"].string == doomed.path } }) else { NSApp.terminate(nil); return }
            let candidate = model.pendingCaptures.first { $0["path"].string == doomed.path }!
            guard let review = await model.reviewDiscard(candidate) else { print("QA projects FAILED review \(model.error)"); NSApp.terminate(nil); return }
            print("QA projects discard review bytes=\(review["bytes"].double) blockers=\(review["blockers"].array.map(\.string)) warnings=\(review["warnings"].array.map(\.string))")
            model.discardCapture(review)
            guard await wait("discard", { !model.busy && !FileManager.default.fileExists(atPath: doomed.path) && !model.pendingCaptures.contains { $0["path"].string == doomed.path } }) else { NSApp.terminate(nil); return }
            let trash = try FileManager.default.url(for: .trashDirectory, in: .userDomainMask, appropriateFor: nil, create: false).appendingPathComponent(name)
            let inTrash = FileManager.default.fileExists(atPath: trash.path)
            if inTrash { try? FileManager.default.removeItem(at: trash) }
            print("QA projects discarded inTrash=\(inTrash) cleaned=\(!FileManager.default.fileExists(atPath: trash.path)) message=\(model.message)")
            print("QA projects OK")
        } catch { print("QA projects failed: \(error)") }
        NSApp.terminate(nil)
    }

    struct ReadmeFrame: View {
        let model: Model
        @Environment(\.creatorPalette) private var palette
        var body: some View {
            VStack(spacing: 0) {
                HStack(spacing: 8) {
                    ForEach([Color.red, .yellow, .green], id: \.self) { Circle().fill($0.opacity(0.85)).frame(width: 12, height: 12) }
                    WindowTopBar(model: model).frame(height: 30)
                }.padding(.horizontal, 12).frame(height: 34)
                WindowRoot(model: model)
            }
        }
    }

    static func fullscreen(_ delegate: AppDelegate) async {
        delegate.show()
        try? await Task.sleep(for: .seconds(1))
        guard let window = delegate.window else { print("QA: no window"); NSApp.terminate(nil); return }
        func report(_ label: String) {
            let item = window.toolbar?.items.first
            let barWindow = item?.view?.window
            let frame = item?.view.map { $0.convert($0.bounds, to: nil) }.flatMap { rect in barWindow?.convertToScreen(rect) } ?? .zero
            print("QA \(label) items=\(window.toolbar?.items.count ?? -1) visible=\(window.toolbar?.visibleItems?.count ?? -1) viewFrame=\(item?.view?.frame ?? .zero) superview=\(String(describing: item?.view?.superview.map { type(of: $0) })) fitting=\(item?.view?.fittingSize ?? .zero)")
            print("QA \(label): fullscreen=\(window.styleMask.contains(.fullScreen)) toolbarVisible=\(window.toolbar?.isVisible ?? false) barWindowVisible=\(barWindow?.isVisible ?? false) barOnScreen=\(barWindow?.occlusionState.contains(.visible) ?? false) barFrame=\(frame) screen=\(window.screen?.frame ?? .zero) options=\(NSApp.currentSystemPresentationOptions.rawValue) autoHideToolbar=\(NSApp.currentSystemPresentationOptions.contains(.autoHideToolbar))")
        }
        report("windowed")
        func dump(_ view: NSView, _ depth: Int) {
            print("QA tree " + String(repeating: "  ", count: depth) + String(describing: type(of: view)) + " \(view.frame)")
            if depth < 6 { view.subviews.forEach { dump($0, depth + 1) } }
        }
        if let viewer = window.toolbar?.items.first?.view?.superview { dump(viewer.superview ?? viewer, 0) }
        window.toggleFullScreen(nil)
        try? await Task.sleep(for: .seconds(3))
        report("fullscreen")
        if let bar = window.toolbar?.items.first?.view, let barWindow = bar.window, let root = barWindow.contentView?.superview {
            func describe(_ view: NSView, _ depth: Int) {
                let layer = view.layer
                let bg = layer?.backgroundColor.flatMap { NSColor(cgColor: $0)?.usingColorSpace(.sRGB) }.map { String(format: "%.2f,%.2f,%.2f,%.2f", $0.redComponent, $0.greenComponent, $0.blueComponent, $0.alphaComponent) } ?? "-"
                print("QA px " + String(repeating: "  ", count: depth) + "\(type(of: view)) hidden=\(view.isHidden) alpha=\(view.alphaValue) opaque=\(view.isOpaque) bg=\(bg) contents=\(layer?.contents != nil) sublayers=\(layer?.sublayers?.count ?? 0) frame=\(view.frame)")
                if depth < 5, !String(describing: type(of: view)).hasPrefix("NSHostingView") { view.subviews.forEach { describe($0, depth + 1) } }
            }
            describe(root, 0)
            print("QA px window bg=\(barWindow.backgroundColor) transparentTitlebar=\(barWindow.titlebarAppearsTransparent) alpha=\(barWindow.alphaValue) style=\(barWindow.styleMask.rawValue)")
            if let bitmap = root.bitmapImageRepForCachingDisplay(in: root.bounds) {
                root.cacheDisplay(in: root.bounds, to: bitmap)
                let out = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("nuun-fs-toolbar.png")
                try? bitmap.representation(using: .png, properties: [:])?.write(to: out)
                print("QA px png \(out.path)")
            }
        }
        if let bar = window.toolbar?.items.first?.view, let barWindow = bar.window {
            print("QA fs window \(type(of: barWindow)) bg=\(barWindow.backgroundColor.description) opaque=\(barWindow.isOpaque) appearance=\(barWindow.appearance?.name.rawValue ?? "nil") effective=\(barWindow.effectiveAppearance.name.rawValue) frame=\(barWindow.frame)")
            var chain: [NSView] = []; var current: NSView? = bar
            while let view = current { chain.append(view); current = view.superview }
            for view in chain.reversed() {
                let layer = view.layer
                print("QA fs chain \(type(of: view)) hiddenChildren=\(view.subviews.filter(\.isHidden).map { String(describing: type(of: $0)) }) frame=\(view.frame) layerBg=\(layer?.backgroundColor.map { String(describing: $0) } ?? "nil") subviews=\(view.subviews.map { String(describing: type(of: $0)) })")
            }
        }
        window.toggleFullScreen(nil)
        try? await Task.sleep(for: .seconds(2))
        report("restored")
        NSApp.terminate(nil)
    }
}
#endif
