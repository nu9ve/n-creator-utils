import SwiftUI
import NuunCore

/// What exists and where it stands: references and counts, not files. The stage comes from receipts;
/// finishing or pausing is declared by the person and deletes nothing.
struct ProjectsView: View {
    @Bindable var model: Model
    @AppStorage("projectFilter") private var filter = "progress"
    @State private var query = ""
    @State private var expanded: String?
    private let filters: [(String, LocalizedStringKey)] = [("progress", "En progreso"), ("backed_up", "Respaldados"), ("awaiting", "Por publicar"), ("finished", "Terminados"), ("all", "Todos")]
    private func matches(_ item: JSONValue, _ key: String) -> Bool {
        switch key {
        case "progress": return ["draft", "in_progress"].contains(item["stage"].string) && item["state"].string != "paused"
        case "backed_up": return item["stage"].string == "backed_up"
        // Released (closed) but with deliveries still on the editing disk: computed by the engine, not here.
        case "awaiting": return item["stage"].string == "finished" && ProjectStageBadge.waiting(item) > 0
        // Finished means nothing left on the editing disk; a released project still waiting is under «Por publicar».
        case "finished": return item["stage"].string == "finished" && ProjectStageBadge.waiting(item) == 0
        default: return true
        }
    }
    private func count(_ key: String) -> Int { model.projectIndex.filter { matches($0, key) }.count }
    private var rows: [JSONValue] {
        model.projectIndex.filter { item in
            matches(item, filter) && (query.isEmpty || ([item["project"].string, item["profile"].string, model.profileName(item["profile"].string)] + item["aliases"].array.map(\.string)).contains { $0.localizedCaseInsensitiveContains(query) })
        }
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            PageHeader(title: "Proyectos", subtitle: "Qué tienes, en qué estado y cuánto tiene ligado")
            ErrorBanner(model: model)
            HStack {
                TextField("Buscar proyecto", text: $query).frame(maxWidth: 260)
                Picker("Estado", selection: $filter) {
                    ForEach(filters, id: \.0) { key, title in Text(title) + Text(" \(count(key))").foregroundStyle(.secondary) }
                }.pickerStyle(.segmented).labelsHidden().fixedSize()
                Spacer()
            }
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(rows, id: \.["key"].string) { item in
                        ProjectRow(model: model, item: item, expanded: Binding(
                            get: { expanded == item["key"].string },
                            set: { expanded = $0 ? item["key"].string : nil }))
                        Divider()
                    }
                }
            }
            .overlay {
                if rows.isEmpty {
                    if model.projectIndex.isEmpty {
                        ContentUnavailableView("Sin proyectos", systemImage: "folder", description: Text("Nombra una sesión en Inicio o respalda algo desde Ingreso."))
                    } else {
                        ContentUnavailableView("Nada aquí", systemImage: "line.3.horizontal.decrease.circle", description: Text("Ningún proyecto coincide con este filtro."))
                    }
                }
            }
        }
        .padding(.horizontal, 20).padding(.top, 14)
        .task { await model.loadSuiteAgent(); await model.loadProjectLeases() }
    }
}

