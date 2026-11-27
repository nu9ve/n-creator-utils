import AppKit
import SwiftUI
import Nu9veActionProgress
import NuunCore

/// A copy between two work disks being prepared (docs/contracts/suite-work-copy-v1.md).
struct WorkCopyRequest: Identifiable {
    let id = UUID()
    var source: String
    var destination: String
    var paths: [String] = []
    /// Replace what changed on the destination, keeping its previous version in nüün's folder there.
    var update = false
}

// MARK: Model

extension Model {
    /// The suggestions for the connected disks. A disk that arrives brings back what «Ahora no» hid; with the person's
    /// automatic schedule, due conversations are backed up here, while nüün is open (no LaunchAgent).
    func loadSuiteSuggestions(arrival: Bool) async {
        if arrival { hiddenSuggestions = [] }
        guard let found = try? await oneShot("suite_suggestions", [:]) else { return }
        suiteSuggestions = found["suggestions"].array
        lastConversationSuggestionCheck = .now
        if arrival || conversations == nil { await loadConversations() }
        if conversations?["settings"]["schedule"]["mode"].string == "auto",
           suiteSuggestions.contains(where: { $0["flow"].string == "conversations_backup" }) {
            await runConversations(when: "due")
        }
    }
    func checkConversationUpdates(force: Bool = false) async {
        let started = ContinuousClock.now
        if !force, let lastConversationSuggestionCheck,
           lastConversationSuggestionCheck.duration(to: started) < .seconds(10) { return }
        lastConversationSuggestionCheck = started
        guard let found = try? await oneShot("suite_suggestions", ["conversations_only": .bool(true)]),
              !Task.isCancelled else {
            if lastConversationSuggestionCheck == started { lastConversationSuggestionCheck = nil }
            return
        }
        guard lastConversationSuggestionCheck == started else { return }
        suiteSuggestions.removeAll { $0["flow"].string == "conversations_update" }
        suiteSuggestions.append(contentsOf: found["suggestions"].array)
    }
    var visibleSuiteSuggestions: [JSONValue] {
        suiteSuggestions.filter { !hiddenSuggestions.contains($0["key"].string) }
    }
    func hideSuggestion(_ key: String) { hiddenSuggestions.insert(key) }
    /// «No volver a preguntar» for this disk and flow; it stays in the catalog until brought back.
    func silenceSuggestion(_ key: String) {
        suiteSuggestions.removeAll { $0["key"].string == key }
        Task {
            do { _ = try await oneShot("suggestions_dismiss", ["key": .string(key), "dismissed": .bool(true)]) }
            catch { self.error = error.localizedDescription }
        }
    }
    func openWorkCopy(_ suggestion: JSONValue? = nil) {
        if let suggestion {
            workCopyRequest = WorkCopyRequest(source: suggestion["source"]["root"].string,
                                              destination: suggestion["destination"]["root"].string,
                                              paths: suggestion["paths"].array.map { $0["path"].string },
                                              update: suggestion["update"].bool)
        } else {
            workCopyRequest = WorkCopyRequest(source: "", destination: "")
        }
    }
    func loadConversations() async {
        conversations = try? await oneShot("conversations_status", [:])
    }
    func conversationSettings(_ params: [String: JSONValue]) {
        Task {
            do {
                _ = try await oneShot("conversations_settings", params)
                await loadConversations()
                await loadSuiteSuggestions(arrival: false)
            } catch { self.error = error.localizedDescription }
        }
    }
    func runConversations(when: String) async {
        guard !conversationsRunning else { return }
        conversationsRunning = true
        // One measure for every collection, read while the one-shot engine copies (U1.1, 2026-11-26).
        let progressID = newProgressID()
        startAction(.conversations, progressID)
        defer { stopAction(.conversations); conversationsRunning = false }
        do {
            let result = try await oneShot("conversations_run", ["when": .string(when), "progress_id": .string(progressID)])
            let copied = result["results"].array.reduce(0) { $0 + Int($1["result"]["copied"].double) + Int($1["result"]["versioned"].double) }
            if !result["results"].array.isEmpty {
                message = String(localized: "Conversaciones respaldadas: \(copied) archivos nuevos o cambiados")
            }
            await loadConversations()
            if let found = try? await oneShot("suite_suggestions", [:]) { suiteSuggestions = found["suggestions"].array }
        } catch { self.error = error.localizedDescription }
    }
    func updateConversationCopy(_ suggestion: JSONValue) async {
        guard !conversationsRunning else { return }
        conversationsRunning = true
        conversationUpdateResult = ""
        defer { conversationsRunning = false }
        do {
            let destination = suggestion["destination"]
            let result = try await oneShot("conversations_export_working", [
                "destination": destination["root"], "volume_id": destination["volume_id"],
                "collections": .array(suggestion["collections"].array.map { $0["id"] }),
                "confirm_personal": .bool(true)
            ])
            let copied = result["results"].array.reduce(0) { $0 + Int($1["copied"].double) }
            let pending = result["results"].array.reduce(0) { $0 + Int($1["changed_during_copy"].double) }
            conversationUpdateResult = pending > 0
                ? String(localized: "Copia actualizada · \(copied) archivos copiados · \(pending) pendientes")
                : String(localized: "Copia de conversaciones actualizada · \(copied) archivos copiados")
            await checkConversationUpdates(force: true)
        } catch { self.error = error.localizedDescription }
    }
}

