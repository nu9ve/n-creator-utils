import AppKit
import SwiftUI
import MediaMenuCore

func bytesLabel(_ value: Double) -> String { ByteCountFormatter.string(fromByteCount: Int64(value), countStyle: .file) }

/// One flow for every window size: the mode only changes density. Each step shows only its
/// decisions; nothing but the planner's real requirements blocks «Revisar plan».
struct IngestFlow: View {
    @Bindable var model: Model
    @State private var inputOptionsExpanded = true
    var compact: Bool { model.presentation != .full }
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            StepBar(model: model, compact: compact).padding(.horizontal, compact ? 14 : 20).padding(.vertical, 10)
            if [.source, .content].contains(model.ingestStep) {
                VStack(alignment: .leading, spacing: 10) {
                    ErrorBanner(model: model)
                    if compact {
                        DisclosureGroup("Fuente y asignación", isExpanded: $inputOptionsExpanded) {
                            SourceStep(model: model, compact: compact)
                        }.fixedSize(horizontal: false, vertical: true)
                    } else { SourceStep(model: model, compact: compact).fixedSize(horizontal: false, vertical: true) }
                    Divider()
                    ContentStep(model: model, compact: compact)
                }
                .padding(.horizontal, compact ? 14 : 20).padding(.vertical, 12)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        ErrorBanner(model: model)
                        if model.ingestStep == .run { RunStep(model: model) }
                        else {
                            if model.sourceKind != "delivery" {
                                DestinationsStep(model: model, compact: compact)
                            }
                            ReviewStep(model: model)
                        }
                    }
                    .padding(.horizontal, compact ? 14 : 20).padding(.vertical, 14)
                    .frame(maxWidth: .infinity, alignment: .topLeading)
                }
                .task(id: model.flowReviewKey) {
                    try? await Task.sleep(for: .milliseconds(350))
                    guard !Task.isCancelled, [.destinations, .review].contains(model.ingestStep), model.plan == nil,
                          !model.busy, model.sourceKind != "delivery", model.canPrepareFlow else { return }
                    model.reviewPlan()
                }
            }
        }
        .safeAreaInset(edge: .bottom, spacing: 0) { IngestFooter(model: model) }
        .onChange(of: model.contentLoaded) { _, loaded in if compact { inputOptionsExpanded = !loaded } }
        .onAppear { if compact && model.contentLoaded { inputOptionsExpanded = false } }
    }
}

struct StepBar: View {
    @Bindable var model: Model
    let compact: Bool
    @Environment(\.creatorPalette) private var palette
    private func done(_ step: IngestStep) -> Bool {
        switch step {
        case .source: return !model.source.isEmpty && model.projectIsReady
        case .content: return model.contentLoaded
        case .destinations: return !model.archive.isEmpty
        case .review: return model.plan != nil
        case .run: return model.lastResult != nil
        }
    }
    var body: some View {
        HStack(spacing: 4) {
            ForEach([IngestStep.source, .destinations, .run]) { step in
                let current = model.ingestStep == step || (step == .source && model.ingestStep == .content) || (step == .destinations && model.ingestStep == .review)
                Button { model.goToStep(step) } label: {
                    HStack(spacing: 5) {
                        Image(systemName: done(step) && !current ? "checkmark.circle.fill" : step.symbol)
                            .foregroundStyle(done(step) && !current ? palette.accentText : (current ? palette.ink : palette.muted))
                        Text(step.title).lineLimit(1)
                    }
                    .font(.callout.weight(current ? .semibold : .regular))
                    .padding(.horizontal, 9).padding(.vertical, 5)
                    .background(current ? palette.accentSoft : .clear, in: palette.pill)
                    .contentShape(palette.pill)
                }
                .buttonStyle(.plain)
                .disabled(model.ingestLocked || (step == .run && model.lastResult == nil && !model.busy))
                if step != .run { Image(systemName: "chevron.compact.right").foregroundStyle(palette.muted.opacity(0.6)) }
            }
        }
        .padding(3)
        .modifier(GlassSurface(shape: palette.pill))
        .frame(maxWidth: .infinity)
    }
}

