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
    @Environment(\.creatorPalette) private var palette
    private var blocked: [JSONValue] {
        model.jobs.filter { ["failed", "paused", "camera_action_required"].contains($0["state"].string)
            || $0["state"].string == "source_retained" && $0["release_stalker_source"].bool }
    }
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
                if !blocked.isEmpty || pendingWork > 0 {
                    SectionTitle("Requiere atención")
                    VStack(alignment: .leading, spacing: 8) {
                        if !blocked.isEmpty {
                            HStack { Label("\(blocked.count) operaciones requieren atención", systemImage: "exclamationmark.triangle").foregroundStyle(.orange); Spacer(); Button("Abrir actividad") { model.open(.activity) } }
                        }
                        if pendingWork > 0 {
                            HStack { Label("\(pendingWork) copias para editar pendientes", systemImage: "externaldrive.badge.timemachine"); Spacer(); Button("Abrir biblioteca") { model.open(.files) } }
                        }
                    }.modifier(Panel())
                }
                HStack {
                    SectionTitle("Stalker")
                    Text("\(model.pendingCaptures.count) pendientes").foregroundStyle(.secondary)
                    Spacer()
                    Button("Actualizar", systemImage: "arrow.clockwise") { model.captureChanged([]) }.controlSize(.small)
                }
                if !model.captureError.isEmpty { Text(model.captureError).font(.caption).foregroundStyle(.orange) }
                ForEach(model.captureRoots, id: \.self) { root in
                    let pendingCount = model.pendingCaptures.filter { $0["root"].string == root }.count
                    HStack {
                        VStack(alignment: .leading, spacing: 3) {
                            Text(URL(fileURLWithPath: root).lastPathComponent).font(.callout.weight(.medium))
                            Text(root).font(.caption2).foregroundStyle(.secondary).textSelection(.enabled)
                        }
                        Spacer()
                        Button("Preparar lote · \(pendingCount)") { model.prepareCaptureBatch(root) }
                            .buttonStyle(CreatorButtonStyle(prominent: pendingCount > 0)).disabled(pendingCount == 0)
                    }.modifier(Panel())
                }
                ForEach(model.pendingCaptures, id: \.["path"].string) { value in
                    CaptureInboxRow(model: model, capture: value)
                    Divider()
                }
                let archived = model.captureSessions.filter { $0["archived"].bool && $0["status"].string == "completed" }
                if !archived.isEmpty {
                    DisclosureGroup("\(archived.count) sesiones archivadas con original local") {
                        ForEach(archived, id: \.["path"].string) { value in
                            HStack { Text(URL(fileURLWithPath: value["path"].string).lastPathComponent).font(.caption); Spacer(); Button("Revisar liberar local…") { model.prepareLocalRelease(value) }.disabled(model.archive.isEmpty || model.busy) }
                        }
                    }
                }
                if model.sourceCards.isEmpty && model.pendingCaptures.isEmpty && blocked.isEmpty {
                    ContentUnavailableView("Todo al día", systemImage: "checkmark.circle", description: Text("Conecta una SD o termina una sesión de Stalker para verla aquí."))
                        .frame(maxWidth: .infinity)
                }
            }.frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 20).padding(.vertical, 14)
        }
    }
}

struct Panel: ViewModifier {
    @Environment(\.creatorPalette) private var palette
    func body(content: Content) -> some View {
        content.padding(12).frame(maxWidth: .infinity, alignment: .leading)
            .background(palette.card, in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).stroke(palette.border.opacity(0.5)))
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
    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            if !capture["preview_path"].string.isEmpty {
                AssetThumbnail(url: URL(fileURLWithPath: capture["path"].string).appendingPathComponent(capture["preview_path"].string), height: 116)
                    .frame(width: 190)
            }
            VStack(alignment: .leading, spacing: 7) {
                Text(URL(fileURLWithPath: capture["path"].string).lastPathComponent).fontWeight(.medium).lineLimit(1)
                Text(captureDetails).font(.caption).foregroundStyle(.secondary)
                if capture["tag"]["name"].string.isEmpty {
                    Text(capture["project_hint"]["name"].string.isEmpty ? "Sin proyecto; asígnalo antes de respaldar" : "Sugerencia del manifiesto; confirma el proyecto").font(.caption).foregroundStyle(.orange)
                }
                HStack(spacing: 7) {
                    TextField("Proyecto", text: $project).frame(maxWidth: 210)
                    TextField("País", text: $country).frame(width: 70)
                    Button("Guardar") { model.tagCapture(capture, project: project, country: country) }.disabled(project.isEmpty || country.isEmpty)
                    Button("Guardar y preparar") { model.tagCapture(capture, project: project, country: country, prepare: true) }.disabled(project.isEmpty || country.isEmpty)
                }
                RecentProjectChips(projects: model.recentProjects, selection: $project)
            }
        }
        .onAppear {
            project = capture["tag"]["name"].string.isEmpty ? capture["project_hint"]["name"].string : capture["tag"]["name"].string
            let saved = capture["tag"]["country"].string.isEmpty ? capture["project_hint"]["country"].string : capture["tag"]["country"].string
            country = saved.isEmpty ? model.country : saved
        }
    }
    private var captureDetails: String {
        let localDate = capture["created_at_local"].string
        var parts = [localDate.isEmpty ? String(capture["created_at"].string.prefix(19)) : localDate]
        if let duration = durationLabel(capture["duration_seconds"].double) { parts.append(duration) }
        parts.append("\(Int(capture["track_count"].double)) pistas")
        return parts.joined(separator: " · ")
    }
}
