import AppKit
import SwiftUI
import MediaMenuCore

/// nu9ve-theme-v1 §2: the common roles, resolved by `ThemeRoles` (pure, tested in Core), plus the
/// skin's shapes and display type (§5–6). Backgrounds and window chrome derive from the same seed.
struct CreatorPalette: Sendable {
    static let defaultHue = ThemeRoles.defaultHue
    static let defaultSaturation = ThemeRoles.defaultSaturation
    let roles: ThemeRoles
    init(skin: ThemeSkin = .glass, dark: Bool, hue: Double, saturation: Double, brightness: Double) {
        roles = ThemeRoles(skin: skin, dark: dark, hue: hue, saturation: saturation, brightness: brightness)
    }
    static func ns(_ color: ThemeColor) -> NSColor { NSColor(srgbRed: color.red, green: color.green, blue: color.blue, alpha: 1) }
    static func color(_ color: ThemeColor) -> Color { Color(.sRGB, red: color.red, green: color.green, blue: color.blue, opacity: 1) }
    var skin: ThemeSkin { roles.skin }
    var dark: Bool { roles.dark }
    var canvasNS: NSColor { Self.ns(roles.canvas) }
    var canvas: Color { Self.color(roles.canvas) }
    var surface: Color { Self.color(roles.surface) }
    var raised: Color { Self.color(roles.raised) }
    var line: Color { Self.color(roles.line).opacity(dark ? skin.lineOpacity.dark : skin.lineOpacity.light) }
    var ink: Color { Self.color(roles.ink) }
    var muted: Color { Self.color(roles.muted) }
    var accent: Color { Self.color(roles.accent) }
    var onAccent: Color { Self.color(roles.onAccent) }
    /// Text, icons, strokes and native links/checkboxes in the accent.
    var accentText: Color { Self.color(roles.accentText) }
    /// Selected row, tab or pill.
    var accentSoft: Color { Self.color(roles.accentSoft) }
    // States (§2): text and icons; never the accent.
    var danger: Color { Self.color(roles.danger) }
    var warning: Color { Self.color(roles.warning) }
    var success: Color { Self.color(roles.success) }
    var info: Color { Self.color(roles.info) }
    /// A warning banner's background.
    var warningSoft: Color { Self.color(roles.warningSoft) }

    // MARK: Shapes (§5)
    /// Buttons, fields and small selections.
    var control: AnyShape { AnyShape(RoundedRectangle(cornerRadius: skin.controlRadius, style: .continuous)) }
    /// Cards and floating panels.
    var panel: AnyShape { AnyShape(RoundedRectangle(cornerRadius: skin.panelRadius, style: .continuous)) }
    /// Navigation, steps and badges: a capsule on Cristal.
    var pill: AnyShape { skin.pillRadius >= 100 ? AnyShape(Capsule()) : AnyShape(RoundedRectangle(cornerRadius: skin.pillRadius, style: .continuous)) }

    // MARK: Type (§6): the skin picks the family, the view picks the role.
    private var displayFamily: String? {
        switch skin {
        case .glass: return nil
        case .classicMac: return "Silom"
        case .terminal: return "Menlo-Bold"
        case .win9x: return "Tahoma Bold"
        case .gothic: return "Luminari"
        }
    }
    /// Wordmark and large titles. Cristal keeps the serif nüün always had.
    func display(_ size: CGFloat, weight: Font.Weight = .semibold) -> Font {
        guard let family = displayFamily else { return .system(size: size, weight: weight, design: .serif) }
        return .custom(family, size: size)
    }
    /// Section headings at a text style, so Dynamic Type still scales them.
    func heading(_ style: Font.TextStyle, weight: Font.Weight = .semibold) -> Font {
        guard let family = displayFamily else { return .system(style, design: .serif).weight(weight) }
        let size: CGFloat = switch style { case .title: 22; case .title2: 17; case .title3: 15; default: 13 }
        return .custom(family, size: size, relativeTo: style)
    }
}

private struct PaletteKey: EnvironmentKey {
    static let defaultValue = CreatorPalette(dark: false, hue: CreatorPalette.defaultHue, saturation: CreatorPalette.defaultSaturation, brightness: 1)
}
extension EnvironmentValues {
    var creatorPalette: CreatorPalette {
        get { self[PaletteKey.self] }
        set { self[PaletteKey.self] = newValue }
    }
}

