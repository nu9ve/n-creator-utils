import AppKit
import SwiftUI
import MediaMenuCore

extension Model {
    var storageTarget: JSONValue? { storageTargets.first { $0["id"].string == storageTargetID } }
    var storageRunning: Bool { storageRun != nil }
    func storageName(_ target: JSONValue) -> String { target["builtin"].bool ? String(localized: "Esta Mac") : target["name"].string }
    /// What the footer acts on: the selected row, otherwise the folder on screen.
    var storageActionPath: String { storageSelection ?? storagePath }
    var storageArchiveReady: Bool { storageTargets.contains { $0["role"].string == "archive" && $0["online"].bool && $0["indexed"].bool } }

    func loadStorage() async {
        guard !storageFixture, let reader, reader.configuration.usable else { return }
        do {
            let data = try await reader.call("storage_overview")
            storageTargets = data["targets"].array
            follow(data["running"])
            if storageTarget == nil {
                let preferred = storageTargets.first { $0["role"].string == "work" && $0["online"].bool } ?? storageTargets.first
                selectStorageTarget(preferred?["id"].string ?? "", reload: false)
            }
            await loadStorageTree()
        } catch { storageError = error.localizedDescription }
    }
    func loadStorageTree() async {
        guard !storageFixture, let reader, let target = storageTarget, target["indexed"].bool else { storageTree = nil; return }
        do {
            storageTree = try await reader.call("storage_tree", ["target": target["id"], "path": .string(storagePath)])
        } catch let failure as BackendFailure where failure.code == "unknown_path" && !storagePath.isEmpty {
            // The folder left the index after a newer scan.
            storagePath = ""; storageSelection = nil
            await loadStorageTree()
        } catch { storageError = error.localizedDescription }
    }
    func selectStorageTarget(_ id: String, reload: Bool = true) {
        storageTargetID = id; storagePath = ""; storageSelection = nil; storageTree = nil
        if reload { Task { await loadStorageTree() } }
    }
    func openStorageFolder(_ path: String) {
        storagePath = path; storageSelection = nil
        Task { await loadStorageTree() }
    }
    func scanStorage() {
        guard let target = storageTarget else { return }
        runStorage("storage_scan", ["target": target["id"]], kind: "scan")
    }
    func compareStorage() {
        guard let target = storageTarget else { return }
        runStorage("storage_compare", ["target": target["id"], "path": .string(storageActionPath)], kind: "compare")
    }
    private func runStorage(_ method: String, _ params: [String: JSONValue], kind: String) {
        guard !storageRunning, let configuration = client?.configuration else { return }
        // Its own process: cancelling it can never reach a transfer, and a transfer can start meanwhile.
        let worker = BackendClient(configuration: configuration)
        storageClient = worker; storageError = ""
        storageRun = .object(["kind": .string(kind), "target_id": params["target"] ?? .null, "files": .integer(0), "allocated": .integer(0)])
        watchStorageRun()
        Task {
            do {
                let result = try await worker.call(method, params)
                if kind == "compare", result["skipped"].double > 0 {
                    storageError = String(localized: "\(Int(result["skipped"].double)) archivos cambiaron desde el último análisis y no se compararon. Actualiza y vuelve a comprobar.")
                }
            } catch { storageError = error.localizedDescription }
            storageClient = nil; storageMonitor?.cancel(); storageMonitor = nil
            await loadStorage()
            storageRun = nil
        }
    }
    func cancelStorage() { storageClient?.cancel() }
    /// A copy always wins over a comparison on the same disks; finished hashes stay for later.
    func yieldStorageToTransfer() {
        if storageRun?["kind"].string == "compare" { cancelStorage() }
    }
    /// Also picks up a scan started from the CLI, which this app does not own.
    private func follow(_ running: JSONValue) {
        guard storageClient == nil else { return }
        if running == .null {
            storageRun = nil
        } else {
            storageRun = running
            if storageMonitor == nil { watchStorageRun() }
        }
    }
    private func watchStorageRun() {
        storageMonitor?.cancel()
        storageMonitor = Task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                guard !Task.isCancelled, let data = try? await reader?.call("storage_progress") else { continue }
                guard !Task.isCancelled else { return }
                if data["running"] != .null { storageRun = data["running"] }
                else if storageClient == nil {
                    storageRun = nil; storageMonitor = nil
                    await loadStorage()
                    return
                }
            }
        }
    }
    func addStorageFolder() {
        pickURL(message: String(localized: "Elige una carpeta o un disco para ver qué ocupa. Nunca se usará como destino.")) { url in
            Task {
                do {
                    let added = try await self.client!.call("storage_add_folder", ["root": .string(url.path)])
                    await self.loadStorage()
                    self.selectStorageTarget(added["id"].string)
                } catch { self.storageError = error.localizedDescription }
            }
        }
    }
    func forgetStorageFolder(_ id: String) {
        Task {
            do {
                _ = try await client!.call("storage_remove_folder", ["target": .string(id)])
                if storageTargetID == id { storageTargetID = ""; storageTree = nil }
                await loadStorage()
            } catch { storageError = error.localizedDescription }
        }
    }
    func revealStoragePath(_ path: String) {
        guard let root = storageTarget?["root"].string else { return }
        let url = path.isEmpty ? URL(fileURLWithPath: root) : URL(fileURLWithPath: root).appendingPathComponent(path)
        guard FileManager.default.fileExists(atPath: url.path) else {
            storageError = String(localized: "No se encontró en el disco; actualiza el análisis."); return
        }
        NSWorkspace.shared.activateFileViewerSelecting([url])
    }
    func reviewStorageRelease() {
        guard let target = storageTarget, target["role"].string == "work", !storageActionPath.isEmpty else { return }
        work = target["id"].string
        auditPath = storageActionPath
        releaseProject = URL(fileURLWithPath: storageActionPath).lastPathComponent
        releasePlan = nil; showRelease = true
    }
    func backupStorageFolder() {
        guard let target = storageTarget, target["role"].string == "work", !storageActionPath.isEmpty else { return }
        work = target["id"].string
        auditPath = storageActionPath
        releaseProject = URL(fileURLWithPath: storageActionPath).lastPathComponent
        archiveAuditFolder()
    }
    func storageRunLabel(_ run: JSONValue) -> String {
        let name = storageTargets.first { $0["id"] == run["target_id"] }?["name"].string ?? ""
        let shown = storageTargets.first { $0["id"] == run["target_id"] }.map(storageName) ?? name
        if run["kind"].string == "compare" {
            let done = Int(run["files"].double), total = Int(run["total_files"].double)
            return String(localized: "Comparando con el respaldo · \(done) de \(total) archivos · \(bytesLabel(run["allocated"].double)) leídos")
        }
        return String(localized: "Analizando \(shown) · \(Int(run["files"].double)) archivos · \(bytesLabel(run["allocated"].double))")
    }
}

