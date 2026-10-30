#if DEBUG
import AppKit
import SwiftUI
import MediaMenuCore

/// Debug-only visual QA without screen-recording permission: `--qa-snapshots DIR` renders each
/// route/step with fixture data to PNG; `--qa-fullscreen` logs whether the toolbar stays visible.
@MainActor enum QASnapshots {
    static func run(_ delegate: AppDelegate) -> Bool {
        let arguments = CommandLine.arguments
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
        if arguments.contains("--qa-fullscreen") {
            Task { await fullscreen(delegate) }
            return true
        }
        return false
    }

    static func fixture(_ model: Model, media: URL) {
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
        model.recentProjects = ["mercado-norte", "taller-luz", "entrevista-ana", "clip-intro", "viaje-sonora", "podcast-07", "demo"].enumerated().map { index, title in
            .object(["project_id": .string("p\(index)"), "project": .string(title)]) }
        model.profiles = [.object(["id": .string("e64"), "name": .string("e64"), "raw_dir": .string("e64_raw")]), .object(["id": .string("9logs"), "name": .string("9logs"), "raw_dir": .string("9logs_raw")])]
        model.assets = (0..<14).map { index in .object(["id": .string("asset\(index)"), "project": .string(["mercado-norte", "taller-luz", "entrevista-ana"][index % 3]), "profile": .string("e64"), "kind": .string(index % 4 == 0 ? "camera_bundle" : "file"), "camera": .string(index % 2 == 0 ? "ILCE-6400" : "dji-drone"), "country": .string("mx"), "city": .string("hermosillo"), "paths": .array([.string("a"), .string("b")]), "work_delivery_status": .string(["pending", "ready", "skipped"][index % 3])]) }
        model.jobs = [
            .object(["id": .string("job-1"), "state": .string("copying"), "updated_at": .string("2026-10-29T10:12:00"), "cleanup_profile": .string("files")]),
            .object(["id": .string("job-2"), "state": .string("camera_action_required"), "updated_at": .string("2026-10-28T18:40:00"), "cleanup_profile": .string("sony_clips_then_format")]),
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

    static func snapshots(_ delegate: AppDelegate, _ folder: URL) async {
        let model = delegate.model
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        try? await Task.sleep(for: .seconds(2))  // let the isolated snapshot refresh settle first
        fixture(model, media: folder.appendingPathComponent("media"))
        let full = NSSize(width: 1180, height: 860), selection = NSSize(width: 780, height: 820), compact = NSSize(width: 440, height: 700)
        model.presentation = .full
        model.route = .home
        await render(MainView(model: model), size: full, to: folder.appendingPathComponent("route-home.png"))
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
        for width in [1180.0, 700.0, 430.0] {
            model.presentation = width < 600 ? .compact : .full
            await render(WindowTopBar(model: model), size: NSSize(width: width, height: 30), to: folder.appendingPathComponent("topbar-\(Int(width)).png"))
        }
        await render(SettingsView(model: model), size: NSSize(width: 760, height: 600), to: folder.appendingPathComponent("settings.png"))
        // README image: compact window, fixture data only (no real libraries, sessions or paths).
        model.captureRoots = ["/Users/demo/Stalker"]; model.pendingCaptures = []; model.captureSessions = []
        if model.units.count > 1 { model.units[1].camera = "dji-drone" }
        model.gridSelection = []; model.error = ""; model.route = .ingest; model.presentation = .compact; model.ingestStep = .source
        await render(ReadmeFrame(model: model), size: NSSize(width: 440, height: 730), to: folder.appendingPathComponent("readme-compact.png"))
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
