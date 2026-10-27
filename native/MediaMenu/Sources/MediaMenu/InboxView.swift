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
                if !model.pendingCaptures.isEmpty {
                    HStack { Label("Stalker · \(model.pendingCaptures.count) sesiones terminadas", systemImage: "record.circle").font(.headline); Spacer(); Button("Asignar proyectos por sesión") { model.prepareSource(model.captureRoot, kind: "stalker") } }
                    ForEach(model.pendingCaptures, id: \.["path"].string) { value in
                        HStack { VStack(alignment: .leading) { Text(URL(fileURLWithPath: value["path"].string).lastPathComponent); Text("Terminada · fuente conservada · " + value["session_id"].string.prefix(8)).font(.caption).foregroundStyle(.secondary) }; Spacer(); Button("Preparar") { model.prepareSource(value["path"].string, kind: "stalker") } }
                        Divider()
                    }
                }
                let pending = model.assets.filter { $0["work_delivery_status"].string == "pending" }
                if !pending.isEmpty { HStack { Label("\(pending.count) copias para editar pendientes", systemImage: "externaldrive.badge.timemachine"); Spacer(); Button("Abrir biblioteca") { model.selectedTab = 1 } }; Divider() }
                let blocked = model.jobs.filter { ["failed", "paused", "camera_action_required"].contains($0["state"].string) }
                if !blocked.isEmpty { HStack { Label("\(blocked.count) operaciones requieren atención", systemImage: "exclamationmark.triangle").foregroundStyle(.orange); Spacer(); Button("Abrir actividad") { model.selectedTab = 2 } }; Divider() }
                if model.cardSuggestions.isEmpty && model.pendingCaptures.isEmpty { ContentUnavailableView("Sin fuentes nuevas sugeridas", systemImage: "tray", description: Text("Conecta una SD con destinos disponibles o configura la biblioteca de Stalker en Ajustes → Capturas.")) }
                HStack { Button("Elegir otra fuente…") { model.selectedTab = 0; model.chooseSource() }; Button("Configurar capturas y discos") { model.selectedTab = 3 } }
                Text("No se elimina Stalker desde esta bandeja. La limpieza de SD sólo ocurre después de ejecutar y verificar un plan. Los lotes de distintas fuentes se revisan por separado.").font(.caption).foregroundStyle(.secondary)
            }.frame(maxWidth: .infinity, alignment: .leading).padding(.vertical, 8)
        }
    }
}
