import AppKit
import SwiftUI
import MediaMenuCore

struct WindowRoot: View {
    @ObservedObject var model: Model
    var body: some View {
        Group {
            if model.presentation == .full { MainView(model: model) }
            else { ImporterSurface(model: model) }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .modifier(CreatorAppearance())
        .task(id: model.presentation == .compact ? "" : model.sourceKey) {
            if model.presentation != .compact { await model.loadSource() }
        }
        .alert("Confirmar sólo respaldo", isPresented: $model.archiveOnlyPrompt) {
            Button("Cancelar", role: .cancel) {}
            Button("Confirmar sólo respaldo") { model.makePlan(archiveOnly: true) }
        } message: {
            Text(model.noWork ? "Se verificará el respaldo en el disco elegido, sin copias para editar ni pendientes. La SD sólo se limpia si lo activaste y después de verificar." : "El SSD no recibirá una copia ahora. La elección pendiente/omitido quedará registrada; revisa el plan antes de ejecutar.")
        }
        .onChange(of: model.project) { _, _ in model.invalidatePlan() }
        .onChange(of: model.profile) { _, _ in model.invalidatePlan() }
        .onChange(of: model.country) { _, _ in model.invalidatePlan() }
        .onChange(of: model.city) { _, _ in model.invalidatePlan() }
        .onChange(of: model.archive) { _, _ in model.invalidatePlan() }
        .onChange(of: model.work) { _, _ in model.invalidatePlan() }
        .onChange(of: model.sourceKind) { _, _ in model.invalidatePlan(); model.loadedSourceKey = "" }
        .onChange(of: model.cleanup) { _, _ in model.invalidatePlan() }
        .onChange(of: model.copyChoice) { _, _ in model.invalidatePlan() }
        .onChange(of: model.units) { _, _ in model.invalidatePlan() }
    }
}

struct AppNavigation: View {
    @ObservedObject var model: Model
    @Environment(\.creatorPalette) private var palette
    var body: some View {
        HStack(spacing: 5) {
            ForEach([(4,"Bandeja","tray.full"),(0,"Ingreso","sdcard"),(1,"Biblioteca","square.stack"),(2,"Actividad","clock.arrow.circlepath"),(5,"Espacio","externaldrive.badge.checkmark"),(3,"Ajustes","slider.horizontal.3")], id: \.0) { id, title, icon in
                Button { model.selectedTab = id } label: { Label(title, systemImage: icon).padding(.vertical, 3).frame(maxWidth: .infinity) }
                    .buttonStyle(CreatorButtonStyle(prominent: model.selectedTab == id))
            }
        }.padding(6)
        .background(palette.inset.opacity(0.5), in: RoundedRectangle(cornerRadius: 13))
        .padding(.bottom, 5) // Leave room for native shadows/focus rings; no clipped TabView toolbar.
    }
}

struct WindowTopBar: View {
    @ObservedObject var model: Model
    @Environment(\.creatorPalette) private var palette
    private var modeButton: some View {
        Button {
            if model.presentation == .full {
                model.changeMode?(.compact)
            } else {
                model.selectedTab = 0
                model.changeMode?(.full)
            }
        } label: {
            Image(systemName: model.presentation == .full ? "arrow.down.right.and.arrow.up.left" : "arrow.up.left.and.arrow.down.right")
                .frame(width: 30, height: 28)
        }
        .buttonStyle(CreatorButtonStyle())
        .help(model.presentation == .full ? "Vista compacta" : "Expandir a vista completa")
    }

