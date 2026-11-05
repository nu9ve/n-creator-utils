import SwiftUI
import MediaMenuCore

extension Model {
    func openProjectClosure(_ item: JSONValue, stored: JSONValue? = nil) {
        guard !busy else { return }
        closureProject = item
        closureSettings = .object([:]); closureCandidates = []; closureProgress = .object([:])
        let id = item["project_id"].string.isEmpty ? item["id"].string : item["project_id"].string
        closurePlan = stored ?? releaseHistory.filter { $0["closure_project_id"].string == id && $0["state"].string != "completed" }.max { $0["created_at"].string < $1["created_at"].string }
        showClosure = true
        operation(String(localized: "Buscando finales del proyecto"), scope: .space) {
            let data = try await self.client!.call("closure_candidates", ["project_id": item["project_id"].string.isEmpty ? item["id"] : item["project_id"]])
            self.closureSettings = data["settings"]
            self.closureCandidates = data["files"].array
            self.closureTruncated = data["truncated"].bool
        }
    }
    func findClosureFinals(_ query: String) {
        guard let item = closureProject else { return }
        operation(String(localized: "Buscando finales del proyecto"), scope: .space) {
            let data = try await self.client!.call("closure_candidates", ["project_id": item["project_id"].string.isEmpty ? item["id"] : item["project_id"], "query": .string(query)])
            self.closureCandidates = data["files"].array
            self.closureTruncated = data["truncated"].bool
        }
    }
    func saveProjectClosure(required: Set<String>, noMore: Bool, published: Bool, finals: [JSONValue], profileDefault: Bool, review: Bool, releaseWork: Bool = true) {
        guard let item = closureProject else { return }
        let id = item["project_id"].string.isEmpty ? item["id"] : item["project_id"]
        operation(String(localized: "Comprobando cierre del proyecto"), scope: .space) {
            self.closurePlan = nil
            self.closureSettings = try await self.client!.call("closure_save", ["project_id": id, "required": .array(required.sorted().map { .string($0) }),
                "no_more": .bool(noMore), "published": .bool(published), "finals": .array(finals), "profile_default": .bool(profileDefault)])
            if review {
                self.closurePlan = try await self.client!.call("closure_plan", ["project_id": id, "archive": .string(self.archive), "work": .string(self.work), "release_work": .bool(releaseWork)])
                self.message = String(localized: "Cierre listo para revisar; no se ha movido ni borrado media")
            } else {
                self.message = String(localized: "Entregas guardadas; no se ha movido ni borrado media")
            }
        }
    }
    func applyProjectClosure() {
        guard let reviewed = closurePlan else { return }
        operation(String(localized: "Respaldando finales y liberando trabajo"), scope: .space) {
            self.closureProgress = .object([:])
            let monitor = Task { @MainActor in
                while !Task.isCancelled {
                    if let value = try? await self.reader?.call("closure_progress", ["plan_id": reviewed["id"]]) { self.closureProgress = value }
                    try? await Task.sleep(for: .milliseconds(500))
                }
            }
            defer { monitor.cancel() }
            self.closurePlan = try await self.client!.call("closure_apply", ["plan_id": reviewed["id"], "digest": reviewed["digest"], "confirm_discard": .bool(true)])
            if let result = self.closurePlan, result["closure_settings"] != .null { self.closureSettings = result["closure_settings"] }
            self.message = reviewed["closure_finish"] == .bool(false) ? String(localized: "Finales publicados respaldados") : String(localized: "Proyecto liberado; fuentes y finales conservados en el respaldo")
            self.captureChanged([])
            self.finalsCheckedAt = nil
        }
    }
}

struct ProjectClosureReminders: View {
    @Bindable var model: Model
    private var pending: [JSONValue] { model.projectClosures.filter { $0["needs_release"].bool } }
    var body: some View {
        if !pending.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                SectionTitle("Falta liberar trabajo")
                Text("Entregas declaradas completas. Revisa el respaldo actual y lo que queda en el disco de edición.").font(.caption).foregroundStyle(.secondary)
                ForEach(pending, id: \.["id"].string) { item in
                    HStack {
                        Label(item["title"].string, systemImage: "folder.badge.checkmark")
                        Text(model.profileName(item["profile"].string)).font(.caption).foregroundStyle(.secondary)
                        Spacer()
                        Button("Revisar cierre…") { model.openProjectClosure(item) }.disabled(model.busy)
                    }
                }
            }
        }
    }
}

