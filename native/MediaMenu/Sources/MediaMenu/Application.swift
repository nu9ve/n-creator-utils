import AppKit
import SwiftUI
import UserNotifications
import MediaMenuCore

struct DraftUnit: Identifiable, Equatable {
    var id: String
    var paths: [String]
    var project = ""
    var profile = ""
    var camera = "auto"
    var copyForEditing = false
    var category = "support"
    var detectedCamera = ""
    var bytes: Double = 0
    var createdAt = ""
    var duration: Double = 0
    var tracks = 0
    var sessionID = ""
    var manifestPreviewPath = ""
    var country = ""
    var suggestedProject = ""
    var suggestedCountry = ""
    var previewPath: String? {
        if !manifestPreviewPath.isEmpty { return manifestPreviewPath }
        return paths.first { path in
            let lower = path.lowercased()
            return !lower.contains("/thmbnl/") && ["jpg", "jpeg", "png", "heic", "dng", "cr2", "cr3", "arw", "mov", "mp4", "mts", "wav", "insv", "insp"].contains(URL(fileURLWithPath: path).pathExtension.lowercased())
        }
    }
}

@MainActor final class Model: ObservableObject {
    @Published var selectedTab = 4
    @Published var presentation: WindowMode = .full
    @Published var profiles: [JSONValue] = []
    @Published var recentProjects: [JSONValue] = []
    @Published var discoveryLoading = false
    @Published var discoveryError = ""
    var loadedSourceKey = ""
    var pendingSourceKey = ""
    var pendingReadID: UUID?
    var changeMode: ((WindowMode) -> Void)?
    @Published var source = "" { didSet { if source != oldValue { units = []; plan = nil; loadedSourceKey = "" } } }
    @Published var bundledRuntime = false
    @Published var captureRoot = ""
    @Published var pendingCaptures: [JSONValue] = []
    var captureWatchers: [CaptureWatcher] = []
    @Published var captureSessions: [JSONValue] = []
    @Published var captureRoots: [String] = []
    @Published var captureError = ""
    @Published var clipperExecutable = ""
    @Published var retainStalkerSource = false
    @Published var releasePlan: JSONValue?
    @Published var releaseHistory: [JSONValue] = []
    @Published var coverageResult: JSONValue?
    @Published var auditPath = ""
    @Published var releaseProject = ""
    var watchedCaptureRoots: [String] = []
    var captureTask: Task<Void, Never>?
    @Published var project = ""
    @Published var lastProject = ""
    @Published var cardSuggestions: [String] = []
    var notifiedCards: Set<String> = []
    @Published var profile = "e64"
    @Published var country = "mx"
    @Published var city = "hermosillo"
    @Published var archive = ""
    @Published var work = ""
    @Published var sourceKind = "card"
    @Published var cleanup = true
    @Published var volumes: [JSONValue] = []
    @Published var assets: [JSONValue] = []
    @Published var jobs: [JSONValue] = []
    @Published var units: [DraftUnit] = []
    @Published var selected: Set<String> = []
    @Published var plan: JSONValue?
    @Published var busy = false
    @Published var message = "Listo"
    @Published var error = ""
    @Published var python = ""
    @Published var script = ""
    @Published var archiveOnlyPrompt = false
    @Published var deferWork = true
    @Published var copyChoice = "none"
    var noWork: Bool { copyChoice == "none" || (copyChoice == "some" && !units.contains(where: \.copyForEditing)) }
    @Published var packagedBackend = false
    var client: BackendClient?
    var openWindow: (() -> Void)?
    var openImporter: (() -> Void)?
    var statusChanged: (() -> Void)?
    var eventTask: Task<Void, Never>?
    var monitorTask: Task<Void, Never>?

