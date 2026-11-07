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
    /// Drone/360 only when explicitly classified; never inferred from the DJI brand.
    var displayCategory: String {
        camera == "dji-drone" ? "drone" : ["dji-360", "insta360"].contains(camera) ? "360" : category
    }
    var needsCameraReview: Bool { !isSupport && camera == "auto" && detectedCamera.lowercased().hasPrefix("dji") }
    var isSupport: Bool { category == "support" }
}

enum Route: String, CaseIterable, Identifiable {
    case home, ingest, flows, library
    var id: String { rawValue }
    var title: LocalizedStringKey {
        switch self {
        case .home: "Inicio"
        case .ingest: "Ingreso"
        case .flows: "Flujos"
        case .library: "Biblioteca"
        }
    }
    var symbol: String {
        switch self {
        case .home: "tray.full"
        case .ingest: "sdcard"
        case .flows: "point.3.connected.trianglepath.dotted"
        case .library: "square.stack"
        }
    }
}

enum LibrarySection: String, CaseIterable, Identifiable {
    case projects, files, disks, activity
    var id: String { rawValue }
    var title: LocalizedStringKey {
        switch self {
        case .projects: "Proyectos"
        case .files: "Archivos"
        case .disks: "Discos"
        case .activity: "Actividad"
        }
    }
    var symbol: String {
        switch self {
        case .projects: "folder"
        case .files: "square.stack"
        case .disks: "externaldrive"
        case .activity: "clock.arrow.circlepath"
        }
    }
}

enum IngestStep: Int, CaseIterable, Identifiable, Comparable {
    case source, content, destinations, review, run
    var id: Int { rawValue }
    static func < (lhs: IngestStep, rhs: IngestStep) -> Bool { lhs.rawValue < rhs.rawValue }
    var title: LocalizedStringKey {
        switch self {
        case .source, .content: "Entrada"
        case .destinations, .review: "Flujo"
        case .run: "Resultado"
        }
    }
    var symbol: String {
        switch self {
        case .source: "sdcard"
        case .content: "photo.stack"
        case .destinations: "externaldrive"
        case .review: "checklist"
        case .run: "arrow.right.circle"
        }
    }
}

/// What a running mutation locks. Navigation and reading never wait on it.
enum OperationScope { case ingest, library, space, settings, source }

/// Folder pickers attach to the window that asked, instead of blocking the whole app.
@MainActor func pickURL(message: String = "", directories: Bool = true, directory: URL? = nil, hidden: Bool = false, _ completion: @escaping (URL) -> Void) {
    let panel = NSOpenPanel()
    panel.canChooseFiles = !directories; panel.canChooseDirectories = directories
    panel.message = message; panel.showsHiddenFiles = hidden
    if let directory { panel.directoryURL = directory }
    if let window = NSApp.keyWindow {
        panel.beginSheetModal(for: window) { response in
            if response == .OK, let url = panel.url { completion(url) }
        }
    } else if panel.runModal() == .OK, let url = panel.url { completion(url) }
}