struct ProjectRow: View {
    @Bindable var model: Model
    let item: JSONValue
    @Binding var expanded: Bool
    @State private var renaming = false
    @State private var newName = ""
    @Environment(\.creatorPalette) private var palette
    private var title: String { item["project"].string }
    private var lease: JSONValue { model.projectLeases[Model.projectKey(item)] ?? .null }
    private var pendingSessions: Int { Int(item["pending_sessions"].double) }
    private var kinds: [(symbol: String, label: String, count: Int)] {
        [("record.circle", "Stalker", "stalker_session"), ("camera", String(localized: "Capturas"), "camera_bundle"), ("doc", String(localized: "Archivos"), "file"), ("film", String(localized: "Entregas"), "delivery")]
            .compactMap { symbol, label, key in
                let total = Int(item["kinds"][key].double)
                return total > 0 ? (symbol, label, total) : nil
            }
    }
    /// The biggest kind stands for the project; a draft only has its sessions.
    private var symbol: String { kinds.max { $0.count < $1.count }?.symbol ?? "record.circle" }
    private var summary: String {
        var parts: [String] = []
        if !item["profile"].string.isEmpty { parts.append(model.profileName(item["profile"].string)) }
        if item["assets"].double > 0 { parts.append(String(localized: "Respaldados: \(Int(item["assets"].double))")) }
        if lease["here"].bool { parts.append(lease["conflict"].bool ? String(localized: "Otra máquina también dice trabajar en él") : String(localized: "Trabajas aquí")) }
        else if lease != .null { parts.append(String(localized: "Se trabaja en \(lease["device_name_untrusted"].string)")) }
        if pendingSessions > 0 { parts.append(String(localized: "Sesiones por respaldar: \(pendingSessions)")) }
        if item["work_pending"].double > 0 { parts.append(String(localized: "Copias pendientes: \(Int(item["work_pending"].double))")) }
        if item["open_jobs"].double > 0 { parts.append(String(localized: "Operaciones abiertas: \(Int(item["open_jobs"].double))")) }
        let deliveries = item["deliveries"]
        if deliveries["published"].double > 0 { parts.append(String(localized: "Publicadas por respaldar: \(Int(deliveries["published"].double))")) }
        if deliveries["missing"].double > 0 { parts.append(String(localized: "Entregas no encontradas: \(Int(deliveries["missing"].double))")) }
        let related = item["related"].array.map(\.["title"].string)
        if !related.isEmpty { parts.append(String(localized: "Vinculado con \(related.joined(separator: ", "))")) }
        if !model.linkingProject.isEmpty && (item["related"].array.contains { $0["id"].string == model.linkingProject } || item["link_suggestions"].array.contains(.string(model.linkingProject))) {
            parts.append(String(localized: "Actualizando vínculo…"))
        }
        return parts.joined(separator: " · ")
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 10) {
                Button { expanded.toggle() } label: {
                    HStack(spacing: 10) {
                        Image(systemName: symbol).font(.title3).foregroundStyle(palette.accentText).frame(width: 28)
                        VStack(alignment: .leading, spacing: 2) {
                            HStack(spacing: 6) {
                                Text(title).fontWeight(.medium).lineLimit(1)
                                ProjectStageBadge(item: item)
                            }
                            Text(summary).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                        }
                        Spacer(minLength: 8)
                        Text(Timestamps.label(item["updated_at"].string)).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                    }.contentShape(Rectangle())
                }.buttonStyle(.plain)
                Menu {
                    Button("Usar en Ingreso", systemImage: "sdcard") { model.useProject(title) }.disabled(item["stage"].string == "finished")
                    Button("Subir a recientes", systemImage: "arrow.up") { model.bumpProject(item) }
                    Button("Renombrar…", systemImage: "pencil") { newName = title; renaming = true; expanded = true }.disabled(model.busy)
                    if !item["project_id"].string.isEmpty {
                        Divider()
                        Button("Ver archivos", systemImage: "square.stack") { model.openFiles(project: title) }
                        if title != "finales" {
                            Button("Completar y liberar…", systemImage: "checkmark.seal") { model.openProjectClosure(item) }.disabled(model.busy)
                            Button("Respaldar transcripciones", systemImage: "text.quote") { model.backupTranscripts(item) }
                                .disabled(model.busy || model.archive.isEmpty)
                            linkMenu
                        }
                        if model.suiteAgent.isRunning {
                            Divider()
                            // exchange §7.1: one machine works on a project at a time; the other sees it read only.
                            if lease["here"].bool {
                                Button("Dejar de trabajar aquí", systemImage: "laptopcomputer.slash") { model.setProjectLease(item, take: false) }.disabled(model.busy)
                            } else {
                                Button("Trabajar aquí", systemImage: "laptopcomputer") { model.setProjectLease(item, take: true) }.disabled(model.busy)
                            }
                        }
                        Divider()
                        if item["state"].string == "active" {
                            Button("Pausar", systemImage: "pause") { model.setProjectState(item, "paused") }
                            Button("Marcar terminado", systemImage: "checkmark.seal") { model.setProjectState(item, "finished") }
                        } else {
                            Button("Reabrir", systemImage: "arrow.uturn.backward") { model.setProjectState(item, "active") }
                        }
                    }
                } label: { Image(systemName: "ellipsis.circle") }
                .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize().accessibilityLabel("Acciones del proyecto")
            }
            if expanded {
                VStack(alignment: .leading, spacing: 6) {
                    ProjectPendingCaptures(model: model, item: item)
                    if !item["project_id"].string.isEmpty && title != "finales" {
                        ProjectMediaReview(model: model, item: item)
                        Button("Completar y liberar…", systemImage: "checkmark.seal") { model.openProjectClosure(item) }.disabled(model.busy)
                        ForEach(model.projectIndex.filter { item["link_suggestions"].array.map(\.string).contains($0["project_id"].string) }, id: \.["project_id"].string) { other in
                            HStack(spacing: 6) {
                                Label(String(localized: "¿«\(other["project"].string)» es una regrabación de este trabajo?"), systemImage: "link")
                                if model.linkingProject == other["project_id"].string {
                                    ProgressView().controlSize(.small)
                                    Text("Vinculando…").foregroundStyle(.secondary)
                                } else {
                                    Button("Vincular") { model.linkProjects(item, with: other["project_id"].string) }.disabled(model.busy)
                                }
                            }
                        }
                    }
                    if renaming { rename }
                    if !kinds.isEmpty {
                        HStack(spacing: 12) {
                            ForEach(kinds, id: \.symbol) { kind in Label(kind.label + " · \(kind.count)", systemImage: kind.symbol) }
                        }
                    }
                    if !item["deliveries"]["awaiting_names"].array.isEmpty {
                        Label(String(localized: "Esperando publicación: \(item["deliveries"]["awaiting_names"].array.map(\.string).joined(separator: ", "))"), systemImage: "film.stack")
                            .lineLimit(2)
                    }
                    if item["assets"].double > 0 {
                        Label(String(localized: "Copias para editar: \(Int(item["work_ready"].double)) listas · \(Int(item["work_pending"].double)) pendientes · \(Int(item["work_skipped"].double)) omitidas"), systemImage: "externaldrive")
                    }
                    if !item["cameras"].array.isEmpty { Label(item["cameras"].array.map(\.string).joined(separator: ", "), systemImage: "camera.aperture") }
                    if !item["places"].array.isEmpty { Label(item["places"].array.map(\.string).joined(separator: ", "), systemImage: "mappin.and.ellipse") }
                    if !item["aliases"].array.isEmpty { Label(String(localized: "Antes: \(item["aliases"].array.map(\.string).joined(separator: ", "))"), systemImage: "clock.arrow.circlepath") }
                    if item["assets"].double == 0 { Text("Todavía sin respaldo: sólo existe como nombre en sus sesiones.").foregroundStyle(.secondary) }
                }
                .font(.caption).padding(.leading, 38)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }.padding(.vertical, 8)
    }
    /// Link a re-recording of the same work. The engine suggests by name; choosing one is the confirmation.
    @ViewBuilder private var linkMenu: some View {
        let linked = Set(item["related"].array.map(\.["id"].string))
        let suggested = Set(item["link_suggestions"].array.map(\.string))
        let others = model.projectIndex.filter {
            !$0["project_id"].string.isEmpty && $0["project_id"] != item["project_id"] && $0["profile"] == item["profile"]
                && $0["project"].string != "finales" && !linked.contains($0["project_id"].string)
        }.sorted { (suggested.contains($0["project_id"].string) ? 0 : 1, $0["project"].string) < (suggested.contains($1["project_id"].string) ? 0 : 1, $1["project"].string) }
        Menu {
            Section("Es una regrabación del mismo trabajo") {
                ForEach(others, id: \.["project_id"].string) { other in
                    Button(suggested.contains(other["project_id"].string) ? String(localized: "\(other["project"].string) · sugerido") : other["project"].string) {
                        model.linkProjects(item, with: other["project_id"].string)
                    }
                }
            }
            if !linked.isEmpty {
                Section("Desvincular") {
                    ForEach(item["related"].array, id: \.["id"].string) { other in
                        Button(other["title"].string) { model.linkProjects(item, with: other["id"].string, linked: false) }
                    }
                }
            }
        } label: { Label("Vincular con otro proyecto…", systemImage: "link") }
        .disabled(model.busy || (others.isEmpty && linked.isEmpty))
    }
    private var trimmedName: String { newName.trimmingCharacters(in: .whitespacesAndNewlines) }
    private var canRename: Bool { !trimmedName.isEmpty && trimmedName != title && !model.busy }
    private var rename: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 6) {
                TextField("Nuevo nombre", text: $newName).frame(maxWidth: 260).onSubmit(commitRename)
                Button("Renombrar", action: commitRename).disabled(!canRename)
                Button("Cancelar") { renaming = false }
            }
            Text("Biblioteca, sesiones y próximas copias usan el nuevo nombre. Las carpetas ya copiadas conservan el suyo.").foregroundStyle(.secondary)
        }
    }
    private func commitRename() {
        guard canRename else { return }
        model.renameProject(item, to: trimmedName)
        renaming = false
    }
}

