import AppKit
import SwiftUI
import NuunCore

// Acumulados en disco (docs/features/storage-map.md#acumulados) y perfiles de la suite en esta Mac
// (nu9ve-profile-v1). Fase 1: sólo lectura. Nada aquí borra ni mueve archivos; el plan sólo dice qué, cuánto,
// cómo se regenera y el riesgo. La ejecución (a la Papelera, con recibo) es la fase 2.

// MARK: Model

extension Model {
    func loadAccumulations() async {
        do { accumulations = try await oneShot("storage_accumulations", [:]) }
        catch { self.error = error.localizedDescription }
    }
    func scanAccumulations() {
        guard !accumulationsRunning else { return }
        accumulationsRunning = true
        Task {
            defer { accumulationsRunning = false }
            do {
                accumulations = try await oneShot("storage_accumulations_scan", [:])
                message = String(localized: "Acumulados actualizados; no se movió ni borró nada")
            } catch { self.error = error.localizedDescription }
        }
    }
    func accumulationRoots(_ roots: [String]?) {
        Task {
            do {
                _ = try await oneShot("storage_accumulations_settings", ["roots": roots.map { .array($0.map(JSONValue.string)) } ?? .null])
                await loadAccumulations()
            } catch { self.error = error.localizedDescription }
        }
    }
    func accumulationThreshold(_ bytes: Double) {
        Task {
            do {
                _ = try await oneShot("storage_accumulations_settings", ["internal_min_free_bytes": .integer(Int64(bytes))])
                await loadAccumulations()
            } catch { self.error = error.localizedDescription }
        }
    }
    func loadSuiteProfiles() async {
        suiteProfiles = try? await oneShot("suite_profiles", [:])
    }
    /// Adds the `nuun` entry to nüün's profiles in profiles.json; the person confirmed it in the dialog.
    func registerSuiteProfiles() {
        operation(String(localized: "Registrando perfiles en la suite"), scope: .settings) {
            self.suiteProfiles = try await self.oneShot("suite_profiles_register", ["confirm": .bool(true)])
            self.message = String(localized: "Perfiles de nüün registrados en esta Mac; las entradas de otras apps no cambiaron")
        }
    }
}

private func ago(_ text: String) -> String {
    guard let date = ISO8601DateFormatter().date(from: text) else { return "—" }
    return RelativeDateTimeFormatter().localizedString(for: date, relativeTo: Date())
}

private func regenTitle(_ value: String) -> String {
    switch value {
    case "yes": String(localized: "Se regenera")
    case "depends": String(localized: "Depende")
    case "partial": String(localized: "Parcial")
    default: String(localized: "No se regenera")
    }
}

// MARK: Acumulados

