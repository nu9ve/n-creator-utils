import AppKit
import SwiftUI
import MediaMenuCore

struct DraftUnit: Identifiable, Equatable {
    var id: String
    var paths: [String]
    var project = ""
    var profile = ""
    var camera = "auto"
}

@MainActor final class Model: ObservableObject {
    @Published var selectedTab = 0
    @Published var source = ""
    @Published var bundledRuntime = false
    @Published var captureRoot = ""
    @Published var pendingCaptures: [JSONValue] = []
    var captureWatcher: CaptureWatcher?
    var captureTask: Task<Void, Never>?
    @Published var project = ""
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
    var client: BackendClient?
    var openWindow: (() -> Void)?
    var statusChanged: (() -> Void)?
    var eventTask: Task<Void, Never>?
    var monitorTask: Task<Void, Never>?

    init() {
        let prefs = UserDefaults.standard
        if let configURL = Bundle.main.url(forResource: "backend-config", withExtension: "json"),
           let data = try? Data(contentsOf: configURL), let config = try? JSONDecoder().decode(BackendConfiguration.self, from: data) {
            python = config.python; script = config.script
        }
        if let resources = Bundle.main.resourceURL {
            let bundledPython = resources.appendingPathComponent("backend/runtime/bin/python3").path
            let bundledScript = resources.appendingPathComponent("backend/source/nu9ve.py").path
            if FileManager.default.isExecutableFile(atPath: bundledPython) { python = bundledPython; script = bundledScript; bundledRuntime = true }
        }
        if !bundledRuntime {
            python = prefs.string(forKey: "python") ?? python
            script = prefs.string(forKey: "script") ?? script
        }
        captureRoot = prefs.string(forKey: "captureRoot") ?? ""
        configure()
        watchCaptures()
    }
    func watchCaptures() {
        guard !busy else { return }
        captureWatcher = nil
        UserDefaults.standard.set(captureRoot, forKey: "captureRoot")
        guard !captureRoot.isEmpty, FileManager.default.fileExists(atPath: captureRoot) else { return }
        captureWatcher = CaptureWatcher(root: captureRoot) { [weak self] paths in
            Task { @MainActor in self?.captureChanged(paths) }
        }
        captureChanged([])
    }
    func captureChanged(_ paths: [String]) {
        captureTask?.cancel()
        captureTask = Task {
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled, !busy, let client, client.configuration.usable, !captureRoot.isEmpty else { return }
            do {
                let data = try await client.call("captures", ["root": .string(captureRoot), "changed_paths": .array(paths.map(JSONValue.string))])
                pendingCaptures = data["pending"].array
                if !pendingCaptures.isEmpty { message = "\(pendingCaptures.count) capturas terminadas pendientes de archivo"; statusChanged?() }
            } catch { /* A disconnected capture library is retried on mount, not polled. */ }
        }
    }
    func kindLabel(_ raw: String) -> String {
        ["file": "Archivo", "camera_bundle": "Captura", "stalker_session": "Stalker", "delivery": "Entrega"][raw] ?? raw
    }
    func workLabel(_ raw: String) -> String {
        ["pending": "Trabajo pendiente", "ready": "Trabajo listo", "skipped": "Trabajo omitido"][raw] ?? raw
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
            if loadDefaults {
                let values = try await client.call("defaults")
                project = values["project"].string
                profile = values["profile"].string.isEmpty ? "e64" : values["profile"].string
                country = values["country"].string.isEmpty ? "mx" : values["country"].string
                city = values["city"].string.isEmpty ? "hermosillo" : values["city"].string
                archive = values["archive"].string
                work = values["work"].string
            }
            if archive.isEmpty { archive = volumes.first(where: { $0["role"].string == "archive" })?["id"].string ?? "" }
            if work.isEmpty { work = volumes.first(where: { $0["role"].string == "work" })?["id"].string ?? "" }
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
        }
    }
    func chooseSource() {
        let panel = NSOpenPanel(); panel.canChooseFiles = false; panel.canChooseDirectories = true
        if panel.runModal() == .OK { source = panel.url?.path ?? ""; units = []; plan = nil }
    }
    func registerVolume(role: String) {
        let panel = NSOpenPanel(); panel.canChooseFiles = false; panel.canChooseDirectories = true
        panel.message = role == "archive" ? "Elige la raíz del disco de archivo (MEDIA)" : "Elige la raíz del SSD de trabajo"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        operation("Registrando disco") {
            _ = try await self.client!.call("register_volume", ["root": .string(url.path), "name": .string(url.lastPathComponent), "role": .string(role)])
            self.message = "Disco registrado"
        }
    }
    func discover() {
        operation("Inspeccionando fuente") {
            let data = try await self.client!.call("discover", ["source": .string(self.source)])
            self.units = data["units"].array.map { value in
                DraftUnit(id: value["id"].string, paths: value["paths"].array.map(\.string))
            }
            self.plan = nil
            self.message = "\(self.units.count) unidades; \(data["skipped"].array.count) sesiones activas omitidas"
        }
    }
    func requestPlan() {
        if sourceKind == "delivery" { makePlan(archiveOnly: true); return }
        let onlineWork = volumes.contains { $0["id"].string == work && $0["online"].bool }
        if !onlineWork { archiveOnlyPrompt = true }
        else { makePlan(archiveOnly: false) }
    }
    func makePlan(archiveOnly: Bool) {
        operation("Preparando plan") {
            var assignments: [String: JSONValue] = [:]
            for unit in self.units {
                var values: [String: JSONValue] = [:]
                if !unit.project.isEmpty { values["project"] = .string(unit.project) }
                if !unit.profile.isEmpty { values["profile"] = .string(unit.profile) }
                if unit.camera != "auto" { values["camera"] = .string(unit.camera) }
                if !values.isEmpty { for path in unit.paths { assignments[path] = .object(values) } }
            }
            var params: [String: JSONValue] = ["source": .string(self.source), "archive": .string(self.archive),
                "project": .string(self.project), "profile": .string(self.profile), "country": .string(self.country), "city": .string(self.city),
                "source_kind": .string(self.sourceKind), "cleanup": .bool(self.cleanup && self.sourceKind == "card"),
                "archive_only": .bool(archiveOnly), "confirm_archive_only": .bool(archiveOnly),
                "work_decision": .string(self.sourceKind == "delivery" ? "skipped" : (self.deferWork ? "pending" : "skipped")), "assignments": .object(assignments)]
            if !self.work.isEmpty { params["work"] = .string(self.work) }
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
                        self.message = self.stateLabel(job["state"].string)
                    }
                }
            }
            let result = try await self.client!.call("apply", ["plan": reviewed, "digest": reviewed["digest"]])
            self.message = self.stateLabel(result["state"].string)
            _ = try await self.client!.call("defaults", ["project": .string(self.project), "profile": .string(self.profile),
                "country": .string(self.country), "city": .string(self.city), "archive": .string(self.archive), "work": .string(self.work)])
            self.plan = nil
        }
    }
    func cancel() { client?.cancel(); message = "Cancelando en un punto recuperable…" }
    func stateLabel(_ state: String) -> String {
        switch state {
        case "copying": return "Copiando y verificando destinos"
        case "backup_verified": return "Archivo verificado"
        case "cleaning_source": return "Limpiando SD"
        case "completed": return "Respaldo verificado · SD libre"
        case "camera_action_required": return "Respaldo verificado · libera la tarjeta desde la cámara"
        case "source_retained": return "Respaldo verificado · origen conservado"
        case "paused": return "Pausado; se puede reanudar"
        case "failed": return "Fallo; revisa el job antes de reanudar"
        default: return state
        }
    }
    func deliver() {
        let ids = selected.sorted()
        operation("Preparando copias de trabajo") {
            _ = try await self.client!.call("deliver_work", ["asset_ids": .array(ids.map(JSONValue.string)), "work": .string(self.work)])
            self.message = "Copias de trabajo verificadas"; self.selected = []
        }
    }
    func skip() {
        let ids = selected.sorted()
        operation("Actualizando pendientes") {
            _ = try await self.client!.call("set_work_state", ["asset_ids": .array(ids.map(JSONValue.string)), "state": .string("skipped")])
            self.message = "Copias de trabajo omitidas"; self.selected = []
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
    func resume(_ id: String) {
        operation("Reanudando job") {
            let result = try await self.client!.call("resume", ["job_id": .string(id)])
            self.message = self.stateLabel(result["state"].string)
        }
    }
    func mountChanged() {
        eventTask?.cancel()
        eventTask = Task {
            try? await Task.sleep(for: .milliseconds(800))
            guard !Task.isCancelled, !busy, let client, client.configuration.usable else { return }
            await refresh()
            captureChanged([])
            guard volumes.contains(where: { $0["online"].bool }) else { return }
            let mounted = FileManager.default.mountedVolumeURLs(includingResourceValuesForKeys: [.volumeIsInternalKey], options: [.skipHiddenVolumes]) ?? []
            for url in mounted {
                guard (try? url.resourceValues(forKeys: [.volumeIsInternalKey]).volumeIsInternal) == false else { continue }
                if let suggestion = try? await client.call("suggest", ["source": .string(url.path)]), suggestion["show"].bool {
                    if source != url.path { source = url.path; units = []; plan = nil; openWindow?() }
                }
            }
            if assets.contains(where: { $0["work_delivery_status"].string == "pending" }) { statusChanged?() }
        }
    }
}

struct MainView: View {
    @ObservedObject var model: Model
    let cameras = ["auto", "generic", "sony", "canon", "nikon", "gopro", "h6", "dji-drone", "dji-360", "dji-action", "insta360"]
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack { Text("Media").font(.title2); Spacer(); Text(model.message); if model.busy { ProgressView().controlSize(.small); Button("Cancelar") { model.cancel() } } }
            if !model.error.isEmpty { Text(model.error).foregroundStyle(.red).textSelection(.enabled) }
            TabView(selection: $model.selectedTab) {
                ingest.tabItem { Label("Ingreso", systemImage: "sdcard") }.tag(0)
                library.tabItem { Label("Biblioteca y pendientes", systemImage: "square.stack") }.tag(1)
                activity.tabItem { Label("Actividad", systemImage: "clock") }.tag(2)
                settings.tabItem { Label("Discos y ajustes", systemImage: "externaldrive") }.tag(3)
            }
        }.padding().frame(minWidth: 880, minHeight: 620)
        .alert("Respaldar sin disco de trabajo", isPresented: $model.archiveOnlyPrompt) {
            Button("Cancelar", role: .cancel) {}
            Button("Confirmar sólo MEDIA") { model.makePlan(archiveOnly: true) }
        } message: { Text("Se verificará el archivo en MEDIA. El SSD no recibirá una copia ahora. La decisión pendiente/omitido elegida abajo quedará registrada y la SD podrá limpiarse tras verificar el archivo.") }
        .onChange(of: model.source) { _, _ in model.plan = nil }
        .onChange(of: model.project) { _, _ in model.plan = nil }
        .onChange(of: model.profile) { _, _ in model.plan = nil }
        .onChange(of: model.country) { _, _ in model.plan = nil }
        .onChange(of: model.city) { _, _ in model.plan = nil }
        .onChange(of: model.archive) { _, _ in model.plan = nil }
        .onChange(of: model.work) { _, _ in model.plan = nil }
        .onChange(of: model.cleanup) { _, _ in model.plan = nil }
        .onChange(of: model.units) { _, _ in model.plan = nil }
    }
    var ingest: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack { TextField("Fuente", text: $model.source); Button("Elegir…") { model.chooseSource() }; Button("Inspeccionar") { model.discover() }.disabled(model.source.isEmpty) }
            HStack { TextField("Proyecto", text: $model.project); TextField("Perfil", text: $model.profile); TextField("País", text: $model.country); TextField("Ciudad", text: $model.city) }
            HStack {
                volumePicker("Archivo", role: "archive", selection: $model.archive)
                volumePicker("Trabajo", role: "work", selection: $model.work)
            }
            HStack {
                Picker("Fuente", selection: $model.sourceKind) { Text("SD/cámara").tag("card"); Text("Stalker").tag("stalker"); Text("Entrega final").tag("delivery") }
                Toggle("Limpiar SD tras verificar", isOn: $model.cleanup).disabled(model.sourceKind != "card")
                Toggle("Trabajo pendiente si falta SSD", isOn: $model.deferWork)
            }
            Text("Asigna proyecto/perfil por unidad. Los pares y bundles se mantienen juntos.").foregroundStyle(.secondary)
            List {
                ForEach($model.units) { $unit in
                    HStack {
                        Text(unit.paths.joined(separator: "\n")).lineLimit(3).frame(maxWidth: .infinity, alignment: .leading)
                        TextField("Proyecto general", text: $unit.project).frame(width: 150)
                        TextField("Perfil general", text: $unit.profile).frame(width: 100)
                        Picker("Cámara", selection: $unit.camera) { ForEach(cameras, id: \.self) { Text($0).tag($0) } }.frame(width: 150)
                    }
                }
            }
            if let plan = model.plan {
                Text("\(plan["files"].array.count) archivos · \(plan["units"].array.count) unidades · \(plan["work_id"] == .null ? "sólo archivo confirmado" : "archivo + trabajo")")
                Text(plan["cleanup_profile"].string == "camera_action_required" ? "Limpieza pendiente en cámara tras verificar." : (plan["cleanup"].bool ? "La SD se limpiará después de verificar." : "El origen se conservará."))
                DisclosureGroup("Ver rutas y clasificación del plan") {
                    ScrollView {
                        VStack(alignment: .leading, spacing: 8) {
                            ForEach(Array(plan["preview"].array.enumerated()), id: \.offset) { _, row in
                                VStack(alignment: .leading) {
                                    Text(row["source"].string + " · " + row["project"].string + " · " + row["camera"].string)
                                    Text("Archivo: " + row["archive_path"].string).font(.caption).textSelection(.enabled)
                                    if row["work_path"] != .null { Text("Trabajo: " + row["work_path"].string).font(.caption).textSelection(.enabled) }
                                }
                            }
                        }.frame(maxWidth: .infinity, alignment: .leading)
                    }.frame(maxHeight: 150)
                }
                Button("Ejecutar plan revisado") { model.apply() }.buttonStyle(.borderedProminent)
            } else {
                Button("Revisar plan") { model.requestPlan() }.disabled(model.source.isEmpty || model.project.isEmpty || model.archive.isEmpty)
            }
        }.padding(.top).disabled(model.busy)
        .onChange(of: model.sourceKind) { _, _ in model.plan = nil }
        .onChange(of: model.deferWork) { _, _ in model.plan = nil }
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
                    Text("\(model.pendingCaptures.count) capturas Stalker terminadas pendientes")
                    Button("Preparar archivo de capturas") { model.selectedTab = 0; model.source = model.captureRoot; model.sourceKind = "stalker"; model.cleanup = false; model.plan = nil }
                }
            }
            List(model.assets, id: \.["id"].string, selection: $model.selected) { asset in
                HStack { Text(asset["project"].string); Text(asset["profile"].string).foregroundStyle(.secondary); Spacer(); Text(model.kindLabel(asset["kind"].string)); Text(model.workLabel(asset["work_delivery_status"].string)) }.tag(asset["id"].string)
            }
            HStack {
                volumePicker("SSD", role: "work", selection: $model.work)
                Button("Copiar selección al SSD") { model.deliver() }.disabled(model.selected.isEmpty || model.work.isEmpty)
                Button("Omitir trabajo") { model.skip() }.disabled(model.selected.isEmpty)
                Button("Restaurar…") { model.restoreSelected() }.disabled(model.selected.count != 1)
                Button("Exportar selección…") { model.exportHandoff() }.disabled(model.selected.isEmpty)
            }
        }.padding(.top).disabled(model.busy)
    }
    var activity: some View {
        List(model.jobs, id: \.["id"].string) { job in
            HStack { VStack(alignment: .leading) { Text(model.stateLabel(job["state"].string)); Text(job["id"].string).font(.caption).foregroundStyle(.secondary) }; Spacer(); if ["failed", "paused", "copying", "cleaning_source", "backup_verified"].contains(job["state"].string) { Button("Reanudar") { model.resume(job["id"].string) }.disabled(model.busy) } }
        }.padding(.top)
    }
    var settings: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack { Button("Registrar MEDIA…") { model.registerVolume(role: "archive") }; Button("Registrar SSD de trabajo…") { model.registerVolume(role: "work") }; Button("Actualizar") { Task { await model.refresh() } } }
            List(model.volumes, id: \.["id"].string) { volume in HStack { Text(volume["name"].string); Text(volume["role"].string == "archive" ? "Archivo" : "Trabajo"); Spacer(); Text(volume["online"].bool ? "Conectado" : "Desconectado") } }
            Text("Runtime").font(.headline)
            TextField("Carpeta Stalker a observar", text: $model.captureRoot)
            Button("Observar capturas terminadas") { model.watchCaptures() }
            TextField("Python ejecutable", text: $model.python).disabled(model.bundledRuntime)
            TextField("nu9ve.py", text: $model.script).disabled(model.bundledRuntime)
            Button("Aplicar runtime") { model.configure() }.disabled(model.bundledRuntime)
            Text(model.bundledRuntime ? "Runtime incluido en la app." : "Desarrollo: runtime externo explícito. Distribución: empaquetar runtime, firmar y notarizar antes de release.").foregroundStyle(.secondary)
        }.padding(.top).disabled(model.busy)
    }
}