private struct ProjectPendingCaptures: View {
    @Bindable var model: Model
    let item: JSONValue

    private var captures: [JSONValue] {
        let names = [item["project"].string] + item["aliases"].array.map(\.string)
        return model.pendingCaptures.filter { capture in
            let tag = capture["tag"]["name"].string
            let name = tag.isEmpty ? capture["project_hint"]["name"].string : tag
            return !name.isEmpty && names.contains { $0.caseInsensitiveCompare(name) == .orderedSame }
        }
    }

    var body: some View {
        ForEach(captures, id: \.["path"].string) { capture in
            HStack {
                Label(URL(fileURLWithPath: capture["path"].string).lastPathComponent, systemImage: "record.circle")
                    .lineLimit(1)
                Spacer()
                Button("Revisar ingreso de Stalker…") {
                    model.prepareCaptureBatch(capture["path"].string)
                }.disabled(model.busy)
            }
        }
    }
}

private struct ProjectMediaReview: View {
    @Bindable var model: Model
    let item: JSONValue
    @State private var showing = false

    var body: some View {
        DisclosureGroup("Ubicaciones y estado del cierre", isExpanded: $showing) {
            if showing { ProjectMediaDetails(model: model, item: item) }
        }
    }
}

private struct ProjectMediaDetails: View {
    @Bindable var model: Model
    let item: JSONValue
    @State private var sourceData: JSONValue?
    @State private var closureData: JSONValue?
    @State private var sourceError: String?
    @State private var closureError: String?
    @State private var showingDeliveries = false
    @State private var showingLocations = false
    @State private var showingClipper = false
    @State private var clipperStatus: JSONValue?
    @State private var clipperError = ""
    @State private var clipperRestore: JSONValue?
    @State private var clipperNotice = ""
    @State private var checkingRestore = false
    @State private var visibleSources = 20
    @State private var reload = 0