    init() {
        let prefs = UserDefaults.standard
        prefs.register(defaults: ["showQuickImporter": true])
        if let configURL = Bundle.main.url(forResource: "backend-config", withExtension: "json"),
           let data = try? Data(contentsOf: configURL), let config = try? JSONDecoder().decode(BackendConfiguration.self, from: data) {
            python = config.python
            script = config.script.hasPrefix("/") ? config.script : Bundle.main.resourceURL!.appendingPathComponent(config.script).path
            packagedBackend = true
        }
        if let resources = Bundle.main.resourceURL {
            let bundledPython = resources.appendingPathComponent("backend/runtime/bin/python3").path
            let bundledScript = resources.appendingPathComponent("backend/source/nu9ve.py").path
            if FileManager.default.isExecutableFile(atPath: bundledPython) { python = bundledPython; script = bundledScript; bundledRuntime = true }
        }
        if !bundledRuntime && !packagedBackend {
            python = prefs.string(forKey: "python") ?? python
            script = prefs.string(forKey: "script") ?? script
        }
        captureRoot = prefs.string(forKey: "captureRoot") ?? ""
        configure()
    }
    func watchCaptures() {
        guard !busy else { return }
        UserDefaults.standard.set(captureRoot, forKey: "captureRoot")
        operation("Actualizando bibliotecas de Stalker") {
            _ = try await self.client!.call("integration_settings", ["capture_override": .string(self.captureRoot),
                "clipper_executable": .string(self.clipperExecutable), "retain_stalker_source": .bool(self.retainStalkerSource)])
        }
    }
    func captureChanged(_ paths: [String]) {
        captureTask?.cancel()
        captureTask = Task {
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled, !busy, let client, client.configuration.usable else { return }
            do {
                let data = try await client.call("capture_libraries", ["override": .string(captureRoot)])
                guard !Task.isCancelled else { return }
                pendingCaptures = data["pending"].array; captureSessions = data["sessions"].array
                captureRoots = data["roots"].array.map(\.string)
                captureError = data["errors"].array.map(\.string).joined(separator: "\n")
                let registry = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/Stalker").path
                let roots = Array(Set(captureRoots + [registry])).filter { FileManager.default.fileExists(atPath: $0) }.sorted()
                if roots != watchedCaptureRoots {
                    watchedCaptureRoots = roots
                    captureWatchers = roots.map { root in CaptureWatcher(root: root) { [weak self] paths in
                        Task { @MainActor in self?.captureChanged(paths) }
                    } }
                }
                statusChanged?()
            } catch { if !Task.isCancelled { captureError = error.localizedDescription } }
        }
    }
    func kindLabel(_ raw: String) -> String {
        ["file": "Archivo", "camera_bundle": "Captura", "stalker_session": "Stalker", "delivery": "Entrega"][raw] ?? raw
    }
    func workLabel(_ raw: String) -> String {
        ["pending": "Copia para editar pendiente", "ready": "Copias para editar listas", "skipped": "Sólo respaldo · sin copias para editar"][raw] ?? raw
    }
    func configure() {
        guard !busy else { return }
        UserDefaults.standard.set(python, forKey: "python")
        UserDefaults.standard.set(script, forKey: "script")
        client = BackendClient(configuration: .init(python: python, script: script))
        Task { await refresh(loadDefaults: true) }
    }
    func refresh(loadDefaults: Bool = false) async {
        guard let client, client.configuration.usable else { return }
        do {
            volumes = try await client.call("volumes").array
            assets = try await client.call("library").array
            jobs = try await client.call("jobs").array
            profiles = try await client.call("profiles").array
            recentProjects = try await client.call("projects").array
            releaseHistory = try await client.call("release_plans").array
            let integrations = try await client.call("integration_settings")
            clipperExecutable = integrations["clipper_executable"].string
            retainStalkerSource = integrations["retain_stalker_source"].bool
            if loadDefaults {
                if !integrations["capture_override"].string.isEmpty { captureRoot = integrations["capture_override"].string }
                else if !captureRoot.isEmpty { _ = try await client.call("integration_settings", ["capture_override": .string(captureRoot)]) }
                let values = try await client.call("defaults")
                // Offered via "Usar último"; a new card never inherits it silently.
                lastProject = values["project"].string
                profile = values["profile"].string.isEmpty ? "e64" : values["profile"].string
                country = values["country"].string.isEmpty ? "mx" : values["country"].string
                city = values["city"].string.isEmpty ? "hermosillo" : values["city"].string
                archive = values["archive"].string
                work = values["work"].string
            }
            if archive.isEmpty { archive = volumes.first(where: { $0["role"].string == "archive" })?["id"].string ?? "" }
            if work.isEmpty { work = volumes.first(where: { $0["role"].string == "work" })?["id"].string ?? "" }
            captureChanged([])
        } catch { self.error = error.localizedDescription }
    }
    func operation(_ title: String, _ body: @escaping @MainActor () async throws -> Void) {
        guard !busy else { return }
        busy = true; error = ""; message = title; statusChanged?()
        Task {
            do { try await body() }
            catch { self.error = error.localizedDescription; message = "Operación detenida; revisa el estado" }
            busy = false; monitorTask?.cancel(); monitorTask = nil
            await refresh(); statusChanged?()
            if presentation != .compact { await loadSource() }
        }
    }
    func chooseSource() {
        let panel = NSOpenPanel(); panel.canChooseFiles = false; panel.canChooseDirectories = true
        if panel.runModal() == .OK { source = panel.url?.path ?? ""; units = []; plan = nil }
    }
    func registerVolume(role: String) {
        let panel = NSOpenPanel(); panel.canChooseFiles = false; panel.canChooseDirectories = true
        panel.message = role == "archive" ? "Elige la raíz del disco de archivo (MEDIA)" : "Elige la raíz del SSD para editar"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        operation("Registrando disco") {
            _ = try await self.client!.call("register_volume", ["root": .string(url.path), "name": .string(url.lastPathComponent), "role": .string(role)])
            self.message = "Disco registrado"
        }
    }
    var sourceKey: String { source + "|" + sourceKind + "|" + archive }
    func discover() { loadedSourceKey = ""; Task { await loadSource() } }
    func loadSource() async {
        let key = sourceKey, path = source
        guard !path.isEmpty, !busy, loadedSourceKey != key,
              let client, client.configuration.usable else { return }
        let readID = UUID()
        pendingReadID = readID
        pendingSourceKey = key; discoveryLoading = true; discoveryError = ""
        let reader = BackendClient(configuration: client.configuration)
        defer { if pendingReadID == readID { pendingSourceKey = ""; pendingReadID = nil; discoveryLoading = false } }
        do {
            try await Task.sleep(for: .milliseconds(250))
            let data = try await withTaskCancellationHandler {
                try await reader.call("discover", ["source": .string(path), "source_kind": .string(sourceKind), "archive": .string(archive)])
            } onCancel: { reader.cancel() }
            guard !Task.isCancelled, sourceKey == key, pendingReadID == readID else { return }
            let existing = Dictionary(uniqueKeysWithValues: units.map { ($0.id, $0) })
            units = data["units"].array.map { value in
                var unit = existing[value["id"].string] ?? DraftUnit(id: value["id"].string, paths: value["paths"].array.map(\.string))
                unit.sessionID = value["session_id"].string
                unit.suggestedProject = value["project_hint"]["name"].string
                unit.suggestedCountry = value["project_hint"]["country"].string
                if existing[value["id"].string] == nil {
                    unit.project = value["tag"]["name"].string
                    unit.country = value["tag"]["country"].string
                }
                unit.category = value["category"].string; unit.detectedCamera = value["camera"].string
                unit.bytes = value["bytes"].double; unit.createdAt = value["created_at"].string
                unit.duration = value["duration_seconds"].double; unit.tracks = Int(value["track_count"].double)
                unit.manifestPreviewPath = value["preview_path"].string
                return unit
            }
            loadedSourceKey = key; plan = nil
            message = "\(units.count) unidades · \(data["skipped"].array.count) capturas activas omitidas"
        } catch {
            if !Task.isCancelled, sourceKey == key, pendingReadID == readID { discoveryError = error.localizedDescription }
        }
    }
    func selectCopyChoice(_ choice: String) {
        invalidatePlan(); copyChoice = choice
        if choice == "some", presentation == .compact { changeMode?(.selection) }
    }
    func invalidatePlan() {
        if plan != nil {
            plan = nil
            if !busy { message = "Cambios pendientes · revisa el plan de nuevo" }
        }
    }
    func selectProfile(_ id: String) {
        profile = id
        if let value = profiles.first(where: { $0["id"].string == id }) {
            if !value["work"].string.isEmpty { work = value["work"].string }
            if !value["archive"].string.isEmpty { archive = value["archive"].string }
        }
        plan = nil
    }
    func prepareSource(_ path: String, kind: String) {
        sourceKind = kind; cleanup = kind == "card"; source = path; selectedTab = 0; plan = nil
        if kind == "stalker" { project = "" }
    }
    func prepareCaptureBatch(_ root: String) {
        prepareSource(root, kind: "stalker")
        changeMode?(.selection)
    }
    func requestPlan() {
        if sourceKind == "delivery" { makePlan(archiveOnly: true); return }
        if noWork { archiveOnlyPrompt = true; return }
        let onlineWork = volumes.contains { $0["id"].string == work && $0["online"].bool }
        if !onlineWork { archiveOnlyPrompt = true }
        else { makePlan(archiveOnly: false) }
    }
    func makePlan(archiveOnly: Bool) {
        operation("Preparando plan") {
            var assignments: [String: JSONValue] = [:]
            for unit in self.units {
                if !unit.sessionID.isEmpty {
                    let title = unit.project.isEmpty ? self.project : unit.project
                    let country = unit.country.isEmpty ? self.country : unit.country
                    if !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        _ = try await self.client!.call("save_capture_tag", ["session_id": .string(unit.sessionID), "project": .string(title), "country": .string(country)])
                    }
                }
                var values: [String: JSONValue] = [:]
                if !unit.project.isEmpty { values["project"] = .string(unit.project) }
                if !unit.profile.isEmpty { values["profile"] = .string(unit.profile) }
                if !unit.country.isEmpty { values["country"] = .string(unit.country) }
                if unit.camera != "auto" { values["camera"] = .string(unit.camera) }
                if self.copyChoice == "some" { values["work"] = .bool(unit.copyForEditing) }
                if !values.isEmpty { for path in unit.paths { assignments[path] = .object(values) } }
            }
            var params: [String: JSONValue] = ["source": .string(self.source), "archive": .string(self.archive),
                "project": .string(self.project), "profile": .string(self.profile), "country": .string(self.country), "city": .string(self.city),
                "source_kind": .string(self.sourceKind), "cleanup": .bool(self.cleanup && self.sourceKind == "card"),
                "assignments": .object(assignments)]
            params.merge(IngestPolicy.parameters(archiveOnly: archiveOnly, skipWork: self.noWork || self.sourceKind == "delivery", deferWork: self.deferWork, work: self.work)) { _, new in new }
            self.plan = try await self.client!.call("plan", params)
            self.message = "Plan listo para revisar; no se ha copiado ni borrado media"
        }
    }
    func apply() {
        guard let reviewed = plan else { return }
        operation("Copiando y verificando") {
            self.monitorTask = Task {
                while !Task.isCancelled {
                    try? await Task.sleep(for: .seconds(2))
                    if Task.isCancelled { break }
                    if let job = try? await self.client!.call("job", ["job_id": reviewed["id"]]) {
                        self.message = self.progressLabel(job)
                    }
                }
            }
            let result = try await self.client!.call("apply", ["plan": reviewed, "digest": reviewed["digest"]])
            self.message = self.stateLabel(result["state"].string)
            let release = result["stalker_release"]
            if release["status"].string == "retained" || release["status"].string == "partial" {
                let reasons = release["retained"].array.map { $0["reason"].string }.filter { !$0.isEmpty }
                self.error = "Respaldo verificado; se conservó el original Stalker. " + reasons.joined(separator: " · ")
            }
            if !result["metadata_backup_error"].string.isEmpty { self.error = "Respaldo de media terminado; falta respaldar catálogo: " + result["metadata_backup_error"].string }
            _ = try await self.client!.call("defaults", ["project": .string(self.project), "profile": .string(self.profile),
                "country": .string(self.country), "city": .string(self.city), "archive": .string(self.archive), "work": .string(self.work)])
            self.lastProject = self.project; self.plan = nil; self.loadedSourceKey = ""
        }
    }
    func cancel() { client?.cancel(); message = "Cancelando en un punto recuperable…" }
    func stateLabel(_ state: String) -> String {
        switch state {
        case "copying": return "Copiando y verificando destinos"
        case "backup_verified": return "Archivo verificado"
        case "cleaning_source": return "Limpiando SD"
        case "completed": return sourceKind == "stalker" ? "Respaldo verificado · original Stalker liberado" : "Respaldo verificado · SD libre"
        case "camera_action_required": return "Respaldo verificado · libera la tarjeta desde la cámara"
        case "source_retained": return sourceKind == "stalker" ? "Respaldo verificado · original Stalker conservado" : "Respaldo verificado · origen conservado"
        case "source_has_new_material": return "Respaldo verificado · aparecieron archivos nuevos en la SD; no están respaldados"
        case "paused": return "Pausado; se puede reanudar"
        case "failed": return "Fallo; revisa el job antes de reanudar"
        default: return state
        }
    }
    func progressLabel(_ job: JSONValue) -> String {
        // Real bytes from Transfer.report; never an invented percentage.
        let p = job["progress"], gb = { (bytes: Double) in String(format: "%.1f", bytes / 1e9) }
        if p["phase"].string == "releasing_stalker" {
            return "Revisando Clipper y liberando sesiones Stalker \(Int(p["sessions_done"].double))/\(Int(p["sessions_total"].double))"
        }
        guard p["bytes_total"].double > 0 else { return stateLabel(job["state"].string) }
        let rate = p["bytes_per_second"].double > 0 ? String(format: " · %.0f MB/s", p["bytes_per_second"].double / 1e6) : ""
        let files = " · \(Int(p["files_done"].double))/\(Int(p["files_total"].double)) archivos"
        switch p["phase"].string {
        case "copying": return "Copiando \(gb(p["copied_bytes"].double)) de \(gb(p["bytes_total"].double)) GB" + rate + files
        case "verifying": return "Verificando destinos \(gb(p["verified_bytes"].double)) de \(gb(p["verify_total"].double)) GB" + files
        case "cleaning": return "Limpiando SD tras verificar"
        default: return stateLabel(job["state"].string)
        }
    }
    func jobLabel(_ job: JSONValue) -> String {
        if job["source_kind"].string == "stalker" {
            if job["state"].string == "completed" { return "Respaldo verificado · original Stalker liberado" }
            if job["state"].string == "source_retained" { return "Respaldo verificado · original Stalker conservado" }
        }
        // Cards holding Stalker bundles end camera_action_required without any camera step.
        if job["state"].string == "camera_action_required", job["cleanup_profile"].string == "files" {
            return "Respaldo verificado · las sesiones Stalker se conservan en el origen"
        }
        if job["state"].string == "camera_action_required", job["cleanup_profile"].string == "sony_clips_then_format" {
            return "Respaldo verificado · clips borrados de la SD · formatéala en la cámara"
        }
        return stateLabel(job["state"].string)
    }
    func jobDetail(_ job: JSONValue) -> String {
        let reasons = job["stalker_release"]["retained"].array.map { $0["reason"].string }.filter { !$0.isEmpty }
        return reasons.isEmpty ? job["id"].string : reasons.joined(separator: " · ")
    }
    func useCard(_ path: String) {
        source = path; units = []; plan = nil; project = ""; selectedTab = 0
    }
    func notifyCard(_ path: String) {
        guard Bundle.main.bundleIdentifier != nil else { return }  // `swift run` has no bundle for notifications.
        Task {
            let center = UNUserNotificationCenter.current()
            guard (try? await center.requestAuthorization(options: [.alert])) == true else { return }
            let content = UNMutableNotificationContent()
            content.title = "Tarjeta detectada"
            content.body = URL(fileURLWithPath: path).lastPathComponent + " · abre nüün para revisar el plan"
            try? await center.add(UNNotificationRequest(identifier: "card:" + path, content: content, trigger: nil))
        }
    }
    func deliver() {
        let ids = selected.sorted()
        operation("Preparando copias para editar") {
            _ = try await self.client!.call("deliver_work", ["asset_ids": .array(ids.map(JSONValue.string)), "work": .string(self.work)])
            self.message = "Copias para editar verificadas"; self.selected = []
        }
    }
    func skip() {
        let ids = selected.sorted()
        operation("Actualizando pendientes") {
            _ = try await self.client!.call("set_work_state", ["asset_ids": .array(ids.map(JSONValue.string)), "state": .string("skipped")])
            self.message = "Copias para editar omitidas"; self.selected = []
        }
    }
    func restoreSelected() {
        guard selected.count == 1, let id = selected.first else { return }
        let panel = NSOpenPanel(); panel.canChooseDirectories = true; panel.canChooseFiles = false
        guard panel.runModal() == .OK, let url = panel.url else { return }
        operation("Restaurando selección") {
            let result = try await self.client!.call("restore", ["asset_id": .string(id), "destination": .string(url.path)])
            self.message = "Restauración verificada"
            NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: result["path"].string)])
        }
    }
    func exportHandoff() {
        let panel = NSSavePanel(); panel.nameFieldStringValue = "media-handoff.json"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        let ids = selected.sorted()
        operation("Preparando selección de biblioteca") {
            let data = try await self.client!.call("handoff", ["asset_ids": .array(ids.map(JSONValue.string))])
            try JSONEncoder().encode(data).write(to: url, options: .atomic)
            self.message = "Selección exportada; requiere soporte de biblioteca en Clipper"
        }
    }
    func confirmCleanup(_ id: String) {
        let panel = NSOpenPanel(); panel.canChooseFiles = false; panel.canChooseDirectories = true
        panel.directoryURL = URL(fileURLWithPath: "/Volumes"); panel.message = "Elige la tarjeta ya formateada en la cámara"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        operation("Comprobando tarjeta formateada") {
            let result = try await self.client!.call("confirm_camera_cleanup", ["job_id": .string(id), "source": .string(url.path)])
            self.message = self.stateLabel(result["state"].string)
            if !result["metadata_backup_error"].string.isEmpty { self.error = "Respaldo de media terminado; falta respaldar catálogo: " + result["metadata_backup_error"].string }
        }
    }
    func resume(_ id: String) {
        operation("Reanudando job") {
            let result = try await self.client!.call("resume", ["job_id": .string(id)])
            self.message = self.stateLabel(result["state"].string)
            if !result["metadata_backup_error"].string.isEmpty { self.error = "Respaldo de media terminado; falta respaldar catálogo: " + result["metadata_backup_error"].string }
        }
    }
    func mountChanged() {
        eventTask?.cancel()
        eventTask = Task {
            try? await Task.sleep(for: .milliseconds(800))
            guard !Task.isCancelled, !busy, let client, client.configuration.usable else { return }
            await refresh()
            captureChanged([])
            guard volumes.contains(where: { $0["online"].bool }) else { cardSuggestions = []; notifiedCards = []; statusChanged?(); return }
            let mounted = FileManager.default.mountedVolumeURLs(includingResourceValuesForKeys: [.volumeIsInternalKey], options: [.skipHiddenVolumes]) ?? []
            var shown: [String] = []
            for url in mounted {
                guard (try? url.resourceValues(forKeys: [.volumeIsInternalKey]).volumeIsInternal) == false else { continue }
                if let suggestion = try? await client.call("suggest", ["source": .string(url.path)]), suggestion["show"].bool { shown.append(url.path) }
            }
            // A mount event notifies once per insertion; it never opens the window or replaces work in progress.
            cardSuggestions = shown
            notifiedCards.formIntersection(shown)
            let idle = units.isEmpty && plan == nil && (source.isEmpty || !FileManager.default.fileExists(atPath: source))
            for path in shown where !notifiedCards.contains(path) {
                notifiedCards.insert(path); notifyCard(path)
                if idle && source != path {
                    useCard(path)
                    if UserDefaults.standard.bool(forKey: "showQuickImporter") { openImporter?() }
                }
            }
            statusChanged?()
            if assets.contains(where: { $0["work_delivery_status"].string == "pending" }) { statusChanged?() }
        }
    }
}

