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
                    Spacer(); Button("Actualizar capturas") { model.captureChanged([]) }
                }
                if !model.captureError.isEmpty { Text(model.captureError).font(.caption).foregroundStyle(.orange) }
                ForEach(model.captureRoots, id: \.self) { root in
                    HStack { Text(root).font(.caption).textSelection(.enabled); Spacer(); Button("Asignar lote") { model.prepareSource(root, kind: "stalker"); model.changeMode?(.selection) } }
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
                let blocked = model.jobs.filter { ["failed", "paused", "camera_action_required"].contains($0["state"].string) }
                if !blocked.isEmpty { HStack { Label("\(blocked.count) operaciones requieren atención", systemImage: "exclamationmark.triangle").foregroundStyle(.orange); Spacer(); Button("Abrir actividad") { model.selectedTab = 2 } }; Divider() }
                if model.cardSuggestions.isEmpty && model.pendingCaptures.isEmpty { ContentUnavailableView("Sin fuentes nuevas sugeridas", systemImage: "tray", description: Text("Conecta una SD o revisa las bibliotecas descubiertas de Stalker.")) }
                HStack { Button("Elegir otra fuente…") { model.selectedTab = 0; model.chooseSource() }; Button("Configurar capturas y discos") { model.selectedTab = 3 } }
                Text("Los Stalker archivados ofrecen una revisión independiente para liberar el original local. La SD se limpia sólo después de verificar el plan. Cada biblioteca se procesa por separado.").font(.caption).foregroundStyle(.secondary)
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
        VStack(alignment: .leading, spacing: 7) {
            Text(URL(fileURLWithPath: capture["path"].string).lastPathComponent).fontWeight(.medium)
            Text(String(capture["created_at"].string.prefix(19)) + " · " + String(Int(capture["track_count"].double)) + " pistas").font(.caption).foregroundStyle(.secondary)
            if capture["tag"]["name"].string.isEmpty {
                Text(capture["project_hint"]["name"].string.isEmpty ? "Sin proyecto; asígnalo antes de respaldar" : "Sugerencia del manifiesto; confirma el proyecto").font(.caption).foregroundStyle(.orange)
            }
            HStack {
                TextField("Proyecto", text: $project)
                Menu("Recientes") { ForEach(Array(model.recentProjects.prefix(15)), id: \.["project_id"].string) { value in Button(value["project"].string) { project = value["project"].string } } }
                TextField("País", text: $country).frame(width: 70)
                Button("Guardar") { model.tagCapture(capture, project: project, country: country) }.disabled(project.isEmpty || country.isEmpty)
                Button("Guardar y preparar") { model.tagCapture(capture, project: project, country: country, prepare: true) }.disabled(project.isEmpty || country.isEmpty)
            }
        }
        .onAppear {
            project = capture["tag"]["name"].string.isEmpty ? capture["project_hint"]["name"].string : capture["tag"]["name"].string
            let saved = capture["tag"]["country"].string.isEmpty ? capture["project_hint"]["country"].string : capture["tag"]["country"].string
            country = saved.isEmpty ? model.country : saved
        }
    }
}