    private var projectID: JSONValue { item["project_id"] }
    private var sourceRows: [JSONValue] {
        guard let sourceData else { return [] }
        let assetIDs = Set(model.assets.filter { asset in
            asset["project_id"] == projectID || asset["projects"].array.contains { $0["id"] == projectID }
        }.map(\.["id"].string))
        return (sourceData["sources"].array + sourceData["offline"].array)
            .filter { assetIDs.contains($0["asset_id"].string) }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Material y entregas").fontWeight(.semibold)
                Spacer()
                Button("Ver pendientes…") { model.openSaveAll(project: item["project"].string) }.disabled(model.busy)
                Button("Revisar sólo raws…") { chooseRawFolder() }.disabled(model.busy)
                Button("Actualizar", systemImage: "arrow.clockwise") { reload += 1 }
                    .labelStyle(.iconOnly)
            }
            if let closureData {
                closureStatus(closureData)
            } else if let closureError {
                Label(closureError, systemImage: "exclamationmark.circle")
            } else {
                ProgressView("Consultando entregas…").controlSize(.small)
            }
            DisclosureGroup("Dependencias de Clipper", isExpanded: $showingClipper) {
                if showingClipper {
                    clipperDetails
                        .task(id: reload) { await loadClipper() }
                }
            }
            if let sourceError {
                Label(sourceError, systemImage: "externaldrive.badge.exclamationmark")
            } else if sourceData == nil {
                ProgressView("Consultando ubicaciones…").controlSize(.small)
            } else if sourceRows.isEmpty {
                Text("Sin ubicaciones de media en el catálogo.").foregroundStyle(.secondary)
            } else {
                DisclosureGroup(isExpanded: $showingLocations) {
                    if showingLocations {
                        ForEach(sourceRows.prefix(visibleSources), id: \.["asset_id"].string) { source in
                            VStack(alignment: .leading, spacing: 3) {
                                Text(source["name"].string).fontWeight(.medium)
                                if !source["mission_association"]["mission"]["plan_name"].string.isEmpty {
                                    Label(source["mission_association"]["mission"]["plan_name"].string, systemImage: "airplane")
                                }
                                if !source["path"].string.isEmpty { location(source) }
                                ForEach(source["alternatives"].array, id: \.["path"].string) { location($0) }
                                ForEach(source["missing"].array, id: \.["path"].string) { location($0, missing: true) }
                            }.padding(.vertical, 3)
                        }
                        if visibleSources < sourceRows.count {
                            Button("Mostrar más ubicaciones") { visibleSources += 20 }
                        }
                    }
                } label: {
                    Text("Ubicaciones de media (\(sourceRows.count))")
                }
            }
        }
        .task(id: reload) {
            sourceData = nil; closureData = nil; sourceError = nil; closureError = nil
            async let sources: Void = loadSources()
            async let closure: Void = loadClosure()
            _ = await (sources, closure)
        }
    }

    @ViewBuilder private var clipperDetails: some View {
        if !clipperError.isEmpty {
            Label(clipperError, systemImage: "exclamationmark.circle")
        } else if let status = clipperStatus {
            if status["queued_assets"].double > 0 {
                DisclosureGroup {
                    ForEach(Array(status["queued_exports"].array.enumerated()), id: \.offset) { _, entry in
                        Text(entry.string).textSelection(.enabled)
                    }
                    Text("Termina o retira esas exportaciones en Clipper antes de liberar los raws.").foregroundStyle(.secondary)
                } label: {
                    Label("Clipper tiene exportaciones en cola", systemImage: "clock")
                }
            }
            if status["used_assets"].double > 0 {
                Label("\(Int(status["used_assets"].double)) fuentes en uso", systemImage: "link")
            }
            if status["missing_work_assets"].double > 0 {
                Label("\(Int(status["missing_work_assets"].double)) rutas de edición ausentes", systemImage: "folder.badge.questionmark")
                Button("Ver respaldos") { showingLocations = true }.buttonStyle(.link)
                ForEach(status["missing"].array, id: \.["path"].string) { row in
                    HStack {
                        Text(URL(fileURLWithPath: row["path"].string).lastPathComponent)
                            .lineLimit(1).truncationMode(.middle)
                        Spacer()
                        Button("Revisar restauración…") { reviewClipperRestore(row) }
                            .disabled(model.busy || checkingRestore)
                    }
                }
            }
            if status["retired"].double > 0 {
                Label("Retiro editorial registrado", systemImage: "archivebox")
            }
            if status["offline_projects"].double > 0 {
                Label("Hay proyectos Clipper desconectados", systemImage: "externaldrive.badge.xmark")
            }
            if status["active"].double == 0 && status["retired"].double == 0 {
                Text(status["available"].bool ? "Sin vínculo registrado en Clipper" : "Clipper sin configurar")
            }
        } else {
            ProgressView().controlSize(.small)
        }
        if !clipperNotice.isEmpty { Text(clipperNotice).foregroundStyle(.secondary) }
        if let plan = clipperRestore {
            HStack {
                Text("\(Int(plan["files"].double)) archivos · \(bytesLabel(plan["bytes"].double)) → \(plan["volume"].string)")
                Spacer()
                Button("Cancelar") { clipperRestore = nil }.disabled(model.busy)
                Button("Restaurar desde el respaldo") { applyClipperRestore(plan) }.disabled(model.busy)
            }
            Text(plan["path"].string).lineLimit(2).truncationMode(.middle).textSelection(.enabled)
        }
    }

    private func reviewClipperRestore(_ row: JSONValue) {
        checkingRestore = true; clipperRestore = nil; clipperNotice = ""
        Task {
            defer { checkingRestore = false }
            do {
                clipperRestore = try await model.oneShot("clipper_restore_plan", [
                    "project_id": projectID, "asset_id": row["asset_id"], "path": row["path"]
                ])
            } catch { clipperNotice = error.localizedDescription }
        }
    }

    private func applyClipperRestore(_ plan: JSONValue) {
        guard let client = model.client else { return }
        model.operation(String(localized: "Restaurando copia de edición"), scope: .space) {
            do {
                let result = try await client.call("clipper_restore_apply", [
                    "project_id": projectID, "asset_id": plan["asset_id"],
                    "path": plan["path"], "digest": plan["digest"]
                ])
                clipperRestore = nil
                if result["status"] != .null { clipperStatus = result["status"] }
                clipperNotice = result["recognized"].bool
                    ? String(localized: "Copia verificada en la ruta que usa Clipper.")
                    : String(localized: "Copia verificada. Falta confirmar la ruta en Clipper.")
                await loadSources()
            } catch { clipperNotice = error.localizedDescription }
        }
    }

    private func loadClipper() async {
        clipperError = ""; clipperStatus = nil
        do {
            let status = try await model.oneShot("clipper_project_status", ["project_id": projectID])
            if !Task.isCancelled { clipperStatus = status }
        } catch {
            if !Task.isCancelled { clipperError = error.localizedDescription }
        }
    }

    @ViewBuilder private func closureStatus(_ data: JSONValue) -> some View {
        let settings = data["settings"]
        let linked = settings["finals"].array + data["group"].array.flatMap { $0["finals"].array }
        let roles = Set(linked.map(\.["role"].string))
        let missing = settings["required"].array.map(\.string).filter { !roles.contains($0) }
        let linkedDeliveries = data["deliveries"].array
        let matches = data["files"].array.filter { candidate in
            candidate["suggested"].bool && !linkedDeliveries.contains {
                $0["volume_id"] == candidate["volume_id"] && $0["path"] == candidate["path"]
            }
        }
        if item["released"].bool {
            Label("Cierre aplicado", systemImage: "checkmark.seal")
        } else {
            if !settings["no_more"].bool {
                Label("Falta declarar completas las entregas.", systemImage: "checkmark.seal")
            }
            if !missing.isEmpty {
                Label {
                    Text("Faltan entregas vinculadas: \(missing.map(deliveryRole).joined(separator: ", "))")
                } icon: { Image(systemName: "film.stack") }
            } else if settings["no_more"].bool {
                Label("Entregas completas. Revisa el plan para comprobar respaldo y dependencias.", systemImage: "checkmark.seal")
            }
        }
        if !matches.isEmpty || !linkedDeliveries.isEmpty {
            let published = matches.filter { $0["published"].bool }.count
            DisclosureGroup(isExpanded: $showingDeliveries) {
                if showingDeliveries {
                    ForEach(Array((linkedDeliveries + matches).enumerated()), id: \.offset) { index, match in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(match["name"].string).textSelection(.enabled)
                            HStack {
                                Text(deliveryRole(match["role"].string))
                                Text(model.volume(match["volume_id"].string)?["name"].string ?? match["volume"].string)
                                Text(match["published"].bool ? "Publicado" : "Por publicar")
                                if index >= linkedDeliveries.count { Text("Sin vincular") }
                                if match["state"].string == "missing" { Text("No encontrada") }
                            }.foregroundStyle(.secondary)
                        }.padding(.vertical, 2)
                    }
                }
            } label: {
                Text("Entregas vinculadas: \(linkedDeliveries.count)")
                if !matches.isEmpty {
                    Text("Coincidencias por nombre: \(matches.count) · publicadas: \(published)")
                }
            }
        }
    }

    private func deliveryRole(_ role: String) -> String {
        switch role {
        case "horizontal": return String(localized: "Horizontal")
        case "vertical": return String(localized: "Vertical")
        case "thumbnail": return String(localized: "Thumbnail")
        case "auxiliary": return String(localized: "Auxiliar")
        default: return role
        }
    }

    private func location(_ value: JSONValue, missing: Bool = false) -> some View {
        let role = value["role"].string == "archive" ? String(localized: "Respaldo") : String(localized: "Copia para editar")
        let path = value["path"].string
        return VStack(alignment: .leading, spacing: 2) {
            HStack {
                Text("\(role) · \(value["volume"].string)")
                if missing { Text("No encontrada").foregroundStyle(.secondary) }
                else if !value["online"].bool { Text("Desconectado").foregroundStyle(.secondary) }
                else {
                    Button("Mostrar en Finder", systemImage: "folder") {
                        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
                    }.labelStyle(.iconOnly)
                }
            }
            Text(path).foregroundStyle(.secondary).textSelection(.enabled)
        }
    }

    private func loadSources() async {
        guard let client = model.client else { return }
        do {
            let data = try await client.call("sources", ["project": item["project"], "purpose": .string("archive")])
            if !Task.isCancelled { sourceData = data }
        } catch let failure as BackendFailure where failure.code == "unknown_project" {
            if !Task.isCancelled { sourceData = .object([:]) }
        } catch {
            if !Task.isCancelled { sourceError = error.localizedDescription }
        }
    }

    private func loadClosure() async {
        guard let reader = model.reader else { return }
        do {
            let data = try await reader.call("closure_candidates", ["project_id": projectID])
            if !Task.isCancelled { closureData = data }
        } catch {
            if !Task.isCancelled { closureError = error.localizedDescription }
        }
    }

    private func chooseRawFolder() {
        let directory = model.volume(model.work).map { URL(fileURLWithPath: $0["root"].string) }
        pickURL(message: String(localized: "Elige la carpeta de raws de este proyecto."), directory: directory) { url in
            let path = url.standardizedFileURL.path
            guard let volume = model.volumes.first(where: {
                let root = $0["root"].string
                return $0["role"].string == "work" && $0["online"].bool
                    && !root.isEmpty && path.hasPrefix(root + "/")
            }) else {
                model.error = String(localized: "Elige una carpeta dentro de un disco de trabajo conectado.")
                return
            }
            model.work = volume["id"].string
            model.auditPath = String(path.dropFirst(volume["root"].string.count + 1))
            model.releaseProject = item["project"].string
            model.releasePlan = nil
            model.showRelease = true
        }
    }
}

struct ProjectStageBadge: View {
    let item: JSONValue
    @Environment(\.creatorPalette) private var palette
    /// Deliveries still on the editing disk: unpublished, or published and waiting for «Respaldar publicados…».
    static func waiting(_ item: JSONValue) -> Int { Int(item["deliveries"]["awaiting"].double + item["deliveries"]["published"].double) }
    private var label: LocalizedStringKey {
        if item["state"].string == "paused" { return "Pausado" }
        switch item["stage"].string {
        case "draft": return "Sin respaldar"
        case "in_progress": return "En progreso"
        case "finished":
            let awaiting = Int(item["deliveries"]["awaiting"].double)
            if awaiting > 0 { return "Terminado · \(awaiting) por publicar" }
            if Self.waiting(item) > 0 { return "Terminado · \(Self.waiting(item)) por respaldar" }
            return "Terminado"
        default: return "Respaldado"
        }
    }
    private var tint: Color { item["stage"].string == "finished" && Self.waiting(item) > 0 ? palette.info : palette.ink }
    var body: some View {
        Text(label).font(.caption2.weight(.medium)).foregroundStyle(tint).padding(.horizontal, 6).padding(.vertical, 1)
            .background(palette.raised.opacity(0.7), in: palette.pill)
    }
}