struct IngestFooter: View {
    @Bindable var model: Model
    var body: some View {
        ActionFooter {
            if model.busy { ProgressView().controlSize(.small) }
            VStack(alignment: .leading, spacing: 1) {
                Text(model.message).font(.caption).lineLimit(1)
                if !model.busy, let blocker = model.planBlockers.first, [.source, .content, .destinations].contains(model.ingestStep) {
                    Text(blocker).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                }
            }
            Spacer(minLength: 0)
            if [.destinations, .review].contains(model.ingestStep) {
                Button("Atrás") { model.previousStep() }.disabled(model.ingestLocked)
            }
            switch model.ingestStep {
            case .source, .content:
                Button("Continuar al flujo", systemImage: "arrow.right") { model.goToStep(.destinations) }
                    .buttonStyle(CreatorButtonStyle(prominent: true)).disabled(model.busy || model.source.isEmpty)
            case .destinations, .review:
                if let plan = model.plan {
                    Button(plan["cleanup"].bool || plan["release_stalker_source"].bool ? "Respaldar y liberar origen" : "Respaldar y conservar origen", systemImage: "arrow.right.circle.fill") { model.apply() }
                        .buttonStyle(CreatorButtonStyle(prominent: true)).disabled(model.busy)
                } else {
                    Button("Preparar plan", systemImage: "checklist") { model.reviewPlan() }
                        .buttonStyle(CreatorButtonStyle(prominent: true)).disabled(model.busy || !model.canPrepareFlow)
                }
            case .run:
                if model.busy { Button("Cancelar") { model.cancel() } }
                else { Button("Nueva ingesta") { model.newIngest() }.buttonStyle(CreatorButtonStyle(prominent: true)) }
            }
        }
    }
}

// MARK: Step 1 · source and project

struct SourceStep: View {
    @Bindable var model: Model
    let compact: Bool
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            SectionTitle("Fuente")
            if !model.ingestDrafts.isEmpty {
                Menu("Retomar otro ingreso de esta sesión") {
                    ForEach(model.ingestDrafts.keys.sorted(), id: \.self) { path in
                        Button(URL(fileURLWithPath: path).lastPathComponent) { model.restoreIngestDraft(path) }
                    }
                }.fixedSize()
            }
            SourceTiles(model: model, compact: compact)
            StalkerFolderChoice(model: model)
            SectionTitle("Proyecto")
            ProjectField(model: model, compact: compact)
            DetailsRow(model: model, compact: compact)
        }.disabled(model.ingestLocked)
    }
}

struct SectionTitle: View {
    let title: LocalizedStringKey
    @Environment(\.creatorPalette) private var palette
    init(_ title: LocalizedStringKey) { self.title = title }
    var body: some View { Text(title).font(palette.heading(.headline)) }
}

struct SourceTiles: View {
    @Bindable var model: Model
    let compact: Bool
    var body: some View {
        let columns = [GridItem(.adaptive(minimum: compact ? 170 : 220), spacing: 10, alignment: .top)]
        LazyVGrid(columns: columns, alignment: .leading, spacing: 10) {
            ForEach(model.sourceCards, id: \.["path"].string) { card in
                let hint = card["hint"]
                SourceTile(selected: model.source == card["path"].string && model.sourceKind == "card",
                           symbol: "sdcard.fill", title: card["name"].string,
                           subtitle: hint == .null ? String(localized: "SD conectada") : model.familyLabel(hint["family"].string) + (hint["capacity_bytes"].double > 0 ? " · " + bytesLabel(hint["used_bytes"].double) + " / " + bytesLabel(hint["capacity_bytes"].double) : ""),
                           detail: hint == .null ? nil : model.recommendationLabel(hint["recommendation"].string),
                           eject: { model.eject(card["path"].string) }) {
                    model.useCard(card["path"].string); model.discover()
                }
            }
            // Every library is one Stalker: the folders only matter once there is something in two of them.
            if !model.captureRoots.isEmpty {
                SourceTile(selected: !model.source.isEmpty && model.sourceKind == "stalker" && model.captureRoots.contains(model.source), symbol: "record.circle",
                           title: "Stalker", subtitle: String(localized: "\(model.pendingCaptures.count) sesiones pendientes"), detail: nil, eject: nil) {
                    model.useStalker()
                }
            }
            let known = model.sourceCards.contains { $0["path"].string == model.source } || model.captureRoots.contains(model.source)
            if !model.source.isEmpty && !known {
                SourceTile(selected: true, symbol: model.sourceKind == "stalker" ? "record.circle" : "folder.fill", title: URL(fileURLWithPath: model.source).lastPathComponent,
                           subtitle: model.source, detail: nil, eject: nil) {}
            }
            SourceTile(selected: false, symbol: "folder.badge.plus", title: String(localized: "Otra carpeta…"),
                       subtitle: String(localized: "Tarjeta, disco o carpeta"), detail: nil, eject: nil) { model.chooseSource() }
        }
        if model.sourceCards.isEmpty && !model.volumes.contains(where: { $0["online"].bool }) {
            Text("Conecta tu disco de respaldo para que nüün sugiera las SD conectadas.").font(.caption).foregroundStyle(.secondary)
        }
    }
}