@MainActor @Observable final class Model {
    var route: Route = .home
    var librarySection: LibrarySection = .projects
    var presentation: WindowMode = .full
    var profiles: [JSONValue] = []
    /// Receipts only, until the first capture overview arrives.
    var snapshotProjects: [JSONValue] = []
    /// Every named project, drafts included, newest first.
    var projectIndex: [JSONValue] = []
    var projectClosures: [JSONValue] = []
    var closureProject: JSONValue?
    var closureSettings: JSONValue = .object([:])
    var closureCandidates: [JSONValue] = []
    var closureTruncated = false
    var closurePlan: JSONValue?
    var closureProgress: JSONValue = .object([:])
    var closureReviewing = false
    var closureApplying = false
    var showClosure = false
    var titlebarCenter: CGFloat = 0
    var ingestDrafts: [String: IngestDraft] = [:]
    /// Quick picks everywhere: the same order as Biblioteca, without finished projects.
    var recentProjects: [JSONValue] {
        var seen = Set<String>()
        return (projectIndex.isEmpty ? snapshotProjects : projectIndex).filter { $0["stage"].string != "finished" && Array($0["kinds"].object.keys) != ["delivery"] && seen.insert($0["project"].string).inserted }
    }
    var sessionProjects: [String] = []
    /// Suggestions across pending captures and drafts; selecting one remains explicit.
    var ingestProjects: [JSONValue] {
        let pending = pendingCaptures.map { $0["tag"]["name"].string.isEmpty ? $0["project_hint"]["name"].string : $0["tag"]["name"].string }
        let draftNames = ingestDrafts.values.flatMap { [$0.project] + $0.units.map(\.project) }
        var seen = Set<String>()
        let names = sessionProjects + draftNames.sorted() + pending + recentProjects.map { $0["project"].string }
        return names.filter { !$0.isEmpty && seen.insert($0).inserted }.map { .object(["project": .string($0)]) }
    }
    var libraryQuery = ""
    /// The project just assigned from Inicio, offered to the sessions still without one.
    var lastTagged = ""
    @ObservationIgnored var tagTask: Task<Void, Never>?
    var finals: JSONValue?
    var flowSelection = "camera"
    /// Reviewing again must rebuild the finals route: a plain delivery plan would nest whole folders elsewhere.
    @ObservationIgnored var finalsGroup: JSONValue?
    /// Moving is the default: publishing and backing up is how the editing disk gets its space back.
    var finalsKeepSource = UserDefaults.standard.bool(forKey: "finalsKeepSource") { didSet { UserDefaults.standard.set(finalsKeepSource, forKey: "finalsKeepSource") } }
    var finalsChecking = false
    @ObservationIgnored var finalsClient: BackendClient?
    @ObservationIgnored var finalsCheckedAt: Date?
    var discoveryLoading = false
    var discoveryError = ""
    var loadedSourceKey = ""
    @ObservationIgnored var pendingReadID: UUID?
    @ObservationIgnored var sourceReadTask: Task<Void, Never>?
    @ObservationIgnored var changeMode: ((WindowMode) -> Void)?
    var source = "" { didSet { if source != oldValue { sourceReadTask?.cancel(); pendingReadID = nil; discoveryLoading = false; archiveExceptionConfirmed = false; units = []; plan = nil; loadedSourceKey = ""; gridSelection = [] } } }
    var bundledRuntime = false
    var captureRoot = ""
    var pendingCaptures: [JSONValue] = []
    @ObservationIgnored var captureWatchers: [CaptureWatcher] = []
    var captureSessions: [JSONValue] = []
    var captureRoots: [String] = []
    var captureError = ""
    var clipperExecutable = ""
    var clipperAvailable = false
    var retainStalkerSource = false
    var releasePlan: JSONValue?
    var releaseHistory: [JSONValue] = []
    var showRelease = false
    var auditPath = ""
    var releaseProject = ""
    var storageTargets: [JSONValue] = []
    var storageTargetID = UserDefaults.standard.string(forKey: "storageTarget") ?? "" { didSet { UserDefaults.standard.set(storageTargetID, forKey: "storageTarget") } }
    var storagePath = ""
    var storageTree: JSONValue?
    var storageSelection: String?
    var storageRun: JSONValue?
    var storageError = ""
    @ObservationIgnored var storageClient: BackendClient?
    @ObservationIgnored var storageMonitor: Task<Void, Never>?
    @ObservationIgnored var storageFixture = false
    @ObservationIgnored var watchedCaptureRoots: [String] = []
    @ObservationIgnored var captureTask: Task<Void, Never>?
    var project = "" { didSet { if project != oldValue { invalidatePlan() } } }
    var lastProject = ""
    var cardSuggestions: [String] = []
    var sourceCards: [JSONValue] = []
    @ObservationIgnored var notifiedCards: Set<String> = []
    var profile = "e64" { didSet { if profile != oldValue { invalidatePlan() } } }
    var country = "mx" { didSet { if country != oldValue { invalidatePlan() } } }
    var city = "hermosillo" { didSet { if city != oldValue { invalidatePlan() } } }
    var archive = "" { didSet { if archive != oldValue { invalidatePlan() } } }
    var work = "" { didSet { if work != oldValue { archiveExceptionConfirmed = false; invalidatePlan() } } }
    var sourceKind = "card" { didSet { if sourceKind != oldValue { invalidatePlan(); loadedSourceKey = ""; units = [] } } }
    var cleanup = true { didSet { if cleanup != oldValue { invalidatePlan() } } }
    var volumes: [JSONValue] = []
    var assets: [JSONValue] = []
    var jobs: [JSONValue] = []
    var units: [DraftUnit] = [] { didSet { if units != oldValue { invalidatePlan() } } }
    /// Library selection (asset IDs).
    var selected: Set<String> = []
    /// Ingest grid selection (unit IDs) for bulk assignment; never part of the plan itself.
    var gridSelection: Set<String> = []
    @ObservationIgnored var selectionAnchor: String?
    var typeFilter = ""
    var quickLookURL: URL?
    @ObservationIgnored var reviewedPlan: ReviewedPlan?
    var plan: JSONValue? { didSet { if plan == nil { reviewedPlan = nil } } }
    var activeScope: OperationScope?
    var busy: Bool { activeScope != nil }
    var ingestLocked: Bool { activeScope == .ingest }
    var message = String(localized: "Listo")
    var error = ""
    var python = ""
    var script = ""
    var archiveExceptionConfirmed = false { didSet { if archiveExceptionConfirmed != oldValue { invalidatePlan() } } }
    var deferWork = true { didSet { if deferWork != oldValue { invalidatePlan() } } }
    var copyChoice = "all" { didSet { if copyChoice != oldValue { archiveExceptionConfirmed = false; invalidatePlan() } } }
    var noWork: Bool { copyChoice == "none" || (copyChoice == "some" && !units.contains(where: \.copyForEditing)) }
    var ingestStep: IngestStep = .source
    var activeJobID = ""
    var jobProgress: JSONValue?
    var lastResult: JSONValue?
    var packagedBackend = false
    @ObservationIgnored var client: BackendClient?
    @ObservationIgnored var reader: PersistentReader?
    @ObservationIgnored var openWindow: (() -> Void)?
    @ObservationIgnored var openImporter: (() -> Void)?
    @ObservationIgnored var openSettings: ((String) -> Void)?
    @ObservationIgnored var statusChanged: (() -> Void)?
    @ObservationIgnored var eventTask: Task<Void, Never>?
    @ObservationIgnored var monitorTask: Task<Void, Never>?

