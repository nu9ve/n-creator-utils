/// An opaque color with a stable preset identifier.
public struct ThemePreset: Sendable, Identifiable, Equatable {
    public let id: String
    public let hex: String
    public static let all = [
        ThemePreset(id: "amber", hex: "#FFBB5C"),
        ThemePreset(id: "knife", hex: "#D52B1E"),
        ThemePreset(id: "steel", hex: "#8A99A8"),
        ThemePreset(id: "glacier", hex: "#7CC6E6"),
    ]
    public var color: ThemeColor { ThemeColor(hex: hex)! }
}