    var body: some View {
        HStack(spacing: 8) {
            Text(model.presentation == .full ? model.message : "Ingreso · " + model.message)
                .font(.caption).lineLimit(1).frame(width: 160, alignment: .leading)
            Spacer(minLength: 2)
            if model.presentation == .full {
                AppNavigation(model: model).frame(maxWidth: 760)
            } else {
                Spacer(minLength: 0)
            }
            Spacer(minLength: 2)
            HStack(spacing: 8) {
                if model.busy {
                    ProgressView().controlSize(.small)
                    Button("Cancelar") { model.cancel() }.font(.caption)
                }
                modeButton
            }
            .frame(width: 150, alignment: .trailing)
        }
        .padding(.horizontal, 14)
        .frame(maxWidth: .infinity)
        .frame(height: 50)
        .background(palette.canvas)
        .overlay(alignment: .bottom) { Rectangle().fill(palette.border.opacity(0.8)).frame(height: 1) }
    }
}

struct SourceKindBadges: View {
    @ObservedObject var model: Model
    var body: some View {
        HStack(spacing: 6) {
            ForEach([("card","SD / cámara","sdcard"),("stalker","Stalker","record.circle"),("delivery","Entrega final","film.stack")], id: \.0) { key,title,icon in
                Button { model.sourceKind = key; model.cleanup = key == "card" } label: { Label(title, systemImage: icon) }
                    .buttonStyle(CreatorButtonStyle(prominent: model.sourceKind == key))
            }
        }
    }
}

struct ProfilePicker: View {
    @ObservedObject var model: Model
    var body: some View {
        Menu {
            ForEach(model.profiles, id: \.["id"].string) { value in
                Button(value["name"].string + " · " + value["raw_dir"].string) { model.selectProfile(value["id"].string) }
            }
            if model.profiles.isEmpty { Text("Sin perfiles guardados") }
            Button("Administrar perfiles…") { model.selectedTab = 3; model.changeMode?(.full) }
        } label: { Label(model.profile, systemImage: "person.crop.square") }
        .help("Perfil creativo y espacio de trabajo")
    }
}

struct ProjectSuggestions: View {
    @ObservedObject var model: Model
    private var hasUnassignedStalkerSessions: Bool {
        model.sourceKind == "stalker" && model.units.contains { $0.project.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
    }
    var body: some View {
        if model.sourceKind == "stalker" {
            if hasUnassignedStalkerSessions {
                RecentProjectChips(projects: model.recentProjects, selection: $model.project)
            }
        } else {
            HStack {
                if !model.lastProject.isEmpty { Button("Último: " + model.lastProject) { model.project = model.lastProject } }
                Menu("Proyectos recientes") {
                    ForEach(Array(model.recentProjects.prefix(15)), id: \.["project_id"].string) { value in
                        Button(value["project"].string + " · " + value["profile"].string) { model.project = value["project"].string; model.selectProfile(value["profile"].string) }
                    }
                }
            }.font(.caption)
        }
    }
}

struct SourceSummary: View {
    @ObservedObject var model: Model
    var archiveDescription: String {
        guard let target = model.volumes.first(where: { $0["id"].string == model.archive }) else { return "Sin disco de respaldo" }
        return "Respaldo: \(target["root"].string)/\(target["prefix"].string) · rutas por tipo y fecha al revisar el plan"
    }
    var workDescription: String {
        let paths = Dictionary(grouping: model.units) { unit -> String in
            let profile = unit.profile.isEmpty ? model.profile : unit.profile
            let saved = model.profiles.first { $0["id"].string == profile }
            let raw = saved?["raw_dir"].string ?? "\(profile)_raw"
            let project = unit.project.isEmpty ? model.project : unit.project
            return raw + "/" + (project.isEmpty ? "Sin proyecto" : project)
        }
        let destinations = paths.keys.sorted().map { path in
            let count = paths[path, default: []].count
            return count > 1 ? "\(path) ×\(count)" : path
        }
        let count = model.units.filter(\.copyForEditing).count
        let scope = model.copyChoice == "all" ? "todo el lote" : "\(count) seleccionadas"
        return "Editar: \(destinations.joined(separator: " · ")) · \(scope)"
    }
    var projectSummary: String {
        let projects = Dictionary(grouping: model.units) { unit in
            let title = unit.project.isEmpty ? model.project : unit.project
            return title.isEmpty ? "Sin proyecto" : title
        }
        let entries = projects.keys.sorted().map { title in
            let count = projects[title, default: []].count
            return count > 1 ? "\(title) (\(count))" : title
        }
        return "Proyectos considerados: " + entries.joined(separator: " · ")
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            if model.discoveryLoading { HStack { ProgressView().controlSize(.small); Text("Leyendo archivos… Las miniaturas se cargarán conforme aparezcan.").font(.caption) } }
            if !model.discoveryError.isEmpty { HStack { Label(model.discoveryError, systemImage: "exclamationmark.triangle").foregroundStyle(.orange); Button("Reintentar") { model.discover() } } }
            if !model.units.isEmpty {
                HStack(spacing: 14) {
                    ForEach([("image","Fotos","photo"),("raw_image","RAW","camera.aperture"),("video","Videos","film"),("drone","Dron","airplane"),("360","360°","view.360"),("audio","Audio","waveform"),("stalker","Stalker","record.circle"),("support","Auxiliares","doc")], id: \.0) { key,label,icon in
                        let count = model.units.filter { unit in
                            let category = unit.camera == "dji-drone" ? "drone" : ["dji-360", "insta360"].contains(unit.camera) ? "360" : unit.category
                            return category == key
                        }.count
                        if count > 0 { Label("\(count) \(label)", systemImage: icon).font(.caption) }
                    }
                    Spacer()
                    Text(ByteCountFormatter.string(fromByteCount: Int64(model.units.reduce(0) { $0 + $1.bytes }), countStyle: .file)).font(.caption)
                }
                let cameras = Set(model.units.map { $0.camera == "auto" ? $0.detectedCamera : $0.camera }.filter { !$0.isEmpty })
                if !cameras.isEmpty { Label("Cámara / evidencia: " + cameras.sorted().joined(separator: ", "), systemImage: "camera").font(.caption) }
                Text(archiveDescription).font(.caption).foregroundStyle(.secondary)
                if model.sourceKind == "stalker" { Text(projectSummary).font(.caption).foregroundStyle(.secondary) }
                if !model.noWork {
                    Label(workDescription, systemImage: "externaldrive").font(.caption)
                }
            }
        }
    }
}

struct PlanReview: View {
    @ObservedObject var model: Model
    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            if let plan = model.plan {
                Label("Plan revisable · \(plan["files"].array.count) archivos", systemImage: "checklist").font(.headline)
                let projects = Dictionary(grouping: plan["units"].array) { $0["project"].string }
                if !projects.isEmpty {
                    let summary = projects.keys.sorted().map { title in
                        let count = projects[title, default: []].count
                        return count > 1 ? "\(title) (\(count))" : title
                    }.joined(separator: " · ")
                    Text("Proyectos incluidos: " + summary).font(.caption.weight(.medium))
                }
                Text(plan["work_id"] == .null ? "Sólo respaldo · " + model.workLabel(plan["work_delivery_status"].string) : "Respaldo y copias para editar verificadas antes de limpiar").font(.caption)
                Text(plan["source_kind"].string == "stalker"
                     ? (plan["release_stalker_source"].bool
                        ? "Después de verificar, se revisa Clipper y se libera el original Stalker. Si la revisión o el reenlace fallan, se conserva."
                        : "El original Stalker se conservará según Ajustes.")
                     : (!plan["cleanup"].bool ? "El origen se conservará." : plan["cleanup_profile"].string == "sony_clips_then_format" ? "Después de verificar se borran los clips; termina con formato en cámara." : "La SD se limpiará después de verificar.")).font(.caption)
                DisclosureGroup("Rutas definitivas por archivo") {
                    ScrollView { LazyVStack(alignment: .leading, spacing: 8) {
                        ForEach(Array(plan["preview"].array.enumerated()), id: \.offset) { _, row in
                            VStack(alignment: .leading) { Text(row["source"].string); Text(row["archive_path"].string).foregroundStyle(.secondary); if row["work_path"] != .null { Text(row["work_path"].string).foregroundStyle(.secondary) } }.font(.caption).textSelection(.enabled)
                        }
                    } }.frame(maxHeight: 110)
                }
            } else {
                Text("Asigna proyecto, revisa las rutas y prepara un plan antes de copiar.").font(.caption).foregroundStyle(.secondary)
            }
        }.disabled(model.busy)
    }
}

