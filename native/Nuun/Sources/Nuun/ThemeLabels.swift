import Foundation
import Nu9veTheme

extension ThemeSkin {
    var name: String {
        switch self {
        case .glass: return String(localized: "Cristal")
        case .terminal: return String(localized: "Terminal")
        case .classicMac: return String(localized: "Macintosh")
        case .win9x: return String(localized: "Windows 9x")
        case .gothic: return String(localized: "Gótico")
        }
    }
    var summary: String {
        switch self {
        case .glass: return String(localized: "Nativo y tranquilo. El de todos los días.")
        case .terminal: return String(localized: "Fósforo, letra monoespaciada y líneas de barrido. Tu color base es la señal.")
        case .classicMac: return String(localized: "1 bit, rayas y cajas: un Macintosh II con tarjeta de color.")
        case .win9x: return String(localized: "Biseles grises y título con degradado.")
        case .gothic: return String(localized: "Noche, marco doble y ornamento. Lo contrario de la retícula suiza.")
        }
    }
}

extension ThemePreset {
    /// Literal keys so the compiler extracts them for Localizable.strings.
    var name: String {
        switch id {
        case "knife": return String(localized: "Navaja")
        case "steel": return String(localized: "Acero")
        case "glacier": return String(localized: "Glaciar")
        default: return String(localized: "Ámbar")
        }
    }
}