struct StalkerFolderChoice: View {
    @Bindable var model: Model
    var body: some View {
        if model.sourceKind == "stalker", model.pendingCaptureRoots.count > 1, model.captureRoots.contains(model.source) {
            VStack(alignment: .leading, spacing: 4) {
                Picker("Carpeta", selection: Binding(get: { model.source }, set: { model.prepareSource($0, kind: "stalker") })) {
                    ForEach(model.pendingCaptureRoots, id: \.self) { root in
                        Text(URL(fileURLWithPath: root).lastPathComponent + " · \(model.pendingSessions(in: root))").tag(root)
                    }
                }.pickerStyle(.segmented).fixedSize()
                Text("Hay sesiones en varias carpetas; cada una se respalda en su propio plan.").font(.caption).foregroundStyle(.secondary)
            }
        }
    }
}

struct SourceTile: View {
    let selected: Bool
    let symbol: String
    let title: String
    let subtitle: String
    let detail: String?
    let eject: (() -> Void)?
    let action: () -> Void
    @Environment(\.creatorPalette) private var palette
    var body: some View {
        Button(action: action) {
            HStack(alignment: .top, spacing: 10) {
                Image(systemName: symbol).font(.title2).foregroundStyle(selected ? palette.accentText : palette.muted).frame(width: 28)
                VStack(alignment: .leading, spacing: 3) {
                    Text(title).font(.callout.weight(.semibold)).lineLimit(1)
                    Text(subtitle).font(.caption).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
                    if let detail { Text(detail).font(.caption2).foregroundStyle(palette.muted).fixedSize(horizontal: false, vertical: true) }
                }
                Spacer(minLength: 0)
            }
            .padding(10)
            .frame(maxWidth: .infinity, minHeight: 64, alignment: .topLeading)
            .background(palette.raised.opacity(selected ? 0.6 : 0.35), in: palette.panel)
            .overlay(palette.panel.stroke(selected ? palette.accentText : palette.line.opacity(0.6), lineWidth: selected ? 1.5 : 1))
            .contentShape(palette.panel)
        }
        .buttonStyle(.plain)
        .overlay(alignment: .topTrailing) {
            if let eject {
                Button(action: eject) { Image(systemName: "eject") }.buttonStyle(.borderless).padding(8)
                    .help("Expulsar SD de forma segura").accessibilityLabel("Expulsar SD de forma segura")
            }
        }
    }
}

struct ProjectField: View {
    @Bindable var model: Model
    let compact: Bool
    private var hint: JSONValue { model.selectedCard?["hint"] ?? .null }
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            TextField(model.sourceKind == "stalker" ? "Proyecto para sesiones sin asignar (opcional)" : "Nombre del proyecto", text: $model.project)
                .font(.title3).textFieldStyle(CreatorTextFieldStyle())
                .onSubmit { if model.planBlockers.isEmpty { model.nextStep() } }
            ProjectNamePreview(model: model, name: model.project)
            HStack(spacing: 6) {
                RecentProjectChips(projects: model.ingestProjects, selection: $model.project, limit: compact ? 3 : 6)
                if model.ingestProjects.count > (compact ? 3 : 6) {
                    Menu("Más") {
                        ForEach(Array(model.ingestProjects.dropFirst(compact ? 3 : 6)), id: \.["project"].string) { item in
                            Button(item["project"].string) { model.project = item["project"].string }
                        }
                    }.fixedSize().controlSize(.small)
                }
            }
            if model.project.isEmpty, !hint["last_project"].string.isEmpty {
                Button("Usado antes con esta tarjeta: \(hint["last_project"].string)") {
                    model.project = hint["last_project"].string
                    if !hint["last_profile"].string.isEmpty { model.profile = hint["last_profile"].string }
                }.buttonStyle(.link).font(.caption)
            } else if model.project.isEmpty, !model.lastProject.isEmpty, model.sourceKind != "stalker" {
                Button("Último proyecto: \(model.lastProject)") { model.project = model.lastProject }.buttonStyle(.link).font(.caption)
            }
            if model.sourceKind == "stalker" {
                Text("Cada sesión conserva su proyecto; éste sólo completa las que no tengan uno. Puedes asignarlas en Contenido.").font(.caption).foregroundStyle(.secondary)
            } else if model.contentLoaded && Set(model.units.map(\.project).filter { !$0.isEmpty }).count > 0 {
                Text("Algunas unidades tienen su propio proyecto; éste completa las demás.").font(.caption).foregroundStyle(.secondary)
            }
        }
    }
}