@MainActor private final class ThemeWindowProbe: NSView {
    var palette = PaletteKey.defaultValue
    var followsSystem = false
    override func viewDidMoveToWindow() { super.viewDidMoveToWindow(); apply() }
    func apply() {
        guard let window else { return }
        window.appearance = followsSystem ? nil : NSAppearance(named: palette.dark ? .darkAqua : .aqua)
        window.backgroundColor = palette.canvasNS
        window.titlebarAppearsTransparent = true
        window.isOpaque = true
        window.hasShadow = true
    }
}
private struct WindowTheme: NSViewRepresentable {
    let palette: CreatorPalette
    let followsSystem: Bool
    func makeNSView(context: Context) -> ThemeWindowProbe { let view = ThemeWindowProbe(); view.palette = palette; view.followsSystem = followsSystem; return view }
    func updateNSView(_ view: ThemeWindowProbe, context: Context) { view.palette = palette; view.followsSystem = followsSystem; view.apply() }
}

/// One filled primary per view; secondary buttons stay light so the screen reads calmly.
/// Each skin draws the button of its era with the same roles.
struct CreatorButtonStyle: ButtonStyle {
    @Environment(\.creatorPalette) private var palette
    @Environment(\.isEnabled) private var enabled
    @Environment(\.controlSize) private var size
    var prominent = false
    func makeBody(configuration: Configuration) -> some View {
        let small = size == .small || size == .mini
        let pressed = configuration.isPressed
        let destructive = configuration.role == .destructive
        let shape = palette.control
        let label = configuration.label
            .fontWeight(prominent ? .semibold : .regular)
            .padding(.horizontal, small ? 8 : 11).padding(.vertical, small ? 3 : 5)
            .contentShape(shape)
        return Group {
            switch palette.skin {
            case .classicMac:
                // Pressed inverts; the default button carries the period's heavy outer ring.
                label
                    .foregroundStyle(pressed ? Color.white : destructive ? palette.danger : (prominent ? palette.accentText : palette.ink))
                    .background(pressed ? palette.ink : palette.surface, in: shape)
                    .overlay(shape.stroke(palette.ink, lineWidth: 1))
                    .padding(prominent ? 3 : 0)
                    .overlay { if prominent { RoundedRectangle(cornerRadius: palette.skin.controlRadius + 3).stroke(palette.ink, lineWidth: 2) } }
            case .win9x:
                label
                    .foregroundStyle(destructive ? palette.danger : palette.ink)
                    .offset(x: pressed ? 1 : 0, y: pressed ? 1 : 0)
                    .background(palette.canvas)
                    .overlay(Win9xBevel(isRaised: !pressed))
                    .overlay { if prominent { Rectangle().stroke(Color.black, lineWidth: 1).padding(-1) } }
            case .gothic:
                label
                    .foregroundStyle(destructive ? palette.danger : (prominent ? palette.onAccent : palette.ink))
                    .background(prominent ? palette.accent : (pressed ? palette.raised : palette.surface), in: shape)
                    .overlay(shape.stroke(prominent ? palette.ink.opacity(0.35) : palette.line, lineWidth: 1))
                    .overlay(RoundedRectangle(cornerRadius: palette.skin.controlRadius).inset(by: 2).stroke(palette.line.opacity(prominent ? 0 : 0.35), lineWidth: 0.5))
                    .opacity(pressed ? 0.8 : 1)
            case .terminal:
                // Inverse video: the primary and a pressed button are lit with the signal.
                let lit = prominent || pressed
                label
                    .foregroundStyle(destructive ? palette.danger : (lit ? palette.onAccent : palette.ink))
                    .background(lit ? palette.accent : palette.canvas, in: shape)
                    .overlay(shape.stroke(prominent ? palette.accent : palette.line, lineWidth: 1))
            case .glass:
                label
                    .foregroundStyle(destructive ? palette.danger : (prominent ? palette.onAccent : palette.ink))
                    .background(prominent ? palette.accent : (pressed ? palette.raised : palette.surface.opacity(0.75)), in: shape)
                    .overlay(shape.stroke(palette.line.opacity(prominent ? 0.9 : 0.5), lineWidth: 1))
                    .opacity(pressed ? 0.8 : 1)
            }
        }
        .opacity(enabled ? 1 : 0.4)
    }
}

struct CreatorTextFieldStyle: TextFieldStyle {
    @Environment(\.creatorPalette) private var palette
    func _body(configuration: TextField<Self._Label>) -> some View {
        let field = configuration.textFieldStyle(.plain).padding(.horizontal, 8).padding(.vertical, 5)
        return Group {
            if palette.skin == .win9x {
                field.background(Color.white).overlay(Win9xBevel(isRaised: false))
            } else {
                field.background(palette.skin == .glass ? palette.surface : palette.raised, in: palette.control)
                    .overlay(palette.control.stroke(palette.line, lineWidth: 1))
            }
        }
    }
}

