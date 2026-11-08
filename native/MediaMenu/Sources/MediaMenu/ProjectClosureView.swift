import SwiftUI
import MediaMenuCore

extension Model {
    var suggestedProjectClosures: [JSONValue] {
        let settled = Set(projectClosures.filter { $0["needs_release"].bool || $0["released"] != .null }.map { $0["id"].string })
        return (finals?["closure_suggestions"].array ?? []).filter { !settled.contains($0["id"].string) }
    }

    func openProjectClosure(_ item: JSONValue, stored: JSONValue? = nil) {
        guard !busy else { return }
        closureProject = item
        closureSettings = .object([:]); closureCandidates = []; closureDeliveries = []; closureProgress = .object([:])
        let id = item["project_id"].string.isEmpty ? item["id"].string : item["project_id"].string
        closurePlan = stored ?? releaseHistory.filter { $0["closure_project_id"].string == id && $0["state"].string != "completed" }.max { $0["created_at"].string < $1["created_at"].string }
        showClosure = true
        operation(String(localized: "Buscando finales del proyecto"), scope: .space) {
            let data = try await self.client!.call("closure_candidates", ["project_id": item["project_id"].string.isEmpty ? item["id"] : item["project_id"]])
            self.closureCandidates = data["files"].array
            self.closureTruncated = data["truncated"].bool
            self.closureDeliveries = data["deliveries"].array
            self.closureSettings = data["settings"]
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
        operation(review ? String(localized: "Preparando revisión del cierre") : String(localized: "Guardando entregas"), scope: .space) {
            self.closureReviewing = review
            self.closureProgress = .object([:])
            defer { self.closureReviewing = false }
            self.closurePlan = nil
            self.closureSettings = try await self.client!.call("closure_save", ["project_id": id, "required": .array(required.sorted().map { .string($0) }),
                "no_more": .bool(noMore), "published": .bool(published), "finals": .array(finals), "profile_default": .bool(profileDefault)])
            if review {
                let reviewID = JSONValue.string(UUID().uuidString)
                let monitor = self.watchClosureProgress(reviewID)
                defer { monitor.cancel() }
                do {
                    self.closurePlan = try await self.client!.call("closure_plan", ["project_id": id, "archive": .string(self.archive), "work": .string(self.work), "release_work": .bool(releaseWork), "review_id": reviewID])
                } catch let error as BackendFailure where error.code == "cancelled" {
                    self.message = String(localized: "Revisión cancelada")
                    return
                }
                self.message = String(localized: "Cierre listo para revisar; no se ha movido ni borrado media")
            } else {
                self.message = String(localized: "Entregas guardadas; no se ha movido ni borrado media")
            }
            self.closureDeliveries = (try? await self.client!.call("closure_deliveries", ["project_id": id]).array) ?? self.closureDeliveries
        }
    }
    private func watchClosureProgress(_ id: JSONValue) -> Task<Void, Never> {
        Task { @MainActor in
            while !Task.isCancelled {
                // A finished action removes its key; keep the last measure instead of blanking the view.
                if let value = try? await self.reader?.call("closure_progress", ["plan_id": id]), !Task.isCancelled, value != .object([:]) {
                    self.closureProgress = value
                }
                try? await Task.sleep(for: .milliseconds(500))
            }
        }
    }
    func applyProjectClosure() {
        guard let reviewed = closurePlan else { return }
        operation(String(localized: "Respaldando finales y liberando trabajo"), scope: .space) {
            self.closureProgress = .object([:])
            self.closureApplying = true
            defer { self.closureApplying = false }
            let monitor = self.watchClosureProgress(reviewed["id"])
            defer { monitor.cancel() }
            self.closurePlan = try await self.client!.call("closure_apply", ["plan_id": reviewed["id"], "digest": reviewed["digest"], "confirm_discard": .bool(true)])
            if let result = self.closurePlan, result["closure_settings"] != .null { self.closureSettings = result["closure_settings"] }
            self.closureDeliveries = (try? await self.client!.call("closure_deliveries", ["project_id": reviewed["closure_project_id"]]).array) ?? self.closureDeliveries
            self.message = reviewed["closure_finish"] == .bool(false) ? String(localized: "Finales publicados respaldados") : String(localized: "Proyecto completado")
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
        if !model.suggestedProjectClosures.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                SectionTitle("Sugerencias de cierre")
                ForEach(model.suggestedProjectClosures, id: \.["id"].string) { value in
                    HStack {
                        Label(value["title"].string, systemImage: "film")
                        Text("Vertical publicado").font(.caption).foregroundStyle(.secondary)
                        Spacer()
                        Button("Revisar entregas…") { model.openProjectClosure(value) }.disabled(model.busy)
                    }
                }
            }
        }
    }
}

