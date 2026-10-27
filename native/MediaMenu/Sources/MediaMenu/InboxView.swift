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
    @ObservedObject var model: Model
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                Label("Bandeja", systemImage: "tray.full").font(.title2)
                Text("Fuentes que puedes revisar y respaldar. Cada acción prepara un plan; nada se mueve automáticamente.").foregroundStyle(.secondary)
                ForEach(model.cardSuggestions, id: \.self) { path in
                    HStack { Image(systemName: "sdcard.fill").foregroundStyle(.orange); VStack(alignment: .leading) { Text("Tarjeta · " + URL(fileURLWithPath: path).lastPathComponent).fontWeight(.semibold); Text(path).font(.caption) }; Spacer(); Button("Revisar") { model.prepareSource(path, kind: "card") }; Button("Ingreso rápido") { model.prepareSource(path, kind: "card"); model.changeMode?(.compact) } }
                    Divider()
                }
                HStack {
                    Label("Stalker · \(model.pendingCaptures.count) pendientes", systemImage: "record.circle").font(.headline)
                    Spacer()
                }
                if !model.captureError.isEmpty { Text(model.captureError).font(.caption).foregroundStyle(.orange) }
                ForEach(model.captureRoots, id: \.self) { root in
                    let pendingCount = model.pendingCaptures.filter { $0["root"].string == root }.count
                    HStack {
                        VStack(alignment: .leading, spacing: 3) {
                            Text(URL(fileURLWithPath: root).lastPathComponent).font(.caption.weight(.medium))
                            Text(root).font(.caption2).foregroundStyle(.secondary).textSelection(.enabled)
                        }
                        Spacer()
                        Button("Preparar lote · \(pendingCount)") { model.prepareCaptureBatch(root) }.disabled(pendingCount == 0)
                    }
                }
                ForEach(model.pendingCaptures, id: \.["path"].string) { value in
                    CaptureInboxRow(model: model, capture: value)
                    Divider()
                }
                let archived = model.captureSessions.filter { $0["archived"].bool && $0["status"].string == "completed" }
                if !archived.isEmpty {
                    DisclosureGroup("\(archived.count) sesiones archivadas con original local") {
                        ForEach(archived, id: \.["path"].string) { value in
                            HStack { Text(URL(fileURLWithPath: value["path"].string).lastPathComponent).font(.caption); Spacer(); Button("Revisar liberar local…") { model.prepareLocalRelease(value) }.disabled(model.archive.isEmpty) }
                        }
                    }
                }
                let pending = model.assets.filter { $0["work_delivery_status"].string == "pending" }
                if !pending.isEmpty { HStack { Label("\(pending.count) copias para editar pendientes", systemImage: "externaldrive.badge.timemachine"); Spacer(); Button("Abrir biblioteca") { model.selectedTab = 1 } }; Divider() }
                let blocked = model.jobs.filter { ["failed", "paused", "camera_action_required"].contains($0["state"].string)
                    || $0["state"].string == "source_retained" && $0["release_stalker_source"].bool }
                if !blocked.isEmpty { HStack { Label("\(blocked.count) operaciones requieren atención", systemImage: "exclamationmark.triangle").foregroundStyle(.orange); Spacer(); Button("Abrir actividad") { model.selectedTab = 2 } }; Divider() }
                if model.cardSuggestions.isEmpty && model.pendingCaptures.isEmpty { ContentUnavailableView("Sin fuentes nuevas sugeridas", systemImage: "tray", description: Text("Conecta una SD o revisa las bibliotecas descubiertas de Stalker.")) }
                HStack { Button("Elegir otra fuente…") { model.selectedTab = 0; model.chooseSource() }; Button("Configurar capturas y discos") { model.selectedTab = 3 } }
                Text("Un lote prepara todas las sesiones pendientes de esa biblioteca en un mismo plan; asigna proyectos por sesión antes de respaldar. Si hay varias bibliotecas, cada una requiere su propio plan. Tras verificar el respaldo se libera el original salvo que Ajustes indique conservarlo; Clipper o cualquier bloqueo puede mantenerlo localmente.").font(.caption).foregroundStyle(.secondary)
            }.frame(maxWidth: .infinity, alignment: .leading).padding(.vertical, 8)
        }
    }
}


struct CaptureInboxRow: View {
    @ObservedObject var model: Model
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