struct DetailsRow: View {
    @Bindable var model: Model
    let compact: Bool
    var body: some View {
        HStack(spacing: 10) {
            ProfilePicker(model: model)
            TextField("País", text: $model.country).frame(width: 52).help("País")
            TextField("Ciudad", text: $model.city).frame(maxWidth: compact ? 120 : 180).help("Ciudad")
            Spacer(minLength: 0)
        }
    }
}

struct ProfilePicker: View {
    @Bindable var model: Model
    var body: some View {
        Menu {
            ForEach(model.profiles, id: \.["id"].string) { value in
                Button(value["name"].string + " · " + value["raw_dir"].string) { model.selectProfile(value["id"].string) }
            }
            if model.profiles.isEmpty { Text("Sin perfiles guardados") }
            Divider()
            Button("Administrar perfiles…") { model.openSettings?("profiles") }
        } label: { Label(model.profileName(model.profile), systemImage: "person.crop.square").lineLimit(1) }
        .fixedSize()
        .help("Perfil creativo y espacio de trabajo")
    }
}

extension Model {
    /// The ID stays in folders and receipts; people read the name they gave it.
    func profileName(_ id: String) -> String {
        let name = profiles.first { $0["id"].string == id }?["name"].string ?? ""
        return name.isEmpty ? id : name
    }
    func selectProfile(_ id: String) {
        profile = id
        if let value = profiles.first(where: { $0["id"].string == id }) {
            if !value["work"].string.isEmpty { work = value["work"].string }
            if !value["archive"].string.isEmpty { archive = value["archive"].string }
        }
    }
    var cardBytes: Double {
        if contentLoaded { return units.reduce(0) { $0 + $1.bytes } }
        return selectedCard?["hint"]["used_bytes"].double ?? 0
    }
    /// Shown before planning; the planner still checks real space and reservations.
    var spaceWarnings: [String] {
        let needed = cardBytes
        guard needed > 0 else { return [] }
        var result: [String] = []
        for id in [archive, noWork ? "" : work] where !id.isEmpty {
            guard let target = volume(id), target["online"].bool, target["free"] != .null else { continue }
            if target["free"].double < needed {
                result.append(String(localized: "\(target["name"].string) tiene \(bytesLabel(target["free"].double)) libres; el lote ocupa \(bytesLabel(needed))"))
            }
        }
        return result
    }
}

struct DestinationSummary: View {
    @Bindable var model: Model
    @Environment(\.creatorPalette) private var palette
    private func name(_ id: String) -> String {
        guard let value = model.volume(id) else { return String(localized: "sin elegir") }
        return value["name"].string + (value["online"].bool ? "" : " · " + String(localized: "desconectado"))
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .firstTextBaseline) {
                SectionTitle("Destinos")
                Spacer()
                Button("Cambiar") { model.goToStep(.destinations) }.buttonStyle(.link)
            }
            VStack(alignment: .leading, spacing: 4) {
                Label("Respaldo: \(name(model.archive))", systemImage: "externaldrive.fill")
                Label(model.copyChoice == "none" ? String(localized: "Sin copia para editar") : String(localized: "Editar en \(name(model.work)) · \(model.copyChoice == "all" ? String(localized: "todo el lote") : String(localized: "elegidos"))"), systemImage: "externaldrive")
                if model.selectedCard != nil {
                    Label(model.cleanup ? String(localized: "Limpiar la SD después de verificar") : String(localized: "Conservar la SD"), systemImage: model.cleanup ? "sparkles" : "lock")
                }
                ForEach(model.spaceWarnings, id: \.self) { Label($0, systemImage: "exclamationmark.triangle").foregroundStyle(palette.warning) }
            }
            .font(.callout)
            .padding(10).frame(maxWidth: .infinity, alignment: .leading)
            .background(palette.raised.opacity(0.35), in: palette.panel)
        }
    }
}

// MARK: Step 2 · content