struct ProjectClosureSheet: View {
    @Environment(\.creatorPalette) private var palette
    @Bindable var model: Model
    @Environment(\.closeSheet) private var dismiss
    @State private var required: Set<String> = []
    @State private var noMore = false
    @State private var profileDefault = false
    @State private var picked: [String: JSONValue] = [:]
    @State private var query = ""
    @State private var discardConfirmed = false
    @State private var showOtherMatches = false
    @State private var showFinalActions = true
    private let roles: [(String, LocalizedStringKey)] = [("horizontal", "Horizontal"), ("vertical", "Vertical"), ("thumbnail", "Thumbnail"), ("auxiliary", "Auxiliar")]
    private var title: String {
        guard let item = model.closureProject else { return "" }
        return item["project"].string.isEmpty ? item["title"].string : item["project"].string
    }
    private func key(_ value: JSONValue) -> String { value["volume_id"].string + "|" + value["path"].string }
    private var rows: [JSONValue] {
        let listed = Set(model.closureCandidates.map(key))
        let visible = model.closureCandidates.filter { showOtherMatches || $0["suggested"].bool || picked[key($0)] != nil }
        return picked.values.filter { !listed.contains(key($0)) }.sorted { key($0) < key($1) } + visible
    }
    private var selectedTypes: Set<String> { Set(picked.values.map { $0["role"].string }).subtracting(["auxiliary"]) }
    private var missing: Set<String> { required.subtracting(selectedTypes) }
    private var hasPublishedWork: Bool {
        picked.values.contains { value in
            value["published"].bool && model.volume(value["volume_id"].string)?["role"].string == "work"
        }
    }
    private var ready: Bool { noMore && !required.isEmpty && missing.isEmpty }
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            PageHeader(title: "Completar y liberar", subtitle: LocalizedStringKey(title))
            ErrorBanner(model: model)
            if model.closureReviewing {
                ActionProgressView(title: "Preparando revisión del cierre",
                                   note: "nüün está leyendo los archivos para verificar sus huellas. Las copias y retiradas empiezan al aplicar el plan.",
                                   state: .verify(model.closureProgress, phase: phase, volume: model.volume(model.work)?["name"].string ?? ""))
            } else if model.closureApplying {
                ActionProgressView(title: "Completando y liberando",
                                   note: "Cada entrega se copia y se verifica antes de retirar su copia de edición. Puedes pausar; lo verificado se conserva.",
                                   state: .transfer(model.closureProgress, phase: phase))
            } else if let plan = model.closurePlan {
                review(plan)
            } else {
                form
            }
            Divider()
            HStack {
                Button("Cerrar") { dismiss() }.keyboardShortcut(.cancelAction).disabled(model.busy)
                Spacer()
                if model.busy {
                    if !model.closureReviewing && !model.closureApplying {
                        ProgressView().controlSize(.small)
                        Text(model.closureProgress["stage"].string.isEmpty ? model.message : model.stageLabel(model.closureProgress["stage"].string)).font(.caption).lineLimit(2)
                    }
                    Button(model.closureReviewing ? "Cancelar revisión" : "Pausar") { model.client?.cancel() }
                } else if let plan = model.closurePlan {
                    if plan["state"].string != "completed" {
                        Button("Editar entregas") { model.closurePlan = nil; discardConfirmed = false }
                        Button(applyTitle(plan), role: plan["entries"].array.isEmpty ? nil : .destructive) { model.applyProjectClosure() }
                            .buttonStyle(CreatorButtonStyle(prominent: true))
                            .disabled((plan["entries"].array.contains { $0["category"].string != "final" } && !discardConfirmed) || !plan["consumers"]["blockers"].array.isEmpty)
                    }
                } else {
                    Button("Guardar") { save(review: false) }.disabled(required.isEmpty)
                    Button("Respaldar publicados…") { save(review: true, releaseWork: false) }
                        .disabled(required.isEmpty || !hasPublishedWork || model.archive.isEmpty || model.work.isEmpty)
                    Button("Preparar revisión…") { save(review: true) }
                        .buttonStyle(CreatorButtonStyle(prominent: true)).disabled(!ready || model.archive.isEmpty || model.work.isEmpty)
                }
            }
        }
        .padding(20).frame(minWidth: 560, idealWidth: 820, maxWidth: 820, minHeight: 440, idealHeight: 680, maxHeight: 680)
        .interactiveDismissDisabled(model.busy)
        .onChange(of: model.closureSettings) { _, config in load(config) }
        .onAppear { load(model.closureSettings); query = title }

    }
    private var phase: String {
        let stage = model.closureProgress["stage"].string
        return stage.isEmpty ? model.message : model.stageLabel(stage)
    }
    private func applyTitle(_ plan: JSONValue) -> LocalizedStringKey {
        if !plan["entries"].array.isEmpty {
            return plan["closure_finish"] == .bool(false) ? "Respaldar y liberar" : "Completar y liberar"
        }
        return plan["closure_copies"].array.isEmpty ? "Completar proyecto" : "Respaldar y completar"
    }
    /// The project's last applied closeout, if any: reopening shows what already happened first.
    private var lastClosure: JSONValue? {
        let item = model.closureProject ?? .null
        let id = item["project_id"].string.isEmpty ? item["id"].string : item["project_id"].string
        return model.releaseHistory.filter { $0["closure_project_id"].string == id && $0["state"].string == "completed" }
            .max { $0["created_at"].string < $1["created_at"].string }
    }
    /// Finals still on the editing disk and not yet published, as the engine reads them (the Projects row uses the same list).
    private var awaitingPublication: [JSONValue] { model.closureDeliveries.filter { $0["state"].string == "awaiting" } }
    /// Published on the editing disk (or moved into the published folder): «Respaldar publicados…» finishes them.
    private var publishedOnWork: [JSONValue] { model.closureDeliveries.filter { ["published", "moved"].contains($0["state"].string) } }
    private var movedDeliveries: [JSONValue] { model.closureDeliveries.filter { $0["state"].string == "moved" } }
    @ViewBuilder private var closedSummary: some View {
        if let last = lastClosure {
            let waiting = awaitingPublication
            let names = waiting.map(\.["name"].string)
            VStack(alignment: .leading, spacing: 6) {
                Label {
                    Text("Cierre aplicado el \(String(last["created_at"].string.prefix(10))) · \(last["entries"].array.count) archivos retirados · \(bytesLabel(last["bytes"].double)) liberados")
                } icon: { Image(systemName: "checkmark.seal.fill").foregroundStyle(palette.success) }
                .font(.callout.weight(.medium))
                if !publishedOnWork.isEmpty {
                    Text("\(publishedOnWork.count) entregas publicadas esperan «Respaldar publicados…» para retirar su copia de edición.")
                        .font(.caption).foregroundStyle(palette.info)
                }
                if waiting.isEmpty && publishedOnWork.isEmpty {
                    Text("No queda nada por liberar: las entregas están publicadas y respaldadas.").font(.caption).foregroundStyle(palette.muted)
                } else if !waiting.isEmpty {
                    Text("\(waiting.count) entregas siguen en edición, esperando publicarse: \(names.prefix(3).joined(separator: ", "))\(names.count > 3 ? "…" : "")")
                        .font(.caption)
                    Text("Ya están respaldadas. Cuando las publiques, «Respaldar publicados…» retira su copia de edición; no hace falta preparar el cierre otra vez.")
                        .font(.caption).foregroundStyle(palette.muted)
                }
            }
            .padding(10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(palette.raised, in: palette.panel)
        }
    }
    private var form: some View {
        VStack(alignment: .leading, spacing: 10) {
            closedSummary
            if !movedDeliveries.isEmpty {
                Label("\(movedDeliveries.count) entregas se movieron a la carpeta de publicados. La selección usa su nueva ubicación como publicada; se guarda al guardar o respaldar.", systemImage: "arrow.right.doc.on.clipboard")
                    .font(.caption).foregroundStyle(palette.info)
            }
            DisclosureGroup("Requisitos del perfil") {
                HStack {
                    ForEach(roles.filter { $0.0 != "auxiliary" }, id: \.0) { role, label in
                        Toggle(label, isOn: Binding(get: { required.contains(role) }, set: { if $0 { required.insert(role) } else { required.remove(role) } })).toggleStyle(.checkbox)
                    }
                }
                Toggle("Usar estos requisitos como predeterminados del perfil", isOn: $profileDefault).font(.caption)
            }
            if !missing.isEmpty {
                HStack(spacing: 6) {
                    Text("Faltan tipos de entrega en la selección").fontWeight(.medium)
                    ForEach(roles.filter { missing.contains($0.0) }, id: \.0) { _, label in
                        Text(label).fontWeight(.medium)
                    }
                }.font(.caption).foregroundStyle(palette.warning)
                if noMore && !selectedTypes.isEmpty {
                    Button("Usar los tipos elegidos para este proyecto") {
                        required = selectedTypes
                        profileDefault = false
                    }.buttonStyle(.link).font(.caption)
                }
            }
            HStack {
                TextField("Buscar finales por nombre", text: $query).onSubmit(search)
                Button("Buscar", action: search)
                Menu {
                    Button("Elegir todos los visibles") { select(rows) }
                    Button("Quitar selección visible") { for item in rows { picked.removeValue(forKey: key(item)) } }
                } label: { Text("\(picked.count) elegidos") }
                .fixedSize()
                .disabled(rows.isEmpty)
            }
            Text("La selección reúne las entregas del proyecto. Cada archivo conserva su estado de publicación.")
                .font(.caption).foregroundStyle(.secondary)
            if !showOtherMatches && model.closureCandidates.contains(where: { !$0["suggested"].bool && picked[key($0)] == nil }) {
                Button("Ver otras coincidencias") { showOtherMatches = true }.buttonStyle(.link).font(.caption)
            }
            ScrollView {
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(roles, id: \.0) { role, label in
                        let items = rows.filter { (picked[key($0)] ?? $0)["role"].string == role }
                        if !items.isEmpty { deliveryGroup(label, items: items) }
                    }
                    if rows.isEmpty { Text("Sin coincidencias. Cambia la búsqueda para encontrar los finales.").foregroundStyle(.secondary).padding() }
                }
            }
            if model.closureTruncated { Text("Hay más resultados. Acota la búsqueda para verlos.").font(.caption).foregroundStyle(palette.warning) }
            Toggle("Estas son todas las entregas definitivas", isOn: $noMore).toggleStyle(.checkbox)
            Text("El cierre respalda también lo pendiente de publicar y conserva su copia de edición.")
                .font(.caption).foregroundStyle(.secondary)
            Text("Preparar la revisión verifica archivos y muestra las acciones antes de aplicarlas.")
                .font(.caption).foregroundStyle(.secondary)
        }.disabled(model.busy)
    }
    private func search() {
        showOtherMatches = true
        model.findClosureFinals(query)
    }
    private func select(_ items: [JSONValue]) {
        for item in items where picked[key(item)] == nil { picked[key(item)] = item }
    }
    private func update(_ item: JSONValue, field: String, to value: JSONValue) {
        var updated = (picked[key(item)] ?? item).object
        updated[field] = value
        picked[key(item)] = .object(updated)
    }
    private func deliveryGroup(_ label: LocalizedStringKey, items: [JSONValue]) -> some View {
        let chosen = items.compactMap { picked[key($0)] }
        let waiting = chosen.filter { !$0["published"].bool }
        return DisclosureGroup {
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Button("Elegir grupo") { select(items) }.disabled(chosen.count == items.count)
                    Menu("Publicación") {
                        Button("Ya publiqué las entregas elegidas") {
                            for item in waiting { update(item, field: "published", to: .bool(true)) }
                        }.disabled(waiting.isEmpty)
                    }
                }.font(.caption)
                ForEach(Array(items.enumerated()), id: \.offset) { _, item in
                    let value = picked[key(item)] ?? item
                    HStack(spacing: 8) {
                        Toggle(isOn: Binding(get: { picked[key(item)] != nil }, set: { if $0 { select([item]) } else { picked.removeValue(forKey: key(item)) } })) {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(URL(fileURLWithPath: item["path"].string).lastPathComponent).lineLimit(1).truncationMode(.middle)
                                Text((item["volume"].string.isEmpty ? volumeName(item) : item["volume"].string) + " · " + item["path"].string)
                                    .font(.caption2).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
                            }
                        }.toggleStyle(.checkbox)
                        Spacer(minLength: 4)
                        Text(value["published"].bool ? "Publicado" : "Pendiente de publicar").font(.caption).foregroundStyle(.secondary)
                        if picked[key(item)] != nil {
                            Menu {
                                Button(value["published"].bool ? "Quitar marca de publicación" : "Marcar como publicado") {
                                    update(item, field: "published", to: .bool(!value["published"].bool))
                                }
                                Menu("Cambiar tipo") {
                                    ForEach(roles, id: \.0) { role, label in
                                        Button(label) { update(item, field: "role", to: .string(role)) }
                                    }
                                }
                            } label: { Image(systemName: "ellipsis.circle") }
                            .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize().accessibilityLabel("Opciones de la entrega")
                        }
                    }
                }
            }.padding(.vertical, 6)
        } label: {
            HStack {
                Text(label).font(.headline)
                Spacer()
                Text("\(chosen.count) de \(items.count) elegidos").foregroundStyle(.secondary)
                Text("\(chosen.count - waiting.count) publicados").foregroundStyle(.secondary)
            }.font(.caption)
        }.modifier(Panel(padding: 10))
    }
    private func review(_ plan: JSONValue) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                Label(plan["state"].string == "completed" ? (plan["closure_finish"] == .bool(false) ? "Finales respaldados" : "Proyecto completado") : "Plan de cierre", systemImage: "checkmark.shield").font(.headline)
                if plan["entries"].array.isEmpty {
                    Text("Sin archivos para retirar").font(.title3)
                } else if plan["state"].string == "completed" {
                    Text("\(plan["entries"].array.count) archivos retirados · \(bytesLabel(plan["bytes"].double))").font(.title3)
                } else {
                    Text("\(plan["entries"].array.count) archivos para retirar · \(bytesLabel(plan["bytes"].double))").font(.title3)
                }
                if plan["closure_finish"] == .bool(false) {
                    Text("Sólo se respaldan y retiran los finales publicados elegidos. Los raws y los intermedios se conservan.").font(.callout)
                } else {
                    Text("Se verifican las fuentes y entregas en el respaldo. Los finales pendientes de publicar conservan su copia de edición.").font(.callout)
                }
                Text("Clipper: el editable se conserva; elimínalo desde Clipper para retirar también su registro. Final Cut y el contenido desconocido se conservan.").font(.caption).foregroundStyle(.secondary)
                ForEach(Array(plan["consumers"]["blockers"].array.enumerated()), id: \.offset) { _, blocker in
                    Label(blocker.string, systemImage: "exclamationmark.triangle").foregroundStyle(palette.warning).textSelection(.enabled)
                }
                if !plan["consumers"]["relinks"].array.isEmpty {
                    Text("Clipper se reenlaza a las copias verificadas durante esta operación; sus colas deben estar libres.").font(.caption)
                }
                finalActions(plan)
                releaseGroup("Copias raw", category: "raw", plan: plan)
                releaseGroup("Intermedios", category: "intermediate", plan: plan)
                DisclosureGroup("Conservar") {
                    ForEach(Array((plan["preserved"].array + plan["retained_editors"].array).enumerated()), id: \.offset) { _, value in Text(value.string).font(.caption).textSelection(.enabled) }
                }
                if !plan["metadata_backup_error"].string.isEmpty { Label(plan["metadata_backup_error"].string, systemImage: "exclamationmark.triangle").foregroundStyle(palette.warning) }
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
    private func finalActions(_ plan: JSONValue) -> some View {
        let finals = plan["closure_finals"].array.filter { $0["role"].string != "source" }
        let archive = model.volume(plan["archive_id"].string)?["name"].string ?? String(localized: "Respaldo")
        return DisclosureGroup("Entregas y respaldo", isExpanded: $showFinalActions) {
            ForEach(Array(finals.enumerated()), id: \.offset) { _, final in
                let path = final["original"]["path"].string
                let action = finalAction(final, plan: plan)
                VStack(alignment: .leading, spacing: 2) {
                    HStack {
                        Text(URL(fileURLWithPath: path.isEmpty ? final["archive_path"].string : path).lastPathComponent)
                        Spacer()
                        actionBadge(action.0, icon: action.1, color: action.2)
                    }.font(.caption)
                    Text(verbatim: archive + " · " + final["archive_path"].string)
                        .font(.caption2).foregroundStyle(.secondary).textSelection(.enabled)
                }.padding(.vertical, 3)
            }
        }
    }
    private func finalAction(_ final: JSONValue, plan: JSONValue) -> (LocalizedStringKey, String, Color) {
        let completed = plan["state"].string == "completed"
        let copies = final["copy_required"] == .null ? plan["closure_copies"].array.contains { transfer in
            transfer["preview"].array.contains { $0["archive_path"].string.hasSuffix("/" + final["archive_path"].string) }
        } : final["copy_required"].bool
        if final["keep_work"].bool {
            return !completed && copies
                ? ("Respaldar · conservar en edición", "doc.on.doc", palette.info)
                : ("Ya en respaldo · conservar en edición", "checkmark.circle", palette.success)
        }
        let original = final["original"]
        let removal = plan["entries"].array.first {
            $0["category"].string == "final" && $0["archive_path"] == final["archive_path"] &&
            (original["path"].string.isEmpty || (original["volume_id"] == plan["work_id"] && $0["path"] == original["path"]))
        }
        if let removal {
            if completed || plan["cleaned"][removal["path"].string].string == "done" {
                return ("Respaldado · copia de edición retirada", "checkmark.circle", palette.success)
            }
            let label: LocalizedStringKey = copies ? "Respaldar · retirar de edición" : "Ya en respaldo · retirar de edición"
            return (label, "arrow.right.circle", palette.warning)
        }
        return ("Ya en respaldo · sin cambios", "checkmark.circle", palette.success)
    }
    private func actionBadge(_ title: LocalizedStringKey, icon: String, color: Color) -> some View {
        Label(title, systemImage: icon)
            .font(.caption.weight(.medium)).foregroundStyle(color)
            .padding(.horizontal, 7).padding(.vertical, 3)
            .background(palette.raised, in: palette.pill)
    }
    private func releaseGroup(_ label: LocalizedStringKey, category: String, plan: JSONValue) -> some View {
        let entries = plan["entries"].array.filter { $0["category"].string == category }
        return Group {
            if !entries.isEmpty {
                DisclosureGroup {
                    ForEach(Array(entries.enumerated()), id: \.offset) { _, entry in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(entry["path"].string).font(.caption).textSelection(.enabled)
                            if category != "intermediate" {
                                Text(entry["archive_path"].string).font(.caption2).foregroundStyle(.secondary).textSelection(.enabled)
                            }
                        }.padding(.vertical, 3).frame(maxWidth: .infinity, alignment: .leading)
                    }
                } label: {
                    HStack {
                        Text(label)
                        Spacer()
                        Text("\(entries.count) archivos · \(bytesLabel(entries.reduce(0) { $0 + $1["stamp"]["size"].double }))")
                            .font(.caption).foregroundStyle(.secondary)
                        if plan["state"].string == "completed" {
                            actionBadge("Retirados", icon: "checkmark.circle", color: palette.success)
                        } else if category == "intermediate" {
                            actionBadge("Descartar sin respaldo", icon: "trash", color: palette.danger)
                        } else {
                            actionBadge("Liberar copia de edición", icon: "arrow.right.circle", color: palette.warning)
                        }
                    }
                }
            } else if plan["closure_finish"].bool && plan["closure_inventory_checked"].bool {
                HStack {
                    Text(label)
                    Spacer()
                    Text("Sin archivos en las rutas revisadas").font(.caption).foregroundStyle(.secondary)
                    actionBadge("Sin acción", icon: "minus.circle", color: palette.muted)
                }
            }
        }
    }
    private func volumeName(_ value: JSONValue) -> String { model.volumes.first { $0["id"] == value["volume_id"] }?["name"].string ?? "" }
    private func load(_ config: JSONValue) {
        required = Set(config["required"].array.map(\.string)); noMore = config["no_more"].bool
        picked = Dictionary(config["finals"].array.map { item in
            var value = item.object
            if value["published"] == nil { value["published"] = .bool(config["published"].bool) }
            if let candidate = model.closureCandidates.first(where: { key($0) == key(item) }), candidate["publication_changed"].bool {
                value["published"] = candidate["published"]
            }
            return (key(item), JSONValue.object(value))
        }, uniquingKeysWith: { _, new in new })
        // The same file moved into the published folder (engine-matched by name, size, mtime and inode): follow it.
        for moved in model.closureDeliveries where moved["state"].string == "moved" {
            guard let old = picked.removeValue(forKey: key(moved)) else { continue }
            var value = old.object
            value["path"] = moved["current_path"]; value["published"] = .bool(true)
            picked[moved["volume_id"].string + "|" + moved["current_path"].string] = .object(value)
        }
        if config["updated_at"].string.isEmpty {
            select(model.closureCandidates.filter { $0["suggested"].bool && $0["published"].bool })
        }
    }
    private func save(review: Bool, releaseWork: Bool = true) {
        model.saveProjectClosure(required: required, noMore: noMore, published: false,
                                 finals: picked.keys.sorted().compactMap { picked[$0] }, profileDefault: profileDefault, review: review, releaseWork: releaseWork)
    }
}
