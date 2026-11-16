/// Shared skins and their window metrics.
public enum ThemeSkin: String, CaseIterable, Sendable, Identifiable {
    case glass, classicMac, win9x, terminal, gothic
    public var id: String { rawValue }
    /// Period skins keep the look of their era whatever the user's mode.
    public var forcedDark: Bool? {
        switch self {
        case .classicMac, .win9x: return false
        case .gothic, .terminal: return true
        default: return nil
        }
    }
    /// Only the system's window keeps rounded corners; the rest draw their own bar and controls.
    public var usesSquareWindow: Bool { self != .glass }
    // Measures from the contract's catalog (§5).
    /// The row with the app's own controls (navigation, disks, settings).
    public var barHeight: Double { self == .gothic ? 36 : 30 }
    /// A period caption above that row, with the title and the window controls. 0: none.
    public var captionHeight: Double {
        switch self {
        case .classicMac: return 20
        case .win9x: return 22
        default: return 0
        }
    }
    public var controlRadius: Double {
        switch self {
        case .glass: return 7
        case .terminal: return 0
        case .classicMac: return 6
        case .win9x: return 0
        case .gothic: return 1
        }
    }
    public var panelRadius: Double {
        switch self {
        case .glass: return 10
        case .terminal: return 0
        case .classicMac, .win9x: return 0
        case .gothic: return 2
        }
    }
    /// Radius of pills (navigation, steps, badges). Large means a capsule.
    public var pillRadius: Double {
        switch self {
        case .glass: return 999
        case .terminal: return 0
        case .classicMac: return 4
        case .win9x: return 0
        case .gothic: return 1
        }
    }
    /// How strongly lines are drawn; period skins use solid rules.
    public var lineOpacity: (light: Double, dark: Double) {
        switch self {
        case .glass: return (0.55, 0.65)
        case .terminal: return (1, 1)
        case .classicMac, .win9x: return (1, 1)
        case .gothic: return (0.9, 0.9)
        }
    }
}
