/// An opaque color with a stable preset identifier.
///
/// The presets are part of an app's identity (nu9ve-theme-v1 §7): each app declares its own list.
/// This package only gives the type, so no app's colors become everyone's.
public struct ThemePreset: Sendable, Identifiable, Equatable {
    public let id: String
    public let hex: String
    /// `hex` must be an opaque sRGB color (`#RRGGBB`); the app's tests check its own presets.
    public init(id: String, hex: String) {
        self.id = id
        self.hex = hex
    }
    public var color: ThemeColor { ThemeColor(hex: hex)! }
}
