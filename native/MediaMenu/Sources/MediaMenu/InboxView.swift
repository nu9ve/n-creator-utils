import SwiftUI
import MediaMenuCore

struct JobStatusIcon: View {
    let state: String
    var body: some View {
        switch state {
        case "completed", "source_retained", "recovered": Image(systemName: "checkmark.circle.fill").foregroundStyle(.green)
        case "failed": Image(systemName: "exclamationmark.octagon.fill").foregroundStyle(.red)
        case "paused", "camera_action_required": Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
        default: Image(systemName: "arrow.triangle.2.circlepath").foregroundStyle(.blue)
        }
    }
}

struct InboxView: View {
    @Bindable var model: Model
    private var pendingWork: Int { model.assets.filter { $0["work_delivery_status"].string == "pending" }.count }
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                PageHeader(title: "Inicio", subtitle: "Lo que puedes respaldar o atender ahora. Nada se mueve sin un plan revisado.") {
                    Button("Elegir fuente…", systemImage: "folder") { model.chooseSource() }
                }
                ErrorBanner(model: model)
                DiskStrip(model: model)
                if !model.sourceCards.isEmpty {
                    SectionTitle("Tarjetas conectadas")
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 300), spacing: 12, alignment: .top)], alignment: .leading, spacing: 12) {
                        ForEach(model.sourceCards, id: \.["path"].string) { card in CardSuggestion(model: model, card: card) }
                    }
                }
                if !model.attentionJobs.isEmpty || pendingWork > 0 {
                    AttentionSection(model: model, pendingWork: pendingWork)
                }
                if !model.captureError.isEmpty { Text(model.captureError).font(.caption).foregroundStyle(.orange) }
                if !model.pendingCaptures.isEmpty { StalkerSection(model: model) }
                FinalsSection(model: model)
                let archived = model.captureSessions.filter { $0["archived"].bool && $0["status"].string == "completed" }
                if !archived.isEmpty {
                    DisclosureGroup("\(archived.count) sesiones archivadas con original local") {
                        ForEach(archived, id: \.["path"].string) { value in
                            HStack { Text(URL(fileURLWithPath: value["path"].string).lastPathComponent).font(.caption); Spacer(); Button("Revisar liberar local…") { model.prepareLocalRelease(value) }.disabled(model.archive.isEmpty || model.busy) }
                        }
                    }
                }
                if model.sourceCards.isEmpty && model.pendingCaptures.isEmpty && model.attentionJobs.isEmpty {
                    ContentUnavailableView("Todo al día", systemImage: "checkmark.circle", description: Text("Conecta una SD o termina una sesión de Stalker para verla aquí."))
                        .frame(maxWidth: .infinity)
                }
            }.frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 20).padding(.vertical, 14)
        }
        // Names and sizes only, and at most every 15 minutes; never a timer of its own.
        .onAppear { model.checkFinals() }
    }
}

/// A group of related things: a tone of the canvas, never a white card on top of it.
struct Panel: ViewModifier {
    var padding: CGFloat = 12
    @Environment(\.creatorPalette) private var palette
    func body(content: Content) -> some View {
        content.padding(padding).frame(maxWidth: .infinity, alignment: .leading)
            .background(palette.inset.opacity(0.35), in: RoundedRectangle(cornerRadius: 10))
    }
}

/// Each reminder on its own line, with its action and its own way out.
struct AttentionSection: View {
    @Bindable var model: Model
    let pendingWork: Int
    private let shown = 3
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                SectionTitle("Requiere atención")
                Spacer()
                if model.attentionJobs.count > shown { Button("Ver las \(model.attentionJobs.count) en Actividad") { model.open(.activity) }.buttonStyle(.link).font(.caption) }
            }
            ForEach(Array(model.attentionJobs.prefix(shown)), id: \.["id"].string) { job in
                HStack(spacing: 10) {
                    JobStatusIcon(state: job["state"].string)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(model.jobLabel(job)).font(.callout)
                        Text(Timestamps.label(job["updated_at"].string)).font(.caption).foregroundStyle(.secondary)
                    }
                    Spacer()
                    JobActions(model: model, job: job)
                    Button { model.dismissJob(job["id"].string) } label: { Image(systemName: "xmark") }
                        .buttonStyle(.borderless).help("Descartar aviso; la operación sigue en Actividad").accessibilityLabel("Descartar aviso")
                }
                Divider()
            }
            if pendingWork > 0 {
                HStack { Label("\(pendingWork) copias para editar pendientes", systemImage: "externaldrive.badge.timemachine"); Spacer(); Button("Abrir biblioteca") { model.open(.files) } }
            }
        }
    }
}