struct ContentStep: View {
    @Environment(\.creatorPalette) private var palette
    @Bindable var model: Model
    let compact: Bool
    private let filters: [(String, LocalizedStringKey, String)] = [("", "Principal", "square.grid.2x2"), ("all", "Todos los archivos", "doc.on.doc"), ("video", "Videos", "film"), ("image", "Fotos", "photo"),
        ("raw_image", "RAW", "camera.aperture"), ("audio", "Audio", "waveform"), ("drone", "Dron", "airplane"), ("360", "360°", "view.360"),
        ("stalker", "Stalker", "record.circle"), ("support", "Auxiliares", "doc"), ("dji", "DJI sin clasificar", "questionmark.circle")]
    private func count(_ key: String) -> Int {
        switch key {
        case "": return model.units.filter { !$0.isSupport }.count
        case "all": return model.units.count
        case "dji": return model.units.filter(\.needsCameraReview).count
        default: return model.units.filter { $0.displayCategory == key }.count
        }
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 6) {
                ScrollView(.horizontal) {
                    HStack(spacing: 6) {
                        ForEach(filters, id: \.0) { key, label, symbol in
                            let total = count(key)
                            if key.isEmpty || total > 0 {
                                Button { model.typeFilter = key; model.gridSelection = [] } label: {
                                    Label { Text(label) + Text(" \(total)").foregroundStyle(.secondary) } icon: { Image(systemName: symbol) }
                                }
                                .buttonStyle(CreatorButtonStyle(prominent: model.typeFilter == key)).controlSize(.small)
                            }
                        }
                    }
                }.scrollIndicators(.hidden)
                Spacer(minLength: 0)
                Text(bytesLabel(model.visibleUnits.reduce(0) { $0 + $1.bytes })).font(.caption).foregroundStyle(.secondary)
                Button(model.units.isEmpty ? "Inspeccionar" : "Actualizar", systemImage: "arrow.clockwise") { model.discover() }
                    .disabled(model.source.isEmpty || model.discoveryLoading).controlSize(.small)
            }
            let support = model.units.filter(\.isSupport)
            if !support.isEmpty && model.typeFilter != "support" && model.typeFilter != "all" {
                Text("Datos de cámara incluidos en el respaldo: \(support.reduce(0) { $0 + $1.paths.count }) archivos").font(.caption).foregroundStyle(.secondary)
            }
            if model.discoveryLoading { HStack { ProgressView().controlSize(.small); Text("Leyendo archivos… Las miniaturas se cargan conforme aparecen.").font(.caption) } }
            if !model.discoveryError.isEmpty { HStack { Label(model.discoveryError, systemImage: "exclamationmark.triangle").foregroundStyle(palette.warning); Button("Reintentar") { model.discover() } } }
            if compact {
                ImportGrid(model: model, dense: true).frame(maxHeight: .infinity)
                AssignPanel(model: model, compact: true)
            } else {
                HStack(alignment: .top, spacing: 16) {
                    ImportGrid(model: model, dense: false).frame(maxHeight: .infinity)
                    ScrollView { AssignPanel(model: model, compact: false) }.frame(width: 280).scrollIndicators(.hidden)
                }.frame(maxHeight: .infinity)
            }
        }.disabled(model.ingestLocked)
    }
}