// MARK: Inicio

/// Suggestions for the connected disks: only what has something new, each with «Ahora no» and «No volver a preguntar».
struct SuiteSuggestionsSection: View {
    @Bindable var model: Model
    var body: some View {
        let groups = Self.grouped(model.visibleSuiteSuggestions)
        if !groups.isEmpty {
            SectionTitle("Discos conectados")
            ForEach(groups, id: \.self) { keys in
                let members = keys.compactMap { key in model.visibleSuiteSuggestions.first { $0["key"].string == key } }
                if members.count == 1 { SuiteSuggestionCard(model: model, suggestion: members[0]) }
                else { SuiteSuggestionGroupCard(model: model, members: members) }
            }
        }
        if !model.conversationUpdateResult.isEmpty {
            Label(model.conversationUpdateResult, systemImage: "externaldrive")
                .font(.callout).foregroundStyle(.secondary)
        }
    }
}

extension SuiteSuggestionsSection {
    /// One card per pair of work disks (both directions together) and one for the conversations to copy to several
    /// disks, so the same two disks do not ask twice (U1.2, 2026-11-26). Keys keep the engine's order.
    static func grouped(_ suggestions: [JSONValue]) -> [[String]] {
        var order: [String] = [], members: [String: [String]] = [:]
        for suggestion in suggestions {
            let flow = suggestion["flow"].string
            let group: String
            if flow.hasPrefix("work_copy") {
                let pair = [suggestion["source"]["volume_id"].string, suggestion["destination"]["volume_id"].string].sorted()
                group = "disks:" + pair.joined(separator: ":")
            } else if flow == "conversations_update" {
                group = "conversations_update"
            } else {
                group = suggestion["key"].string
            }
            if members[group] == nil { order.append(group) }
            members[group, default: []].append(suggestion["key"].string)
        }
        return order.compactMap { members[$0] }
    }
}

/// Several suggestions about the same disks in one card: one line and one button per direction or disk.
struct SuiteSuggestionGroupCard: View {
    @Environment(\.creatorPalette) private var palette
    @Bindable var model: Model
    let members: [JSONValue]
    @State private var confirming: JSONValue?
    private var conversations: Bool { members.first?["flow"].string == "conversations_update" }
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .top) {
                Image(systemName: conversations ? "bubble.left.and.text.bubble.right" : "arrow.triangle.2.circlepath")
                    .font(.title2).foregroundStyle(palette.accent)
                VStack(alignment: .leading, spacing: 4) {
                    Text(title).font(.headline)
                    ForEach(members, id: \.["key"].string) { member in
                        Text(verbatim: line(member)).font(.caption).foregroundStyle(.secondary).lineLimit(2)
                    }
                }
                Spacer()
            }
            HStack {
                Button("Ahora no") { members.forEach { model.hideSuggestion($0["key"].string) } }
                Button("No volver a preguntar") { members.forEach { model.silenceSuggestion($0["key"].string) } }
                    .help("Para estos discos y este flujo. Se puede volver a activar.")
                Spacer()
                if conversations && model.conversationsRunning { ProgressView().controlSize(.small) }
                ForEach(members, id: \.["key"].string) { member in
                    Button(button(member)) {
                        if conversations { confirming = member } else { model.openWorkCopy(member) }
                    }
                    .buttonStyle(CreatorButtonStyle(prominent: member["key"] == members.first?["key"]))
                    .disabled(model.busy || model.conversationsRunning)
                }
            }
        }.modifier(Panel())
        .confirmationDialog(String(localized: "¿Actualizar la copia en \(confirming?["destination"]["label"].string ?? "")?"),
                            isPresented: Binding(get: { confirming != nil }, set: { if !$0 { confirming = nil } })) {
            Button("Sí, actualizar") {
                if let member = confirming { Task { await model.updateConversationCopy(member) } }
                confirming = nil
            }
        }
    }
    private var title: String {
        if conversations {
            return String(localized: "Conversaciones por copiar a \(members.map { $0["destination"]["label"].string }.joined(separator: " y "))")
        }
        let labels = [members[0]["source"]["label"].string, members[0]["destination"]["label"].string]
        return String(localized: "\(labels[0]) y \(labels[1]) tienen trabajo por llevar de uno a otro")
    }
    private func line(_ member: JSONValue) -> String {
        if conversations {
            return member["destination"]["label"].string + ": " + member["collections"].array.map {
                String(localized: "\($0["title"].string) · \(Int($0["count"].double)) archivos")
            }.joined(separator: " · ")
        }
        return "\(member["source"]["label"].string) → \(member["destination"]["label"].string): "
            + String(localized: "\(Int(member["count"].double)) archivos · \(bytesLabel(member["bytes"].double))")
            + " · " + member["paths"].array.prefix(2).map(\.["path"].string).joined(separator: " · ")
    }
    private func button(_ member: JSONValue) -> String {
        conversations
            ? String(localized: "Actualizar \(member["destination"]["label"].string)…")
            : "\(member["source"]["label"].string) → \(member["destination"]["label"].string)…"
    }
}