/// Every library reads as one list. A folder only shows up when there is more than one to prepare.
struct StalkerSection: View {
    @Bindable var model: Model
    private var unassigned: [JSONValue] { model.pendingCaptures.filter { $0["tag"]["name"].string.isEmpty } }
    private var roots: [String] { model.pendingCaptureRoots }
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                SectionTitle("Stalker")
                Text("\(model.pendingCaptures.count) pendientes").foregroundStyle(.secondary)
                Spacer()
                ForEach(roots, id: \.self) { root in
                    Button(roots.count > 1 ? "Preparar lote · \(URL(fileURLWithPath: root).lastPathComponent) · \(model.pendingSessions(in: root))" : "Preparar lote · \(model.pendingSessions(in: root))") { model.prepareCaptureBatch(root) }
                        .buttonStyle(CreatorButtonStyle(prominent: root == roots.first))
                }
                Button { model.captureChanged([]) } label: { Image(systemName: "arrow.clockwise") }
                    .buttonStyle(.borderless).help("Actualizar").accessibilityLabel("Actualizar")
            }
            if !model.lastTagged.isEmpty && !unassigned.isEmpty {
                Button("Usar «\(model.lastTagged)» en las \(unassigned.count) sin proyecto") {
                    model.tagCaptures(unassigned, project: model.lastTagged, country: model.country)
                }.buttonStyle(.link).font(.callout)
            }
            ForEach(model.pendingCaptures, id: \.["path"].string) { value in
                CaptureInboxRow(model: model, capture: value)
                Divider()
            }
        }
    }
}

/// Read-only until finals have a route of their own in the engine: today's would duplicate whole folders.
struct FinalsSection: View {
    @Bindable var model: Model
    private var groups: [JSONValue] { (model.finals?["groups"].array ?? []).filter { $0["pending"].double > 0 } }
    var body: some View {
        if !groups.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    SectionTitle("Finales sin respaldar")
                    Text(verbatim: bytesLabel(groups.reduce(0) { $0 + $1["pending_bytes"].double })).foregroundStyle(.secondary)
                    Spacer()
                    if model.finalsChecking { ProgressView().controlSize(.small) }
                    Button { model.checkFinals(force: true) } label: { Image(systemName: "arrow.clockwise") }
                        .buttonStyle(.borderless).help("Comprobar de nuevo").accessibilityLabel("Comprobar de nuevo").disabled(model.finalsChecking || model.busy)
                }
                ForEach(groups, id: \.["folder"].string) { group in
                    DisclosureGroup {
                        VStack(alignment: .leading, spacing: 3) {
                            ForEach(Array(group["files"].array.prefix(30).enumerated()), id: \.offset) { _, file in
                                HStack { Text(file["name"].string).lineLimit(1).truncationMode(.middle); Spacer(); Text(bytesLabel(file["bytes"].double)).foregroundStyle(.secondary) }
                            }
                            if group["pending"].double > 30 { Text("y \(Int(group["pending"].double) - 30) más").foregroundStyle(.secondary) }
                            Button("Mostrar en Finder") { NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: group["folder"].string)]) }.buttonStyle(.link)
                        }.font(.caption).padding(.top, 4)
                    } label: {
                        HStack(spacing: 8) {
                            Text(group["profile"].string).font(.callout.weight(.medium))
                            Text("\(Int(group["pending"].double)) de \(Int(group["total"].double)) · \(bytesLabel(group["pending_bytes"].double))").font(.caption).foregroundStyle(.secondary)
                            Spacer()
                            Label(group["destination"].string, systemImage: "arrow.right").font(.caption).foregroundStyle(.secondary).lineLimit(1)
                        }
                    }
                }
                Text("Compara nombre y tamaño contra la carpeta del perfil en el respaldo, sin leer contenido. Copiarlos desde nüün aún no está disponible.").font(.caption).foregroundStyle(.secondary)
            }
        }
    }
}

struct DiskStrip: View {
    @Bindable var model: Model
    var body: some View {
        ScrollView(.horizontal) {
            HStack(spacing: 8) {
                ForEach(model.volumes, id: \.["id"].string) { value in
                    let online = value["online"].bool
                    HStack(spacing: 6) {
                        Image(systemName: online ? "externaldrive.fill.badge.checkmark" : "externaldrive.badge.xmark").foregroundStyle(online ? .green : .secondary)
                        Text(value["name"].string).font(.caption.weight(.semibold))
                        Text(online ? (value["free"] == .null ? String(localized: "conectado") : String(localized: "\(bytesLabel(value["free"].double)) libres")) : String(localized: "desconectado"))
                            .font(.caption).foregroundStyle(.secondary)
                    }
                    .padding(.horizontal, 10).padding(.vertical, 6)
                    .modifier(Chip())
                }
                Button("Discos…") { model.openSettings?("disks") }.buttonStyle(.link).font(.caption)
            }
        }.scrollIndicators(.hidden)
    }
}

struct Chip: ViewModifier {
    @Environment(\.creatorPalette) private var palette
    func body(content: Content) -> some View { content.background(palette.inset.opacity(0.5), in: Capsule()) }
}

