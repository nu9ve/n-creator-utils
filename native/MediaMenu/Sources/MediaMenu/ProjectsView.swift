import SwiftUI
import MediaMenuCore

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
                            linkMenu
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
                    if !item["project_id"].string.isEmpty && title != "finales" {
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