struct SuiteSuggestionCard: View {
    @Environment(\.creatorPalette) private var palette
    @Bindable var model: Model
    let suggestion: JSONValue
    @State private var confirmingUpdate = false
    var body: some View {
        let key = suggestion["key"].string
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .top) {
                Image(systemName: symbol).font(.title2).foregroundStyle(palette.accent)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.headline)
                    Text(detail).font(.caption).foregroundStyle(.secondary)
                }
                Spacer()
            }
            HStack {
                Button("Ahora no") { model.hideSuggestion(key) }
                Button("No volver a preguntar") { model.silenceSuggestion(key) }
                    .help("Para este disco y este flujo. Se puede volver a activar.")
                Spacer()
                if suggestion["flow"].string == "conversations_backup" && model.conversationsRunning {
                    ActionLine(snapshot: model.actionSnapshot(.conversations), idle: conversationPhase(model.actionStage(.conversations)))
                } else if suggestion["flow"].string == "conversations_update" && model.conversationsRunning {
                    ProgressView().controlSize(.small)
                }
                Button(action) {
                    if suggestion["flow"].string.hasPrefix("work_copy") { model.openWorkCopy(suggestion) }
                    else if suggestion["flow"].string == "conversations_update" { confirmingUpdate = true }
                    else { Task { await model.runConversations(when: "now") } }
                }
                .buttonStyle(CreatorButtonStyle(prominent: true))
                .disabled(model.busy || model.conversationsRunning)
            }
        }.modifier(Panel())
        .confirmationDialog(String(localized: "¿Actualizar la copia en \(suggestion["destination"]["label"].string)?"),
                            isPresented: $confirmingUpdate) {
            Button("Sí, actualizar") { Task { await model.updateConversationCopy(suggestion) } }
        } message: {
            Text(detail)
        }
    }
    private var symbol: String {
        switch suggestion["flow"].string {
        case "work_copy": "externaldrive.badge.plus"
        case "work_copy_changed": "arrow.triangle.2.circlepath"
        default: "bubble.left.and.text.bubble.right"
        }
    }
    private var title: String {
        switch suggestion["flow"].string {
        case "work_copy":
            String(localized: "\(suggestion["source"]["label"].string) tiene trabajo que no está en \(suggestion["destination"]["label"].string)")
        case "work_copy_changed":
            String(localized: "\(suggestion["source"]["label"].string) tiene trabajo nuevo o cambiado desde la última copia con \(suggestion["destination"]["label"].string)")
        case "conversations_backup":
            String(localized: "Conversaciones sin respaldar en \(suggestion["archive"]["name"].string)")
        case "conversations_update":
            String(localized: "Conversaciones por copiar a \(suggestion["destination"]["label"].string)")
        default:
            String(localized: "La copia de trabajo de tus conversaciones quedó atrás")
        }
    }
    private var detail: String {
        switch suggestion["flow"].string {
        case "work_copy":
            suggestion["paths"].array.prefix(3).map { "\($0["path"].string) (\($0["app"].string))" }.joined(separator: " · ")
                + (suggestion["count"].double > 3 ? " …" : "")
        case "work_copy_changed":
            String(localized: "\(Int(suggestion["count"].double)) archivos · \(bytesLabel(suggestion["bytes"].double))") + " · "
                + suggestion["paths"].array.prefix(3).map(\.["path"].string).joined(separator: " · ")
        case "conversations_update":
            suggestion["collections"].array.map {
                String(localized: "\($0["title"].string) · \(Int($0["count"].double)) archivos")
            }.joined(separator: "  ·  ")
        default:
            suggestion["collections"].array.map(\.["title"].string).joined(separator: ", ")
        }
    }
    private var action: LocalizedStringKey {
        switch suggestion["flow"].string {
        case "work_copy", "work_copy_changed": "Revisar copia…"
        case "conversations_backup": "Respaldar ahora"
        case "conversations_update": "Actualizar copia de trabajo…"
        default: "Actualizar copia"
        }
    }
}

