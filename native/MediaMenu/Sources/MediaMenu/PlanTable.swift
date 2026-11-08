import SwiftUI
import MediaMenuCore

/// What one planned unit gets: one row, read at a glance (icons per destination), never a card per file.
enum WorkMark: String, Comparable { case copy, existing, pending, none
    static func < (a: WorkMark, b: WorkMark) -> Bool { a.rawValue < b.rawValue } }
enum SourceMark: String, Comparable { case release, keep, format, discard
    static func < (a: SourceMark, b: SourceMark) -> Bool { a.rawValue < b.rawValue } }

struct PlanUnitRow: Identifiable, Hashable {
    let id: String
    let name: String
    let project: String
    let group: String
    let detail: String
    let bytes: Double
    let files: Int
    let archived: Bool
    let work: WorkMark
    let source: SourceMark
}

struct PlanGroupRow: Identifiable, Hashable {
    let id: String
    let units: Int
    let bytes: Double
    let archived: Int
    let copies: Int
    let existing: Int
    let pending: Int
    let source: SourceMark
}

enum PlanRows {
    /// Built once per plan (1k+ units): dictionaries, no per-render scans.
    static func make(_ plan: JSONValue) -> (units: [PlanUnitRow], groups: [PlanGroupRow]) {
        let preview = Dictionary(plan["preview"].array.map { ($0["source"].string, $0) }, uniquingKeysWith: { a, _ in a })
        let root = plan["destinations"][plan["archive_id"].string]["root"].string
        let prefix = plan["destinations"][plan["archive_id"].string]["prefix"].string
        var rows: [PlanUnitRow] = []
        for unit in plan["units"].array {
            let paths = unit["paths"].array.map(\.string)
            let previews = paths.compactMap { preview[$0] }
            guard let first = previews.first else { continue }
            let support = unit["support"].bool
            let existing = unit["work_existing"].object
            let mission = unit["mission"]
            let work: WorkMark = previews.contains { $0["work_path"] != .null } ? .copy
                : !existing.isEmpty ? .existing
                : mission["state"].string == "pending_export" || plan["work_delivery_status"].string == "pending" && !support ? .pending : .none
            let actions = Set(previews.map { $0["source_action"].string })
            let source: SourceMark = actions.contains("camera_format") ? .format
                : actions.contains("release_verified") || actions.contains("release_clipper") ? .release : .keep
            rows.append(PlanUnitRow(id: unit["id"].string, name: unitName(unit, paths: paths),
                project: support ? String(localized: "Datos de cámara") : unit["project"].string,
                group: support ? String(localized: "Datos de cámara") : folder(first["archive_path"].string, root: root, prefix: prefix),
                detail: missionLabel(mission), bytes: previews.reduce(0) { $0 + $1["bytes"].double }, files: paths.count,
                archived: true, work: work, source: source))
        }
        for item in plan["discarded"].array {
            let paths = item["paths"].array.map(\.string)
            rows.append(PlanUnitRow(id: "discard:" + (paths.first ?? ""), name: unitName(.null, paths: paths), project: "—",
                group: plan["cleanup"].bool ? String(localized: "Descartadas · se borran sin copia") : String(localized: "Descartadas · se quedan en el origen"),
                detail: "", bytes: item["bytes"].double, files: paths.count,
                archived: false, work: .none, source: plan["cleanup"].bool ? .discard : .keep))
        }
        let grouped = Dictionary(grouping: rows, by: \.group)
        let groups = grouped.map { name, members in
            PlanGroupRow(id: name, units: members.count, bytes: members.reduce(0) { $0 + $1.bytes },
                         archived: members.filter(\.archived).count, copies: members.filter { $0.work == .copy }.count,
                         existing: members.filter { $0.work == .existing }.count, pending: members.filter { $0.work == .pending }.count,
                         source: members.map(\.source).max() ?? .keep)
        }.sorted { $0.id < $1.id }
        return (rows, groups)
    }
    static func unitName(_ unit: JSONValue, paths: [String]) -> String {
        if unit["kind"].string == "stalker_session" { return URL(fileURLWithPath: unit["bundle"].string).lastPathComponent }
        let stems = paths.map { URL(fileURLWithPath: $0).deletingPathExtension().lastPathComponent }
        let extensions = paths.map { URL(fileURLWithPath: $0).pathExtension.uppercased() }.sorted()
        return Set(stems).count == 1 ? stems[0] + " · " + extensions.joined(separator: "+") : URL(fileURLWithPath: paths[0]).lastPathComponent
    }
    /// Destination folder below the archive prefix, e.g. `dron/misiones/2026/11/26NOV07-153422_CALLESHUECOS-45M`.
    static func folder(_ path: String, root: String, prefix: String) -> String {
        var folder = URL(fileURLWithPath: path).deletingLastPathComponent().path
        for head in [root + "/" + prefix + "/", root + "/"] where folder.hasPrefix(head) { folder.removeFirst(head.count); break }
        return folder
    }
    static func missionLabel(_ mission: JSONValue) -> String {
        switch mission["state"].string {
        case "matched": return mission["plan_name"].string
        case "pending_export": return mission["likely_mission"].bool ? String(localized: "Falta el export · parece de misión") : String(localized: "Falta el export")
        case "none": return mission["likely_mission"].bool ? String(localized: "Sin registro · parece de misión") : String(localized: "Sin misión")
        default: return ""
        }
    }
}