@MainActor final class AppDelegate: NSObject, NSApplicationDelegate {
    let model = Model()
    var status: NSStatusItem!
    var window: NSWindow?
    var observers: [NSObjectProtocol] = []
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        status = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        status.button?.image = NSImage(systemSymbolName: "externaldrive.badge.checkmark", accessibilityDescription: "Media")
        let menu = NSMenu()
        menu.addItem(withTitle: "Abrir Media…", action: #selector(show), keyEquivalent: "")
        menu.addItem(withTitle: "Cancelar operación", action: #selector(cancel), keyEquivalent: "")
        menu.addItem(.separator())
        menu.addItem(withTitle: "Salir", action: #selector(quit), keyEquivalent: "q")
        for item in menu.items { item.target = self }
        status.menu = menu
        model.openWindow = { [weak self] in self?.show() }
        model.statusChanged = { [weak self] in
            guard let self else { return }
            self.status.button?.title = self.model.busy ? " …" : ((self.model.assets.contains { $0["work_delivery_status"].string == "pending" } || !self.model.pendingCaptures.isEmpty) ? " •" : "")
            self.status.button?.toolTip = self.model.message
        }
        let center = NSWorkspace.shared.notificationCenter
        for name in [NSWorkspace.didMountNotification, NSWorkspace.didUnmountNotification, NSWorkspace.didRenameVolumeNotification, NSWorkspace.didWakeNotification] {
            observers.append(center.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                Task { @MainActor in self?.model.mountChanged() }
            })
        }
        if CommandLine.arguments.contains("--show") { show() }
        // Normal startup has no popup or recurring idle timer. Initial catalog read only.
    }
    @objc func show() {
        if window == nil {
            let value = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1040, height: 720), styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
            value.title = "Media · n-creator-utils"; value.isReleasedWhenClosed = false
            value.contentView = NSHostingView(rootView: MainView(model: model)); value.center(); window = value
        }
        NSApp.activate(ignoringOtherApps: true); window?.makeKeyAndOrderFront(nil)
    }
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