// MARK: Copiar al otro disco

/// Copy work from one suite work disk to another: choose, review the plan, copy and verify. Nothing is deleted.
struct WorkCopySheet: View {
    @Environment(\.creatorPalette) private var palette
    @Bindable var model: Model
    @State var request: WorkCopyRequest
    @State private var apps: JSONValue?
    @State private var chosen: Set<String> = []
    @State private var plan: JSONValue?
    @State private var done: JSONValue?
    @State private var inFlight = false
    @State private var copying = false
    @State private var failure = ""
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if copying {
                ActionProgressScreen(model.actionSnapshot(.action), title: "Copiando y verificando",
                                     note: "Mantén conectados ambos discos hasta que aparezca el recibo.")
            } else {
                selection
            }
        }
        .padding(20).frame(minWidth: 560, minHeight: 440)
        .task {
            chosen = Set(request.paths)
            if !request.source.isEmpty { await loadApps() }
        }
    }
    private var selection: some View {
        Group {
            Label("Copiar al otro disco", systemImage: "externaldrive.badge.plus").font(.title3.weight(.semibold))
            Text("Copia verificada entre dos discos de trabajo: conserva rutas y fechas, deja un recibo en los dos discos y no borra nada.")
                .font(.callout).foregroundStyle(.secondary)
            disk("Desde", path: request.source) { request.source = $0; chosen = []; plan = nil; Task { await loadApps() } }
            disk("Hacia", path: request.destination) { request.destination = $0; plan = nil }
            if let apps {
                candidates(apps)
            } else if !request.source.isEmpty && failure.isEmpty {
                HStack { ProgressView().controlSize(.small); Text("Leyendo los índices del disco…").font(.caption).foregroundStyle(.secondary) }
            }
            if let plan, done == nil { review(plan) }
            if let done { result(done) }
            if !failure.isEmpty { Label(failure, systemImage: "exclamationmark.triangle").foregroundStyle(palette.warning).textSelection(.enabled) }
            Spacer(minLength: 0)
            HStack {
                if inFlight { ProgressView().controlSize(.small) }
                Spacer()
                Button(done == nil ? "Cancelar" : "Cerrar") { model.workCopyRequest = nil }.disabled(inFlight && plan != nil)
                if done == nil {
                    if let plan {
                        Button("Copiar y verificar") { apply(plan) }.buttonStyle(CreatorButtonStyle(prominent: true)).disabled(inFlight)
                    } else {
                        Button("Revisar") { review() }.buttonStyle(CreatorButtonStyle(prominent: true))
                            .disabled(inFlight || chosen.isEmpty || request.source.isEmpty || request.destination.isEmpty)
                    }
                }
            }
        }
    }
    private func disk(_ label: LocalizedStringKey, path: String, set: @escaping (String) -> Void) -> some View {
        HStack {
            Text(label).frame(width: 60, alignment: .leading)
            Text(path.isEmpty ? String(localized: "Sin elegir") : path).font(.callout).lineLimit(1).truncationMode(.middle)
                .foregroundStyle(path.isEmpty ? .secondary : .primary)
            Spacer()
            Button("Elegir disco…") {
                pickURL(message: String(localized: "Elige la raíz de un disco de trabajo"), directory: URL(fileURLWithPath: "/Volumes")) { set($0.path) }
            }.disabled(inFlight)
        }
    }
    private func candidates(_ apps: JSONValue) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Qué copiar").font(.headline)
            let projects = projectsOf(apps)
            if !projects.isEmpty {
                Text("Proyectos de nüün en este disco · índice del \(projects[0].indexed)").font(.caption).foregroundStyle(.secondary)
                ForEach(projects, id: \.key) { project in
                    Toggle(isOn: Binding(get: { project.paths.allSatisfy(chosen.contains) }, set: { on in
                        if on { chosen.formUnion(project.paths) } else { chosen.subtract(project.paths) }
                        plan = nil
                    })) {
                        VStack(alignment: .leading, spacing: 1) {
                            Text(project.title).font(.callout)
                            Text("\(project.paths.joined(separator: " · ")) · \(bytesLabel(project.bytes))").font(.caption).foregroundStyle(.secondary)
                                .lineLimit(2).truncationMode(.middle)
                            if project.includesMissionMetadata {
                                Text("Incluye el historial de misiones y el mapa de fotos.").font(.caption).foregroundStyle(.secondary)
                            }
                        }
                    }.toggleStyle(.checkbox).disabled(inFlight)
                }
            }
            let roots = rootsOf(apps)
            if roots.isEmpty && projects.isEmpty {
                Text("Las apps de este disco no declaran carpetas en su índice. Puedes elegir una carpeta a mano.").font(.caption).foregroundStyle(.secondary)
            }
            ForEach(roots, id: \.path) { root in
                Toggle(isOn: Binding(get: { chosen.contains(root.path) }, set: { on in
                    if on { chosen.insert(root.path) } else { chosen.remove(root.path) }
                    plan = nil
                })) {
                    VStack(alignment: .leading, spacing: 1) {
                        Text(root.path).font(.callout)
                        Text(root.note).font(.caption).foregroundStyle(.secondary)
                    }
                }.toggleStyle(.checkbox).disabled(inFlight)
            }
            ForEach(chosen.filter { path in !roots.contains { $0.path == path } && !projects.contains { $0.paths.contains(path) } }.sorted(), id: \.self) { path in
                HStack { Image(systemName: "folder"); Text(path).font(.callout); Spacer()
                    Button("Quitar") { chosen.remove(path); plan = nil }.buttonStyle(.borderless) }
            }
            Button("Otra carpeta…", systemImage: "folder.badge.plus") {
                pickURL(message: String(localized: "Elige una carpeta del disco de origen"), directory: URL(fileURLWithPath: request.source)) { url in
                    let base = URL(fileURLWithPath: request.source).standardizedFileURL.path
                    let path = url.standardizedFileURL.path
                    guard path.hasPrefix(base + "/") else { failure = String(localized: "Esa carpeta no está en el disco de origen"); return }
                    chosen.insert(String(path.dropFirst(base.count + 1))); plan = nil
                }
            }.disabled(inFlight || request.source.isEmpty)
            Toggle(isOn: Binding(get: { request.update }, set: { request.update = $0; plan = nil })) {
                VStack(alignment: .leading, spacing: 1) {
                    Text("Actualizar lo que cambió")
                    Text("Si un archivo ya está en el destino con otro contenido, lo reemplaza después de verificar la copia y guarda la versión anterior en la carpeta de nüün de ese disco. Sin esto, el plan se detiene.")
                        .font(.caption).foregroundStyle(.secondary)
                }
            }.toggleStyle(.checkbox).disabled(inFlight)
        }
    }
    private struct Project {
        let key: String
        let title: String
        let paths: [String]
        let bytes: Double
        let indexed: String
        let includesMissionMetadata: Bool
    }
    private func nuunDatasets(_ apps: JSONValue) -> [JSONValue] {
        apps["apps"].array.first { $0["app"].string == "nuun" && $0["index"].string == "ok" }?["datasets"].array ?? []
    }
    /// nüün's own index on the disk (published from the Mac that has the catalog): its projects and their folders,
    /// so another Mac can copy a project in progress without that catalog.
    private func projectsOf(_ apps: JSONValue) -> [Project] {
        guard let nuun = apps["apps"].array.first(where: { $0["app"].string == "nuun" && $0["index"].string == "ok" }) else { return [] }
        let titles = Dictionary(nuun["projects"].array.map { ($0["project_key"].string, $0["title"].string) }, uniquingKeysWith: { a, _ in a })
        var folders: [String: [String: Double]] = [:]
        for dataset in nuun["datasets"].array {
            for key in dataset["projects"].array.map(\.string) { folders[key, default: [:]][dataset["path"].string, default: 0] += dataset["size"]["bytes"].double }
        }
        return folders.map { key, found in
            let paths = Self.outermost(Array(found.keys))
            return Project(key: key, title: titles[key] ?? key, paths: paths, bytes: found.values.reduce(0, +),
                           indexed: String(nuun["generated_at"].string.prefix(10)),
                           includesMissionMetadata: !missionMetadata(for: Set(paths)).isEmpty)
        }.sorted { $0.title.localizedStandardCompare($1.title) == .orderedAscending }
    }
    private func missionMetadata(for paths: Set<String>) -> [String] {
        guard let apps else { return [] }
        return nuunDatasets(apps).filter { dataset in
            let path = dataset["path"].string
            return paths.contains { path == $0 || path.hasPrefix($0 + "/") }
        }.flatMap { $0["work_copy_companions"].array.map(\.string) }
    }
    /// The paths not inside another one of them (a plan refuses overlapping paths).
    static func outermost(_ paths: [String]) -> [String] {
        let all = Set(paths.filter { !$0.isEmpty })
        return all.filter { path in !all.contains { $0 != path && path.hasPrefix($0 + "/") } }.sorted()
    }
    private struct Root { let path: String; let note: String }
    private func rootsOf(_ apps: JSONValue) -> [Root] {
        apps["apps"].array.filter { $0["index"].string == "ok" && $0["app"].string != "nuun" }.flatMap { app in
            app["roots"].array.compactMap { root -> Root? in
                let path = root["volume_path"].string
                guard !path.isEmpty else { return nil }
                let projects = app["root_projects"][path].array.map(\.string).joined(separator: ", ")
                let note = app["app"].string + (projects.isEmpty ? "" : " · " + projects) + " · " + String(localized: "índice del \(app["generated_at"].string.prefix(10))")
                return Root(path: path, note: note)
            }
        }
    }
    private func review(_ plan: JSONValue) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("Plan revisado").font(.headline)
            Text("\(Int(plan["file_count"].double)) archivos · \(bytesLabel(plan["bytes"].double)) por copiar · \(Int(plan["adopt"]["files"].double)) ya están iguales")
            if plan["replace"]["files"].double > 0 {
                Text("\(Int(plan["replace"]["files"].double)) cambiaron y se actualizan; su versión anterior queda en la carpeta de nüün del destino.")
                    .font(.caption).foregroundStyle(palette.warning)
            }
            let projects = plan["context"]["projects"].array.map { plan["context"]["titles"][$0.string].string.isEmpty ? $0.string : plan["context"]["titles"][$0.string].string }
            if !projects.isEmpty { Text("Proyectos: \(projects.joined(separator: ", "))").font(.caption) }
            let carried = Set(plan["carried"].array.map(\.["app"].string)).sorted()
            if !carried.isEmpty {
                Text("Lleva una copia exacta de los índices de \(carried.joined(separator: ", ")) en la carpeta de nüün del destino, para que esa app registre la copia sin volver a calcular hashes.")
                    .font(.caption).foregroundStyle(.secondary)
            }
        }.modifier(Panel())
    }
    private func result(_ done: JSONValue) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Label("Copia verificada", systemImage: "checkmark.seal.fill").foregroundStyle(palette.success).font(.headline)
            Text("\(Int(done["receipt"]["copied"].double)) copiados · \(Int(done["receipt"]["replaced"].double)) actualizados · \(Int(done["receipt"]["adopted"].double)) ya estaban iguales")
            Text("Recibo en los dos discos: \(done["receipt"]["path"].string)").font(.caption).textSelection(.enabled)
        }.modifier(Panel())
    }
    private func loadApps() async {
        failure = ""; apps = nil
        do { apps = try await model.oneShot("volume_apps", ["root": .string(request.source)]) }
        catch { failure = error.localizedDescription }
    }
    private func review() {
        inFlight = true; failure = ""
        Task {
            defer { inFlight = false }
            do {
                let paths = Self.outermost(Array(chosen) + missionMetadata(for: chosen))
                plan = try await model.oneShot("work_copy_plan", ["source": .string(request.source), "destination": .string(request.destination),
                                                                  "paths": .array(paths.map(JSONValue.string)),
                                                                  "update": .bool(request.update)])
            } catch let failure as BackendFailure where failure.code == "destination_conflict" {
                self.failure = String(localized: "Hay archivos distintos con el mismo nombre en el destino. Marca «Actualizar lo que cambió» para reemplazarlos guardando su versión anterior, o elige otra carpeta.")
            } catch { failure = error.localizedDescription }
        }
    }
    private func apply(_ plan: JSONValue) {
        inFlight = true; copying = true; failure = ""
        Task {
            model.watchAction("work_copy", plan["id"].string)
            defer {
                model.stopAction(.action)
                model.runningAction = ""
                inFlight = false
                copying = false
            }
            do {
                done = try await model.oneShot("work_copy_apply", ["plan_id": plan["id"], "digest": plan["digest"]])
                await model.loadSuiteSuggestions(arrival: false)
            } catch { failure = error.localizedDescription }
        }
    }
}