    init() {
        let prefs = UserDefaults.standard
        prefs.register(defaults: ["showQuickImporter": true, "ingestFlow": "quick"])
        if let configURL = Bundle.main.url(forResource: "backend-config", withExtension: "json"),
           let data = try? Data(contentsOf: configURL), let config = try? JSONDecoder().decode(BackendConfiguration.self, from: data),
           let resources = Bundle.main.resourceURL {
            python = config.python
            script = config.script.hasPrefix("/") ? config.script : resources.appendingPathComponent(config.script).path
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

    // MARK: Backend

    func configure() {
        guard !busy else { return }
        UserDefaults.standard.set(python, forKey: "python")
        UserDefaults.standard.set(script, forKey: "script")
        let configuration = BackendConfiguration(python: python, script: script)
        reader?.stop()
        client = BackendClient(configuration: configuration)
        reader = PersistentReader(configuration: configuration)
        Task { await refresh(loadDefaults: true) }
    }
    /// One read for everything the shell shows at rest (was nine processes).
    func refresh(loadDefaults: Bool = false) async {
        guard let reader, reader.configuration.usable else { return }
        do {
            let data = try await reader.call("snapshot")
            volumes = data["volumes"].array
            assets = data["library"].array
            jobs = data["jobs"].array
            profiles = data["profiles"].array
            snapshotProjects = data["projects"].array
            releaseHistory = data["release_plans"].array
            projectClosures = data["project_closures"].array
            clipperAvailable = data["capabilities"]["clipper"]["available"].bool
            let integrations = data["integration_settings"]
            clipperExecutable = integrations["clipper_executable"].string
            retainStalkerSource = integrations["retain_stalker_source"].bool
            sourceCards = data["source_cards"].array
            if !data["errors"]["source_cards"].string.isEmpty {
                error = String(localized: "No se pudieron actualizar las SDs: ") + data["errors"]["source_cards"].string
            }
            if loadDefaults {
                if !integrations["capture_override"].string.isEmpty { captureRoot = integrations["capture_override"].string }
                else if !captureRoot.isEmpty { _ = try await client?.call("integration_settings", ["capture_override": .string(captureRoot)]) }
                let values = data["defaults"]
                // Offered via a suggestion; a new card never inherits it silently.
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
            statusChanged?()
        } catch { self.error = error.localizedDescription }
    }
    func operation(_ title: String, scope: OperationScope = .ingest, _ body: @escaping @MainActor () async throws -> Void) {
        guard !busy else { return }
        if scope != .settings { yieldStorageToTransfer(); finalsClient?.cancel() }
        activeScope = scope; error = ""; message = title; statusChanged?()
        Task {
            do { try await body() }
            catch {
                if let failure = error as? BackendFailure, failure.code == "plan_changed" {
                    self.plan = nil
                    self.error = String(localized: "El documento del plan no coincide con su digest. Prepara y revisa un plan nuevo antes de copiar. ") + failure.message
                } else if let failure = error as? BackendFailure, failure.code == "camera_review_required" {
                    self.error = String(localized: "Hay archivos DJI sin clasificar. Elige dron, 360 o action para cada uno y revisa de nuevo.")
                    self.typeFilter = "dji"; self.goToStep(.content)
                } else { self.error = error.localizedDescription }
                message = String(localized: "Operación detenida; revisa el estado")
            }
            activeScope = nil; monitorTask?.cancel(); monitorTask = nil
            await refresh(); statusChanged?()
            await reconcileMountedSonyCards()
        }
    }

    // MARK: Stalker libraries

    func watchCaptures() {
        guard !busy else { return }
        UserDefaults.standard.set(captureRoot, forKey: "captureRoot")
        operation(String(localized: "Actualizando bibliotecas de Stalker"), scope: .settings) {
            _ = try await self.client!.call("integration_settings", ["capture_override": .string(self.captureRoot),
                "clipper_executable": .string(self.clipperExecutable), "retain_stalker_source": .bool(self.retainStalkerSource)])
        }
    }
    #if DEBUG
    /// QA fixtures own the capture list: a watcher firing later must not swap in the real Stalker library.
    @ObservationIgnored var capturesFrozenForQA = false
    #endif
    func captureChanged(_ paths: [String]) {
        #if DEBUG
        if capturesFrozenForQA { return }
        #endif
        captureTask?.cancel()
        captureTask = Task {
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled, !busy, let reader, reader.configuration.usable else { return }
            do {
                let data = try await reader.call("capture_overview", ["override": .string(captureRoot)])
                guard !Task.isCancelled else { return }
                pendingCaptures = data["pending"].array; captureSessions = data["sessions"].array
                projectIndex = data["projects"].array
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

    // MARK: Labels

    func kindLabel(_ raw: String) -> String {
        ["file": String(localized: "Archivo"), "camera_bundle": String(localized: "Captura"), "stalker_session": "Stalker", "delivery": String(localized: "Entrega")][raw] ?? raw
    }
    func workLabel(_ raw: String) -> String {
        ["pending": String(localized: "Copia para editar pendiente"), "ready": String(localized: "Copias para editar listas"), "skipped": String(localized: "Sólo respaldo · sin copias para editar")][raw] ?? raw
    }
    func familyLabel(_ raw: String) -> String {
        ["sony": "Sony", "h6": "Zoom H6", "gopro": "GoPro", "dji": "DJI", "insta360": "Insta360", "canon": "Canon", "stalker": "Stalker"][raw] ?? String(localized: "Tarjeta")
    }
    func recommendationLabel(_ raw: String) -> String {
        switch raw {
        case "sony_format_in_camera": return String(localized: "Respaldo y copia para editar; al terminar, formatea en la cámara")
        case "audio_needs_work_disk": return String(localized: "Audio: conecta el SSD de trabajo o confirma sólo respaldo")
        case "dji_choose_category": return String(localized: "Elige dron, 360 o action en Contenido antes de revisar")
        case "stalker_batch": return String(localized: "Sesiones Stalker: proyecto por sesión en un lote")
        default: return String(localized: "Respaldo y copia para editar")
        }
    }
    /// `kind` is the job's own; only the ingest in progress may fall back to the current source.
    func stateLabel(_ state: String, kind: String? = nil) -> String {
        let stalker = (kind ?? sourceKind) == "stalker"
        switch state {
        case "copying": return String(localized: "Copiando y verificando destinos")
        case "backup_verified": return String(localized: "Archivo verificado")
        case "cleaning_source": return (kind ?? sourceKind) == "delivery" ? String(localized: "Liberando el disco de edición") : String(localized: "Limpiando SD")
        case "completed" where (kind ?? sourceKind) == "delivery": return String(localized: "Respaldo verificado · liberado del disco de edición")
        case "source_has_new_material" where (kind ?? sourceKind) == "delivery": return String(localized: "Respaldo verificado · llegaron archivos nuevos a la carpeta; no están respaldados")
        case "completed": return stalker ? String(localized: "Respaldo verificado · original Stalker liberado") : String(localized: "Respaldo verificado · SD libre")
        case "camera_action_required": return String(localized: "Respaldo verificado · libera la tarjeta desde la cámara")
        case "source_retained": return stalker ? String(localized: "Respaldo verificado · original Stalker conservado") : String(localized: "Respaldo verificado · origen conservado")
        case "source_has_new_material": return String(localized: "Respaldo verificado · aparecieron archivos nuevos en la SD; no están respaldados")
        case "paused": return String(localized: "Pausado; se puede reanudar")
        case "failed": return String(localized: "Fallo; revisa el job antes de reanudar")
        default: return state
        }
    }
    func stageLabel(_ stage: String) -> String {
        switch stage {
        case "review_inventory": return String(localized: "Revisando inventario y destinos")
        case "review_raw_archive": return String(localized: "Verificando fuentes en el respaldo")
        case "review_intermediates": return String(localized: "Leyendo intermedios")
        case "review_final": return String(localized: "Verificando entregas elegidas")
        case "review_final_archive": return String(localized: "Comparando con el final en el respaldo")
        case "review_finishing": return String(localized: "Preparando el plan para mostrarlo")
        case "copying_finals": return String(localized: "Copiando y verificando finales")
        case "verifying_finals": return String(localized: "Comprobando las entregas en el respaldo")
        case "preparing": return String(localized: "Preparando liberación")
        case "hashing_source": return String(localized: "Verificando archivos del original")
        case "matching_archive": return String(localized: "Comparando con el respaldo")
        case "validating_bundle": return String(localized: "Validando bundle Stalker")
        case "inspecting_clipper": return String(localized: "Revisando dependencias de Clipper")
        case "verifying_work_copy": return String(localized: "Verificando copia de trabajo")
        case "revalidating_source": return String(localized: "Revalidando origen")
        case "revalidating_clipper": return String(localized: "Revalidando dependencias de Clipper")
        case "verifying_archive": return String(localized: "Revalidando respaldo")
        case "metadata_backup_before", "metadata_backup_after": return String(localized: "Guardando snapshot del catálogo")
        case "relinking": return String(localized: "Actualizando referencias de Clipper")
        case "releasing_originals": return String(localized: "Liberando archivos originales")
        case "relink_complete": return String(localized: "Referencias actualizadas")
        case "release_complete", "session_finished": return String(localized: "Sesión liberada")
        default: return String(localized: "Liberando sesión Stalker")
        }
    }
    func progressLabel(_ job: JSONValue) -> String {
        // Real bytes from Transfer.report; never an invented percentage.
        let p = job["progress"], gb = { (bytes: Double) in String(format: "%.1f", bytes / 1e9) }
        if p["phase"].string == "releasing_stalker" {
            var parts = [stageLabel(p["stage"].string), String(localized: "Sesión \(Int(p["session_index"].double))/\(Int(p["sessions_total"].double))")]
            if !p["session_name"].string.isEmpty { parts.append(p["session_name"].string) }
            if p["files_total"].double > 0 { parts.append(String(localized: "\(Int(p["files_done"].double))/\(Int(p["files_total"].double)) archivos")) }
            if p["bytes_total"].double > 0 { parts.append("\(gb(p["bytes_done"].double)) / \(gb(p["bytes_total"].double)) GB") }
            if !p["path"].string.isEmpty { parts.append((p["path"].string as NSString).lastPathComponent) }
            return parts.joined(separator: " · ")
        }
        guard p["bytes_total"].double > 0 else { return stateLabel(job["state"].string) }
        let rate = p["bytes_per_second"].double > 0 ? String(format: " · %.0f MB/s", p["bytes_per_second"].double / 1e6) : ""
        let files = " · " + String(localized: "\(Int(p["files_done"].double))/\(Int(p["files_total"].double)) archivos")
        switch p["phase"].string {
        case "copying": return String(localized: "Copiando \(gb(p["copied_bytes"].double)) de \(gb(p["bytes_total"].double)) GB") + rate + files
        case "verifying": return String(localized: "Verificando destinos \(gb(p["verified_bytes"].double)) de \(gb(p["verify_total"].double)) GB") + files
        case "cleaning": return String(localized: "Limpiando SD tras verificar")
        case "checking_card": return String(localized: "Comprobando el estado de la SD")
        default: return stateLabel(job["state"].string)
        }
    }
    /// 0…1 from measured bytes, or nil when the phase has no byte total.
    func progressFraction(_ job: JSONValue) -> Double? {
        let p = job["progress"]
        if p["global_fraction"] != .null { return p["global_fraction"].double }
        switch p["phase"].string {
        case "copying" where p["bytes_total"].double > 0: return min(1, p["copied_bytes"].double / p["bytes_total"].double)
        case "verifying" where p["verify_total"].double > 0: return min(1, p["verified_bytes"].double / p["verify_total"].double)
        case "releasing_stalker" where p["bytes_total"].double > 0: return min(1, p["bytes_done"].double / p["bytes_total"].double)
        default: return nil
        }
    }
    /// The list carries these fields at the top; a single job only inside its plan.
    func cleanupProfile(_ job: JSONValue) -> String { job["cleanup_profile"].string.isEmpty ? job["plan"]["cleanup_profile"].string : job["cleanup_profile"].string }
    func jobLabel(_ job: JSONValue) -> String {
        let listed = job["source_kind"].string
        let kind = listed.isEmpty ? job["plan"]["source_kind"].string : listed
        if job["sony_removal_recorded"].bool {
            return String(localized: "Retiro de clips registrado · falta comprobar la SD")
        }
        switch job["sony_card_check"].string {
        case "ready": return String(localized: "Respaldo verificado · SD comprobada al reconectar")
        case "originals_present": return String(localized: "Respaldo verificado · reaparecieron archivos en la SD")
        default: break
        }
        // Cards holding Stalker bundles end camera_action_required without any camera step.
        if job["state"].string == "camera_action_required", cleanupProfile(job) == "files" {
            return String(localized: "Respaldo verificado · las sesiones Stalker se conservan en el origen")
        }
        if job["state"].string == "camera_action_required", cleanupProfile(job) == "sony_clips_then_format" {
            return String(localized: "Respaldo verificado · clips borrados de la SD · formatéala en la cámara")
        }
        return stateLabel(job["state"].string, kind: kind.isEmpty ? nil : kind)
    }
    func jobDetail(_ job: JSONValue) -> String {
        let reasons = job["stalker_release"]["retained"].array.map { $0["reason"].string }.filter { !$0.isEmpty }
        if !reasons.isEmpty { return reasons.joined(separator: " · ") }
        return job["error"].string
    }
    func resumable(_ job: JSONValue) -> Bool {
        ["failed", "paused", "copying", "cleaning_source", "backup_verified"].contains(job["state"].string)
            || job["state"].string == "source_retained" && job["release_stalker_source"].bool
    }
    func needsAttention(_ job: JSONValue) -> Bool {
        if sonyCardFollowup(job) { return false }
        return ["failed", "paused", "camera_action_required"].contains(job["state"].string)
            || job["state"].string == "source_retained" && job["release_stalker_source"].bool
    }
    func sonyCardFollowup(_ job: JSONValue) -> Bool {
        job["sony_removal_recorded"].bool ||
            job["state"].string == "camera_action_required" && cleanupProfile(job) == "sony_clips_then_format"
    }
    var attentionJobs: [JSONValue] { jobs.filter { needsAttention($0) && !$0["dismissed"].bool } }
    /// Only stops the reminder: the job keeps its state and its actions in Actividad.
    func dismissJob(_ id: String, dismissed: Bool = true) {
        func mark(_ value: Bool) {
            jobs = jobs.map { job in
                guard job["id"].string == id else { return job }
                var updated = job.object; updated["dismissed"] = .bool(value)
                return .object(updated)
            }
            statusChanged?()
        }
        mark(dismissed)
        Task {
            do { _ = try await oneShot("dismiss_job", ["job_id": .string(id), "dismissed": .bool(dismissed)]) }
            catch { mark(!dismissed); self.error = error.localizedDescription }
        }
    }
    /// Small catalog writes that must not wait for, or be dropped by, a running operation.
    func oneShot(_ method: String, _ params: [String: JSONValue]) async throws -> JSONValue {
        guard let client else { throw BackendFailure(code: "backend_unavailable", message: String(localized: "El motor no está disponible")) }
        return try await BackendClient(configuration: client.configuration).call(method, params)
    }

    // MARK: Sources

    func open(_ section: LibrarySection) { route = .library; librarySection = section }
    func openFiles(project: String) { libraryQuery = project; open(.files) }
    func chooseSource() {
        pickURL(message: String(localized: "Elige la carpeta o tarjeta de origen")) { url in
            if self.card(url.path) != nil { self.useCard(url.path) }
            else { self.prepareSource(url.path, kind: Self.holdsStalkerSessions(url) ? "stalker" : "card") }
        }
    }
    /// A source says what it is: nobody should have to tell nüün that a folder of bundles is Stalker.
    nonisolated static func holdsStalkerSessions(_ url: URL) -> Bool {
        if url.pathExtension.lowercased() == "stalker" { return true }
        let names = (try? FileManager.default.contentsOfDirectory(atPath: url.path)) ?? []
        return names.contains { $0.lowercased().hasSuffix(".stalker") }
    }
    func pendingSessions(in root: String) -> Int { pendingCaptures.filter { $0["root"].string == root }.count }
    /// Folders with something to back up; each one is still its own reviewed plan.
    var pendingCaptureRoots: [String] { captureRoots.filter { pendingSessions(in: $0) > 0 } }
    func useStalker() {
        guard let root = pendingCaptureRoots.first ?? captureRoots.first else { return }
        prepareSource(root, kind: "stalker")
    }
    var selectedCard: JSONValue? { sourceCards.first { $0["path"].string == source && sourceKind == "card" } }
    func card(_ path: String) -> JSONValue? { sourceCards.first { $0["path"].string == path } }
    func ejectSource() { if !source.isEmpty { eject(source) } }
    func eject(_ path: String) {
        guard !(discoveryLoading && path == source), let card = card(path) else { return }
        operation(String(localized: "Expulsando SD"), scope: .source) {
            _ = try await self.client!.call("eject_card", ["source": .string(path), "identity": card["identity"]])
            if self.source == path { self.source = "" }
            self.sourceCards.removeAll { $0["path"].string == path }
            self.cardSuggestions.removeAll { $0 == path }
            self.message = String(localized: "SD expulsada · puedes retirarla")
        }
    }
    func registerVolume(role: String) {
        pickURL(message: role == "archive" ? String(localized: "Elige la raíz del disco de archivo (MEDIA)") : String(localized: "Elige la raíz del SSD para editar")) { url in
            self.operation(String(localized: "Registrando disco"), scope: .settings) {
                _ = try await self.client!.call("register_volume", ["root": .string(url.path), "name": .string(url.lastPathComponent), "role": .string(role)])
                self.message = String(localized: "Disco registrado")
            }
        }
    }
    func volume(_ id: String) -> JSONValue? { volumes.first { $0["id"].string == id } }
    var sourceKey: String { source + "|" + sourceKind + "|" + archive }
    var contentLoaded: Bool { !source.isEmpty && loadedSourceKey == sourceKey }
    func discover() {
        sourceReadTask?.cancel(); loadedSourceKey = ""
        sourceReadTask = Task { await loadSource() }
    }
    func loadSource() async {
        let key = sourceKey, path = source
        guard !path.isEmpty, !ingestLocked, loadedSourceKey != key,
              let client, client.configuration.usable else { return }
        let readID = UUID()
        pendingReadID = readID
        discoveryLoading = true; discoveryError = ""
        let reader = BackendClient(configuration: client.configuration)
        defer { if pendingReadID == readID { pendingReadID = nil; discoveryLoading = false } }
        do {
            try await Task.sleep(for: .milliseconds(150))
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
            gridSelection = gridSelection.intersection(units.map(\.id))
            loadedSourceKey = key; plan = nil
            message = String(localized: "\(units.count) unidades · \(data["skipped"].array.count) capturas activas omitidas")
        } catch {
            if !Task.isCancelled, sourceKey == key, pendingReadID == readID { discoveryError = error.localizedDescription }
        }
    }
    func discoverAndWait() async {
        if contentLoaded { return }
        discover()
        await sourceReadTask?.value
    }
    func prepareSource(_ path: String, kind: String) {
        if source != path || sourceKind != kind { parkIngestDraft(); copyChoice = "all" }
        sourceKind = kind; cleanup = kind == "card" && card(path) != nil; source = path; route = .ingest; ingestStep = .source; plan = nil; lastResult = nil
        if kind == "stalker" { project = "" }
    }
    func prepareCaptureBatch(_ root: String) {
        prepareSource(root, kind: "stalker")
        goToStep(.content)
    }
    func useCard(_ path: String) {
        if source != path { parkIngestDraft() }
        ingestDrafts.removeValue(forKey: path)
        sourceReadTask?.cancel(); pendingReadID = nil; discoveryLoading = false; loadedSourceKey = ""
        sourceKind = "card"; cleanup = true; source = path; units = []; plan = nil; project = ""; lastResult = nil; copyChoice = "all"
        route = .ingest; ingestStep = .source
        if UserDefaults.standard.bool(forKey: "autoInspect") { discover() }
    }
    func newIngest() {
        copyChoice = "all"
        source = ""; project = ""; lastResult = nil; activeJobID = ""; jobProgress = nil; error = ""; typeFilter = ""
        ingestStep = .source; message = String(localized: "Listo")
    }

    // MARK: Ingest flow

    func invalidatePlan() {
        if plan != nil {
            plan = nil
            if !busy { message = String(localized: "Cambios pendientes · revisa el plan de nuevo") }
        }
    }

    var projectIsReady: Bool {
        let hasBatchProject = !project.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        if hasBatchProject { return true }
        let media = units.filter { !$0.isSupport }
        return !units.isEmpty && media.allSatisfy { !$0.project.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
    }
    var flowReviewKey: String {
        [sourceKey, loadedSourceKey, project, profile, country, city, work, copyChoice,
         String(cleanup), String(deferWork), String(archiveExceptionConfirmed),
         units.map { [$0.id, $0.project, $0.profile, $0.country, $0.camera, String($0.copyForEditing)].joined(separator: "|") }.joined(separator: ";")].joined(separator: "\n")
    }
    var canPrepareFlow: Bool {
        if sourceKind == "delivery" { return !source.isEmpty && !archive.isEmpty }
        let offlineWork = !noWork && !volumes.contains { $0["id"].string == work && $0["online"].bool }
        return !source.isEmpty && !archive.isEmpty && (!contentLoaded || planBlockers.isEmpty) && (!offlineWork || archiveExceptionConfirmed)
    }
    /// Only what the planner truly needs; everything else has a default or is reviewed in the plan.
    var planBlockers: [String] {
        var reasons: [String] = []
        if source.isEmpty { reasons.append(String(localized: "Elige una fuente")) }
        // Stalker sessions carry their own projects: inspect first, then decide.
        if !projectIsReady && !(sourceKind == "stalker" && !contentLoaded) { reasons.append(sourceKind == "stalker" ? String(localized: "Nombra el proyecto del lote o de cada sesión") : String(localized: "Nombra el proyecto")) }
        if archive.isEmpty { reasons.append(String(localized: "Elige un disco de respaldo")) }
        if copyChoice == "some" && !units.contains(where: \.copyForEditing) && contentLoaded { reasons.append(String(localized: "Marca qué copiar al SSD o elige «Ninguno»")) }
        if units.contains(where: \.needsCameraReview) { reasons.append(String(localized: "Clasifica los archivos DJI")) }
        return reasons
    }
    func goToStep(_ step: IngestStep) {
        guard !ingestLocked || step == .run else { return }
        ingestStep = step
        if step == .content {
            if !source.isEmpty && !contentLoaded { discover() }
        }
    }
    func nextStep() {
        switch ingestStep {
        case .source, .content: goToStep(.destinations)
        case .destinations, .review: if plan != nil { apply() } else { reviewPlan() }
        case .run: newIngest()
        }
    }
    func previousStep() {
        guard !ingestLocked else { return }
        goToStep(.content)
    }
    /// Stalker needs per-session projects: inspect on demand instead of asking the user to.
    func reviewPlan() {
        guard !busy else { return }
        if sourceKind == "delivery" {
            if let finalsGroup { planFinals(finalsGroup) } else { error = String(localized: "Prepara los finales desde Inicio o Flujos.") }
            return
        }
        let expectedSource = sourceKey
        Task {
            if !contentLoaded { await discoverAndWait() }
            guard sourceKey == expectedSource else { return }
            if !planBlockers.isEmpty { ingestStep = units.contains(where: \.needsCameraReview) ? .content : ingestStep; return }
            requestPlan()
        }
    }
    func requestPlan() {
        guard sourceKind != "stalker" || (contentLoaded && !units.isEmpty) else {
            error = String(localized: "No hay sesiones Stalker terminadas pendientes en esta fuente"); return
        }
        if noWork { makePlan(archiveOnly: true); return }
        let onlineWork = volumes.contains { $0["id"].string == work && $0["online"].bool }
        if !onlineWork && archiveExceptionConfirmed { makePlan(archiveOnly: true) }
        else if !onlineWork { error = String(localized: "Confirma aquí continuar sin el SSD para esta operación") }
        else { makePlan(archiveOnly: false) }
    }
    func makePlan(archiveOnly: Bool) {
        operation(String(localized: "Preparando plan")) {
            var assignments: [String: JSONValue] = [:]
            var tags: [String: (project: String, country: String, sessions: [JSONValue])] = [:]
            for unit in self.units where !unit.sessionID.isEmpty {
                let title = unit.project.isEmpty ? self.project : unit.project
                let country = unit.country.isEmpty ? self.country : unit.country
                guard !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { continue }
                tags[title + "|" + country, default: (title, country, [])].sessions.append(.string(unit.sessionID))
            }
            for tag in tags.values {
                _ = try await self.client!.call("save_capture_tags", ["session_ids": .array(tag.sessions), "project": .string(tag.project), "country": .string(tag.country)])
            }
            for unit in self.units {
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
            params.merge(IngestPolicy.parameters(archiveOnly: archiveOnly, skipWork: self.noWork, deferWork: self.deferWork, work: self.work)) { _, new in new }
            let document = try await self.client!.call("plan_document", params).string
            let reviewed = try ReviewedPlan(document: document)
            self.plan = reviewed.display
            self.reviewedPlan = reviewed
            self.sessionProjects = Array(Set(self.sessionProjects + reviewed.display["units"].array.filter { !$0["support"].bool }.map { $0["project"].string })).sorted()
            self.ingestStep = .review
            self.message = String(localized: "Plan listo para revisar; no se ha copiado ni borrado media")
        }
    }
    func apply() {
        guard let reviewed = reviewedPlan else { return }
        let jobID = reviewed.display["id"].string
        activeJobID = jobID; jobProgress = nil; lastResult = nil; ingestStep = .run
        operation(String(localized: "Copiando y verificando")) {
            self.monitorTask = Task {
                while !Task.isCancelled {
                    try? await Task.sleep(for: .seconds(1))
                    if Task.isCancelled { break }
                    if let job = try? await self.reader?.call("job", ["job_id": .string(jobID)]) {
                        self.jobProgress = job
                        self.message = self.progressLabel(job)
                    }
                }
            }
            let result = try await self.client!.call("apply", reviewed.applyParameters)
            self.lastResult = result
            self.message = self.stateLabel(result["state"].string)
            let release = result["stalker_release"]
            if release["status"].string == "retained" || release["status"].string == "partial" {
                let reasons = release["retained"].array.map { $0["reason"].string }.filter { !$0.isEmpty }
                self.error = String(localized: "Respaldo verificado; se conservó el original Stalker. ") + reasons.joined(separator: " · ")
            }
            if !result["metadata_backup_error"].string.isEmpty { self.error = String(localized: "Respaldo de media terminado; falta respaldar catálogo: ") + result["metadata_backup_error"].string }
            _ = try await self.client!.call("defaults", ["project": .string(self.project), "profile": .string(self.profile),
                "country": .string(self.country), "city": .string(self.city), "archive": .string(self.archive), "work": .string(self.work)])
            self.lastProject = self.project; self.plan = nil; self.loadedSourceKey = ""
        }
    }
    func cancel() { client?.cancel(); message = String(localized: "Cancelando en un punto recuperable…") }

    // MARK: Bulk assignment on the grid

    var visibleUnits: [DraftUnit] {
        switch typeFilter {
        case "": return units.filter { !$0.isSupport }
        case "all": return units
        case "dji": return units.filter { $0.detectedCamera.lowercased().hasPrefix("dji") }
        default: return units.filter { $0.displayCategory == typeFilter }
        }
    }
    /// The selection, or every visible unit when nothing is selected.
    var targetIDs: Set<String> { gridSelection.isEmpty ? Set(visibleUnits.map(\.id)) : gridSelection }
    func updateTargets(_ change: (inout DraftUnit) -> Void) {
        let ids = targetIDs
        var next = units
        for index in next.indices where ids.contains(next[index].id) { change(&next[index]) }
        units = next
    }
    func select(_ id: String, extend: Bool, range: Bool) {
        defer { if !range { selectionAnchor = id } }
        if range, let anchor = selectionAnchor, let from = visibleUnits.firstIndex(where: { $0.id == anchor }), let to = visibleUnits.firstIndex(where: { $0.id == id }) {
            gridSelection.formUnion(visibleUnits[min(from, to)...max(from, to)].map(\.id))
        } else if extend {
            if gridSelection.contains(id) { gridSelection.remove(id) } else { gridSelection.insert(id) }
        } else {
            gridSelection = gridSelection == [id] ? [] : [id]
        }
    }
    func previewURL(_ unit: DraftUnit) -> URL {
        URL(fileURLWithPath: source).appendingPathComponent(unit.previewPath ?? unit.paths.first ?? "")
    }

    // MARK: Library and jobs

    func deliver() {
        let ids = selected.sorted()
        operation(String(localized: "Preparando copias para editar"), scope: .library) {
            _ = try await self.client!.call("deliver_work", ["asset_ids": .array(ids.map(JSONValue.string)), "work": .string(self.work)])
            self.message = String(localized: "Copias para editar verificadas"); self.selected = []
        }
    }
    func skip() {
        let ids = selected.sorted()
        operation(String(localized: "Actualizando pendientes"), scope: .library) {
            _ = try await self.client!.call("set_work_state", ["asset_ids": .array(ids.map(JSONValue.string)), "state": .string("skipped")])
            self.message = String(localized: "Copias para editar omitidas"); self.selected = []
        }
    }
    func restoreSelected() {
        guard selected.count == 1, let id = selected.first else { return }
        pickURL(message: String(localized: "Elige dónde restaurar")) { url in
            self.operation(String(localized: "Restaurando selección"), scope: .library) {
                let result = try await self.client!.call("restore", ["asset_id": .string(id), "destination": .string(url.path)])
                self.message = String(localized: "Restauración verificada")
                NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: result["path"].string)])
            }
        }
    }
    func exportHandoff() {
        let panel = NSSavePanel(); panel.nameFieldStringValue = "media-handoff.json"
        let ids = selected.sorted()
        let finish: (URL) -> Void = { url in
            self.operation(String(localized: "Preparando selección de biblioteca"), scope: .library) {
                let data = try await self.client!.call("handoff", ["asset_ids": .array(ids.map(JSONValue.string))])
                try JSONEncoder().encode(data).write(to: url, options: .atomic)
                self.message = String(localized: "Selección exportada; requiere soporte de biblioteca en Clipper")
            }
        }
        if let window = NSApp.keyWindow {
            panel.beginSheetModal(for: window) { response in if response == .OK, let url = panel.url { finish(url) } }
        } else if panel.runModal() == .OK, let url = panel.url { finish(url) }
    }
    func resume(_ id: String) {
        activeJobID = id
        operation(String(localized: "Reanudando job"), scope: .library) {
            self.monitorTask = Task {
                while !Task.isCancelled {
                    try? await Task.sleep(for: .seconds(1))
                    if Task.isCancelled { break }
                    if let job = try? await self.reader?.call("job", ["job_id": .string(id)]) { self.jobProgress = job; self.message = self.progressLabel(job) }
                }
            }
            let result = try await self.client!.call("resume", ["job_id": .string(id)])
            self.message = self.stateLabel(result["state"].string)
            if !result["metadata_backup_error"].string.isEmpty { self.error = String(localized: "Respaldo de media terminado; falta respaldar catálogo: ") + result["metadata_backup_error"].string }
        }
    }

    // MARK: Mounts

    @ObservationIgnored private var sonyCheckPending = false

    private func reconcileMountedSonyCards() async {
        guard sonyCheckPending, !busy, !Task.isCancelled else { return }
        sonyCheckPending = false
        let pending = Set(jobs.filter {
            $0["state"].string == "camera_action_required" && cleanupProfile($0) == "sony_clips_then_format"
        }.map { $0["source_volume_uuid"].string }.filter { !$0.isEmpty })
        let candidates = sourceCards.filter { pending.contains($0["identity"]["volume_uuid"].string) }
        var changed = false
        for card in candidates {
            if Task.isCancelled { break }
            do {
                let result = try await oneShot("reconcile_sony_cleanup", ["source": card["path"], "identity": card["identity"]])
                changed = changed || !result["updated"].array.isEmpty
                if let last = lastResult, let observation = result["checked"].object[last["id"].string] {
                    var updated = last.object
                    updated["sony_card_check"] = observation
                    if observation.string == "ready" { updated["state"] = .string("completed") }
                    lastResult = .object(updated)
                }
            } catch let failure as BackendFailure where failure.code == "job_busy" {
                sonyCheckPending = true
            } catch {
                // Unplugging or a transient mount failure leaves the pending state intact.
            }
        }
        if changed && !Task.isCancelled { await refresh() }
    }

    func notifyCard(_ path: String) {
        guard Bundle.main.bundleIdentifier != nil else { return }  // `swift run` has no bundle for notifications.
        let card = card(path)
        Task {
            let center = UNUserNotificationCenter.current()
            guard (try? await center.requestAuthorization(options: [.alert])) == true else { return }
            let content = UNMutableNotificationContent()
            content.title = card.map { familyLabel($0["hint"]["family"].string) + " · " + $0["name"].string } ?? String(localized: "Tarjeta detectada")
            content.body = card.map { recommendationLabel($0["hint"]["recommendation"].string) } ?? String(localized: "Abre nüün para revisar el plan")
            try? await center.add(UNNotificationRequest(identifier: "card:" + path, content: content, trigger: nil))
        }
    }
    func mountChanged(arrival: String? = nil) {
        if let arrival { notifiedCards.remove(arrival) }
        eventTask?.cancel()
        eventTask = Task {
            try? await Task.sleep(for: .milliseconds(300))
            guard !Task.isCancelled, let reader, reader.configuration.usable else { return }
            await refresh()
            guard !Task.isCancelled else { return }
            if route == .library && librarySection == .disks { await loadStorage() }
            // No registered destination online: mounts stay silent (no popup, no content hints).
            guard volumes.contains(where: { $0["online"].bool }) else { cardSuggestions = []; notifiedCards = []; statusChanged?(); return }
            let shown = sourceCards.map { $0["path"].string }
            // A mount event notifies once per insertion; it never replaces work in progress.
            cardSuggestions = shown
            notifiedCards.formIntersection(shown)
            var opened = false
            for path in shown where !notifiedCards.contains(path) {
                notifiedCards.insert(path); notifyCard(path)
                if !busy && !opened && UserDefaults.standard.bool(forKey: "showQuickImporter") {
                    if source != path || lastResult != nil || (units.isEmpty && plan == nil) { useCard(path) }
                    openImporter?()
                    opened = true
                }
            }
            statusChanged?()
            sonyCheckPending = true
            await reconcileMountedSonyCards()
        }
    }
}
