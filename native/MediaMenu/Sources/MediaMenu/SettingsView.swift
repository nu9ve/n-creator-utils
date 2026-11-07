import AppKit
import SwiftUI
import MediaMenuCore

struct SettingsView: View {
    @Bindable var model: Model
    @AppStorage("settingsSection") private var section = "profiles"
    @Environment(\.creatorPalette) private var palette
    @State private var id = ""
    @State private var name = ""
    @State private var raw = ""
    @State private var clipperProfile = ""
    @State private var work = ""
    @State private var archive = ""
    private let sections: [(String, LocalizedStringKey, String)] = [("profiles", "Perfiles", "person.crop.square"), ("disks", "Discos", "externaldrive"),
        ("ingest", "Ingreso", "sdcard"), ("capture", "Capturas", "record.circle"), ("appearance", "Apariencia", "paintpalette"),
        ("language", "Idioma", "globe"), ("runtime", "Diagnóstico", "stethoscope")]
    var body: some View {
        HStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 2) {
                ForEach(sections, id: \.0) { key, title, icon in
                    Button { section = key } label: {
                        Label(title, systemImage: icon).frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal, 10).padding(.vertical, 6)
                            .background(section == key ? palette.accentSoft : .clear, in: palette.control)
                            .contentShape(Rectangle())
                    }.buttonStyle(.plain)
                }
                Spacer()
            }.frame(width: 170).padding(10)
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    ErrorBanner(model: model)
                    switch section {
                    case "profiles": profiles
                    case "disks": disks
                    case "ingest": IngestSettings(model: model)
                    case "appearance": AppearanceSettings()
                    case "capture": captures
                    case "language": LanguageSettings()
                    default: runtime
                    }
                }.frame(maxWidth: .infinity, alignment: .leading).padding(20)
            }
        }
        .onAppear {
            guard id.isEmpty else { return }
            // Saving must not blank what the current profile already has.
            if let saved = model.profiles.first(where: { $0["id"].string == model.profile }) { load(saved) }
            else { id = model.profile; name = model.profile; raw = model.profile + "_raw"; work = model.work; archive = model.archive }
        }
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
            HStack { Text("Configurar perfil").font(.headline); Spacer(); Button("Nuevo") { id = ""; name = ""; raw = ""; clipperProfile = ""; work = model.work; archive = model.archive } }
            HStack { TextField("ID: 9logs", text: $id); TextField("Nombre: 9logs", text: $name) }
            TextField("Carpeta de raws: 9logs_raw", text: $raw)
            Text("Ruta relativa dentro del espacio de trabajo del SSD. Se añade /PROYECTO. Cambiarla no mueve copias anteriores ni cambia planes ya revisados.").font(.caption).foregroundStyle(.secondary)
            TextField("Perfil en Clipper: erikiado64", text: $clipperProfile)
            Text("Cómo se llama este perfil en Clipper. Vacío: sin indicar, y Clipper decide por su cuenta. No cambia rutas ni copias anteriores.").font(.caption).foregroundStyle(.secondary)
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
            Button("Respaldar catálogo y perfiles") { model.backupMetadata() }.disabled(model.archive.isEmpty || model.busy)
            Text("Incluye proyectos, etiquetas, perfiles, decisiones y registro de operaciones. También se guarda después del ingreso y antes/después de liberar.").font(.caption).foregroundStyle(.secondary)
        }
    }
    var captures: some View {
        VStack(alignment: .leading, spacing: 14) {
            Label("Biblioteca de Stalker", systemImage: "record.circle").font(.title3.weight(.semibold))
            TextField("Carpeta de capturas Stalker", text: $model.captureRoot)
            Button("Elegir biblioteca…") {
                pickURL(message: String(localized: "Elige la biblioteca de Stalker")) { url in model.captureRoot = url.path; model.watchCaptures() }
            }
            Button("Guardar y redescubrir bibliotecas") { model.watchCaptures() }
            Text("Vacío: biblioteca publicada por Stalker. Una carpeta manual se añade a la actual y a las anteriores.").font(.caption)
            ForEach(model.captureRoots, id: \.self) { Text($0).font(.caption).textSelection(.enabled) }
            if !model.captureError.isEmpty { Text(model.captureError).font(.caption).foregroundStyle(.orange) }
            Divider()
            TextField("Ejecutable de Clipper (para dependencias y reenlace)", text: $model.clipperExecutable)
            Button("Elegir ejecutable Clipper…") {
                pickURL(directories: false, hidden: true) { url in model.clipperExecutable = url.path; model.watchCaptures() }
            }
            Text("Sin Clipper disponible, proyectos desconectados o exportaciones en cola, liberar espacio queda bloqueado. Guardar usa el botón de arriba.").font(.caption).foregroundStyle(.secondary)
            Toggle("Conservar el original Stalker después del respaldo", isOn: $model.retainStalkerSource)
                .onChange(of: model.retainStalkerSource) { _, _ in model.watchCaptures() }
            Text("Desactivado: tras verificar MEDIA/ESSD, se revisa Clipper y se libera el original en el mismo plan. Si hay dependencias desconocidas, colas o fallos, se conserva para liberar después desde Espacio. Las pistas y el bundle completo quedan en el respaldo.").font(.caption).foregroundStyle(.secondary)
            Text("La bandeja sólo ofrece sesiones terminadas. Los originales, micrófono, eventos y pistas se conservan juntos dentro del bundle archivado.").font(.caption).foregroundStyle(.secondary)
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
    func load(_ value: JSONValue) { id = value["id"].string; name = value["name"].string; raw = value["raw_dir"].string; work = value["work"].string; archive = value["archive"].string; clipperProfile = value["clipper_profile"].string }
    func save() {
        let params: [String: JSONValue] = ["id": .string(id), "name": .string(name), "raw_dir": .string(raw), "work": .string(work), "archive": .string(archive),
                                             "clipper_profile": clipperProfile.trimmingCharacters(in: .whitespaces).isEmpty ? .null : .string(clipperProfile.trimmingCharacters(in: .whitespaces))]
        model.operation(String(localized: "Guardando perfil"), scope: .settings) { _ = try await model.client!.call("save_profile", params); model.message = String(localized: "Perfil guardado; los planes anteriores no cambian") }
    }
}

struct IngestSettings: View {
    @Bindable var model: Model
    @AppStorage("autoInspect") private var autoInspect = false
    @AppStorage("showQuickImporter") private var showQuickImporter = true
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Label("Ingreso", systemImage: "sdcard").font(.title3.weight(.semibold))
            Text("Entrada → Flujo → Resultado. El botón final autoriza el plan mostrado, incluida la limpieza verificada que hayas elegido. Las excepciones se resuelven en la misma pantalla.").font(.caption).foregroundStyle(.secondary)
            Toggle("Inspeccionar automáticamente la SD al abrir Ingreso", isOn: $autoInspect)
            Text("Desactivado: elegir una fuente no recorre sus archivos hasta que abres Contenido o revisas el plan.").font(.caption).foregroundStyle(.secondary)
            Divider()
            Toggle("Mostrar el importador rápido al conectar una tarjeta", isOn: $showQuickImporter)
            Text("Sólo con un disco de respaldo o de trabajo conectado. No inicia copias ni limpia automáticamente.").font(.caption).foregroundStyle(.secondary)
            Button("Probar vista del importador…") { model.openImporter?() }
        }
    }
}