struct ProjectClosureSheet: View {
    @Bindable var model: Model
    @Environment(\.dismiss) private var dismiss
    @State private var required: Set<String> = []
    @State private var noMore = false
    @State private var published = false
    @State private var profileDefault = false
    @State private var picked: [String: JSONValue] = [:]
    @State private var query = ""
    @State private var discardConfirmed = false
    @State private var filesExpanded = true
    private let roles: [(String, LocalizedStringKey)] = [("horizontal", "Horizontal"), ("vertical", "Vertical"), ("thumbnail", "Thumbnail"), ("auxiliary", "Auxiliar")]
    private var projectID: String { model.closureProject?["project_id"].string ?? "" }
    private var title: String {
        guard let item = model.closureProject else { return "" }
        return item["project"].string.isEmpty ? item["title"].string : item["project"].string
    }
    private func key(_ value: JSONValue) -> String { value["volume_id"].string + "|" + value["path"].string }
    private var rows: [JSONValue] {
        let listed = Set(model.closureCandidates.map(key))
        return picked.values.filter { !listed.contains(key($0)) }.sorted { key($0) < key($1) } + model.closureCandidates
    }
    private var ready: Bool { noMore && !picked.isEmpty && picked.values.allSatisfy { $0["published"].bool } && !required.isEmpty && required.isSubset(of: Set(picked.values.map { $0["role"].string })) }
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            PageHeader(title: "Completar y liberar", subtitle: LocalizedStringKey(title))
            ErrorBanner(model: model)
            if let plan = model.closurePlan {
                review(plan)
            } else {
                form
            }
            Divider()
            HStack {
                Button("Cerrar") { dismiss() }.keyboardShortcut(.cancelAction).disabled(model.busy)
                Spacer()
                if model.busy {
                    ProgressView().controlSize(.small)
                    Text(model.closureProgress["stage"].string.isEmpty ? model.message : model.stageLabel(model.closureProgress["stage"].string)).font(.caption).lineLimit(2)
                    Button("Pausar") { model.client?.cancel() }
                } else if let plan = model.closurePlan {
                    if plan["state"].string != "completed" {
                        Button("Editar entregas") { model.closurePlan = nil; discardConfirmed = false }
                        Button("Respaldar y liberar", role: .destructive) { model.applyProjectClosure() }
                            .buttonStyle(CreatorButtonStyle(prominent: true))
                            .disabled((plan["entries"].array.contains { $0["category"].string != "final" } && !discardConfirmed) || !plan["consumers"]["blockers"].array.isEmpty)
                    }
                } else {
                    Button("Guardar") { save(review: false) }.disabled(required.isEmpty)
                    Button("Respaldar publicados…") { save(review: true, releaseWork: false) }
                        .disabled(required.isEmpty || !picked.values.contains { $0["published"].bool } || model.archive.isEmpty || model.work.isEmpty)
                    Button("Revisar liberación") { save(review: true) }
                        .buttonStyle(CreatorButtonStyle(prominent: true)).disabled(!ready || model.archive.isEmpty || model.work.isEmpty)
                }
            }
        }
        .padding(20).frame(width: 820, height: 680)
        .interactiveDismissDisabled(model.busy)
        .onChange(of: model.closureSettings) { _, config in load(config) }
        .onAppear { load(model.closureSettings); query = title }

    }
    private var form: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("Entregas requeridas").font(.headline)
                ForEach(roles.filter { $0.0 != "auxiliary" }, id: \.0) { role, label in
                    Toggle(label, isOn: Binding(get: { required.contains(role) }, set: { if $0 { required.insert(role) } else { required.remove(role) } })).toggleStyle(.checkbox)
                }
            }
            Toggle("Usar estos requisitos como predeterminados del perfil", isOn: $profileDefault).font(.caption)
            HStack {
                Toggle("Ya no habrá más entregas", isOn: $noMore)
                Toggle("Marcar las elegidas como publicadas", isOn: Binding(get: { !picked.isEmpty && picked.values.allSatisfy { $0["published"].bool } }, set: { value in
                    for key in Array(picked.keys) { var item = picked[key]!.object; item["published"] = .bool(value); picked[key] = .object(item) }
                }))
            }.toggleStyle(.checkbox)
            Text("Vincula los finales de este proyecto, también si ya están en el respaldo. Los nombres sólo sugieren; tú confirmas la selección y el tipo.").font(.caption).foregroundStyle(.secondary)
            HStack {
                TextField("Buscar finales por nombre", text: $query).onSubmit { model.findClosureFinals(query) }
                Button("Buscar") { model.findClosureFinals(query) }
                Text("\(picked.count) elegidos").font(.caption).monospacedDigit()
            }
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 6) {
                    ForEach(Array(rows.enumerated()), id: \.offset) { _, item in
                        HStack(spacing: 10) {
                            Toggle(isOn: Binding(get: { picked[key(item)] != nil }, set: { if $0 { picked[key(item)] = item } else { picked.removeValue(forKey: key(item)) } })) {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(URL(fileURLWithPath: item["path"].string).lastPathComponent).lineLimit(1).truncationMode(.middle)
                                    Text((item["volume"].string.isEmpty ? volumeName(item) : item["volume"].string) + " · " + item["path"].string).font(.caption2).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
                                }
                            }.toggleStyle(.checkbox)
                            Spacer(minLength: 4)
                            if picked[key(item)] != nil {
                                Toggle("Publicado", isOn: Binding(get: { picked[key(item)]?["published"].bool ?? false }, set: { value in
                                    var next = picked[key(item)]!.object; next["published"] = .bool(value); picked[key(item)] = .object(next)
                                })).toggleStyle(.checkbox).font(.caption)
                                Picker("Tipo", selection: Binding(get: { picked[key(item)]?["role"].string ?? item["role"].string }, set: { value in
                                    var next = (picked[key(item)] ?? item).object; next["role"] = .string(value); picked[key(item)] = .object(next)
                                })) {
                                    ForEach(roles, id: \.0) { value, label in Text(label).tag(value) }
                                }.labelsHidden().frame(width: 125)
                            }
                        }.padding(.vertical, 3)
                        Divider()
                    }
                    if rows.isEmpty { Text("Sin coincidencias. Cambia la búsqueda para encontrar los finales.").foregroundStyle(.secondary).padding() }
                }
            }
            if model.closureTruncated { Text("Hay más resultados. Acota la búsqueda para verlos.").font(.caption).foregroundStyle(.orange) }
            Text("Guardar sólo registra tus decisiones. Revisar calcula el respaldo necesario y los archivos que se pueden liberar.").font(.caption).foregroundStyle(.secondary)
        }.disabled(model.busy)
    }
    private func review(_ plan: JSONValue) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                Label(plan["state"].string == "completed" ? (plan["closure_finish"] == .bool(false) ? "Finales respaldados" : "Proyecto liberado") : "Plan de cierre", systemImage: "checkmark.shield").font(.headline)
                Text("\(plan["entries"].array.count) archivos · " + bytesLabel(plan["bytes"].double)).font(.title3)
                if plan["closure_finish"] == .bool(false) {
                    Text("Sólo se respaldan y retiran los finales publicados elegidos. Los raws y los intermedios se conservan.").font(.callout)
                } else {
                    Text("Sólo se conserva en el respaldo lo original y los finales. Los intermedios listados se descartan sin respaldarlos.").font(.callout)
                }
                Text("Clipper: el editable se conserva; elimínalo desde Clipper para retirar también su registro. Final Cut y el contenido desconocido se conservan.").font(.caption).foregroundStyle(.secondary)
                if !plan["closure_copies"].array.isEmpty { Text("Finales por copiar y verificar: \(plan["closure_copies"].array.count)") }
                ForEach(Array(plan["consumers"]["blockers"].array.enumerated()), id: \.offset) { _, blocker in
                    Label(blocker.string, systemImage: "exclamationmark.triangle").foregroundStyle(.orange).textSelection(.enabled)
                }
                if !plan["consumers"]["relinks"].array.isEmpty {
                    Text("Clipper se reenlaza a las copias verificadas durante esta operación; sus colas deben estar libres.").font(.caption)
                }
                DisclosureGroup("Archivos del plan", isExpanded: $filesExpanded) {
                    ForEach(Array(plan["entries"].array.enumerated()), id: \.offset) { _, entry in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(entry["path"].string).font(.caption).textSelection(.enabled)
                            if entry["category"].string == "intermediate" {
                                Text("Descartar intermedio · sin respaldo").font(.caption2).foregroundStyle(.orange)
                            } else {
                                Text("Liberar tras verificar: " + entry["archive_path"].string).font(.caption2).foregroundStyle(.secondary)
                            }
                        }.padding(.vertical, 3).frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
                DisclosureGroup("Conservar") {
                    ForEach(Array((plan["preserved"].array + plan["retained_editors"].array).enumerated()), id: \.offset) { _, value in Text(value.string).font(.caption).textSelection(.enabled) }
                }
                if !plan["metadata_backup_error"].string.isEmpty { Label(plan["metadata_backup_error"].string, systemImage: "exclamationmark.triangle").foregroundStyle(.orange) }
                if plan["state"].string != "completed" && plan["entries"].array.contains(where: { $0["category"].string != "final" }) {
                    Toggle("Revisé la lista: los intermedios son descartables y ninguna otra edición necesita los archivos a retirar", isOn: $discardConfirmed)
                        .toggleStyle(.checkbox).disabled(model.busy)
                }
                if model.busy && model.closureProgress["released_files"].double > 0 {
                    Text("Archivos retirados: \(Int(model.closureProgress["released_files"].double)) de \(plan["entries"].array.count)").font(.caption).monospacedDigit()
                }
            }.frame(maxWidth: .infinity, alignment: .leading)
        }
    }
    private func volumeName(_ value: JSONValue) -> String { model.volumes.first { $0["id"] == value["volume_id"] }?["name"].string ?? "" }
    private func load(_ config: JSONValue) {
        required = Set(config["required"].array.map(\.string)); noMore = config["no_more"].bool; published = config["published"].bool
        picked = Dictionary(config["finals"].array.map { item in
            var value = item.object
            if value["published"] == nil { value["published"] = .bool(config["published"].bool) }
            return (key(item), JSONValue.object(value))
        }, uniquingKeysWith: { _, new in new })
    }
    private func save(review: Bool, releaseWork: Bool = true) {
        model.saveProjectClosure(required: required, noMore: noMore, published: false,
                                 finals: picked.keys.sorted().compactMap { picked[$0] }, profileDefault: profileDefault, review: review, releaseWork: releaseWork)
    }
}