/// Bulk assignment for the selection, or for everything visible when nothing is selected.
struct AssignPanel: View {
    @Bindable var model: Model
    let compact: Bool
    @State private var title = ""
    @State private var place = ""
    @Environment(\.creatorPalette) private var palette
    private let cameras = ["auto", "dji-drone", "dji-360", "dji-action", "sony", "canon", "nikon", "gopro", "h6", "insta360", "generic"]
    var body: some View {
        let count = model.targetIDs.count
        VStack(alignment: .leading, spacing: compact ? 8 : 10) {
            HStack {
                Text(model.gridSelection.isEmpty ? "Aplicar a los \(count) visibles" : "Aplicar a \(count) seleccionados").font(.callout.weight(.semibold))
                Spacer()
                if !model.gridSelection.isEmpty { Button("Quitar selección") { model.gridSelection = [] }.buttonStyle(.link).font(.caption) }
                else if !model.visibleUnits.isEmpty { Button("Seleccionar todo") { model.gridSelection = Set(model.visibleUnits.map(\.id)) }.buttonStyle(.link).font(.caption) }
            }
            if !model.units.contains(where: { model.targetIDs.contains($0.id) && !$0.isSupport }) {
                Text("Los auxiliares se respaldan con el ingreso; no necesitan proyecto propio.").font(.caption).foregroundStyle(.secondary)
            } else if compact {
                projectField
                HStack(spacing: 6) { metadataMenus; Spacer(minLength: 0) }
                HStack(spacing: 6) { chips; Spacer(minLength: 0); copyControls }
            } else {
                projectField
                chips
                Button("Usar el proyecto del lote") { model.updateTargets { $0.project = "" } }.buttonStyle(.link).font(.caption)
                HStack(spacing: 8) { metadataMenus }.controlSize(.small)
                Divider()
                copyControls
                Text("Clic selecciona · ⌘ o ⇧ amplían · espacio abre vista rápida").font(.caption2).foregroundStyle(.secondary)
            }
        }
        .modifier(Panel(padding: compact ? 10 : 12))
    }
    private var projectField: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 6) {
                TextField("Proyecto", text: $title).onSubmit(applyTitle)
                Button("Aplicar", action: applyTitle).fixedSize().disabled(title.trimmingCharacters(in: .whitespaces).isEmpty)
            }
            ProjectNamePreview(model: model, name: title)
        }
    }
    private var chips: some View {
        RecentProjectChips(projects: model.ingestProjects, selection: Binding(get: { "" }, set: { value in
            title = value; applyTitle()
        }), limit: compact ? 3 : 5)
    }
    @ViewBuilder private var metadataMenus: some View {
        Menu {
            Button("Perfil del lote: \(model.profileName(model.profile))") { model.updateTargets { $0.profile = "" } }
            ForEach(model.profiles, id: \.["id"].string) { item in
                Button(item["name"].string) { model.updateTargets { $0.profile = item["id"].string } }
            }
        } label: { Label("Perfil", systemImage: "person.crop.square") }.fixedSize()
        Menu {
            ForEach(cameras, id: \.self) { value in Button(value == "auto" ? String(localized: "Detectada") : value) { model.updateTargets { $0.camera = value } } }
        } label: { Label("Cámara", systemImage: "camera") }.fixedSize()
        TextField("País", text: $place).frame(width: 72).onSubmit { let value = place; model.updateTargets { $0.country = value } }
    }
    @ViewBuilder private var copyControls: some View {
        if model.copyChoice == "some" {
            HStack {
                Button("Copiar al SSD") { model.updateTargets { if $0.previewPath != nil { $0.copyForEditing = true } } }
                Button("No copiar") { model.updateTargets { $0.copyForEditing = false } }
            }.controlSize(.small)
            if !compact { Text("\(model.units.filter(\.copyForEditing).count) de \(model.units.count) se copiarán al SSD para editar").font(.caption).foregroundStyle(.secondary) }
        } else if compact {
            Button("Elegir qué copiar al SSD") { model.copyChoice = "some" }.buttonStyle(.link).font(.caption)
        } else {
            Text(model.copyChoice == "all" ? "Todo el lote se copia al SSD para editar." : "Sólo respaldo: nada se copia al SSD.").font(.caption).foregroundStyle(.secondary)
            Button("Elegir qué copiar al SSD") { model.copyChoice = "some" }.buttonStyle(.link).font(.caption)
        }
    }
    private func applyTitle() {
        let value = title.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !value.isEmpty else { return }
        model.updateTargets { $0.project = value }
    }
}

// MARK: Step 3 · destinations

struct DestinationsStep: View {
    @Environment(\.creatorPalette) private var palette
    @Bindable var model: Model
    let compact: Bool
    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            SectionTitle("Respaldo")
            VolumeChoices(model: model, role: "archive", selection: $model.archive, allowNone: false)
            SectionTitle("Copia para editar")
            EditingChoice(model: model, compact: compact)
            if model.copyChoice != "none" {
                VolumeChoices(model: model, role: "work", selection: $model.work, allowNone: true)
                Toggle("Si el SSD no está conectado, dejar la copia para editar pendiente", isOn: $model.deferWork)
            }
            if !model.noWork && !model.volumes.contains(where: { $0["id"].string == model.work && $0["online"].bool }) {
                Toggle("Esta vez, continuar sin el SSD y respaldar primero", isOn: $model.archiveExceptionConfirmed).toggleStyle(.checkbox)
            }
            SectionTitle("Después de verificar")
            if model.sourceKind == "stalker" {
                Text(model.retainStalkerSource ? "El original Stalker se conserva según Ajustes." : "Después de verificar se revisa Clipper y se libera el original Stalker; si algo falla, se conserva.")
                    .font(.caption).foregroundStyle(.secondary)
            } else {
                Toggle("Limpiar la SD después de verificar todos los destinos", isOn: $model.cleanup).disabled(model.selectedCard == nil)
                Text("Sony: se borran los clips verificados; los índices se conservan hasta formatear en la cámara.").font(.caption).foregroundStyle(.secondary)
            }
            ForEach(model.spaceWarnings, id: \.self) { Label($0, systemImage: "exclamationmark.triangle").foregroundStyle(palette.warning) }
            Button("Registrar otro disco…") { model.openSettings?("disks") }.buttonStyle(.link)
        }.disabled(model.ingestLocked)
    }
}