/// Floating controls: Liquid Glass on Cristal (macOS 26), and each period skin's own surface.
struct GlassSurface<S: Shape>: ViewModifier {
    let shape: S
    /// Glass is composited by the window server; offscreen QA renders draw the fallback instead.
    static var offscreenQA: Bool {
        #if DEBUG
        CommandLine.arguments.contains("--qa-snapshots") || CommandLine.arguments.contains("--qa-disks") || CommandLine.arguments.contains("--qa-finals") || CommandLine.arguments.contains("--qa-closeout") || CommandLine.arguments.contains("--qa-simple") || CommandLine.arguments.contains("--qa-skins")
        #else
        false
        #endif
    }
    @Environment(\.creatorPalette) private var palette
    func body(content: Content) -> some View {
        switch palette.skin {
        case .glass:
            if #available(macOS 26.0, *), !GlassSurface<Rectangle>.offscreenQA {
                content.glassEffect(.regular.tint(palette.canvas.opacity(0.35)), in: shape)
            } else {
                content.background(palette.surface.opacity(0.95), in: shape).overlay(shape.stroke(palette.line.opacity(0.5)))
            }
        case .terminal:
            content.background(palette.surface, in: shape).overlay(shape.stroke(palette.line, lineWidth: 1))
        case .classicMac:
            // A 1-bit window: white, black rule and a hard drop shadow.
            content.background(palette.surface, in: shape).overlay(shape.stroke(palette.ink, lineWidth: 1))
                .background(shape.fill(palette.ink).offset(x: 2, y: 2))
        case .win9x:
            content.background(palette.canvas).overlay(Win9xBevel())
        case .gothic:
            content.background(palette.surface, in: shape).overlay(shape.stroke(palette.line, lineWidth: 1))
                .overlay(shape.stroke(palette.accent.opacity(0.35), lineWidth: 0.5).padding(2))
        }
    }
}

/// Lists sit directly on the canvas; only fields and floating bars are raised.
struct CreatorListSurface: ViewModifier {
    func body(content: Content) -> some View { content.scrollContentBackground(.hidden).alternatingRowBackgrounds(.disabled) }
}

struct CreatorAppearance: ViewModifier {
    @Environment(\.colorScheme) private var systemScheme
    @AppStorage("appearance") private var appearance = "light"
    @AppStorage("skin") private var skinID = ThemeSkin.glass.rawValue
    @AppStorage("accentHue") private var hue = CreatorPalette.defaultHue
    @AppStorage("accentSaturation") private var saturation = CreatorPalette.defaultSaturation
    @AppStorage("accentBrightness") private var brightness = 1.0
    func body(content: Content) -> some View {
        let skin = ThemeSkin(rawValue: skinID) ?? .glass
        let requested = appearance == "dark" || (appearance == "system" && systemScheme == .dark)
        let palette = CreatorPalette(skin: skin, dark: requested, hue: hue, saturation: saturation, brightness: brightness)
        let followsSystem = appearance == "system" && skin.forcedDark == nil
        content
            .buttonStyle(CreatorButtonStyle())
            .textFieldStyle(CreatorTextFieldStyle())
            .tint(palette.accentText)
            .foregroundStyle(palette.ink)
            // Terminal sets every system text style in the monospaced design, not only headings.
            .fontDesign(skin == .terminal ? .monospaced : nil)
            .overlay { if skin == .terminal { Scanlines().ignoresSafeArea() } }
            .background(SkinAtmosphere().ignoresSafeArea())
            .background(palette.canvas.ignoresSafeArea())
            .background(WindowTheme(palette: palette, followsSystem: followsSystem))
            .toolbarBackground(palette.canvas, for: .windowToolbar)
            .environment(\.creatorPalette, palette)
            .preferredColorScheme(followsSystem ? nil : palette.dark ? .dark : .light)
    }
}