struct MainView: View {
    @ObservedObject var model: Model
    let cameras = ["auto", "generic", "sony", "canon", "nikon", "gopro", "h6", "dji-drone", "dji-360", "dji-action", "insta360"]
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if !model.error.isEmpty { Text(model.error).foregroundStyle(.red).textSelection(.enabled) }
            Group {
                switch model.selectedTab {
                case 0: ingest
                case 1: library
                case 2: activity
                case 3: SettingsView(model: model)
                case 5: StorageView(model: model)
                default: InboxView(model: model)
                }
            }.frame(maxWidth: .infinity, maxHeight: .infinity)
        }.padding(20).frame(maxWidth: .infinity, maxHeight: .infinity)
            .safeAreaInset(edge: .bottom, spacing: 0) { MainActionFooter(model: model) }
    }
    var ingest: some View {
        VStack(alignment: .leading, spacing: 12) {
            ForEach(model.cardSuggestions.filter { $0 != model.source }, id: \.self) { path in
                HStack { Label("Tarjeta detectada: " + URL(fileURLWithPath: path).lastPathComponent, systemImage: "sdcard"); Button("Usar esta tarjeta") { model.useCard(path) } }
            }
            HStack { TextField("Fuente", text: $model.source); Button("Elegir…") { model.chooseSource() } }
            HStack {
                TextField(model.sourceKind == "stalker" ? "Proyecto para sesiones sin asignar" : "Proyecto", text: $model.project)
                if !model.lastProject.isEmpty && model.project != model.lastProject { Button("Usar último: " + model.lastProject) { model.project = model.lastProject } }
                ProfilePicker(model: model); TextField("País", text: $model.country); TextField("Ciudad", text: $model.city) }
            HStack {
                volumePicker("Archivo", role: "archive", selection: $model.archive)
                if !model.noWork { volumePicker("SSD para editar", role: "work", selection: $model.work) }
            }
            EditingChoice(model: model)
            if model.noWork { Text("Se usará únicamente el disco de archivo seleccionado, aunque el SSD esté conectado.").foregroundStyle(.secondary) }
            HStack {
                SourceKindBadges(model: model)
                Toggle("Limpiar SD tras verificar", isOn: $model.cleanup).disabled(model.sourceKind != "card")
                if !model.noWork { Toggle("Preparar copia para editar después si falta SSD", isOn: $model.deferWork) }
            }
            SourceSummary(model: model)
            ImportGrid(model: model).frame(minHeight: 180)
            DisclosureGroup("Asignación avanzada · perfil y cámara") {
            ScrollView { VStack {
                ForEach($model.units) { $unit in
                    HStack {
                        Text(unit.paths.joined(separator: "\n")).lineLimit(3).frame(maxWidth: .infinity, alignment: .leading)
                        TextField("Proyecto general", text: $unit.project).frame(width: 150)
                        TextField("Perfil general", text: $unit.profile).frame(width: 100)
                        Picker("Cámara", selection: $unit.camera) { ForEach(cameras, id: \.self) { Text($0).tag($0) } }.frame(width: 150)
                    }
                }
            } }
            }.frame(maxHeight: 140)
            PlanReview(model: model)
        }.padding(.top).disabled(model.busy)
        .onChange(of: model.sourceKind) { _, _ in model.plan = nil }
        .onChange(of: model.deferWork) { _, _ in model.plan = nil }
        .onChange(of: model.project) { _, _ in model.invalidatePlan() }
    }
    func volumePicker(_ title: String, role: String, selection: Binding<String>) -> some View {
        Picker(title, selection: selection) {
            Text("Sin seleccionar").tag("")
            ForEach(model.volumes.filter { $0["role"].string == role }, id: \.["id"].string) { value in
                Text(value["name"].string + (value["online"].bool ? "" : " · desconectado")).tag(value["id"].string)
            }
        }
    }
    var library: some View {
        VStack(alignment: .leading) {
            if !model.pendingCaptures.isEmpty {
                HStack {
                    Text("\(model.pendingCaptures.count) sesiones Stalker pendientes")
                    Spacer()
                    Button("Preparar desde Bandeja") { model.selectedTab = 4 }
                }
            }
            List(model.assets, id: \.["id"].string, selection: $model.selected) { asset in
                HStack { Text(asset["project"].string); Text(asset["profile"].string).foregroundStyle(.secondary); Spacer(); Text(model.kindLabel(asset["kind"].string)); Text(model.workLabel(asset["work_delivery_status"].string)) }.tag(asset["id"].string)
            }.modifier(CreatorListSurface()).frame(maxHeight: .infinity)
        }.padding(.top).disabled(model.busy)
    }
    var activity: some View {
        List(model.jobs, id: \.["id"].string) { job in
            HStack { JobStatusIcon(state: job["state"].string); VStack(alignment: .leading) { Text(model.jobLabel(job)); Text(model.jobDetail(job)).font(.caption).foregroundStyle(.secondary).lineLimit(2) }; Spacer(); if ["failed", "paused", "copying", "cleaning_source", "backup_verified"].contains(job["state"].string) || job["state"].string == "source_retained" && job["release_stalker_source"].bool { Button("Reanudar") { model.resume(job["id"].string) }.disabled(model.busy) }
                if job["state"].string == "camera_action_required" && ["camera_action_required", "sony_clips_then_format"].contains(job["cleanup_profile"].string) { Button("Ya la formateé…") { model.confirmCleanup(job["id"].string) }.disabled(model.busy) } }
        }.modifier(CreatorListSurface()).padding(.top)
    }

}