struct VolumeChoices: View {
    @Bindable var model: Model
    let role: String
    @Binding var selection: String
    let allowNone: Bool
    @Environment(\.creatorPalette) private var palette
    var body: some View {
        let options = model.volumes.filter { $0["role"].string == role }
        VStack(alignment: .leading, spacing: 6) {
            if allowNone { row(id: "", title: String(localized: "Sin SSD"), detail: String(localized: "Sólo respaldo ahora"), online: true) }
            ForEach(options, id: \.["id"].string) { value in
                row(id: value["id"].string, title: value["name"].string,
                    detail: value["online"].bool ? (value["free"] == .null ? String(localized: "Conectado") : String(localized: "\(bytesLabel(value["free"].double)) libres")) : String(localized: "Desconectado"),
                    online: value["online"].bool)
            }
            if options.isEmpty { Text("No hay discos registrados para este rol.").font(.caption).foregroundStyle(.secondary) }
        }
    }
    private func row(id: String, title: String, detail: String, online: Bool) -> some View {
        Button { selection = id } label: {
            HStack {
                Image(systemName: selection == id ? "largecircle.fill.circle" : "circle").foregroundStyle(selection == id ? palette.accentText : palette.muted)
                Text(title).fontWeight(selection == id ? .semibold : .regular)
                Text(detail).font(.caption).foregroundStyle(online ? Color.secondary : palette.warning)
                Spacer()
            }.padding(.vertical, 5).padding(.horizontal, 8).contentShape(Rectangle())
            .background(selection == id ? palette.raised.opacity(0.5) : .clear, in: palette.control)
        }.buttonStyle(.plain)
    }
}

// MARK: Step 4 · review

struct ReviewStep: View {
    @Bindable var model: Model
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            if model.plan != nil {
                PlanReview(model: model)
            } else if model.busy {
                HStack { ProgressView().controlSize(.small); Text(model.message) }
            } else {
                SectionTitle("Antes de revisar")
                if model.planBlockers.isEmpty {
                    Text("Todo listo. «Revisar plan» calcula rutas, espacio y verificaciones; todavía no copia nada.").foregroundStyle(.secondary)
                } else {
                    ForEach(model.planBlockers, id: \.self) { Label($0, systemImage: "circle.dashed") }
                    HStack {
                        Button("Ir a fuente y proyecto") { model.goToStep(.source) }
                        if model.units.contains(where: \.needsCameraReview) { Button("Clasificar en Contenido") { model.typeFilter = "dji"; model.goToStep(.content) } }
                    }
                }
            }
        }
    }
}

struct PlanReview: View {
    @Bindable var model: Model
    @Environment(\.creatorPalette) private var palette
    var body: some View {
        if let plan = model.plan {
            let files = plan["files"].array
            let bytes = files.reduce(0.0) { $0 + $1["stamp"]["size"].double }
            VStack(alignment: .leading, spacing: 10) {
                Label("Plan revisable · \(files.count) archivos · \(bytesLabel(bytes))", systemImage: "checklist").font(palette.heading(.title3))
                let projects = Dictionary(grouping: plan["units"].array.filter { !$0["support"].bool }) { $0["project"].string }
                FlowSummary(items: projects.keys.sorted().map { title in
                    let count = projects[title, default: []].count
                    return count > 1 ? "\(title) (\(count))" : title
                })
                VStack(alignment: .leading, spacing: 4) {
                    Label("Respaldo en \(model.volume(plan["archive_id"].string)?["name"].string ?? plan["archive_id"].string)", systemImage: "externaldrive.fill")
                    Label(plan["work_id"] == .null ? model.workLabel(plan["work_delivery_status"].string) : String(localized: "Copias para editar en \(model.volume(plan["work_id"].string)?["name"].string ?? "") verificadas antes de limpiar"), systemImage: "externaldrive")
                    Label(cleanupText(plan), systemImage: plan["cleanup"].bool || plan["release_stalker_source"].bool ? "exclamationmark.triangle" : "lock")
                        .foregroundStyle(plan["cleanup"].bool || plan["release_stalker_source"].bool ? palette.warning : palette.ink)
                }.font(.callout)
                if plan["source_kind"].string == "delivery" {
                    Toggle("Conservar en el disco de edición", isOn: Binding(get: { model.finalsKeepSource }, set: { value in
                        model.finalsKeepSource = value
                        if let group = model.finalsGroup { model.planFinals(group) }
                    }))
                    .toggleStyle(.checkbox).font(.callout).disabled(model.busy)
                    let skipped = plan["already_archived"].array.count
                    if skipped > 0 { Label("Ya en el respaldo con igual nombre y tamaño; se quedan en la carpeta: \(skipped)", systemImage: "equal.circle").font(.callout) }
                }
                ForEach(Array(plan["units"].array.enumerated()), id: \.offset) { _, unit in
                    PlannedUnitRow(model: model, plan: plan, unit: unit)
                }
            }
            .modifier(Panel(padding: 14))
        }
    }
    func cleanupText(_ plan: JSONValue) -> String {
        if plan["source_kind"].string == "stalker" {
            return plan["release_stalker_source"].bool
                ? String(localized: "Después de verificar se revisa Clipper y se libera el original Stalker; si algo falla, se conserva.")
                : String(localized: "El original Stalker se conserva según Ajustes.")
        }
        if !plan["cleanup"].bool { return String(localized: "El origen se conserva.") }
        if plan["source_kind"].string == "delivery" { return String(localized: "Después de verificar y volver a leer cada archivo, se borra del disco de edición. Las carpetas se quedan.") }
        return plan["cleanup_profile"].string == "sony_clips_then_format"
            ? String(localized: "Después de verificar se borran los clips; termina con formato en la cámara.")
            : String(localized: "La SD se limpia después de verificar.")
    }
}