struct PlanAction: View {
    @ObservedObject var model: Model
    private var projectIsReady: Bool {
        let hasBatchProject = !model.project.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        guard model.sourceKind == "stalker" else { return hasBatchProject }
        return hasBatchProject || (!model.units.isEmpty && model.units.allSatisfy {
            !$0.project.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        })
    }
    private var canRequestPlan: Bool {
        !model.source.isEmpty
            && projectIsReady
            && !model.archive.isEmpty && !model.discoveryLoading
            && !(model.copyChoice == "some" && model.units.isEmpty)
    }
    var body: some View {
        if model.plan != nil {
            Button { model.apply() } label: { Label("Ejecutar plan revisado", systemImage: "arrow.right.circle.fill") }
                .buttonStyle(CreatorButtonStyle(prominent: true)).disabled(model.busy)
        } else {
            Button { model.requestPlan() } label: { Label("Revisar plan", systemImage: "checklist") }
                .buttonStyle(CreatorButtonStyle(prominent: true)).disabled(model.busy || !canRequestPlan)
        }
    }
}

struct ActionFooter<Content: View>: View {
    @Environment(\.creatorPalette) private var palette
    let content: Content
    init(@ViewBuilder content: () -> Content) { self.content = content() }
    var body: some View {
        HStack(spacing: 9) { content }
            .padding(.horizontal, 20).padding(.vertical, 10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(palette.canvas.opacity(0.98))
            .overlay(alignment: .top) { Rectangle().fill(palette.border.opacity(0.8)).frame(height: 1) }
    }
}

struct MainActionFooter: View {
    @ObservedObject var model: Model
    private var pendingRoots: [(String, Int)] {
        model.captureRoots.compactMap { root in
            let count = model.pendingCaptures.filter { $0["root"].string == root }.count
            return count > 0 ? (root, count) : nil
        }
    }
    @ViewBuilder var body: some View {
        if [0, 1, 2, 4].contains(model.selectedTab) {
          ActionFooter {
            switch model.selectedTab {
            case 0:
                if model.busy { ProgressView().controlSize(.small); Text(model.message).font(.caption).lineLimit(1) }
                Spacer(minLength: 0)
                PlanAction(model: model)
            case 1:
                Picker("SSD", selection: $model.work) {
                    Text("Sin SSD").tag("")
                    ForEach(model.volumes.filter { $0["role"].string == "work" }, id: \.["id"].string) { value in
                        Text(value["name"].string + (value["online"].bool ? "" : " · desconectado")).tag(value["id"].string)
                    }
                }.frame(maxWidth: 190)
                Button("Omitir copias") { model.skip() }.disabled(model.selected.isEmpty || model.busy)
                Button("Restaurar…") { model.restoreSelected() }.disabled(model.selected.count != 1 || model.busy)
                Button("Exportar selección…") { model.exportHandoff() }.disabled(model.selected.isEmpty || model.busy)
                Spacer(minLength: 0)
                Button("Copiar al SSD", systemImage: "externaldrive.badge.plus") { model.deliver() }
                    .buttonStyle(CreatorButtonStyle(prominent: true)).disabled(model.selected.isEmpty || model.work.isEmpty || model.busy)
            case 2:
                Button("Actualizar actividad") { Task { await model.refresh() } }.disabled(model.busy)
                Spacer(minLength: 0)
                if model.busy { ProgressView().controlSize(.small); Text(model.message).font(.caption).lineLimit(1) }
            case 4:
                Button("Actualizar capturas") { model.captureChanged([]) }.disabled(model.busy)
                Spacer(minLength: 0)
                if pendingRoots.count == 1, let root = pendingRoots.first {
                    Button("Preparar lote · \(root.1)", systemImage: "record.circle") { model.prepareCaptureBatch(root.0) }
                        .buttonStyle(CreatorButtonStyle(prominent: true))
                } else if !pendingRoots.isEmpty {
                    Menu {
                        ForEach(pendingRoots, id: \.0) { item in
                            Button(URL(fileURLWithPath: item.0).lastPathComponent + " · \(item.1) sesiones") { model.prepareCaptureBatch(item.0) }
                        }
                    } label: { Label("Preparar lote · \(model.pendingCaptures.count)", systemImage: "record.circle") }
                        .buttonStyle(CreatorButtonStyle(prominent: true))
                } else {
                    Button("Elegir fuente…") { model.selectedTab = 0; model.chooseSource() }
                }
            default:
                EmptyView()
            }
        }
        }
    }
}

struct ImporterSurface: View {
    @ObservedObject var model: Model
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack { Text(URL(fileURLWithPath: model.source).lastPathComponent).font(.caption); Spacer(); Button("Elegir fuente…") { model.chooseSource() } }
            SourceKindBadges(model: model)
            TextField(model.sourceKind == "stalker" ? "Proyecto para sesiones sin asignar" : "Nombre del proyecto", text: $model.project)
                .frame(maxWidth: model.sourceKind == "stalker" ? 300 : .infinity, alignment: .leading)
            ProjectSuggestions(model: model)
            HStack { ProfilePicker(model: model); Spacer(); Text(model.volumes.first { $0["id"].string == model.archive }?["name"].string ?? "Sin disco de respaldo").font(.caption) }
            EditingChoice(model: model, compact: model.presentation == .compact)
            if model.presentation == .selection {
                SourceSummary(model: model)
                ImportGrid(model: model, dense: true).frame(minHeight: 160, maxHeight: .infinity)
            }
            Toggle("Limpiar SD después de verificar", isOn: $model.cleanup).disabled(model.sourceKind != "card")
            Text("Todo se respalda. La selección sólo cambia las copias para editar.").font(.caption).foregroundStyle(.secondary)
            if model.busy { HStack { ProgressView().controlSize(.small); Text(model.message).font(.caption); Button("Cancelar") { model.cancel() } } }
            if !model.error.isEmpty { Text(model.error).font(.caption).foregroundStyle(.red) }
            if model.presentation == .compact { Spacer(minLength: 0) }
        }.padding(20).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .safeAreaInset(edge: .bottom, spacing: 0) {
                ActionFooter {
                    if model.busy { ProgressView().controlSize(.small); Text(model.message).font(.caption).lineLimit(1) }
                    Spacer(minLength: 0)
                    PlanAction(model: model)
                }
            }
            .onChange(of: model.project) { _, _ in model.invalidatePlan() }
    }
}
