import Foundation

/// Destination policy shared by the native form and its request tests.
public enum IngestPolicy {
    public static func parameters(archiveOnly: Bool, skipWork: Bool, deferWork: Bool, work: String) -> [String: JSONValue] {
        let pending = archiveOnly && !skipWork && deferWork
        var values: [String: JSONValue] = [
            "archive_only": .bool(archiveOnly), "confirm_archive_only": .bool(archiveOnly),
            "work_decision": .string(archiveOnly ? (pending ? "pending" : "skipped") : "ready")
        ]
        if !work.isEmpty && (!archiveOnly || pending) { values["work"] = .string(work) }
        return values
    }
}