struct AccumulationsSheet: View {
    @Environment(\.creatorPalette) private var palette
    @Environment(\.closeSheet) private var dismiss
    @Bindable var model: Model
    @State private var group = ""
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            PageHeader(title: "Acumulados", subtitle: "Builds, dependencias, cachés y respaldos que crecen solos. Esta vista sólo lee: no borra ni mueve nada.")
            ErrorBanner(model: model)
            let data = model.accumulations
            if let data {
                volumes(data["volumes"].array)
                if data["state"].string == "none" {
                    ContentUnavailableView("Sin escanear", systemImage: "shippingbox",
                                           description: Text("Se leen sólo nombres, tamaños y fechas de tus carpetas de código y de las cachés conocidas. Sólo se abre el .git/config de cada repo, para su remoto."))
                } else {
                    summary(data)
                    HStack(alignment: .top, spacing: 12) {
                        plan(data["plan"]["groups"].array).frame(minWidth: 280, maxWidth: 360)
                        findings(data["findings"].array.filter { group.isEmpty || $0["rule"].string == group })
                    }
                    libraries(data["libraries"].array)
                }
            } else {
                ProgressView().frame(maxWidth: .infinity)
            }
            HStack {
                if model.accumulationsRunning { ProgressView().controlSize(.small); Text("Escaneando…").font(.caption) }
                Menu("Carpetas") {
                    Button("Elegir carpeta de código…") {
                        pickURL(message: String(localized: "Elige una carpeta donde guardas código")) { url in
                            let current = model.accumulations?["settings"]["roots"].array.map(\.string) ?? []
                            model.accumulationRoots(Array(Set(current + [url.path])).sorted())
                        }
                    }
                    Button("Volver a las de siempre (~/code…)") { model.accumulationRoots(nil) }
                }.fixedSize()
                Spacer()
                Button("Cerrar") { dismiss() }.keyboardShortcut(.cancelAction)
                Button("Escanear", systemImage: "arrow.clockwise") { model.scanAccumulations() }
                    .buttonStyle(CreatorButtonStyle(prominent: true)).disabled(model.accumulationsRunning)
            }
        }
        .padding(20)
        .frame(minWidth: 720, idealWidth: 920, minHeight: 520, idealHeight: 680)
        .task { await model.loadAccumulations() }
    }
    private func volumes(_ list: [JSONValue]) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(list, id: \.["mount"].string) { volume in
                HStack {
                    Image(systemName: volume["internal"].bool ? "internaldrive" : "externaldrive")
                    Text(volume["label"].string).fontWeight(.medium)
                    Text(String(localized: "\(bytesLabel(volume["free_bytes"].double)) libres de \(bytesLabel(volume["total_bytes"].double))")).font(.caption).monospacedDigit()
                    if volume["accumulated"]["bytes"].double > 0 {
                        Text(String(localized: "· \(bytesLabel(volume["accumulated"]["bytes"].double)) acumulados")).font(.caption).foregroundStyle(.secondary)
                    }
                    Spacer()
                    if volume["almost_full"].bool {
                        Label("Casi lleno", systemImage: "exclamationmark.triangle.fill").foregroundStyle(palette.warning).font(.caption)
                            .help(String(localized: "Quedan menos de \(bytesLabel(volume["min_free_bytes"].double)) libres"))
                    }
                }
            }
            if let internalVolume = list.first(where: { $0["internal"].bool }) {
                HStack {
                    Text("Avisar en esta Mac con menos de").font(.caption).foregroundStyle(.secondary)
                    Picker("", selection: Binding(get: { internalVolume["min_free_bytes"].double },
                                                  set: { model.accumulationThreshold($0) })) {
                        ForEach([5.0, 10, 20, 50, 100], id: \.self) { gib in Text("\(Int(gib)) GiB").tag(gib * 1_073_741_824) }
                    }.labelsHidden().fixedSize()
                }
            }
        }.modifier(Panel())
    }
    private func summary(_ data: JSONValue) -> some View {
        let history = data["history"].array
        let previous = history.dropLast().last?["summary"]["bytes"].double
        let now = data["summary"]["bytes"].double
        return HStack {
            Text(String(localized: "\(bytesLabel(now)) en \(Int(data["summary"]["findings"].double)) hallazgos")).font(.headline)
            if let previous { Text(String(localized: "(\(now >= previous ? "+" : "−")\(bytesLabel(abs(now - previous))) desde el escaneo anterior)")).font(.caption).foregroundStyle(.secondary) }
            Spacer()
            Text(String(localized: "Escaneado \(ago(data["scanned_at"].string))")).font(.caption).foregroundStyle(.secondary)
            if data["truncated"].bool { Label("Incompleto", systemImage: "exclamationmark.circle").font(.caption).help("Se alcanzó el tope de entradas; elige carpetas más pequeñas.") }
        }
    }
    private func plan(_ groups: [JSONValue]) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Plan por tipo").font(.headline)
            Text("Nada se borra desde aquí. Por omisión irá a la Papelera; lo que no se regenera pedirá dos confirmaciones.").font(.caption).foregroundStyle(.secondary)
            ScrollView {
                VStack(alignment: .leading, spacing: 6) {
                    Button { group = "" } label: { Text("Todos").fontWeight(group.isEmpty ? .semibold : .regular) }.buttonStyle(.plain)
                    ForEach(groups, id: \.["rule"].string) { g in
                        Button { group = g["rule"].string } label: {
                            VStack(alignment: .leading, spacing: 2) {
                                HStack {
                                    Text(g["label"].string).fontWeight(group == g["rule"].string ? .semibold : .regular)
                                    Spacer()
                                    Text(bytesLabel(g["bytes"].double)).monospacedDigit()
                                }
                                HStack(spacing: 6) {
                                    Text(regenTitle(g["regenerates"].string))
                                    Text("· " + g["cost_note"].string).lineLimit(2)
                                }.font(.caption).foregroundStyle(.secondary)
                                if g["action"].string == "manual" {
                                    Label("Se libera desde su herramienta, no a la Papelera", systemImage: "wrench.and.screwdriver").font(.caption2).foregroundStyle(.secondary)
                                } else if g["confirmations"].double >= 2 {
                                    Label("Pide dos confirmaciones", systemImage: "exclamationmark.shield").font(.caption2).foregroundStyle(palette.warning)
                                }
                            }.contentShape(Rectangle())
                        }.buttonStyle(.plain)
                        Divider()
                    }
                }
            }
        }.modifier(Panel())
    }
    private func findings(_ rows: [JSONValue]) -> some View {
        Table(rows.prefix(500).map(AccumulationRow.init)) {
            TableColumn("Ruta") { row in
                Text(row.path).lineLimit(1).truncationMode(.head).help(row.path)
                    .contextMenu { Button("Mostrar en Finder") { NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: row.path)]) } }
            }
            TableColumn("Tipo") { Text($0.label).lineLimit(1) }.width(min: 80, ideal: 110)
            TableColumn("Tamaño") { Text(bytesLabel($0.bytes)).monospacedDigit() }.width(min: 60, ideal: 76)
            TableColumn("Último uso") { Text(ago($0.lastUsed)) }.width(min: 70, ideal: 100)
            TableColumn("Actividad") { row in
                Text(row.activity.isEmpty ? "—" : ago(row.activity)).foregroundStyle(.secondary)
                    .help("Último commit o checkout del repo, o la última conversación de agentes del proyecto")
            }.width(min: 70, ideal: 100)
            TableColumn("Dueño") { row in
                HStack(spacing: 4) {
                    ProjectIcon(path: row.icon)
                    Text(row.owner).lineLimit(1).foregroundStyle(.secondary)
                }.help(row.remote.isEmpty ? row.owner : row.owner + " · " + row.remote)
            }.width(min: 60, ideal: 120)
            TableColumn("Remoto") { row in
                Text(row.remote.isEmpty ? "—" : row.remote).lineLimit(1).truncationMode(.middle).foregroundStyle(.secondary).textSelection(.enabled)
            }.width(min: 70, ideal: 150)
        }.modifier(CreatorListSurface())
    }
    private func libraries(_ list: [JSONValue]) -> some View {
        DisclosureGroup("Bibliotecas de los perfiles (datos vivos, nunca acumulados)") {
            ForEach(list, id: \.["profile_id"].string) { profile in
                VStack(alignment: .leading, spacing: 2) {
                    Text(profile["display_name"].string + " · " + profile["profile_id"].string).fontWeight(.medium)
                    ForEach(profile["apps"].array, id: \.["app"].string) { app in
                        Text(app["app"].string + ": " + app["library"].string).font(.caption).textSelection(.enabled)
                            .foregroundStyle(app["present"].bool ? .primary : .secondary)
                    }
                }
            }
            if list.isEmpty { Text("Sin perfiles de la suite en esta Mac.").font(.caption).foregroundStyle(.secondary) }
        }.font(.callout)
    }
}