enum DiskLens: String, CaseIterable, Identifiable {
    case safety, content
    var id: String { rawValue }
    var title: LocalizedStringKey { self == .safety ? "Seguridad" : "Contenido" }
}

private let safetyOrder = ["only_here", "probable", "unknown", "verified", "derived", "backup"]
private let kindOrder = ["video", "raw_image", "image", "audio", "capture", "editable", "derived", "other"]

func diskClassTitle(_ key: String) -> String {
    switch key {
    case "only_here": String(localized: "Sólo aquí")
    case "probable": String(localized: "Probable")
    case "unknown": String(localized: "Sin revisar")
    case "verified": String(localized: "En respaldo")
    case "derived": String(localized: "Regenerable (sugerido)")
    case "backup": String(localized: "Respaldo")
    case "mixed": String(localized: "Mixto")
    case "video": String(localized: "Video")
    case "raw_image": "RAW"
    case "image": String(localized: "Fotos")
    case "audio": String(localized: "Audio")
    case "capture": String(localized: "Capturas")
    case "editable": String(localized: "Proyectos de edición")
    default: String(localized: "Otros")
    }
}
func diskClassColor(_ key: String) -> Color {
    switch key {
    case "only_here": .orange
    case "probable": .yellow
    case "verified": .green
    case "backup", "video": .blue
    case "raw_image": .indigo
    case "image": .teal
    case "audio": .pink
    case "capture": .purple
    case "editable": .mint
    default: .gray
    }
}
func diskClassSymbol(_ key: String) -> String {
    switch key {
    case "only_here": "exclamationmark.triangle"
    case "probable": "questionmark.circle"
    case "unknown": "circle.dashed"
    case "verified": "checkmark.circle"
    case "derived": "arrow.triangle.2.circlepath"
    case "backup": "externaldrive"
    case "mixed": "circle.lefthalf.filled"
    case "video": "film"
    case "raw_image": "camera.aperture"
    case "image": "photo"
    case "audio": "waveform"
    case "capture": "record.circle"
    case "editable": "scissors"
    default: "doc"
    }
}
private func amounts(_ value: JSONValue, order: [String]) -> [(key: String, bytes: Double)] {
    order.compactMap { key in value[key].double > 0 ? (key, value[key].double) : nil }
}
/// A folder only takes a class when nearly all of it is that class: half a folder without a copy must never look safe.
private func dominant(_ value: JSONValue, order: [String]) -> String {
    let parts = amounts(value, order: order), total = parts.reduce(0) { $0 + $1.bytes }
    guard let top = parts.max(by: { $0.bytes < $1.bytes }) else { return "unknown" }
    return top.bytes >= total * 0.9 ? top.key : "mixed"
}
private func indexedLabel(_ text: String) -> String {
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    guard let date = formatter.date(from: text) else { return text }
    let relative = RelativeDateTimeFormatter()
    relative.locale = Locale(identifier: Bundle.main.preferredLocalizations.first ?? "es")
    return relative.localizedString(for: date, relativeTo: Date())
}