struct CardSuggestion: View {
    @Bindable var model: Model
    let card: JSONValue
    var body: some View {
        let hint = card["hint"], path = card["path"].string
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .top) {
                Image(systemName: "sdcard.fill").font(.title2).foregroundStyle(.orange)
                VStack(alignment: .leading, spacing: 2) {
                    Text(card["name"].string).font(.headline)
                    Text(model.familyLabel(hint["family"].string) + (hint["capacity_bytes"].double > 0 ? " · " + bytesLabel(hint["used_bytes"].double) + " / " + bytesLabel(hint["capacity_bytes"].double) : ""))
                        .font(.caption).foregroundStyle(.secondary)
                }
                Spacer()
                Button { model.eject(path) } label: { Image(systemName: "eject") }.buttonStyle(.borderless).help("Expulsar SD de forma segura").disabled(model.busy)
            }
            if hint != .null { Label(model.recommendationLabel(hint["recommendation"].string), systemImage: "lightbulb").font(.callout) }
            if !hint["last_project"].string.isEmpty { Text("Usada antes en: \(hint["last_project"].string)").font(.caption).foregroundStyle(.secondary) }
            HStack {
                Button("Ingreso rápido") { model.useCard(path); model.changeMode?(.compact) }
                Spacer()
                Button("Ingresar", systemImage: "arrow.right") { model.useCard(path) }.buttonStyle(CreatorButtonStyle(prominent: true))
            }
        }.modifier(Panel())
    }
}

struct CaptureInboxRow: View {
    @Bindable var model: Model
    let capture: JSONValue
    @State private var project = ""
    @State private var country = "mx"
    private var savedProject: String { capture["tag"]["name"].string }
    private var savedCountry: String { capture["tag"]["country"].string }
    private var title: String { project.trimmingCharacters(in: .whitespacesAndNewlines) }
    private var place: String { country.trimmingCharacters(in: .whitespacesAndNewlines) }
    private var canSave: Bool { !title.isEmpty && !place.isEmpty && (title != savedProject || place != savedCountry) }
    /// The recent ones to start with; while typing, whatever matches.
    private var suggestions: [JSONValue] {
        guard !title.isEmpty, title != savedProject else { return model.recentProjects }
        return model.recentProjects.filter { $0["project"].string.localizedCaseInsensitiveContains(title) }
    }
    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            if !capture["preview_path"].string.isEmpty {
                AssetThumbnail(url: URL(fileURLWithPath: capture["path"].string).appendingPathComponent(capture["preview_path"].string), height: 116)
                    .frame(width: 190)
            }
            VStack(alignment: .leading, spacing: 7) {
                Text(URL(fileURLWithPath: capture["path"].string).lastPathComponent).fontWeight(.medium).lineLimit(1)
                Text(captureDetails).font(.caption).foregroundStyle(.secondary)
                if savedProject.isEmpty {
                    Text(capture["project_hint"]["name"].string.isEmpty ? "Sin proyecto; asígnalo antes de respaldar" : "Sugerencia del manifiesto; confirma el proyecto").font(.caption).foregroundStyle(.orange)
                } else {
                    Label("Proyecto guardado: \(savedProject)", systemImage: "checkmark.circle.fill").font(.caption).foregroundStyle(.secondary)
                }
                HStack(spacing: 7) {
                    TextField("Proyecto", text: $project).frame(maxWidth: 210).onSubmit(save)
                    TextField("País", text: $country).frame(width: 70).onSubmit(save)
                    Button("Guardar", action: save).disabled(!canSave)
                    Button(canSave ? "Guardar y preparar" : "Preparar") { model.tagCaptures([capture], project: title, country: place, prepare: true) }
                        .disabled(title.isEmpty || place.isEmpty)
                }
                // One click assigns: the picks are the same list the other rows and Ingreso see.
                RecentProjectChips(projects: suggestions, selection: Binding(get: { savedProject }, set: { value in
                    project = value
                    if value != savedProject { model.tagCaptures([capture], project: value, country: place.isEmpty ? model.country : place) }
                }), limit: 5)
            }
        }
        .onAppear {
            project = savedProject.isEmpty ? capture["project_hint"]["name"].string : savedProject
            let saved = savedCountry.isEmpty ? capture["project_hint"]["country"].string : savedCountry
            country = saved.isEmpty ? model.country : saved
        }
        // A bulk assignment or a rename elsewhere must show here, and the engine may normalize what was typed.
        .onChange(of: capture["tag"]) { _, tag in
            if !tag["name"].string.isEmpty { project = tag["name"].string; country = tag["country"].string }
        }
    }
    private func save() {
        guard canSave else { return }
        model.tagCaptures([capture], project: title, country: place)
    }
    private var captureDetails: String {
        let localDate = capture["created_at_local"].string
        var parts = [Timestamps.label(localDate.isEmpty ? capture["created_at"].string : localDate)]
        if let duration = durationLabel(capture["duration_seconds"].double) { parts.append(duration) }
        parts.append("\(Int(capture["track_count"].double)) pistas")
        return parts.joined(separator: " · ")
    }
}