/// A project's own icon (favicon or app icon) at 14 pt, or nothing.
struct ProjectIcon: View {
    let path: String
    var body: some View {
        if !path.isEmpty, let image = NSImage(contentsOfFile: path) {
            Image(nsImage: image).resizable().interpolation(.high).frame(width: 14, height: 14).clipShape(RoundedRectangle(cornerRadius: 3))
        }
    }
}

struct AccumulationRow: Identifiable {
    let id: String, path: String, label: String, bytes: Double, lastUsed: String, activity: String, owner: String
    /// U2: the repository's remote (without credentials) and the project's own icon, when there are.
    let remote: String, icon: String
    init(_ value: JSONValue) {
        remote = value["repo_remote"].string; icon = value["project_icon"].string
        id = value["id"].string; path = value["path"].string; label = value["label"].string
        bytes = value["bytes_on_disk"].double; lastUsed = value["last_used_at"].string
        activity = [value["repo_activity_at"].string, value["agent_activity_at"].string].max() ?? ""
        let repo = value["repo"].string
        owner = !value["owner_app"].string.isEmpty ? value["owner_app"].string
            : repo.isEmpty ? "" : URL(fileURLWithPath: repo).lastPathComponent
    }
}

// MARK: Perfiles en esta Mac

struct SuiteProfilesSection: View {
    @Environment(\.creatorPalette) private var palette
    @Bindable var model: Model
    @State private var confirming = false
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("Perfiles en esta Mac", systemImage: "person.2.crop.square.stack").font(.headline)
            Text("Los perfiles que la suite conoce en esta computadora y en qué apps tienen biblioteca. Lo que no es de nüün sólo se muestra.")
                .font(.caption).foregroundStyle(.secondary)
            let data = model.suiteProfiles
            switch data?["file"]["state"].string ?? "" {
            case "invalid":
                Label("El archivo de perfiles no es válido (\(data?["file"]["problem"].string ?? "")); nüün no lo modifica.", systemImage: "exclamationmark.triangle")
                    .foregroundStyle(palette.warning).font(.caption)
            case "missing":
                Text("Aún no hay archivo de perfiles en esta Mac.").font(.caption)
            case "unavailable":
                Text("Sin archivo de perfiles en esta raíz de pruebas.").font(.caption)
            default: EmptyView()
            }
            ForEach(data?["profiles"].array ?? [], id: \.["profile_id"].string) { profile in
                VStack(alignment: .leading, spacing: 3) {
                    HStack {
                        Text(profile["display_name"].string).fontWeight(.medium)
                        Text(profile["profile_id"].string).foregroundStyle(.secondary)
                        if !profile["aliases"].array.isEmpty { Text("(" + profile["aliases"].array.map(\.string).joined(separator: ", ") + ")").font(.caption).foregroundStyle(.secondary) }
                        Spacer()
                        Text(kind(profile["kind"].string)).font(.caption).foregroundStyle(.secondary)
                    }
                    ForEach(profile["apps"].array, id: \.["app"].string) { app in
                        HStack(spacing: 6) {
                            Image(systemName: app["own"].bool ? "checkmark.circle.fill" : "lock").foregroundStyle(app["own"].bool ? palette.success : palette.muted)
                            Text(app["app"].string).frame(width: 50, alignment: .leading)
                            Text(app["library"].string).lineLimit(1).truncationMode(.middle).textSelection(.enabled)
                                .foregroundStyle(app["library_present"].bool ? .primary : .secondary)
                            if app["default"].bool { Text("predeterminado").font(.caption2).foregroundStyle(.secondary) }
                        }.font(.caption)
                    }
                    if profile["apps"].array.isEmpty { Text("Sin biblioteca en ninguna app").font(.caption).foregroundStyle(.secondary) }
                }
                Divider()
            }
            let pending = (data?["nuun"].array ?? []).filter { !$0["registered"].bool && $0["suite_id_ok"].bool }
            if !pending.isEmpty, data?["file"]["state"].string != "invalid", data?["file"]["state"].string != "unavailable" {
                HStack {
                    Text("Sin registrar: " + pending.map(\.["id"].string).joined(separator: ", ")).font(.caption)
                    Spacer()
                    Button("Registrar en esta Mac…") { confirming = true }.disabled(model.busy)
                }
            }
        }
        .task { await model.loadSuiteProfiles() }
        .confirmationDialog("¿Agregar los perfiles de nüün al archivo de perfiles de esta Mac?", isPresented: $confirming) {
            Button("Registrar") { model.registerSuiteProfiles() }
        } message: {
            Text("nüün agrega su entrada en cada perfil (y crea los que no existan). No cambia lo que otras apps guardan ni mueve bibliotecas.")
        }
    }
    private func kind(_ value: String) -> String {
        switch value {
        case "team": String(localized: "Equipo")
        case "viewer": String(localized: "Ver como")
        default: String(localized: "Persona")
        }
    }
}
