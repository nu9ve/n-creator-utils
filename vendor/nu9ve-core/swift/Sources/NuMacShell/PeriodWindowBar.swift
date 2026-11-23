#if os(macOS)
import AppKit
import Nu9veTheme
import SwiftUI

// nu9ve-theme-v1 §5 and §8: the bar of the three commons (Macintosh, Windows 9x and Terminal),
// the same in every app. Only the palette changes; the app passes it as `PeriodBarColors`.
// The square window itself is `ShellWindow`, the drag `TitleBarDragArea`.

/// One of the three commons, the skins that draw a period window.
public enum PeriodEra: String, CaseIterable, Sendable {
    case classicMac, win9x, terminal

    /// Nil for a skin that is not a common: the app draws that one itself.
    public init?(_ skin: ThemeSkin) {
        switch skin {
        case .classicMac: self = .classicMac
        case .win9x: self = .win9x
        case .terminal: self = .terminal
        default: return nil
        }
    }

    public var skin: ThemeSkin {
        switch self {
        case .classicMac: return .classicMac
        case .win9x: return .win9x
        case .terminal: return .terminal
        }
    }

    /// The caption with the title and the controls; 0 for Terminal, whose row carries them.
    public var captionHeight: CGFloat { CGFloat(skin.captionHeight) }
    /// The row with the app's own controls.
    public var rowHeight: CGFloat { CGFloat(skin.barHeight) }
    /// Everything the bar takes above the content.
    public var height: CGFloat { captionHeight + rowHeight }
    /// The margin of the controls from the window's edge.
    public static let inset: CGFloat = 8
}

/// The colors the bar takes from the app's palette. The rest belongs to the era.
public struct PeriodBarColors: Equatable {
    /// Terminal's row and the row under a caption.
    public var canvas: Color
    /// Terminal's row background.
    public var surface: Color
    /// Text and the era's rules.
    public var ink: Color
    /// Terminal's close sign and prompt rule.
    public var accent: Color
    /// The two ends of the Windows 9x caption.
    public var caption: [Color]

    public init(canvas: Color, surface: Color, ink: Color, accent: Color, caption: [Color]) {
        self.canvas = canvas
        self.surface = surface
        self.ink = ink
        self.accent = accent
        self.caption = caption
    }

    /// From the common roles: canvas, surface, ink, `accentText` and the caption from the seed.
    public init(_ roles: ThemeRoles) {
        self.init(
            canvas: Self.color(roles.canvas),
            surface: Self.color(roles.surface),
            ink: Self.color(roles.ink),
            accent: Self.color(roles.accentText),
            caption: Self.win9xCaption(seed: roles.seed)
        )
    }

    /// Like Windows 98's appearance settings, the caption takes the user's color, dark enough
    /// for white text.
    public static func win9xCaption(seed: ThemeColor) -> [Color] {
        let white = ThemeColor.white
        return [
            color(seed.mixed(with: .black, amount: 0.55).readable(on: [white], minimum: 4.5)),
            color(seed.mixed(with: .black, amount: 0.15).readable(on: [white], minimum: 4.5))
        ]
    }

    static func color(_ c: ThemeColor) -> Color {
        Color(.sRGB, red: c.red, green: c.green, blue: c.blue, opacity: 1)
    }
}

/// Close, minimize and zoom, in the order and place of macOS (nu9ve-theme-v1 §8).
public enum PeriodWindowControl: CaseIterable, Sendable {
    case close, minimize, zoom

    /// What VoiceOver reads. Also the key in the app's own strings, for its translations.
    public var label: String {
        switch self {
        case .close: return "Cerrar ventana"
        case .minimize: return "Minimizar ventana"
        case .zoom: return "Ampliar ventana"
        }
    }

    public var help: String {
        switch self {
        case .close: return "Cerrar ventana · ⌘W"
        case .minimize: return "Minimizar ventana · ⌘M"
        case .zoom: return "Ampliar ventana · con Opción: pantalla completa"
        }
    }

    /// The same signs as the system's buttons, always visible.
    public var sign: String {
        switch self {
        case .close: return "×"
        case .minimize: return "−"
        case .zoom: return "+"
        }
    }