struct DiskRow: Identifiable {
    let id: String
    let name: String
    let isFolder: Bool
    let opaque: Bool
    let allocated: Double
    let files: Int
    let value: JSONValue
    let safetyKey: String
    let contentKey: String
    var stateTitle: String { diskClassTitle(safetyKey) }
    init(folder: JSONValue) {
        id = folder["path"].string; name = folder["name"].string; isFolder = true; opaque = folder["opaque"].bool
        allocated = folder["allocated"].double; files = Int(folder["files"].double); value = folder
        safetyKey = dominant(folder["safety"], order: safetyOrder)
        contentKey = folder["kind"].string.isEmpty ? dominant(folder["kinds"], order: kindOrder) : folder["kind"].string
    }
    init(file: JSONValue) {
        id = file["path"].string; name = file["name"].string; isFolder = false; opaque = false
        allocated = file["allocated"].double; files = 1; value = file
        safetyKey = file["state"].string; contentKey = file["kind"].string
    }
    func key(_ lens: DiskLens) -> String { lens == .safety ? safetyKey : contentKey }
    func parts(_ lens: DiskLens) -> [(key: String, bytes: Double)] {
        guard isFolder else { return [] }
        return lens == .safety ? amounts(value["safety"], order: safetyOrder) : amounts(value["kinds"], order: kindOrder)
    }
}

/// How a folder divides, drawn to scale.
struct ClassStrip: View {
    let parts: [(key: String, bytes: Double)]
    var body: some View {
        let total = max(1, parts.reduce(0) { $0 + $1.bytes })
        GeometryReader { geometry in
            HStack(spacing: 1) {
                ForEach(parts, id: \.key) { part in
                    Rectangle().fill(diskClassColor(part.key).opacity(part.key == "unknown" ? 0.45 : 0.85))
                        .frame(width: max(1, (geometry.size.width - CGFloat(parts.count - 1)) * part.bytes / total))
                }
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 2))
        .accessibilityHidden(true)
    }
}

