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
        .safeAreaInset(edge: .top, spacing: 0) {
            if model.windowIsFullscreen { WindowTopBar(model: model) }
        }
        .modifier(CreatorAppearance())
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
        .onChange(of: model.sourceKind) { _, _ in model.invalidatePlan(); model.loadedSourceKey = ""; model.units = [] }
        .onChange(of: model.cleanup) { _, _ in model.invalidatePlan() }
        .onChange(of: model.deferWork) { _, _ in model.invalidatePlan() }
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
                Button { model.selectedTab = id } label: { Label(title, systemImage: icon).font(.caption).frame(maxWidth: .infinity) }
                    .buttonStyle(CreatorButtonStyle(prominent: model.selectedTab == id))
            }
        }.padding(3)
        .background(palette.inset.opacity(0.5), in: RoundedRectangle(cornerRadius: 13))
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
            HStack(spacing: 5) {
                Image(nsImage: NSApp.applicationIconImage).resizable().frame(width: 22, height: 22)
                Text("nüün").font(.system(size: 15, weight: .semibold, design: .rounded))
            }.help(model.message)
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
            .fixedSize()
        }
        .padding(.horizontal, 6)
        .frame(maxWidth: .infinity)
        .frame(height: 38)
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
        } label: { Label(model.profile, systemImage: "person.crop.square").lineLimit(1) }
        .fixedSize()
        .help("Perfil creativo y espacio de trabajo")
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
            && (model.sourceKind != "stalker" || model.loadedSourceKey == model.sourceKey)
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
                if model.busy { ProgressView().controlSize(.small) }
                Text(model.message).font(.caption).lineLimit(1)
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
        ScrollViewReader { proxy in
          ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                IngestHeader(model: model)
                BatchProjectPicker(model: model)
                IngestOptions(model: model)
                if model.presentation == .selection {
                    SourceSummary(model: model)
                    ImportGrid(model: model, dense: true).frame(height: 260)
                }
                Text("Todo se respalda. La selección sólo cambia las copias para editar.").font(.caption).foregroundStyle(.secondary)
                PlanReview(model: model).id("ingest-plan")
                if !model.error.isEmpty {
                    Label(model.error, systemImage: "exclamationmark.triangle")
                        .font(.caption).foregroundStyle(.red)
                        .fixedSize(horizontal: false, vertical: true)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .id("ingest-error")
                }
            }
            .disabled(model.busy)
            .padding(20)
            .frame(maxWidth: .infinity, alignment: .topLeading)
        }
          .onChange(of: model.error) { _, value in
              if !value.isEmpty { withAnimation { proxy.scrollTo("ingest-error", anchor: .top) } }
          }
          .onChange(of: model.plan) { _, value in
              if value != nil { withAnimation { proxy.scrollTo("ingest-plan", anchor: .bottom) } }
          }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .safeAreaInset(edge: .bottom, spacing: 0) {
            ActionFooter {
                if model.busy { ProgressView().controlSize(.small) }
                Text(model.message).font(.caption).lineLimit(2)
                Spacer(minLength: 0)
                PlanAction(model: model).fixedSize()
            }
        }
            .onChange(of: model.project) { _, _ in model.invalidatePlan() }
    }
}


