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
            ForEach([(4,"Bandeja","tray.full"),(0,"Ingreso","sdcard"),(1,"Biblioteca","square.stack"),(2,"Actividad","clock.arrow.circlepath"),(3,"Ajustes","slider.horizontal.3")], id: \.0) { id, title, icon in
                Button { model.selectedTab = id } label: { Label(title, systemImage: icon).padding(.vertical, 3).frame(maxWidth: .infinity) }
                    .buttonStyle(CreatorButtonStyle(prominent: model.selectedTab == id))
            }
        }.padding(6)
        .background(palette.inset.opacity(0.5), in: RoundedRectangle(cornerRadius: 13))
        .padding(.bottom, 5) // Leave room for native shadows/focus rings; no clipped TabView toolbar.
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
    var body: some View {
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

struct SourceSummary: View {
    @ObservedObject var model: Model
    var archiveDescription: String {
        guard let target = model.volumes.first(where: { $0["id"].string == model.archive }) else { return "Sin disco de respaldo" }
        return "Respaldo: \(target["root"].string)/\(target["prefix"].string) · rutas por tipo y fecha al revisar el plan"
    }
    var workDescription: String {
        let saved = model.profiles.first { $0["id"].string == model.profile }
        let raw = saved?["raw_dir"].string ?? "\(model.profile)_raw"
        let project = model.project.isEmpty ? "PROYECTO" : model.project
        let count = model.units.filter(\.copyForEditing).count
        let scope = model.copyChoice == "all" ? "todo el lote" : "\(count) seleccionadas"
        return "Editar: \(raw)/\(project) · \(scope)"
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
                Text(plan["work_id"] == .null ? "Sólo respaldo · " + model.workLabel(plan["work_delivery_status"].string) : "Respaldo y copias para editar verificadas antes de limpiar").font(.caption)
                Text(!plan["cleanup"].bool ? "El origen se conservará." : plan["cleanup_profile"].string == "sony_clips_then_format" ? "Después de verificar se borran los clips; termina con formato en cámara." : "La SD se limpiará después de verificar.").font(.caption)
                DisclosureGroup("Rutas definitivas por archivo") {
                    ScrollView { LazyVStack(alignment: .leading, spacing: 8) {
                        ForEach(Array(plan["preview"].array.enumerated()), id: \.offset) { _, row in
                            VStack(alignment: .leading) { Text(row["source"].string); Text(row["archive_path"].string).foregroundStyle(.secondary); if row["work_path"] != .null { Text(row["work_path"].string).foregroundStyle(.secondary) } }.font(.caption).textSelection(.enabled)
                        }
                    } }.frame(maxHeight: 110)
                }
                Button { model.apply() } label: { Label("Ejecutar plan revisado", systemImage: "arrow.right.circle.fill") }.buttonStyle(CreatorButtonStyle(prominent: true))
            } else {
                Button { model.requestPlan() } label: { Label("Revisar plan", systemImage: "checklist") }
                    .buttonStyle(CreatorButtonStyle(prominent: true))
                    .disabled(model.source.isEmpty || model.project.isEmpty || model.archive.isEmpty || model.discoveryLoading || (model.copyChoice == "some" && model.units.isEmpty))
            }
        }.disabled(model.busy)
    }
}

struct ImporterSurface: View {
    @ObservedObject var model: Model
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack { Label("nüün · Ingreso", systemImage: "sdcard").font(.title3.weight(.semibold)); Spacer()
                if model.presentation == .selection { Button { model.changeMode?(.compact) } label: { Image(systemName: "arrow.down.right.and.arrow.up.left") }.help("Compactar") }
                Button { model.changeMode?(.full); model.selectedTab = 0 } label: { Image(systemName: "arrow.up.left.and.arrow.down.right") }.help("Expandir a vista completa")
            }
            HStack { Text(URL(fileURLWithPath: model.source).lastPathComponent).font(.caption); Spacer(); Button("Elegir fuente…") { model.chooseSource() } }
            SourceKindBadges(model: model)
            TextField("Nombre del proyecto", text: $model.project)
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
            PlanReview(model: model)
            if model.presentation == .compact { Spacer(minLength: 0) }
        }.padding(20).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}
