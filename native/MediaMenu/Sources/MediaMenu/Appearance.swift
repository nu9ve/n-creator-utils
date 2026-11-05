import AppKit
import SwiftUI

/// Shared surface palette: backgrounds and window chrome derive from the same seed.
struct CreatorPalette: Sendable {
    static let defaultHue = 95.0 / (163.0 * 6.0)
    static let defaultSaturation = 163.0 / 255.0
    let dark: Bool
    let hue: Double
    let saturation: Double
    let brightness: Double
    var chosen: NSColor {
        let h = (hue == 1 ? 0 : hue) * 6, sector = Int(h)
        let f = h - Double(sector), p = brightness * (1 - saturation)
        let q = brightness * (1 - saturation * f), t = brightness * (1 - saturation * (1 - f))
        let rgb = [(brightness,t,p),(q,brightness,p),(p,brightness,t),(p,q,brightness),(t,p,brightness),(brightness,p,q)][max(0, min(5, sector))]
        return NSColor(srgbRed: rgb.0, green: rgb.1, blue: rgb.2, alpha: 1)
    }
    func blend(_ fraction: CGFloat) -> NSColor { chosen.blended(withFraction: fraction, of: dark ? .black : .white)! }
    var canvasNS: NSColor { blend(dark ? 0.91 : 0.80) }
    var canvas: Color { Color(nsColor: canvasNS) }
    var card: Color { Color(nsColor: blend(dark ? 0.85 : 0.94)) }
    var inset: Color { Color(nsColor: blend(dark ? 0.88 : 0.88)) }
    var border: Color { Color(nsColor: blend(dark ? 0.64 : 0.57)).opacity(dark ? 0.65 : 0.55) }
    var accent: Color { Color(nsColor: chosen) }
    var accentInk: Color {
        let rgb = chosen.usingColorSpace(.sRGB)!
        func linear(_ v: CGFloat) -> Double { let n = Double(v); return n <= 0.04045 ? n / 12.92 : pow((n + 0.055) / 1.055, 2.4) }
        let luminance = 0.2126 * linear(rgb.redComponent) + 0.7152 * linear(rgb.greenComponent) + 0.0722 * linear(rgb.blueComponent)
        return luminance > 0.179 ? .black : .white
    }
    var ink: Color { dark ? Color(white: 0.95) : Color(nsColor: chosen.blended(withFraction: 0.86, of: .black)!) }
    var muted: Color { ink.opacity(0.68) }
    // Native link/checkbox colors need more contrast than a pale orange seed.
    var control: Color { dark ? accent : Color(nsColor: chosen.blended(withFraction: 0.48, of: .black)!) }
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
            .foregroundStyle(configuration.role == .destructive ? Color.red : (prominent ? palette.accentInk : palette.ink))
            .background(prominent ? palette.accent : (configuration.isPressed ? palette.inset : palette.card.opacity(0.75)), in: RoundedRectangle(cornerRadius: 7))
            .overlay(RoundedRectangle(cornerRadius: 7).stroke(palette.border.opacity(prominent ? 0.9 : 0.5), lineWidth: 1))
            .opacity(enabled ? (configuration.isPressed ? 0.8 : 1) : 0.4)
            .contentShape(RoundedRectangle(cornerRadius: 7))
    }
}

struct CreatorTextFieldStyle: TextFieldStyle {
    @Environment(\.creatorPalette) private var palette
    func _body(configuration: TextField<Self._Label>) -> some View {
        configuration.textFieldStyle(.plain).padding(.horizontal, 8).padding(.vertical, 5)
            .background(palette.card, in: RoundedRectangle(cornerRadius: 7))
            .overlay(RoundedRectangle(cornerRadius: 7).stroke(palette.border, lineWidth: 1))
    }
}

/// Liquid Glass on macOS 26 for floating controls; the same shape stays opaque on earlier systems.
struct GlassSurface<S: Shape>: ViewModifier {
    let shape: S
    /// Glass is composited by the window server; offscreen QA renders draw the fallback instead.
    static var offscreenQA: Bool {
        #if DEBUG
        CommandLine.arguments.contains("--qa-snapshots") || CommandLine.arguments.contains("--qa-disks") || CommandLine.arguments.contains("--qa-finals") || CommandLine.arguments.contains("--qa-closeout")
        #else
        false
        #endif
    }
    @Environment(\.creatorPalette) private var palette
    func body(content: Content) -> some View {
        if #available(macOS 26.0, *), !GlassSurface<Rectangle>.offscreenQA {
            content.glassEffect(.regular.tint(palette.canvas.opacity(0.35)), in: shape)
        } else {
            content.background(palette.card.opacity(0.95), in: shape).overlay(shape.stroke(palette.border.opacity(0.5)))
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
            .tint(palette.control)
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
            HStack { Text("Matiz (H)").frame(width: 105, alignment: .leading); Slider(value: $hue, in: 0...1) }
            HStack { Text("Saturación (S)").frame(width: 105, alignment: .leading); Slider(value: $saturation, in: 0...1) }
            HStack { Text("Brillo (V)").frame(width: 105, alignment: .leading); Slider(value: $brightness, in: 0.15...1) }
            HStack {
                RoundedRectangle(cornerRadius: 6).fill(palette.accent).frame(width: 32, height: 24)
                Button("Restaurar claro · #FFBB5C") { appearance = "light"; hue = CreatorPalette.defaultHue; saturation = CreatorPalette.defaultSaturation; brightness = 1 }
            }
        }
    }
}