struct LanguageSettings: View {
    @State private var language = (UserDefaults.standard.array(forKey: "AppleLanguages") as? [String])?.first.flatMap { ["es", "en"].contains(String($0.prefix(2))) ? String($0.prefix(2)) : nil } ?? ""
    @State private var changed = false
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Label("Idioma", systemImage: "globe").font(.title3.weight(.semibold))
            Picker("Idioma de la app", selection: $language) {
                Text("Igual que el sistema").tag("")
                Text("Español").tag("es")
                Text("English").tag("en")
            }.pickerStyle(.radioGroup)
            .onChange(of: language) { _, value in
                if value.isEmpty { UserDefaults.standard.removeObject(forKey: "AppleLanguages") }
                else { UserDefaults.standard.set([value], forKey: "AppleLanguages") }
                changed = true
            }
            Text("El cambio se aplica al reiniciar nüün. Los mensajes técnicos del motor pueden aparecer en inglés.").font(.caption).foregroundStyle(.secondary)
            if changed {
                Button("Reiniciar nüün ahora") {
                    let configuration = NSWorkspace.OpenConfiguration(); configuration.createsNewApplicationInstance = true
                    NSWorkspace.shared.openApplication(at: Bundle.main.bundleURL, configuration: configuration) { _, _ in
                        Task { @MainActor in NSApp.terminate(nil) }
                    }
                }
            }
        }
    }
}