@MainActor final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate, UNUserNotificationCenterDelegate {
    let model = Model()
    var status: NSStatusItem!
    var window: NSWindow?
    var topBarAccessory: NSTitlebarAccessoryViewController?
    var pendingMode: WindowMode?
    var observers: [NSObjectProtocol] = []
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        status = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        status.button?.image = NSImage(systemSymbolName: "externaldrive.badge.checkmark", accessibilityDescription: "nüün")
        let menu = NSMenu()
        menu.addItem(withTitle: "Abrir nüün…", action: #selector(show), keyEquivalent: "")
        menu.addItem(withTitle: "Importador rápido…", action: #selector(showImporter), keyEquivalent: "")
        menu.addItem(withTitle: "Cancelar operación", action: #selector(cancel), keyEquivalent: "")
        menu.addItem(.separator())
        menu.addItem(withTitle: "Salir", action: #selector(quit), keyEquivalent: "q")
        for item in menu.items { item.target = self }
        status.menu = menu
        installMenus()
        model.changeMode = { [weak self] mode in self?.present(mode) }
        model.openWindow = { [weak self] in self?.show() }
        model.openImporter = { [weak self] in self?.showImporter() }
        model.statusChanged = { [weak self] in
            guard let self else { return }
            self.status.button?.title = self.model.busy ? " …" : ((self.model.assets.contains { $0["work_delivery_status"].string == "pending" } || !self.model.pendingCaptures.isEmpty || !self.model.cardSuggestions.isEmpty) ? " •" : "")
            self.status.button?.toolTip = self.model.message
        }
        let center = NSWorkspace.shared.notificationCenter
        for name in [NSWorkspace.didMountNotification, NSWorkspace.didUnmountNotification, NSWorkspace.didRenameVolumeNotification, NSWorkspace.didWakeNotification] {
            observers.append(center.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                Task { @MainActor in self?.model.mountChanged() }
            })
        }
        if Bundle.main.bundleIdentifier != nil { UNUserNotificationCenter.current().delegate = self }
        if !CommandLine.arguments.contains("--background") { show() }
        model.mountChanged()
        // Mounts never open windows; explicit launcher opens do. No recurring idle timer.
    }
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if let window, window.isVisible { NSApp.activate(ignoringOtherApps: true); window.makeKeyAndOrderFront(nil) }
        else { present(model.presentation) }
        return true
    }
    @objc func show() { present(.full) }
    func present(_ mode: WindowMode) {
        if let window, window.styleMask.contains(.fullScreen), mode != .full {
            pendingMode = mode; window.toggleFullScreen(nil); return
        }
        if window == nil {
            let value = NSWindow(contentRect: NSRect(x: 0, y: 0, width: mode.width, height: mode.height), styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
            value.title = "nüün"; value.delegate = self; value.isReleasedWhenClosed = false
            value.collectionBehavior.insert(.fullScreenPrimary)
            let host = NSHostingView(rootView: WindowRoot(model: model))
            // AppKit owns the three sizes; transient SwiftUI branch measurements must not resize the window.
            host.sizingOptions = []
            host.autoresizingMask = [.width, .height]
            value.contentView = host; value.center(); window = value

            let accessory = NSTitlebarAccessoryViewController()
            accessory.layoutAttribute = .bottom
            accessory.fullScreenMinHeight = 50
            let topBar = NSHostingView(rootView: WindowTopBar(model: model).modifier(CreatorAppearance()))
            topBar.sizingOptions = []
            topBar.frame = NSRect(x: 0, y: 0, width: mode.width, height: 50)
            accessory.view = topBar
            value.addTitlebarAccessoryViewController(accessory)
            topBarAccessory = accessory
        }
        guard let window else { return }
        let sameMode = model.presentation == mode && window.isVisible
        window.minSize = NSSize(width: mode == .full ? 920 : mode == .selection ? 680 : 408, height: mode == .full ? 720 : 580)
        withAnimation(.easeInOut(duration: NSWorkspace.shared.accessibilityDisplayShouldReduceMotion ? 0 : 0.22)) { model.presentation = mode }
        if !sameMode && !window.styleMask.contains(.fullScreen) {
            let visible = window.screen?.visibleFrame ?? NSScreen.main!.visibleFrame
            let width = min(mode.width, visible.width), height = min(mode.height, visible.height)
            let frame = NSRect(x: min(max(visible.minX, window.frame.midX - width / 2), visible.maxX - width), y: min(max(visible.minY, window.frame.maxY - height), visible.maxY - height), width: width, height: height)
            NSAnimationContext.runAnimationGroup { context in context.duration = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion ? 0 : 0.24; window.animator().setFrame(frame, display: true) }
        }
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true); window.makeKeyAndOrderFront(nil)
    }
    func windowDidExitFullScreen(_ notification: Notification) { if let mode = pendingMode { pendingMode = nil; present(mode) } }
    func windowWillEnterFullScreen(_ notification: Notification) { model.presentation = .full; model.selectedTab = 0 }
    func installMenus() {
        let bar = NSMenu()
        let appItem = NSMenuItem(); bar.addItem(appItem)
        let appMenu = NSMenu(title: "nüün"); appItem.submenu = appMenu
        appMenu.addItem(withTitle: "Acerca de nüün", action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")
        appMenu.addItem(withTitle: "Ajustes…", action: #selector(settings), keyEquivalent: ",").target = self
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "Ocultar nüün", action: #selector(NSApplication.hide(_:)), keyEquivalent: "h")
        appMenu.addItem(withTitle: "Salir de nüün", action: #selector(quit), keyEquivalent: "q").target = self
        let fileItem = NSMenuItem(); bar.addItem(fileItem)
        let file = NSMenu(title: "Archivo"); fileItem.submenu = file
        file.addItem(withTitle: "Abrir dashboard", action: #selector(show), keyEquivalent: "1").target = self
        file.addItem(withTitle: "Importador rápido…", action: #selector(showImporter), keyEquivalent: "i").target = self
        file.addItem(withTitle: "Cerrar ventana", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")
        let editItem = NSMenuItem(); bar.addItem(editItem)
        let edit = NSMenu(title: "Edición"); editItem.submenu = edit
        edit.addItem(withTitle: "Deshacer", action: Selector(("undo:")), keyEquivalent: "z")
        edit.addItem(withTitle: "Cortar", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        edit.addItem(withTitle: "Copiar", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        edit.addItem(withTitle: "Pegar", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        edit.addItem(withTitle: "Seleccionar todo", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        NSApp.mainMenu = bar
    }
    @objc func settings() { model.selectedTab = 3; show() }
    @objc func showImporter() { present(.compact) }
    func windowWillClose(_ notification: Notification) { NSApp.setActivationPolicy(.accessory) }
    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse) async {
        await MainActor.run { model.selectedTab = 0; showImporter() }
    }
    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent notification: UNNotification) async -> UNNotificationPresentationOptions { [.banner] }
    @objc func cancel() { model.cancel() }
    @objc func quit() { NSApp.terminate(nil) }
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard model.busy else { return .terminateNow }
        model.cancel()
        Task {
            while model.busy { try? await Task.sleep(for: .milliseconds(200)) }
            NSApp.reply(toApplicationShouldTerminate: true)
        }
        return .terminateLater
    }
}

@main enum MediaApplication {
    @MainActor static func main() {
        let app = NSApplication.shared, delegate = AppDelegate()
        app.delegate = delegate
        withExtendedLifetime(delegate) { app.run() }
    }
}