// MARK: Ajustes → Conversaciones

/// Which conversations leave this Mac, to which disk and when; and the working copy. Every «sí» is recorded with its date.
/// What the conversations backup is doing while there is no amount yet, from the record's common stage (U1.1).
func conversationPhase(_ stage: String) -> String {
    switch stage {
    case "copying", "verifying": String(localized: "Copiando y verificando el historial")
    case "finishing": String(localized: "Colección respaldada")
    case "listing", "scanning": String(localized: "Leyendo el historial")
    default: String(localized: "Preparando")
    }
}

struct ConversationsSettings: View {
    @Environment(\.creatorPalette) private var palette
    @Bindable var model: Model
    @State private var confirming: (id: String, title: String)?
    @State private var confirmWorking: URL?
    @State private var exportCollections: Set<String> = []
    @State private var exportDisk: URL?
    @State private var exporting = false
    @State private var exportResult = ""
    var body: some View {
        let status = model.conversations, settings = status?["settings"] ?? .null
        VStack(alignment: .leading, spacing: 12) {
          if model.conversationsRunning {
            // While it copies, the action replaces the pane (nu9ve-interaction-v1 §7): stage, files and bytes.
            ActionProgressScreen(model.actionSnapshot(.conversations), title: "Respaldando conversaciones",
                                 note: "Puedes seguir usando nüün. Mantén conectado \(settings["archive"].string) hasta que aparezca el resultado.")
                .frame(height: 460)
          } else {
            Label("Conversaciones", systemImage: "bubble.left.and.text.bubble.right").font(.title3.weight(.semibold))
            Text("Respalda el historial de tus agentes (Claude Code, Codex, Claude) en un disco de respaldo, en una carpeta por dispositivo. Nunca credenciales, ajustes ni cachés. Nada sale de esta Mac sin que lo decidas aquí.")
                .foregroundStyle(.secondary)
            Picker("Disco de respaldo", selection: Binding(get: { settings["archive"].string }, set: { name in
                model.conversationSettings(["archive": name.isEmpty ? .null : .string(name), "confirm_personal": .bool(true)])
            })) {
                Text("Sin elegir").tag("")
                ForEach(model.volumes.filter { $0["role"].string == "archive" }, id: \.["name"].string) { Text($0["name"].string).tag($0["name"].string) }
            }.frame(maxWidth: 360)
            Text("En \(settings["base"].string)/<colección>/<dispositivo>/. Cambiar de disco cuenta como una decisión nueva.").font(.caption).foregroundStyle(.secondary)
            ForEach(status?["collections"].array ?? [], id: \.["id"].string) { collection in row(collection) }
            HStack {
                Picker("Cuándo", selection: Binding(get: { settings["schedule"]["mode"].string }, set: { mode in
                    model.conversationSettings(["schedule": .object(["mode": .string(mode), "every_days": settings["schedule"]["every_days"]])])
                })) {
                    Text("A pedido").tag("manual")
                    Text("Sugerir").tag("suggest")
                    Text("Automático").tag("auto")
                }.frame(maxWidth: 260)
                Stepper("cada \(Int(settings["schedule"]["every_days"].double)) días", value: Binding(
                    get: { Int(settings["schedule"]["every_days"].double) },
                    set: { days in model.conversationSettings(["schedule": .object(["mode": settings["schedule"]["mode"], "every_days": .integer(Int64(days))])]) }),
                    in: 1...365).disabled(settings["schedule"]["mode"].string == "manual")
            }
            Text("Automático respalda cuando toca, con nüün abierto y el disco conectado. Sugerir lo propone en Inicio al conectar el disco.")
                .font(.caption).foregroundStyle(.secondary)
            Divider()
            Text("Copia de trabajo").font(.headline)
            let working = status?["working"] ?? .null
            if working["decided_at"].string.isEmpty {
                Text("Opcional: después de cada respaldo, una copia verificada con la misma forma en un disco de trabajo o una carpeta de análisis, para que nove la lea en sitio.")
                    .font(.caption).foregroundStyle(.secondary)
            } else {
                Label("\(working["root"].string) · decidido el \(working["decided_at"].string.prefix(10))", systemImage: "checkmark.circle")
                    .font(.callout).textSelection(.enabled)
            }
            HStack {
                Button(working["decided_at"].string.isEmpty ? "Elegir carpeta…" : "Cambiar…") {
                    pickURL(message: String(localized: "Elige el disco o la carpeta de la copia de trabajo")) { confirmWorking = $0 }
                }
                if !working["decided_at"].string.isEmpty { Button("Dejar de copiar") { model.conversationSettings(["working": .null]) } }
            }
            HStack {
                Spacer()
                if model.conversationsRunning { ProgressView().controlSize(.small) }
                Button("Respaldar ahora") { Task { await model.runConversations(when: "now") } }
                    .buttonStyle(CreatorButtonStyle(prominent: true))
                    .disabled(model.conversationsRunning || settings["archive"].string.isEmpty
                              || !(status?["collections"].array.contains { !$0["decided_at"].string.isEmpty } ?? false))
            }
            Divider()
            DisclosureGroup("Exportar historial a un SSD de trabajo") {
                Text("Crea una copia verificable para consultar en otra Mac. Vuelve a exportar cuando quieras actualizarla.")
                    .font(.caption).foregroundStyle(.secondary)
                ForEach((status?["collections"].array ?? []).filter { $0["present"].bool }, id: \.["id"].string) { collection in
                    let id = collection["id"].string
                    Toggle(collection["title"].string, isOn: Binding(get: { exportCollections.contains(id) }, set: { enabled in
                        if enabled { exportCollections.insert(id) } else { exportCollections.remove(id) }
                    })).toggleStyle(.checkbox).disabled(exporting)
                }
                Button("Elegir SSD…") {
                    pickURL(message: String(localized: "Elige la raíz de un SSD de trabajo"), directory: URL(fileURLWithPath: "/Volumes")) { exportDisk = $0 }
                }.disabled(exportCollections.isEmpty || exporting || model.conversationsRunning)
                if exporting { ProgressView().controlSize(.small) }
                if !exportResult.isEmpty { Text(exportResult).font(.caption).textSelection(.enabled) }
            }
          }
        }
        .task { await model.loadConversations() }
        .confirmationDialog(confirming.map { String(localized: "¿Respaldar \($0.title) en \(settings["archive"].string)?") } ?? "",
                            isPresented: Binding(get: { confirming != nil }, set: { if !$0 { confirming = nil } })) {
            Button("Sí, respaldar") {
                if let id = confirming?.id { model.conversationSettings(["collections": .object([id: .bool(true)]), "confirm_personal": .bool(true)]) }
                confirming = nil
            }
        } message: {
            Text("Tus conversaciones son personales. Su historial saldrá de esta Mac hacia ese disco, en la carpeta de este dispositivo, cada vez que respaldes. Puedes quitarlo cuando quieras; lo ya copiado se queda en el disco.")
        }
        .confirmationDialog(String(localized: "¿Dejar una copia de trabajo en \(confirmWorking?.path ?? "")?"),
                            isPresented: Binding(get: { confirmWorking != nil }, set: { if !$0 { confirmWorking = nil } })) {
            Button("Sí, copiar ahí") {
                if let url = confirmWorking { model.conversationSettings(["working": .object(["root": .string(url.path)]), "confirm_personal": .bool(true)]) }
                confirmWorking = nil
            }
        } message: {
            Text("Una copia de tus conversaciones respaldadas quedará también ahí, para analizarlas. No va a un disco de respaldo.")
        }
        .confirmationDialog(String(localized: "¿Exportar el historial a \(exportDisk?.lastPathComponent ?? "")?"),
                            isPresented: Binding(get: { exportDisk != nil }, set: { if !$0 { exportDisk = nil } })) {
            Button("Sí, exportar") {
                if let disk = exportDisk { export(to: disk) }
                exportDisk = nil
            }
        } message: {
            Text("Los historiales elegidos son personales. Se copiarán en una carpeta de este dispositivo. Puedes actualizar la copia cuando lo decidas.")
        }
    }
    private func export(to disk: URL) {
        exporting = true
        exportResult = ""
        let collections = exportCollections.sorted().map(JSONValue.string)
        Task {
            defer { exporting = false }
            do {
                let response = try await model.oneShot("conversations_export_working", ["destination": .string(disk.path),
                    "collections": .array(collections), "confirm_personal": .bool(true)])
                let results = response["results"].array
                let copied = results.reduce(0) { $0 + Int($1["copied"].double) }
                let changed = results.reduce(0) { $0 + Int($1["changed_during_copy"].double) }
                exportResult = String(localized: "Copia en \(disk.lastPathComponent) · \(copied) archivos copiados · \(changed) cambiaron durante la copia")
                await model.checkConversationUpdates(force: true)
            } catch { exportResult = error.localizedDescription }
        }
    }
    private func row(_ collection: JSONValue) -> some View {
        let decided = !collection["decided_at"].string.isEmpty
        return HStack(alignment: .top) {
            Toggle(isOn: Binding(get: { decided }, set: { on in
                if on { confirming = (collection["id"].string, collection["title"].string) }
                else { model.conversationSettings(["collections": .object([collection["id"].string: .bool(false)])]) }
            })) {
                VStack(alignment: .leading, spacing: 1) {
                    Text(collection["title"].string).fontWeight(.medium)
                    Text(caption(collection)).font(.caption).foregroundStyle(.secondary)
                }
            }
            .toggleStyle(.checkbox)
            .disabled(!collection["present"].bool || model.conversations?["settings"]["archive"].string.isEmpty != false)
            Spacer()
            if collection["due"].bool { Text("Toca respaldar").font(.caption).foregroundStyle(palette.warning) }
        }
    }
    private func caption(_ c: JSONValue) -> String {
        guard c["present"].bool else { return String(localized: "No está en esta Mac") }
        var parts = [String(localized: "\(Int(c["history"]["files"].double)) archivos · \(bytesLabel(c["history"]["bytes"].double))")]
        if !c["decided_at"].string.isEmpty { parts.append(String(localized: "decidido el \(String(c["decided_at"].string.prefix(10)))")) }
        let last = c["backup"]["updated_at"].string
        parts.append(last.isEmpty ? String(localized: "nunca respaldado") : String(localized: "último respaldo \(String(last.prefix(10)))"))
        return parts.joined(separator: " · ")
    }
}