    var symbol: String {
        switch self {
        case .close: return "xmark"
        case .minimize: return "minus"
        case .zoom: return "plus"
        }
    }

    /// A zoom box zooms; with Option, or already in fullscreen, it toggles fullscreen.
    @MainActor public func perform(on window: NSWindow, option: Bool) {
        switch self {
        case .close: window.performClose(nil)
        case .minimize: window.performMiniaturize(nil)
        case .zoom:
            if option || window.styleMask.contains(.fullScreen) { window.toggleFullScreen(nil) } else { window.performZoom(nil) }
        }
    }
}

/// Lets a SwiftUI view reach its window without holding it.
@MainActor public final class WindowHandle {
    public weak var window: NSWindow?
    public init() {}
}

/// Put it in a view's background: it keeps `handle.window` pointing at that view's window.
public struct WindowHandleReader: NSViewRepresentable {
    let handle: WindowHandle

    public init(handle: WindowHandle) {
        self.handle = handle
    }

    public final class Probe: NSView {
        var handle: WindowHandle?
        override public func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            handle?.window = window
        }
    }

    public func makeNSView(context: Context) -> Probe {
        let view = Probe()
        view.handle = handle
        return view
    }

    public func updateNSView(_ view: Probe, context: Context) {
        view.handle = handle
        handle.window = view.window
    }
}

/// × − + drawn by the era. Without `window`, the controls act on the window they are in.
public struct PeriodWindowControls: View {
    let era: PeriodEra
    let colors: PeriodBarColors
    let zoomEnabled: Bool
    let window: (@MainActor () -> NSWindow?)?
    @State private var handle = WindowHandle()

    /// A window people cannot resize passes `zoomEnabled` false: + stays visible and dimmed, as the
    /// system dims its button. `window` is read on each click, so the controls never hold it.
    public init(
        era: PeriodEra,
        colors: PeriodBarColors,
        zoomEnabled: Bool = true,
        window: (@MainActor () -> NSWindow?)? = nil
    ) {
        self.era = era
        self.colors = colors
        self.zoomEnabled = zoomEnabled
        self.window = window
    }

    public var body: some View {
        HStack(spacing: era == .win9x ? 2 : 5) {
            ForEach(PeriodWindowControl.allCases, id: \.self) { control in
                let enabled = control != .zoom || zoomEnabled
                Button {
                    guard let target = window?() ?? handle.window else { return }
                    control.perform(on: target, option: NSEvent.modifierFlags.contains(.option))
                } label: {
                    glyph(control).contentShape(Rectangle()).opacity(enabled ? 1 : 0.4)
                }
                .buttonStyle(.plain)
                .disabled(!enabled)
                // Keys in the app's own strings: an app with translations keeps them.
                .accessibilityLabel(Text(LocalizedStringKey(control.label)))
                .help(Text(LocalizedStringKey(control.help)))
            }
        }
        .background(WindowHandleReader(handle: handle))
    }

    @ViewBuilder private func glyph(_ control: PeriodWindowControl) -> some View {
        let image = Image(systemName: control.symbol).font(.system(size: 8, weight: .black))
        switch era {
        case .terminal:
            // The same three signs, in brackets as a prompt would print them.
            Text(verbatim: "[\(control.sign)]")
                .font(.system(size: 12, weight: .bold, design: .monospaced))
                .foregroundStyle(control == .close ? colors.accent : colors.ink)
        case .win9x:
            image.foregroundStyle(.black)
                .frame(width: 17, height: 15)
                .background(Win9xBevel.face)
                .overlay(Win9xBevel())
        case .classicMac:
            image.foregroundStyle(.black)
                .frame(width: 13, height: 13)
                .background(Color.white)
                .overlay(Rectangle().strokeBorder(.black, lineWidth: 1))
                .padding(.horizontal, 2)
                .background(Color.white)
        }
    }
}

/// The caption's background, or Terminal's row: pinstripes, the caption gradient, or the surface
/// with the prompt rule. Also a skin card's preview strip in Ajustes.
public struct PeriodBarBackground: View {
    let era: PeriodEra
    let colors: PeriodBarColors

    public init(era: PeriodEra, colors: PeriodBarColors) {
        self.era = era
        self.colors = colors
    }

