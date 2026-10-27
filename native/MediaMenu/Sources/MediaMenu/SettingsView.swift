import AppKit
import SwiftUI
import MediaMenuCore

struct SettingsView: View {
    @ObservedObject var model: Model
    @State private var section = "profiles"
    @State private var id = ""
    @State private var name = ""
    @State private var raw = ""
    @State private var work = ""
    @State private var archive = ""
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                ForEach([("profiles","Perfiles","person.crop.square"),("disks","Discos","externaldrive"),("appearance","Apariencia","paintpalette"),("capture","Capturas","record.circle"),("runtime","Diagnóstico","stethoscope")], id: \.0) { key,title,icon in
                    Button { section = key } label: { Label(title, systemImage: icon) }.buttonStyle(CreatorButtonStyle(prominent: section == key))
                }
            }
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    switch section {
                    case "profiles": profiles
                    case "disks": disks
                    case "appearance": AppearanceSettings()
                    case "capture": captures
                    default: runtime
                    }
                }.frame(maxWidth: .infinity, alignment: .leading).padding(3)
            }
        }.disabled(model.busy)
        .onAppear { if id.isEmpty { id = model.profile; name = model.profile; raw = model.profile + "_raw"; work = model.work; archive = model.archive } }
    }
    var profiles: some View {
        VStack(alignment: .leading, spacing: 12) {
            Label("Perfiles y espacios de trabajo", systemImage: "person.crop.square").font(.title3.weight(.semibold))
            Text("Un perfil es un canal o contexto creativo (e64, 9logs…). El proyecto y país/ciudad siguen siendo independientes.").foregroundStyle(.secondary)
            ForEach(model.profiles, id: \.["id"].string) { value in
                HStack { Image(systemName: "folder"); Text(value["name"].string).fontWeight(.medium); Text(value["id"].string).foregroundStyle(.secondary); Spacer(); Text(value["raw_dir"].string).font(.caption); Button("Editar") { load(value) }; Button("Usar") { model.selectProfile(value["id"].string) } }
                Divider()
            }
            if model.profiles.isEmpty { Text("Aún no hay perfiles guardados. e64 usa e64_raw hasta que lo configures.").font(.caption) }
            HStack { Text("Configurar perfil").font(.headline); Spacer(); Button("Nuevo") { id = ""; name = ""; raw = ""; work = model.work; archive = model.archive } }
            HStack { TextField("ID: 9logs", text: $id); TextField("Nombre: 9logs", text: $name) }
            TextField("Carpeta de raws: 9logs_raw", text: $raw)
            Text("Ruta relativa dentro del espacio de trabajo del SSD. Se añade /PROYECTO. Cambiarla no mueve copias anteriores ni cambia planes ya revisados.").font(.caption).foregroundStyle(.secondary)
            volume("SSD sugerido", role: "work", value: $work)
            volume("Respaldo sugerido", role: "archive", value: $archive)
            Button("Guardar perfil") { save() }.disabled(id.isEmpty || name.isEmpty || raw.isEmpty)
            Text("Ejemplo: ESSD / 001_proyectos / 9logs_raw / PROYECTO. Los discos sugeridos se aplican al elegir el perfil; el plan siempre muestra los destinos reales.").font(.caption)
        }
    }
    var disks: some View {
        VStack(alignment: .leading, spacing: 12) {
            Label("Discos registrados", systemImage: "externaldrive").font(.title3.weight(.semibold))
            HStack { Button("Añadir disco de respaldo…") { model.registerVolume(role: "archive") }; Button("Añadir SSD para editar…") { model.registerVolume(role: "work") }; Button("Actualizar estado") { Task { await model.refresh() } } }
            ForEach(model.volumes, id: \.["id"].string) { value in
                HStack(alignment: .top) {
                    Image(systemName: value["online"].bool ? "externaldrive.fill.badge.checkmark" : "externaldrive.badge.xmark").foregroundStyle(value["online"].bool ? .green : .secondary)
                    VStack(alignment: .leading) { Text(value["name"].string).fontWeight(.semibold); Text(value["root"].string + "/" + value["prefix"].string).font(.caption).textSelection(.enabled) }
                    Spacer(); Text(value["role"].string == "archive" ? "Respaldo" : "Edición"); Text(value["online"].bool ? "Conectado" : "Ausente").foregroundStyle(.secondary)
                }.padding(.vertical, 6)
                Divider()
            }
        }
    }
    var captures: some View {
        VStack(alignment: .leading, spacing: 14) {
            Label("Biblioteca de Stalker", systemImage: "record.circle").font(.title3.weight(.semibold))
            TextField("Carpeta de capturas Stalker", text: $model.captureRoot)
            Button("Elegir biblioteca…") {
                let panel = NSOpenPanel(); panel.canChooseFiles = false; panel.canChooseDirectories = true
                if panel.runModal() == .OK { model.captureRoot = panel.url!.path; model.watchCaptures() }
            }
            Button("Guardar y redescubrir bibliotecas") { model.watchCaptures() }
            Text("Vacío: biblioteca publicada por Stalker. Una carpeta manual se añade a la actual y a las anteriores.").font(.caption)
            ForEach(model.captureRoots, id: \.self) { Text($0).font(.caption).textSelection(.enabled) }
            if !model.captureError.isEmpty { Text(model.captureError).font(.caption).foregroundStyle(.orange) }
            Divider()
            TextField("Ejecutable de Clipper (para dependencias y reenlace)", text: $model.clipperExecutable)
            Button("Elegir ejecutable Clipper…") {
                let panel = NSOpenPanel(); panel.canChooseFiles = true; panel.canChooseDirectories = false; panel.showsHiddenFiles = true
                if panel.runModal() == .OK { model.clipperExecutable = panel.url!.path; model.watchCaptures() }
            }
            Text("Sin Clipper disponible, proyectos desconectados o exportaciones en cola, liberar espacio queda bloqueado. Guardar usa el botón de arriba.").font(.caption).foregroundStyle(.secondary)
            Text("La bandeja sólo ofrece sesiones terminadas. Los originales, micrófono, eventos y pistas se conservan juntos; un respaldo de Stalker conserva la fuente.").font(.caption).foregroundStyle(.secondary)
            Divider(); QuickImporterSettings(model: model)
        }
    }
    var runtime: some View {
        VStack(alignment: .leading, spacing: 12) {
            Label("Diagnóstico", systemImage: "stethoscope").font(.title3.weight(.semibold))
            Text("Build: " + (Bundle.main.object(forInfoDictionaryKey: "Nu9veBuildID") as? String ?? "desde código")).textSelection(.enabled)
            TextField("Python", text: $model.python).disabled(model.packagedBackend || model.bundledRuntime)
            TextField("Motor", text: $model.script).disabled(model.packagedBackend || model.bundledRuntime)
            Text("Una ventana · tres modos · miniaturas bajo demanda · sin sondeo periódico en reposo.").font(.caption)
            if !model.packagedBackend && !model.bundledRuntime { Button("Aplicar runtime") { model.configure() } }
            Text("Instalación local de desarrollo; el runtime externo y la firma de distribución siguen siendo gates de release.").font(.caption).foregroundStyle(.secondary)
        }
    }
    func volume(_ title: String, role: String, value: Binding<String>) -> some View {
        Picker(title, selection: value) { Text("Sin sugerencia").tag(""); ForEach(model.volumes.filter { $0["role"].string == role }, id: \.["id"].string) { item in Text(item["name"].string).tag(item["id"].string) } }
    }
    func load(_ value: JSONValue) { id = value["id"].string; name = value["name"].string; raw = value["raw_dir"].string; work = value["work"].string; archive = value["archive"].string }
    func save() {
        let params: [String: JSONValue] = ["id": .string(id), "name": .string(name), "raw_dir": .string(raw), "work": .string(work), "archive": .string(archive)]
        model.operation("Guardando perfil") { _ = try await model.client!.call("save_profile", params); model.message = "Perfil guardado; los planes anteriores no cambian" }
    }
}