// Shared controls: changing presentation never creates another draft/model.
struct IngestHeader: View {
    @ObservedObject var model: Model
    private var sourceName: String {
        model.source.isEmpty ? "Elegir fuente" : URL(fileURLWithPath: model.source).lastPathComponent
    }
    private var sourcePicker: some View {
        HStack(spacing: 4) {
            Menu {
                ForEach(model.sourceCards, id: \.["path"].string) { card in
                    Button("SD · " + card["name"].string) { model.useCard(card["path"].string) }
                }
                if !model.captureRoots.isEmpty {
                    Divider()
                    ForEach(model.captureRoots, id: \.self) { root in
                        Button("Stalker · " + URL(fileURLWithPath: root).lastPathComponent) { model.prepareSource(root, kind: "stalker") }
                    }
                }
                Divider()
                Button("Otra…") { model.chooseSource() }
                Button("Actualizar fuentes") { Task { await model.refresh() } }
            } label: { Label(sourceName, systemImage: model.sourceKind == "stalker" ? "record.circle" : "sdcard").lineLimit(1) }
            .help(model.source.isEmpty ? "SDs conectadas y bibliotecas conocidas" : model.source)
            if model.selectedCard != nil {
                Button { model.ejectSource() } label: { Image(systemName: "eject") }
                    .help("Expulsar SD de forma segura").accessibilityLabel("Expulsar SD de forma segura")
                    .disabled(model.busy || model.discoveryLoading)
            }
        }
    }
    private func destination(_ title: String, role: String, selection: Binding<String>) -> some View {
        Picker(title, selection: selection) {
            Text("Sin seleccionar").tag("")
            ForEach(model.volumes.filter { $0["role"].string == role }, id: \.["id"].string) { volume in
                Text(volume["name"].string + (volume["online"].bool ? "" : " · desconectado")).tag(volume["id"].string)
            }
        }.lineLimit(1)
    }
    private var location: some View {
        HStack(spacing: 8) {
            ProfilePicker(model: model)
            TextField("País", text: $model.country).frame(width: model.presentation == .compact ? 38 : 48).help("País")
            TextField("Ciudad", text: $model.city).frame(width: model.presentation == .compact ? 85 : 120).help("Ciudad")
        }
    }
    private var destinations: some View {
        HStack(spacing: 10) {
            destination("Archivo", role: "archive", selection: $model.archive)
            destination("SSD", role: "work", selection: $model.work)
        }
    }
    var body: some View {
        Group {
            if model.presentation == .full {
                HStack(spacing: 14) {
                    sourcePicker.frame(maxWidth: .infinity, alignment: .leading)
                    location.frame(maxWidth: .infinity)
                    destinations.frame(maxWidth: .infinity, alignment: .trailing)
                }
            } else {
                VStack(alignment: .leading, spacing: 9) {
                    HStack(spacing: 10) {
                        sourcePicker.frame(maxWidth: .infinity, alignment: .leading)
                        location.fixedSize(horizontal: true, vertical: false)
                    }
                    destinations
                }
            }
        }.disabled(model.busy)
    }
}

struct BatchProjectPicker: View {
    @ObservedObject var model: Model
    @State private var manual = false
    private var otherProjects: some View {
        HStack(spacing: 6) {
            if model.recentProjects.count > 3 {
                Menu("Más recientes") {
                    ForEach(Array(model.recentProjects.dropFirst(3)), id: \.["project_id"].string) { item in
                        Button(item["project"].string) { model.project = item["project"].string }
                    }
                }.fixedSize().font(.caption)
            }
            Button("Otro…") { manual.toggle() }.font(.caption)
        }
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if model.presentation == .compact {
                HStack {
                    Text("Proyecto").font(.caption).foregroundStyle(.secondary)
                    Spacer(minLength: 0)
                    otherProjects
                }
                RecentProjectChips(projects: model.recentProjects, selection: $model.project)
            } else {
                HStack(spacing: 6) {
                    Text("Proyecto").font(.caption).foregroundStyle(.secondary)
                    RecentProjectChips(projects: model.recentProjects, selection: $model.project)
                    otherProjects
                }
            }
            if manual {
                HStack {
                    TextField(model.sourceKind == "stalker" ? "Proyecto para sesiones sin asignar" : "Nombre del proyecto", text: $model.project)
                    Button("Listo") { manual = false }.disabled(model.project.isEmpty)
                }
            } else if !model.project.isEmpty && !model.recentProjects.prefix(3).contains(where: { $0["project"].string == model.project }) {
                Button(model.project, systemImage: "pencil") { manual = true }.font(.caption)
            }
            if model.sourceKind == "stalker" {
                Text("Se aplica sólo a sesiones sin proyecto propio.").font(.caption2).foregroundStyle(.secondary)
            }
        }.disabled(model.busy)
    }
}

struct IngestOptions: View {
    @ObservedObject var model: Model
    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            SourceKindBadges(model: model)
            EditingChoice(model: model, compact: model.presentation == .compact)
            Toggle("Limpiar SD después de verificar", isOn: $model.cleanup).disabled(model.sourceKind != "card")
            if !model.noWork {
                Toggle("Preparar copia para editar después si falta SSD", isOn: $model.deferWork)
            }
            HStack {
                Button(model.units.isEmpty ? "Inspeccionar archivos" : "Actualizar archivos", systemImage: "photo.stack") { model.inspectSource() }
                    .disabled(model.source.isEmpty || model.discoveryLoading)
                if model.units.isEmpty { Text("Elegir fuente no recorre sus archivos.").font(.caption2).foregroundStyle(.secondary) }
            }
        }.disabled(model.busy)
    }
}
