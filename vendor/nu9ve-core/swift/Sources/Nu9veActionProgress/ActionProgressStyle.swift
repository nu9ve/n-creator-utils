#if os(macOS)
import Nu9veAction
import Nu9veFormat
import Nu9veTheme
import SwiftUI

/// The skin and the colors a long action is drawn with. The app sets it once, high in its views, with
/// `.environment(\.actionProgressStyle, ActionProgressStyle(skin: skin, roles: roles))`.
public struct ActionProgressStyle: Equatable, Sendable {
    public var skin: ThemeSkin
    public var ink: Color
    public var muted: Color
    public var line: Color
    public var raised: Color
    public var accent: Color
    public var onAccent: Color
    public var accentText: Color
    /// nu9ve-interaction-v1 §7: blue copies, amber releases, green is done, red discards.
    public var info: Color
    public var warning: Color
    public var success: Color
    public var danger: Color

    public init(skin: ThemeSkin, ink: Color, muted: Color, line: Color, raised: Color, accent: Color, onAccent: Color,
                accentText: Color, info: Color, warning: Color, success: Color, danger: Color) {
        self.skin = skin
        self.ink = ink
        self.muted = muted
        self.line = line
        self.raised = raised
        self.accent = accent
        self.onAccent = onAccent
        self.accentText = accentText
        self.info = info
        self.warning = warning
        self.success = success
        self.danger = danger
    }

    /// From the common roles of the skin in use.
    public init(skin: ThemeSkin, roles: ThemeRoles) {
        func color(_ c: ThemeColor) -> Color { Color(.sRGB, red: c.red, green: c.green, blue: c.blue, opacity: 1) }
        self.init(skin: skin, ink: color(roles.ink), muted: color(roles.muted), line: color(roles.line), raised: color(roles.raised),
                  accent: color(roles.accent), onAccent: color(roles.onAccent), accentText: color(roles.accentText),
                  info: color(roles.info), warning: color(roles.warning), success: color(roles.success), danger: color(roles.danger))
    }

    /// Glass with a neutral blue, for previews and for an app that has not set its own.
    public static let fallback = ActionProgressStyle(
        skin: .glass, roles: ThemeRoles(skin: .glass, dark: false, base: ThemeColor(hex: "#2F78D6")!))

    /// The color of a log kind or of the file in hand's action.
    func color(forKind kind: String) -> Color {
        switch kind {
        case "copy", "copied", "ingest", "ingested": return info
        case "release", "released", "cleaned", "removed": return warning
        case "failed", "discarded": return danger
        case "verified", "done", "written": return success
        default: return muted
        }
    }

    var controlShape: AnyShape { skin == .glass ? AnyShape(Capsule()) : AnyShape(Rectangle()) }
}

extension EnvironmentValues {
    @Entry public var actionProgressStyle = ActionProgressStyle.fallback
}

// MARK: - Texts

/// The texts of the view. They are Spanish keys looked up in the app's own strings (`Bundle.main`), like the
/// rest of the shell: each app translates them in its catalog.
enum ActionText {
    static func stage(_ record: ActionRecord) -> LocalizedStringKey {
        switch record.commonStage {
        case .preparing: return "Preparando"
        case .listing: return "Contando qué hay que hacer"
        case .scanning: return "Escaneando"
        case .reviewing: return "Revisando lo que ya está en el destino"
        case .copying: return "Copiando"
        case .verifying: return "Verificando"
        case .rendering: return "Renderizando"
        case .analyzing: return "Analizando"
        case .releasing: return "Liberando originales"
        case .writing: return "Escribiendo el resultado"
        case .finishing: return "Cerrando"
        case nil: return "Trabajando"
        }
    }

    /// The amounts in one line: percent, measure, files and time left.
    static func summary(_ snapshot: ActionSnapshot) -> String {
        var parts: [String] = []
        if let percent = snapshot.percent { parts.append("\(percent)\u{00A0}%") }
        if let record = snapshot.record {
            if let total = record.overallTotal, total > 0 {
                parts.append(String(localized: "\(amount(record.overallDone, record.unit)) de \(amount(total, record.unit))"))
            } else if record.overallDone > 0 {
                parts.append(amount(record.overallDone, record.unit))
            }
            if let filesTotal = record.filesTotal, filesTotal > 0, record.unit != .files {
                parts.append(String(localized: "\(record.filesDone ?? 0) de \(filesTotal) archivos"))
            }
        }
        switch snapshot.eta {
        case .shown:
            if let left = snapshot.secondsLeft {
                let minutes = Int((left / 60).rounded(.up))
                parts.append(minutes <= 1 ? String(localized: "queda menos de un minuto") : String(localized: "quedan ~\(minutes) min"))
            }
        case .pending:
            parts.append(String(localized: "ETA pendiente"))
        case .none:
            break
        }
        return parts.joined(separator: " · ")
    }

    static func amount(_ value: Int, _ unit: ActionUnit) -> String {
        switch unit {
        case .bytes: return NumberText.bytes(Int64(value))
        case .files: return String(localized: "\(value) archivos")
        case .items: return String(localized: "\(value) elementos")
        case .seconds: return Timecode.format(Double(value))
        case .work: return "\(value)"
        }
    }

    /// The verb of a log line, unless the engine wrote its own `text`.
    static func verb(_ entry: ActionLogEntry) -> String {
        if let text = entry.text { return text }
        switch entry.kind {
        case "copied": return String(localized: "copiado y verificado")
        case "verified": return String(localized: "verificado")
        case "released": return String(localized: "liberado")
        case "cleaned", "removed": return String(localized: "retirado")
        case "written": return String(localized: "escrito")
        default: return entry.kind
        }
    }

    static func route(_ from: String, _ to: String) -> String {
        to.isEmpty ? from : from.isEmpty ? to : from + " → " + to
    }
}

#endif