struct PlanTable: View {
    @Bindable var model: Model
    let plan: JSONValue
    @State private var units: [PlanUnitRow] = []
    @State private var groups: [PlanGroupRow] = []
    @State private var group: String?
    @State private var order = [KeyPathComparator(\PlanUnitRow.group), KeyPathComparator(\PlanUnitRow.name)]
    @Environment(\.creatorPalette) private var palette
    private var archiveName: String { model.volume(plan["archive_id"].string)?["name"].string ?? "" }
    private var workName: String { model.volume(plan["work_id"].string)?["name"].string ?? model.volume(plan["expected_work_id"].string)?["name"].string ?? "SSD" }
    private var shown: [PlanUnitRow] { (group.map { g in units.filter { $0.group == g } } ?? units).sorted(using: order) }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 14) {
                legend("checkmark.circle.fill", String(localized: "Respaldo en \(archiveName)"), palette.accentText)
                legend("externaldrive.badge.plus", String(localized: "Copia en \(workName)"), palette.ink)
                legend("equal.circle", String(localized: "Ya está en \(workName)"), palette.muted)
                legend("clock", String(localized: "Pendiente"), palette.warning)
                legend("trash", String(localized: "Se libera de la SD"), palette.warning)
            }.font(.caption)
            Table(groups, selection: $group) {
                // The session folder is the end of the path: cut from the start so it stays visible.
                TableColumn("Destino") { row in Text(row.id).lineLimit(1).truncationMode(.head).help(row.id) }
                TableColumn("Unidades") { row in Text("\(row.units)").monospacedDigit() }.width(70)
                TableColumn("Tamaño") { row in Text(bytesLabel(row.bytes)).monospacedDigit() }.width(80)
                TableColumn(archiveName) { row in count(row.archived, of: row.units, symbol: "checkmark.circle.fill", tint: palette.accentText) }.width(80)
                TableColumn(workName) { row in workSummary(row) }.width(min: 110, ideal: 150)
                TableColumn("Origen") { row in sourceMark(row.source) }.width(min: 90, ideal: 120)
            }
            .frame(height: min(CGFloat(groups.count) * 24 + 30, 240))
            .modifier(CreatorListSurface())
            HStack {
                Text(group == nil ? String(localized: "\(units.count) unidades") : String(localized: "\(shown.count) en \(group ?? "")")).font(.caption).foregroundStyle(.secondary)
                Spacer()
                if group != nil { Button("Ver todas") { group = nil }.buttonStyle(.link).font(.caption) }
            }
            Table(shown, sortOrder: $order) {
                TableColumn("Archivo", value: \.name) { row in Text(row.name).lineLimit(1).truncationMode(.middle) }
                TableColumn("Proyecto", value: \.project) { row in Text(row.project).lineLimit(1) }.width(min: 80, ideal: 110)
                TableColumn("Destino", value: \.group) { row in
                    Text(row.detail.isEmpty ? row.group : row.detail).lineLimit(1).truncationMode(row.detail.isEmpty ? .head : .tail).help(row.group)
                }
                TableColumn(archiveName) { row in
                    Image(systemName: row.archived ? "checkmark.circle.fill" : "minus").foregroundStyle(row.archived ? palette.accentText : palette.muted)
                }.width(70)
                TableColumn(workName) { row in workMark(row.work) }.width(min: 80, ideal: 100)
                TableColumn("Origen", value: \.source) { row in sourceMark(row.source) }.width(min: 90, ideal: 120)
                TableColumn("Tamaño", value: \.bytes) { row in Text(bytesLabel(row.bytes)).monospacedDigit() }.width(80)
            }
            .frame(height: 340)
            .modifier(CreatorListSurface())
        }
        .task(id: plan["digest"].string) {
            let built = PlanRows.make(plan)
            units = built.units; groups = built.groups; group = nil
        }
    }
    private func legend(_ symbol: String, _ text: String, _ tint: Color) -> some View {
        Label { Text(text) } icon: { Image(systemName: symbol).foregroundStyle(tint) }
    }
    private func count(_ value: Int, of total: Int, symbol: String, tint: Color) -> some View {
        HStack(spacing: 4) {
            Image(systemName: value == 0 ? "minus" : symbol).foregroundStyle(value == 0 ? palette.muted : tint)
            if value != total && value > 0 { Text("\(value)").monospacedDigit() }
        }
    }
    @ViewBuilder private func workSummary(_ row: PlanGroupRow) -> some View {
        HStack(spacing: 8) {
            if row.copies > 0 { Label("\(row.copies)", systemImage: "externaldrive.badge.plus") }
            if row.existing > 0 { Label("\(row.existing)", systemImage: "equal.circle").foregroundStyle(palette.muted) }
            if row.pending > 0 { Label("\(row.pending)", systemImage: "clock").foregroundStyle(palette.warning) }
            if row.copies + row.existing + row.pending == 0 { Image(systemName: "minus").foregroundStyle(palette.muted) }
        }.monospacedDigit()
    }
    @ViewBuilder private func workMark(_ mark: WorkMark) -> some View {
        switch mark {
        case .copy: Label("Copia", systemImage: "externaldrive.badge.plus")
        case .existing: Label("Ya está", systemImage: "equal.circle").foregroundStyle(palette.muted)
        case .pending: Label("Pendiente", systemImage: "clock").foregroundStyle(palette.warning)
        case .none: Image(systemName: "minus").foregroundStyle(palette.muted)
        }
    }
    @ViewBuilder private func sourceMark(_ mark: SourceMark) -> some View {
        switch mark {
        case .release: Label("Se libera", systemImage: "trash").foregroundStyle(palette.warning)
        case .keep: Label("Se conserva", systemImage: "lock")
        case .format: Label("Formato en cámara", systemImage: "camera")
        case .discard: Label("Se borra sin copia", systemImage: "xmark.bin").foregroundStyle(palette.warning)
        }
    }
}
