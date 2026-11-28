import Foundation

/// U1.7 (2026-11-27): when a work disk arrives and a copy between it and the other connected work disk has changes
/// (`work_copy_changed`), nüün opens the compact window with it. «Ahora no» to that pair with those same changes is
/// remembered, so the same changes never open it again; new changes do.
public enum WorkCopyPrompt {
    /// The pair and its changes: the suggestion's key, how many files and bytes, and which paths.
    public static func signature(_ suggestion: JSONValue) -> String {
        let paths = suggestion["paths"].array.map { $0["path"].string.isEmpty ? $0.string : $0["path"].string }.sorted()
        return [suggestion["key"].string, String(Int(suggestion["count"].double)), String(Int(suggestion["bytes"].double)),
                paths.joined(separator: "\n")].joined(separator: "\u{1F}")
    }
    /// The suggestion that should open the window for a disk mounted at `arrived`, if any: a copy with changes
    /// involving that disk whose signature the person has not declined.
    public static func toOpen(_ suggestions: [JSONValue], arrived: String, declined: [String: String]) -> JSONValue? {
        let disk = arrived.hasSuffix("/") ? String(arrived.dropLast()) : arrived
        return suggestions.first { suggestion in
            guard suggestion["flow"].string == "work_copy_changed", suggestion["count"].double > 0 else { return false }
            let roots = [suggestion["source"]["root"].string, suggestion["destination"]["root"].string]
            guard roots.contains(where: { $0 == disk || $0 == disk + "/" }) else { return false }
            return declined[suggestion["key"].string] != signature(suggestion)
        }
    }
}