struct AppearanceSettings: View {
    @Environment(\.creatorPalette) private var palette
    @AppStorage("appearance") private var appearance = "light"
    @AppStorage("skin") private var skinID = ThemeSkin.glass.rawValue
    @AppStorage("accentHue") private var hue = CreatorPalette.defaultHue
    @AppStorage("accentSaturation") private var saturation = CreatorPalette.defaultSaturation
    @AppStorage("accentBrightness") private var brightness = 1.0
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Label("Apariencia", systemImage: "paintpalette").font(.title3.weight(.semibold))
            Picker("Tema", selection: $appearance) { Text("Claro").tag("light"); Text("Oscuro").tag("dark"); Text("Sistema").tag("system") }
                .pickerStyle(.segmented).fixedSize()
                .disabled(palette.skin.forcedDark != nil)
            if let forced = palette.skin.forcedDark {
                Text(forced ? "\(palette.skin.name) siempre es oscuro; tu tema vuelve al elegir otro skin." : "\(palette.skin.name) siempre es claro; tu tema vuelve al elegir otro skin.")
                    .font(.caption).foregroundStyle(palette.muted)
            }
            Divider()
            Text("El color base coordina fondo, superficies, controles y barra de título.").font(.caption).foregroundStyle(palette.muted)
            HStack(spacing: 8) {
                ForEach(ThemePreset.all) { preset in
                    let selected = palette.roles.seed.hex == preset.hex
                    Button { apply(preset) } label: {
                        HStack(spacing: 6) {
                            RoundedRectangle(cornerRadius: 4).fill(Color(.sRGB, red: preset.color.red, green: preset.color.green, blue: preset.color.blue))
                                .frame(width: 16, height: 16)
                                .overlay(RoundedRectangle(cornerRadius: 4).stroke(palette.line, lineWidth: 1))
                            Text(verbatim: preset.name)
                        }
                        .padding(.horizontal, 8).padding(.vertical, 4)
                        .background(selected ? palette.accentSoft : .clear, in: palette.control)
                        .contentShape(palette.control)
                    }
                    .buttonStyle(.plain)
                    .help(Text(verbatim: preset.hex))
                    .accessibilityAddTraits(selected ? .isSelected : [])
                }
            }
            HStack { Text("Matiz (H)").frame(width: 105, alignment: .leading); Slider(value: $hue, in: 0...1) }
            HStack { Text("Saturación (S)").frame(width: 105, alignment: .leading); Slider(value: $saturation, in: 0...1) }
            HStack { Text("Brillo (V)").frame(width: 105, alignment: .leading); Slider(value: $brightness, in: 0.15...1) }
            HStack {
                palette.control.fill(palette.accent).frame(width: 32, height: 24)
                Button("Restaurar claro · #FFBB5C") { appearance = "light"; skinID = ThemeSkin.glass.rawValue; hue = CreatorPalette.defaultHue; saturation = CreatorPalette.defaultSaturation; brightness = 1 }
            }
            Divider()
            SkinPicker(skinID: $skinID, hue: hue, saturation: saturation, brightness: brightness, dark: palette.dark)
        }
    }
    /// A preset only moves the color; light/dark stays the user's choice.
    private func apply(_ preset: ThemePreset) {
        let hsv = preset.color.hsv
        hue = hsv.hue; saturation = hsv.saturation; brightness = max(0.15, hsv.value)
    }
}

/// Optional skins for people who like to set up their space. Each card is drawn with its own skin.
private struct SkinPicker: View {
    @Environment(\.creatorPalette) private var palette
    @Binding var skinID: String
    let hue: Double, saturation: Double, brightness: Double, dark: Bool
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Skin").font(.headline)
            Text("Cristal es el de todos los días. Los demás cambian la ventana completa: barra, controles, botones y letra de títulos.")
                .font(.caption).foregroundStyle(palette.muted).fixedSize(horizontal: false, vertical: true)
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 150), spacing: 10)], alignment: .leading, spacing: 10) {
                ForEach(ThemeSkin.allCases) { skin in
                    let selected = skin.rawValue == skinID
                    Button { skinID = skin.rawValue } label: { SkinCard(skin: skin, palette: CreatorPalette(skin: skin, dark: dark, hue: hue, saturation: saturation, brightness: brightness), selected: selected) }
                        .buttonStyle(.plain)
                        .help(Text(verbatim: skin.summary))
                        .accessibilityLabel(Text(verbatim: skin.name))
                        .accessibilityHint(Text(verbatim: skin.summary))
                        .accessibilityAddTraits(selected ? .isSelected : [])
                }
            }
        }
    }
}

private struct SkinCard: View {
    @Environment(\.creatorPalette) private var current
    let skin: ThemeSkin
    let palette: CreatorPalette
    let selected: Bool
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            SkinBarBackground(skin: skin).frame(height: 14).environment(\.creatorPalette, palette)
            VStack(alignment: .leading, spacing: 6) {
                Text(verbatim: skin.name).font(palette.display(15)).foregroundStyle(palette.ink).lineLimit(1)
                HStack(spacing: 5) {
                    Text(verbatim: "Aa").font(.caption2.weight(.semibold)).foregroundStyle(palette.onAccent)
                        .padding(.horizontal, 6).padding(.vertical, 2).background(palette.accent, in: palette.pill)
                    Text(verbatim: "Aa").font(.caption2).foregroundStyle(palette.ink)
                        .padding(.horizontal, 6).padding(.vertical, 2).background(palette.accentSoft, in: palette.pill)
                    Text(verbatim: "Aa").font(.caption2).foregroundStyle(palette.muted)
                }
            }
            .padding(8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(SkinAtmosphere().environment(\.creatorPalette, palette))
            .background(palette.canvas)
        }
        .clipShape(palette.panel)
        .overlay(palette.panel.stroke(selected ? current.accentText : current.line, lineWidth: selected ? 2.5 : 1))
        .contentShape(Rectangle())
    }
}
