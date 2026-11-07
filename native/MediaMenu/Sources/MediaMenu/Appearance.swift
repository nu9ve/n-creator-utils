import AppKit
import SwiftUI
import MediaMenuCore

/// nu9ve-theme-v1 §2: the common roles, resolved by `ThemeRoles` (pure, tested in Core).
/// Backgrounds and window chrome derive from the same seed.
struct CreatorPalette: Sendable {
    static let defaultHue = ThemeRoles.defaultHue
    static let defaultSaturation = ThemeRoles.defaultSaturation
    let roles: ThemeRoles
    init(dark: Bool, hue: Double, saturation: Double, brightness: Double) {
        roles = ThemeRoles(dark: dark, hue: hue, saturation: saturation, brightness: brightness)
    }
    private static func ns(_ color: ThemeColor) -> NSColor { NSColor(srgbRed: color.red, green: color.green, blue: color.blue, alpha: 1) }
    private static func color(_ color: ThemeColor) -> Color { Color(.sRGB, red: color.red, green: color.green, blue: color.blue, opacity: 1) }
    var dark: Bool { roles.dark }
    var canvasNS: NSColor { Self.ns(roles.canvas) }
    var canvas: Color { Self.color(roles.canvas) }
    var surface: Color { Self.color(roles.surface) }
    var raised: Color { Self.color(roles.raised) }
    var line: Color { Self.color(roles.line).opacity(dark ? 0.65 : 0.55) }
    var ink: Color { Self.color(roles.ink) }
    var muted: Color { Self.color(roles.muted) }
    var accent: Color { Self.color(roles.accent) }
    var onAccent: Color { Self.color(roles.onAccent) }
    /// Text, icons, strokes and native links/checkboxes in the accent.
    var accentText: Color { Self.color(roles.accentText) }
    /// Selected row, tab or pill.
    var accentSoft: Color { Self.color(roles.accentSoft) }
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
struct CreatorButtonStyle: ButtonStyle {
    @Environment(\.creatorPalette) private var palette
    @Environment(\.isEnabled) private var enabled
    @Environment(\.controlSize) private var size
    var prominent = false
    func makeBody(configuration: Configuration) -> some View {
        let small = size == .small || size == .mini
        configuration.label
            .fontWeight(prominent ? .semibold : .regular)
            .padding(.horizontal, small ? 8 : 11).padding(.vertical, small ? 3 : 5)
            .foregroundStyle(configuration.role == .destructive ? Color.red : (prominent ? palette.onAccent : palette.ink))
            .background(prominent ? palette.accent : (configuration.isPressed ? palette.raised : palette.surface.opacity(0.75)), in: RoundedRectangle(cornerRadius: 7))
            .overlay(RoundedRectangle(cornerRadius: 7).stroke(palette.line.opacity(prominent ? 0.9 : 0.5), lineWidth: 1))
            .opacity(enabled ? (configuration.isPressed ? 0.8 : 1) : 0.4)
            .contentShape(RoundedRectangle(cornerRadius: 7))
    }
}

struct CreatorTextFieldStyle: TextFieldStyle {
    @Environment(\.creatorPalette) private var palette
    func _body(configuration: TextField<Self._Label>) -> some View {
        configuration.textFieldStyle(.plain).padding(.horizontal, 8).padding(.vertical, 5)
            .background(palette.surface, in: RoundedRectangle(cornerRadius: 7))
            .overlay(RoundedRectangle(cornerRadius: 7).stroke(palette.line, lineWidth: 1))
    }
}

/// Liquid Glass on macOS 26 for floating controls; the same shape stays opaque on earlier systems.
struct GlassSurface<S: Shape>: ViewModifier {
    let shape: S
    /// Glass is composited by the window server; offscreen QA renders draw the fallback instead.
    static var offscreenQA: Bool {
        #if DEBUG
        CommandLine.arguments.contains("--qa-snapshots") || CommandLine.arguments.contains("--qa-disks") || CommandLine.arguments.contains("--qa-finals") || CommandLine.arguments.contains("--qa-closeout") || CommandLine.arguments.contains("--qa-simple")
        #else
        false
        #endif
    }
    @Environment(\.creatorPalette) private var palette
    func body(content: Content) -> some View {
        if #available(macOS 26.0, *), !GlassSurface<Rectangle>.offscreenQA {
            content.glassEffect(.regular.tint(palette.canvas.opacity(0.35)), in: shape)
        } else {
            content.background(palette.surface.opacity(0.95), in: shape).overlay(shape.stroke(palette.line.opacity(0.5)))
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
    @AppStorage("accentHue") private var hue = CreatorPalette.defaultHue
    @AppStorage("accentSaturation") private var saturation = CreatorPalette.defaultSaturation
    @AppStorage("accentBrightness") private var brightness = 1.0
    func body(content: Content) -> some View {
        let dark = appearance == "dark" || (appearance == "system" && systemScheme == .dark)
        let palette = CreatorPalette(dark: dark, hue: hue, saturation: saturation, brightness: brightness)
        content
            .buttonStyle(CreatorButtonStyle())
            .textFieldStyle(CreatorTextFieldStyle())
            .tint(palette.accentText)
            .foregroundStyle(palette.ink)
            .background(palette.canvas.ignoresSafeArea())
            .background(WindowTheme(palette: palette, followsSystem: appearance == "system"))
            .toolbarBackground(palette.canvas, for: .windowToolbar)
            .environment(\.creatorPalette, palette)
            .preferredColorScheme(appearance == "system" ? nil : dark ? .dark : .light)
    }
}

struct AppearanceSettings: View {
    @Environment(\.creatorPalette) private var palette
    @AppStorage("appearance") private var appearance = "light"
    @AppStorage("accentHue") private var hue = CreatorPalette.defaultHue
    @AppStorage("accentSaturation") private var saturation = CreatorPalette.defaultSaturation
    @AppStorage("accentBrightness") private var brightness = 1.0
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Label("Apariencia", systemImage: "paintpalette").font(.title3.weight(.semibold))
            Picker("Tema", selection: $appearance) { Text("Claro").tag("light"); Text("Oscuro").tag("dark"); Text("Sistema").tag("system") }
                .pickerStyle(.segmented).fixedSize()
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
                        .background(selected ? palette.accentSoft : .clear, in: RoundedRectangle(cornerRadius: 7))
                        .contentShape(RoundedRectangle(cornerRadius: 7))
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
                RoundedRectangle(cornerRadius: 6).fill(palette.accent).frame(width: 32, height: 24)
                Button("Restaurar claro · #FFBB5C") { appearance = "light"; hue = CreatorPalette.defaultHue; saturation = CreatorPalette.defaultSaturation; brightness = 1 }
            }
        }
    }
    /// A preset only moves the color; light/dark stays the user's choice.
    private func apply(_ preset: ThemePreset) {
        let hsv = preset.color.hsv
        hue = hsv.hue; saturation = hsv.saturation; brightness = max(0.15, hsv.value)
    }
}