struct FlowSummary: View {
    let items: [String]
    @Environment(\.creatorPalette) private var palette
    var body: some View {
        ScrollView(.horizontal) {
            HStack(spacing: 6) {
                ForEach(items, id: \.self) { Text($0).font(.caption.weight(.medium)).padding(.horizontal, 8).padding(.vertical, 3).background(palette.raised.opacity(0.6), in: palette.pill) }
            }
        }.scrollIndicators(.hidden)
    }
}

// MARK: Step 5 · copy and result

struct RunStep: View {
    @Environment(\.creatorPalette) private var palette
    @Bindable var model: Model
    private var runNote: LocalizedStringKey {
        guard let plan = model.plan else { return "Cancelar se detiene en un punto recuperable." }
        if plan["cleanup"].bool {
            return "Cada archivo se copia y se verifica en cada destino; el origen se relee antes de retirarlo. Cancelar se detiene en un punto recuperable y lo verificado se conserva."
        }
        return "Cada archivo se copia y se verifica en cada destino; el origen se conserva. Cancelar se detiene en un punto recuperable y lo verificado se conserva."
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            if model.busy {
                let progress = model.jobProgress?["progress"] ?? .object([:])
                ActionProgressView(title: model.plan?["source_kind"].string == "delivery" ? "Respaldando publicados" : "Copiando y verificando",
                                   note: runNote,
                                   state: .transfer(progress, phase: model.jobProgress.map { model.progressLabel($0) } ?? model.message))
                    .frame(minHeight: 380)
                if let job = model.jobProgress, job["progress"]["eta_seconds"].double > 0 {
                    let minutes = max(1, Int((job["progress"]["eta_seconds"].double / 60).rounded(.up)))
                    Text(job["progress"]["eta_scope"].string == "backup" ? "Respaldo: ~\(minutes) min; después se revisa la liberación" : "Restante aproximado: \(minutes) min")
                        .font(.caption).foregroundStyle(.secondary).frame(maxWidth: .infinity)
                }
            } else if let result = model.lastResult {
                Label(model.jobLabel(result), systemImage: "checkmark.seal.fill").font(palette.heading(.title3)).foregroundStyle(palette.success)
                if result["state"].string == "camera_action_required", model.cleanupProfile(result) == "sony_clips_then_format" {
                    Text("Formatea la tarjeta en la cámara. Nüün la comprobará cuando la vuelvas a conectar.").foregroundStyle(.secondary)
                }
                HStack {
                    if model.selectedCard != nil { Button("Expulsar SD", systemImage: "eject") { model.ejectSource() } }
                    Button("Abrir biblioteca") { model.open(.files); model.changeMode?(.full) }
                    Button("Abrir actividad") { model.open(.activity); model.changeMode?(.full) }
                }
            } else {
                Text("La operación se detuvo. Revisa el mensaje y el estado en Actividad.").foregroundStyle(.secondary)
                HStack {
                    Button("Volver a revisar") { model.goToStep(.review) }
                    Button("Abrir actividad") { model.open(.activity); model.changeMode?(.full) }
                }
            }
        }
    }
}


/// Ask the same engine that names projects; no second slug implementation in Swift.
struct ProjectNamePreview: View {
    let model: Model
    let name: String
    @State private var resolved = ""
    @State private var resolvedInput = ""
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if !resolved.isEmpty && resolvedInput == name {
                Text("Se guardará como: \(resolved)").font(.caption).foregroundStyle(.secondary)
                    .textSelection(.enabled).fixedSize(horizontal: false, vertical: true)
            }
        }
        .task(id: name) {
            resolved = ""
            guard !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
            try? await Task.sleep(for: .milliseconds(200))
            guard !Task.isCancelled, let reader = model.reader else { return }
            if let value = try? await reader.call("project_preview", ["name": .string(name)]), !Task.isCancelled {
                resolvedInput = name; resolved = value["name"].string
            }
        }
    }
}