struct DisksView: View {
    @Bindable var model: Model
    @AppStorage("diskLens") private var lens = DiskLens.safety
    @State private var order = [KeyPathComparator(\DiskRow.allocated, order: .reverse)]
    private var rows: [DiskRow] {
        guard let tree = model.storageTree else { return [] }
        // System leftovers with nothing in them only add noise.
        return (tree["folders"].array.map(DiskRow.init(folder:)) + tree["files"].array.map(DiskRow.init(file:)))
            .filter { $0.allocated > 0 || !$0.name.hasPrefix(".") }.sorted(using: order)
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            PageHeader(title: "Discos", subtitle: "Qué ocupa cada disco y qué ya tiene copia. Esta vista sólo lee: no borra ni mueve nada.") {
                Button("Añadir carpeta…", systemImage: "folder.badge.plus") { model.addStorageFolder() }
            }
            ErrorBanner(model: model)
            if !model.storageError.isEmpty {
                Label(model.storageError, systemImage: "exclamationmark.triangle").font(.callout).foregroundStyle(.orange).textSelection(.enabled)
            }
            TargetStrip(model: model)
            if let target = model.storageTarget {
                if target["indexed"].bool, model.storageTree != nil {
                    DiskSummary(target: target, lens: lens)
                    if target["role"].string != "archive" && !model.storageArchiveReady {
                        Label("Conecta y analiza tu disco de respaldo para saber qué ya tiene copia.", systemImage: "externaldrive.badge.questionmark")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                    navigation
                    HStack(alignment: .top, spacing: 10) {
                        TreemapView(rows: rows.filter { $0.allocated > 0 }, rest: model.storageTree?["rest"].double ?? 0, lens: lens, selection: model.storageSelection) { row in
                            if row.isFolder && !row.opaque { model.openStorageFolder(row.id) } else { model.storageSelection = row.id }
                        }
                        .frame(minWidth: 260, maxWidth: .infinity, maxHeight: .infinity)
                        list.frame(minWidth: 340, maxWidth: .infinity, maxHeight: .infinity)
                    }
                    SelectionDetail(model: model, row: rows.first { $0.id == model.storageSelection })
                } else {
                    notIndexed(target).frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            } else {
                ContentUnavailableView("Sin discos", systemImage: "externaldrive", description: Text("Registra tus discos en Ajustes o añade una carpeta para ver qué ocupa."))
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .padding(.horizontal, 20).padding(.top, 14)
        .safeAreaInset(edge: .bottom, spacing: 0) { footer }
        .task { await model.loadStorage() }
    }
    private func notIndexed(_ target: JSONValue) -> some View {
        ContentUnavailableView {
            Label(target["online"].bool ? String(localized: "\(model.storageName(target)) aún no se ha analizado") : String(localized: "\(model.storageName(target)) está desconectado"), systemImage: "externaldrive")
        } description: {
            if target["builtin"].bool {
                Text("Se leen sólo nombres, tamaños y fechas. No se abre ningún archivo. Puede tardar varios minutos y macOS puede pedirte permisos.")
            } else {
                Text(target["online"].bool ? "Se leen sólo nombres, tamaños y fechas. No se abre ningún archivo." : "Conéctalo para analizarlo. Después podrás consultarlo aunque esté guardado.")
            }
        } actions: {
            if target["online"].bool {
                Button("Analizar") { model.scanStorage() }.buttonStyle(CreatorButtonStyle(prominent: true)).disabled(model.storageRunning)
            }
        }
    }
    private var navigation: some View {
        HStack(spacing: 4) {
            let parts = model.storagePath.isEmpty ? [] : model.storagePath.split(separator: "/").map(String.init)
            Button(model.storageTarget.map(model.storageName) ?? "") { model.openStorageFolder("") }.buttonStyle(.link).font(.callout)
            ForEach(parts.indices, id: \.self) { index in
                Image(systemName: "chevron.right").font(.caption2).foregroundStyle(.secondary)
                Button(parts[index]) { model.openStorageFolder(parts[0...index].joined(separator: "/")) }
                    .buttonStyle(.link).font(.callout).lineLimit(1)
            }
            Spacer(minLength: 8)
            Picker("Ver por", selection: $lens) { ForEach(DiskLens.allCases) { Text($0.title).tag($0) } }
                .pickerStyle(.segmented).fixedSize().labelsHidden()
        }
    }
    private var list: some View {
        Table(rows, selection: $model.storageSelection, sortOrder: $order) {
            TableColumn("Nombre", value: \.name) { row in
                Label {
                    Text(row.name).lineLimit(1).truncationMode(.middle)
                } icon: {
                    Image(systemName: row.isFolder ? (row.opaque ? "shippingbox" : "folder") : diskClassSymbol(row.contentKey))
                }
            }
            TableColumn("Tamaño", value: \.allocated) { Text(bytesLabel($0.allocated)).monospacedDigit() }.width(min: 60, ideal: 76)
            TableColumn("Archivos", value: \.files) { Text("\($0.files)").monospacedDigit() }.width(min: 44, ideal: 56)
            TableColumn("Estado", value: \.stateTitle) { row in
                HStack(spacing: 6) {
                    Label(row.stateTitle, systemImage: diskClassSymbol(row.safetyKey)).foregroundStyle(row.safetyKey == "mixed" ? .secondary : diskClassColor(row.safetyKey)).font(.caption).lineLimit(1)
                    if row.safetyKey == "mixed" { ClassStrip(parts: row.parts(.safety)).frame(width: 44, height: 5) }
                }
            }.width(min: 90, ideal: 150)
        }
        .contextMenu(forSelectionType: String.self) { _ in
        } primaryAction: { ids in
            if let id = ids.first, let row = rows.first(where: { $0.id == id }), row.isFolder, !row.opaque { model.openStorageFolder(row.id) }
        }
        .modifier(CreatorListSurface())
    }
    private var footer: some View {
        let target = model.storageTarget, online = target?["online"].bool ?? false, indexed = target?["indexed"].bool ?? false
        let isWork = target?["role"].string == "work", isArchive = target?["role"].string == "archive"
        let path = model.storageActionPath
        return ActionFooter {
            if let run = model.storageRun {
                ProgressView().controlSize(.small)
                Text(model.storageRunLabel(run)).font(.caption).monospacedDigit().lineLimit(1)
                if model.storageClient != nil { Button("Cancelar") { model.cancelStorage() } }
            } else {
                if target?["observed"].bool == true, target?["builtin"].bool != true, let id = target?["id"].string {
                    Button("Olvidar carpeta") { model.forgetStorageFolder(id) }
                }
                if indexed {
                    Button("Actualizar", systemImage: "arrow.clockwise") { model.scanStorage() }.disabled(!online)
                    Button("Mostrar en Finder", systemImage: "folder") { model.revealStoragePath(path) }.disabled(!online)
                }
            }
            Spacer(minLength: 0)
            if isWork && !path.isEmpty && indexed {
                Button("Respaldar…") { model.backupStorageFolder() }.disabled(!online || model.busy)
                Button("Revisar liberación…") { model.reviewStorageRelease() }.disabled(!online || model.busy)
            }
            if indexed && !isArchive {
                Button("Comprobar", systemImage: "checkmark.shield") { model.compareStorage() }
                    .buttonStyle(CreatorButtonStyle(prominent: true))
                    .disabled(!online || model.storageRunning || model.busy || !model.storageArchiveReady)
                    .help(model.storageArchiveReady ? "Lee y compara el contenido con el respaldo. No modifica nada." : "Conecta y analiza el disco de respaldo para comparar.")
            }
        }
    }
}

struct TargetStrip: View {
    @Bindable var model: Model
    @Environment(\.creatorPalette) private var palette
    var body: some View {
        ScrollView(.horizontal) {
            HStack(spacing: 8) {
                ForEach(model.storageTargets, id: \.["id"].string) { target in
                    let selected = target["id"].string == model.storageTargetID
                    Button { model.selectStorageTarget(target["id"].string) } label: {
                        HStack(spacing: 6) {
                            Image(systemName: target["builtin"].bool ? "laptopcomputer" : target["observed"].bool ? "folder" : target["online"].bool ? "externaldrive.fill" : "externaldrive.badge.xmark")
                                .foregroundStyle(target["online"].bool ? palette.ink : .secondary)
                            Text(model.storageName(target)).font(.caption.weight(.semibold))
                            Text(usage(target)).font(.caption).foregroundStyle(.secondary).monospacedDigit()
                        }
                        .padding(.horizontal, 10).padding(.vertical, 6)
                        .background(selected ? palette.accentSoft : palette.raised.opacity(0.5), in: palette.pill)
                        .contentShape(palette.pill)
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(selected ? .isSelected : [])
                }
            }
        }.scrollIndicators(.hidden)
    }
    private func usage(_ target: JSONValue) -> String {
        if target["fs_total"] == .null { return String(localized: "desconectado") }
        let free = String(localized: "\(bytesLabel(target["fs_free"].double)) libres")
        return target["online"].bool ? free : String(localized: "desconectado · \(free)")
    }
}

struct DiskSummary: View {
    let target: JSONValue
    let lens: DiskLens
    private var parts: [(key: String, bytes: Double)] {
        lens == .safety ? amounts(target["safety"], order: safetyOrder) : amounts(target["kinds"], order: kindOrder)
    }
    private var uncountedTitle: String { target["observed"].bool && !target["builtin"].bool ? String(localized: "Resto del disco") : String(localized: "Sin contar") }
    var body: some View {
        let total = max(1, target["fs_total"].double), uncounted = target["uncounted"].double
        VStack(alignment: .leading, spacing: 6) {
            GeometryReader { geometry in
                HStack(spacing: 1) {
                    ForEach(parts, id: \.key) { part in
                        Rectangle().fill(diskClassColor(part.key).opacity(0.75))
                            .frame(width: max(1, geometry.size.width * part.bytes / total))
                            .help(diskClassTitle(part.key) + " · " + bytesLabel(part.bytes))
                    }
                    if uncounted > 0 {
                        Rectangle().fill(Color.gray.opacity(0.3)).frame(width: geometry.size.width * uncounted / total).help(uncountedTitle)
                    }
                    Spacer(minLength: 0)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.gray.opacity(0.1))
            }
            .frame(height: 10).clipShape(RoundedRectangle(cornerRadius: 4))
            .accessibilityHidden(true)
            HStack(spacing: 12) {
                ForEach(parts, id: \.key) { part in
                    Label { Text("\(diskClassTitle(part.key)) \(bytesLabel(part.bytes))") } icon: {
                        Image(systemName: diskClassSymbol(part.key)).foregroundStyle(diskClassColor(part.key))
                    }
                }
                if uncounted > 0 { Text("\(uncountedTitle) \(bytesLabel(uncounted))").foregroundStyle(.secondary) }
                Text("\(bytesLabel(target["fs_free"].double)) libres").foregroundStyle(.secondary)
                Spacer(minLength: 0)
            }
            .font(.caption).lineLimit(1).minimumScaleFactor(0.85)
            HStack(spacing: 6) {
                Text("Analizado \(indexedLabel(target["indexed_at"].string)) · \(Int(target["files"].double)) archivos")
                if !target["online"].bool { Text("· datos de la última conexión") }
                if target["unreadable"].double > 0 {
                    Text("· \(Int(target["unreadable"].double)) carpetas sin acceso").foregroundStyle(.orange)
                        .help(target["samples"].array.prefix(10).map(\.string).joined(separator: "\n"))
                }
            }.font(.caption).foregroundStyle(.secondary)
        }
    }
}

struct TreemapView: View {
    let rows: [DiskRow]
    let rest: Double
    let lens: DiskLens
    let selection: String?
    let action: (DiskRow) -> Void
    @Environment(\.creatorPalette) private var palette
    private var tiles: [DiskRow] { Array(rows.sorted { $0.allocated > $1.allocated }.prefix(40)) }
    var body: some View {
        GeometryReader { geometry in
            let shown = tiles
            let others = rest + rows.dropFirst(0).filter { row in !shown.contains { $0.id == row.id } }.reduce(0) { $0 + $1.allocated }
            let rects = Treemap.layout(shown.map(\.allocated) + [others], in: CGRect(origin: .zero, size: geometry.size))
            ZStack(alignment: .topLeading) {
                ForEach(Array(shown.enumerated()), id: \.element.id) { index, row in
                    tile(row, rects[index])
                }
                if others > 0, let rect = rects.last, rect.width > 1 {
                    VStack(alignment: .leading, spacing: 1) {
                        if rect.width > 56 && rect.height > 32 {
                            Text("Otros").font(.caption2)
                            Text(bytesLabel(others)).font(.caption2).foregroundStyle(.secondary)
                        }
                    }
                    .padding(5)
                    .frame(width: max(0, rect.width - 2), height: max(0, rect.height - 2), alignment: .topLeading)
                    .background(palette.raised, in: RoundedRectangle(cornerRadius: 4))
                    .offset(x: rect.minX + 1, y: rect.minY + 1)
                    .help(String(localized: "Otros") + " · " + bytesLabel(others))
                }
            }
        }
        .background(palette.surface, in: palette.panel)
        .clipShape(palette.panel)
        .overlay { if rows.isEmpty && rest <= 0 { Text("Esta carpeta está vacía").font(.callout).foregroundStyle(.secondary) } }
    }
    private func tile(_ row: DiskRow, _ rect: CGRect) -> some View {
        let key = row.key(lens)
        return Button { action(row) } label: {
            VStack(alignment: .leading, spacing: 1) {
                if rect.width > 56 && rect.height > 32 {
                    HStack(spacing: 3) {
                        Image(systemName: diskClassSymbol(key)).font(.caption2).foregroundStyle(key == "mixed" ? .secondary : diskClassColor(key))
                        Text(row.name).font(.caption2.weight(.medium)).lineLimit(1).truncationMode(.middle)
                    }
                    Text(bytesLabel(row.allocated)).font(.caption2).foregroundStyle(.secondary).monospacedDigit()
                }
                if key == "mixed" && rect.height > 44 && rect.width > 30 {
                    Spacer(minLength: 0)
                    ClassStrip(parts: row.parts(lens)).frame(height: 5)
                }
            }
            .padding(5)
            .frame(width: max(0, rect.width - 2), height: max(0, rect.height - 2), alignment: .topLeading)
            .background(key == "mixed" ? palette.raised : diskClassColor(key).opacity(key == "unknown" || key == "other" ? 0.14 : 0.28), in: RoundedRectangle(cornerRadius: 4))
            .overlay(RoundedRectangle(cornerRadius: 4).stroke(selection == row.id ? palette.ink : .clear, lineWidth: 1.5))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .offset(x: rect.minX + 1, y: rect.minY + 1)
        .help(row.name + " · " + bytesLabel(row.allocated) + " · " + diskClassTitle(key))
        .accessibilityLabel(Text(row.name + ", " + bytesLabel(row.allocated) + ", " + diskClassTitle(key)))
    }
}

/// Plain words for the row in focus. Never says "safe to delete": only a release plan can decide that.
struct SelectionDetail: View {
    @Bindable var model: Model
    let row: DiskRow?
    private var folderName: String { model.storagePath.isEmpty ? (model.storageTarget.map(model.storageName) ?? "") : (model.storageTree?["name"].string ?? "") }
    var body: some View {
        let safety = row?.isFolder == false ? JSONValue.null : (row?.value["safety"] ?? model.storageTree?["safety"] ?? .null)
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 6) {
                Text(row?.name ?? folderName).font(.callout.weight(.medium)).lineLimit(1).truncationMode(.middle).textSelection(.enabled)
                Text(bytesLabel(row?.allocated ?? model.storageTree?["allocated"].double ?? 0)).font(.callout).foregroundStyle(.secondary).monospacedDigit()
            }
            if let row, !row.isFolder {
                Text(fileExplanation(row)).font(.caption).foregroundStyle(.secondary).textSelection(.enabled).lineLimit(2)
            } else {
                Text(amounts(safety, order: safetyOrder).map { diskClassTitle($0.key) + " " + bytesLabel($0.bytes) }.joined(separator: " · "))
                    .font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
    private func fileExplanation(_ row: DiskRow) -> String {
        switch row.safetyKey {
        case "verified":
            let copy = row.value["copy"], place = copy["volume"].string + " › " + copy["path"].string
            return copy["origin"].string == "receipt" ? String(localized: "Copia en \(place), según su recibo") : String(localized: "Copia idéntica en \(place)")
        case "probable": return String(localized: "Hay un archivo del mismo tamaño en el respaldo. Usa Comprobar para saber si es el mismo.")
        case "only_here": return String(localized: "Sin copia conocida en el respaldo.")
        case "derived": return String(localized: "Por su carpeta parece regenerable. Es sólo una sugerencia.")
        case "backup": return String(localized: "Este disco es el respaldo.")
        default: return String(localized: "Analiza el disco de respaldo para comparar.")
        }
    }
}