    public var body: some View {
        Group {
            switch era {
            case .classicMac:
                ZStack { Color.white; ClassicPinstripes() }
                    .overlay(alignment: .bottom) { Rectangle().fill(.black).frame(height: 1) }
            case .win9x:
                LinearGradient(colors: colors.caption, startPoint: .leading, endPoint: .trailing)
            case .terminal:
                colors.surface.overlay(alignment: .bottom) { Rectangle().fill(colors.accent).frame(height: 2) }
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// The app's row under a caption: the window's canvas with the era's rule below. Terminal has no
/// caption, so its row is `PeriodBarBackground`.
public struct PeriodRowBackground: View {
    let era: PeriodEra
    let colors: PeriodBarColors

    public init(era: PeriodEra, colors: PeriodBarColors) {
        self.era = era
        self.colors = colors
    }

    public var body: some View {
        Group {
            switch era {
            case .terminal:
                PeriodBarBackground(era: era, colors: colors)
            case .win9x:
                colors.canvas.overlay(alignment: .bottom) { Win9xBevel(isRaised: false).frame(height: 2) }
            case .classicMac:
                colors.canvas.overlay(alignment: .bottom) { Rectangle().fill(colors.ink).frame(height: 1) }
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// The period caption: its background, × − + at the left and the title where the era puts it.
/// Terminal has none; `PeriodWindowBar` puts its controls in the row.
public struct PeriodCaption: View {
    let title: String
    let era: PeriodEra
    let colors: PeriodBarColors
    let controls: PeriodWindowControls

    public init(title: String, era: PeriodEra, colors: PeriodBarColors, controls: PeriodWindowControls) {
        self.title = title
        self.era = era
        self.colors = colors
        self.controls = controls
    }

    public var body: some View {
        ZStack {
            PeriodBarBackground(era: era, colors: colors)
            HStack(spacing: 6) {
                controls
                if era == .win9x {
                    Text(verbatim: title).font(.custom("Tahoma Bold", size: 12)).foregroundStyle(.white).lineLimit(1)
                        .padding(.leading, 4).allowsHitTesting(false)
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, PeriodEra.inset)
            if era == .classicMac {
                Text(verbatim: title).font(.custom("Silom", size: 12)).foregroundStyle(.black).lineLimit(1)
                    .padding(.horizontal, 8).background(Color.white)
                    .frame(maxWidth: 420)
                    .allowsHitTesting(false)
            }
        }
        .background(TitleBarDragArea())
        .accessibilityElement(children: .contain)
    }
}

/// A window's whole bar with a common: the caption (title and × − +), then the app's `row` in a
/// row of its own. Terminal has no caption, so × − + open the row. Drawn also in fullscreen, so the
/// controls stay. Keep its height (`era.height`, or `era.captionHeight` without a row) above the content.
///
/// The row gets no margins: the app pads its own content. Under a caption it starts at the window's
/// edge; in Terminal it follows the controls.
public struct PeriodWindowBar<Row: View>: View {
    let title: String
    let era: PeriodEra
    let colors: PeriodBarColors
    let zoomEnabled: Bool
    let showsRow: Bool
    let drawsRowBackground: Bool
    let rowSpacing: CGFloat
    let window: (@MainActor () -> NSWindow?)?
    let row: Row

    /// `showsRow` false leaves only the caption, for windows without global actions (About, Help).
    /// In Terminal the row stays, with the controls and `title`. `drawsRowBackground` false leaves the
    /// row's background to `row`.
    public init(
        title: String,
        era: PeriodEra,
        colors: PeriodBarColors,
        zoomEnabled: Bool = true,
        showsRow: Bool = true,
        drawsRowBackground: Bool = true,
        rowSpacing: CGFloat = 6,
        window: (@MainActor () -> NSWindow?)? = nil,
        @ViewBuilder row: () -> Row
    ) {
        self.title = title
        self.era = era
        self.colors = colors
        self.zoomEnabled = zoomEnabled
        self.showsRow = showsRow
        self.drawsRowBackground = drawsRowBackground
        self.rowSpacing = rowSpacing
        self.window = window
        self.row = row()
    }

    public var body: some View {
        let controls = PeriodWindowControls(era: era, colors: colors, zoomEnabled: zoomEnabled, window: window)
        VStack(spacing: 0) {
            if era.captionHeight > 0 {
                PeriodCaption(title: title, era: era, colors: colors, controls: controls).frame(height: era.captionHeight)
                if showsRow { rowView { row } }
            } else {
                rowView {
                    controls.padding(.leading, PeriodEra.inset)
                    if showsRow {
                        row
                    } else {
                        Text(verbatim: title).font(.system(size: 12, weight: .bold, design: .monospaced))
                            .foregroundStyle(colors.ink).lineLimit(1).allowsHitTesting(false)
                        Spacer(minLength: 0)
                    }
                }
            }
        }
    }

    private func rowView<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        HStack(spacing: rowSpacing) { content() }
            .frame(height: era.rowHeight)
            .background {
                if drawsRowBackground { PeriodRowBackground(era: era, colors: colors) }
            }
            .background(TitleBarDragArea())
    }
}

/// Windows 9x: a raised or sunken two-pixel bevel. Only a drawing.
public struct Win9xBevel: View {
    var isRaised: Bool
    /// The silver of a button's face.
    public static let face = Color(.sRGB, red: 192 / 255, green: 192 / 255, blue: 192 / 255)
    public static let highlight = Color.white
    public static let lightShadow = Color(.sRGB, red: 128 / 255, green: 128 / 255, blue: 128 / 255)
    public static let darkShadow = Color(.sRGB, red: 10 / 255, green: 10 / 255, blue: 10 / 255)

    public init(isRaised: Bool = true) {
        self.isRaised = isRaised
    }

    public var body: some View {
        GeometryReader { geometry in
            let size = geometry.size
            Path { path in
                path.addRect(CGRect(x: 0, y: 0, width: size.width, height: 1))
                path.addRect(CGRect(x: 0, y: 0, width: 1, height: size.height))
            }.fill(isRaised ? Self.highlight : Self.darkShadow)
            Path { path in
                path.addRect(CGRect(x: 0, y: size.height - 1, width: size.width, height: 1))
                path.addRect(CGRect(x: size.width - 1, y: 0, width: 1, height: size.height))
            }.fill(isRaised ? Self.darkShadow : Self.highlight)
            Path { path in
                path.addRect(CGRect(x: 1, y: size.height - 2, width: max(0, size.width - 2), height: 1))
                path.addRect(CGRect(x: size.width - 2, y: 1, width: 1, height: max(0, size.height - 2)))
            }.fill(Self.lightShadow)
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// The horizontal rules of a classic Macintosh title bar.
public struct ClassicPinstripes: View {
    var color: Color

    public init(color: Color = .black) {
        self.color = color
    }

    public var body: some View {
        Canvas { context, size in
            var y: CGFloat = 3
            while y < size.height - 2 {
                context.fill(Path(CGRect(x: 0, y: y, width: size.width, height: 1)), with: .color(color))
                y += 2
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// Where to grab a resizable window, as its era drew it, for its bottom right corner. Only a
/// drawing: the window's edges take the clicks. Terminal draws none.
public struct PeriodGrowBox: View {
    let era: PeriodEra

    public init(era: PeriodEra) {
        self.era = era
    }

    public var body: some View {
        Group {
            switch era {
            case .classicMac:
                ZStack(alignment: .topLeading) {
                    Rectangle().fill(.white)
                    Rectangle().strokeBorder(.black, lineWidth: 1).frame(width: 9, height: 9).padding(2)
                    Rectangle().strokeBorder(.black, lineWidth: 1).frame(width: 6, height: 6).padding(.leading, 7).padding(.top, 7)
                }
                .frame(width: 15, height: 15)
                .overlay(Rectangle().strokeBorder(.black, lineWidth: 1))
            case .win9x:
                Canvas { canvas, size in
                    for offset in stride(from: CGFloat(3), to: size.width, by: 4) {
                        var line = Path()
                        line.move(to: CGPoint(x: size.width, y: offset))
                        line.addLine(to: CGPoint(x: offset, y: size.height))
                        canvas.stroke(line, with: .color(.black.opacity(0.65)), lineWidth: 1)
                    }
                }
                .frame(width: 15, height: 15)
            case .terminal:
                EmptyView()
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}
#endif
