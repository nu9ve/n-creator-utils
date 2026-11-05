import SwiftUI
import MediaMenuCore

/// The unit is the thing the person recognizes. Its exact members remain inspectable.
struct PlannedUnitRow: View {
    let model: Model
    let plan: JSONValue
    let unit: JSONValue
    @State private var expanded = false
    private var rows: [JSONValue] {
        let paths = Set(unit["paths"].array.map(\.string))
        return plan["preview"].array.filter { paths.contains($0["source"].string) }
    }
    private var name: String {
        if unit["support"].bool { return String(localized: "Datos de cámara") }
        if unit["kind"].string == "stalker_session" && unit["bundle"].string == "." { return URL(fileURLWithPath: plan["source"].string).lastPathComponent }
        if !unit["bundle"].string.isEmpty && unit["bundle"].string != "." { return URL(fileURLWithPath: unit["bundle"].string).lastPathComponent }
        return URL(fileURLWithPath: rows.first(where: { $0["work_path"] != .null })?["source"].string ?? rows.first?["source"].string ?? "").lastPathComponent
    }
    private func action(_ value: String) -> String {
        switch value {
        case "release_verified": return String(localized: "Liberar tras verificar")
        case "release_clipper": return String(localized: "Liberar tras verificar y comprobar Clipper")
        case "camera_format": return String(localized: "Conservar índices · formato en cámara")
        default: return String(localized: "Conservar origen")
        }
    }
    var body: some View {
        DisclosureGroup(isExpanded: $expanded) {
            ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
                VStack(alignment: .leading, spacing: 2) {
                    Text(row["source"].string).fontWeight(.medium)
                    Text(row["archive_path"].string)
                    if row["work_path"] != .null { Text(row["work_path"].string) }
                    Text(action(row["source_action"].string))
                }.font(.caption).foregroundStyle(.secondary).textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading).padding(.vertical, 3)
            }
        } label: {
            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Label(name, systemImage: unit["support"].bool ? "doc.on.doc" : unit["kind"].string == "stalker_session" ? "record.circle" : "doc")
                        .lineLimit(1).truncationMode(.middle)
                    Spacer()
                    Text(bytesLabel(rows.reduce(0) { $0 + $1["bytes"].double })).font(.caption).monospacedDigit()
                }
                if !unit["support"].bool { Text(unit["project"].string).font(.caption).foregroundStyle(.secondary) }
                HStack(spacing: 12) {
                    Label(model.volume(plan["archive_id"].string)?["name"].string ?? "", systemImage: "externaldrive.fill")
                    if rows.contains(where: { $0["work_path"] != .null }) {
                        Label(model.volume(plan["work_id"].string)?["name"].string ?? "", systemImage: "externaldrive")
                    }
                    Text("\(rows.count) archivos")
                }.font(.caption)
                ForEach(Array(Set(rows.map { $0["source_action"].string })).sorted(), id: \.self) { value in
                    Text(action(value)).font(.caption).foregroundStyle(.secondary)
                }
            }.padding(.vertical, 6)
        }
        Divider()
    }
}
