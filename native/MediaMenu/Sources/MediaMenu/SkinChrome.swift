import AppKit
import SwiftUI
import MediaMenuCore

// nu9ve-theme-v1 §5 y §8 · origen: ios-fractos Apps/macOS/FractosMac/ThemedWindowBar.swift y
// FractosDesign/ThemeChrome.swift (Win9xBevel, ClassicPinstripes, WindowGrowBox).
// Divergencias: nüün tiene dos filas en Macintosh y Windows 9x (título de época arriba, la barra
// propia de nüün abajo, como la barra de herramientas de una ventana de la época), porque su barra
// lleva navegación y discos que no se leen sobre rayas ni sobre el degradado. Los controles de ventana
// siguen el orden y lugar de macOS (cerrar, minimizar, ampliar, a la izquierda) con ×, − y + siempre
// visibles, para quien nunca usó un Mac; Fractos usa los glifos y lugares de cada época. Sin sonido ni háptica.

/// Decoration behind the content: a candle vignette for Gótico.
/// Static, cached, never intercepts clicks and never reaches VoiceOver.
struct SkinAtmosphere: View {
    @Environment(\.creatorPalette) private var palette
    var body: some View {
        Group {
            switch palette.skin {
            case .gothic:
                RadialGradient(colors: [.clear, .black.opacity(0.55)], center: .center, startRadius: 120, endRadius: 900)
            default:
                Color.clear
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// Terminal: static CRT scanlines over the whole window. Drawn once, no animation, no clicks.
/// nu9ve-theme-v1 §5 · origen: ios-fractos FractosDesign ThemeChrome.swift `FractosScreenOverlay`.
struct Scanlines: View {
    var body: some View {
        Canvas { context, size in
            var y: CGFloat = 0
            while y < size.height {
                context.fill(Path(CGRect(x: 0, y: y, width: size.width, height: 1)), with: .color(.black.opacity(0.22)))
                y += 3
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// Windows 9x: a raised or sunken two-pixel bevel. Only a drawing.
struct Win9xBevel: View {
    var isRaised = true
    static let highlight = Color.white
    static let lightShadow = Color(.sRGB, red: 128 / 255, green: 128 / 255, blue: 128 / 255)
    static let darkShadow = Color(.sRGB, red: 10 / 255, green: 10 / 255, blue: 10 / 255)
    var body: some View {
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
struct ClassicPinstripes: View {
    var color: Color = .black
    var body: some View {
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

/// The period caption's background: pinstripes, a gradient caption, or the gothic lintel.
/// Also the skin card's preview strip in Settings.
struct SkinBarBackground: View {
    @Environment(\.creatorPalette) private var palette
    let skin: ThemeSkin
    var body: some View {
        Group {
            switch skin {
            case .classicMac:
                ZStack { Color.white; ClassicPinstripes() }
                    .overlay(alignment: .bottom) { Rectangle().fill(.black).frame(height: 1) }
            case .win9x:
                LinearGradient(colors: Self.caption(palette), startPoint: .leading, endPoint: .trailing)
            case .gothic:
                LinearGradient(colors: [palette.surface, palette.canvas], startPoint: .top, endPoint: .bottom)
                    .overlay(alignment: .bottom) {
                        VStack(spacing: 2) {
                            Rectangle().fill(palette.line).frame(height: 1)
                            Rectangle().fill(palette.accent.opacity(0.5)).frame(height: 1)
                        }
                    }
            case .terminal:
                palette.surface.overlay(alignment: .bottom) { Rectangle().fill(palette.accentText).frame(height: 2) }
            case .glass:
                palette.canvas
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
    /// Like Windows 98's appearance settings, the caption takes the user's color, kept dark
    /// enough for white text.
    static func caption(_ palette: CreatorPalette) -> [Color] {
        let white = ThemeColor.white
        let start = palette.roles.seed.mixed(with: .black, amount: 0.55).readable(on: [white], minimum: 4.5)
        let end = palette.roles.seed.mixed(with: .black, amount: 0.15).readable(on: [white], minimum: 4.5)
        return [CreatorPalette.color(start), CreatorPalette.color(end)]
    }
}

/// The window's own bar for skins with square windows: a period caption with the title and
/// close/minimize/zoom, then `content` (nüün's bar) in a row of its own.
struct SkinWindowBar<Content: View>: View {
    @Environment(\.creatorPalette) private var palette
    let title: String
    /// Secondary windows under a period caption need no second row.
    var showsRow = true
    /// A fixed-size window (About) has no zoom control.
    var resizable = true
    let content: Content
    @State private var handle = WindowHandle()
    init(title: String, showsRow: Bool = true, resizable: Bool = true, @ViewBuilder content: () -> Content) {
        self.title = title
        self.showsRow = showsRow
        self.resizable = resizable
        self.content = content()
    }
    var body: some View {
        let skin = palette.skin
        VStack(spacing: 0) {
            if skin.captionHeight > 0 {
                caption.frame(height: skin.captionHeight)
            }
            // The system toolbar gave nüün's bar its margins; here the bar gets the same 8 pt on both
            // sides, aligned with the caption's controls above it.
            HStack(spacing: 6) {
                if skin.captionHeight == 0 { windowControls.padding(.leading, Self.inset) }
                content
            }
            .padding(.trailing, Self.inset)
            .frame(height: skin.barHeight)
            .background(skin == .gothic || skin == .terminal ? AnyView(SkinBarBackground(skin: skin)) : AnyView(barRow))
            .background(TitleBarDragArea())
            .opacity(showsRow ? 1 : 0)
            .frame(height: showsRow ? nil : 0)
            .clipped()
        }
        .background(WindowHandleReader(handle: handle))
    }

    /// nüün's row under a period caption: the window's surface with the era's rule below.
    private var barRow: some View {
        Group {
            switch palette.skin {
            case .win9x:
                palette.canvas.overlay(alignment: .bottom) { Win9xBevel(isRaised: false).frame(height: 2) }
            default:
                palette.canvas.overlay(alignment: .bottom) { Rectangle().fill(palette.ink).frame(height: 1) }
            }
        }
        .allowsHitTesting(false)
    }

    @ViewBuilder private var caption: some View {
        ZStack {
            SkinBarBackground(skin: palette.skin)
            // Every skin keeps the macOS order and place: close, minimize, zoom at the left, each with a
            // glyph that says what it does. Only their drawing belongs to the era.
            HStack(spacing: 6) {
                windowControls
                if palette.skin == .win9x {
                    Text(verbatim: title).font(.custom("Tahoma Bold", size: 12)).foregroundStyle(.white).lineLimit(1)
                        .padding(.leading, 4).allowsHitTesting(false)
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, Self.inset)
            if palette.skin == .classicMac {
                Text(verbatim: title).font(.custom("Silom", size: 12)).foregroundStyle(.black)
                    .padding(.horizontal, 8).background(Color.white)
                    .allowsHitTesting(false)
            }
        }
        .background(TitleBarDragArea())
    }

    static var inset: CGFloat { 8 }

    private var windowControls: some View {
        HStack(spacing: palette.skin == .win9x ? 2 : 5) {
            control(.close); control(.minimize)
            if resizable { control(.zoom) }
        }
    }

    private enum Action { case close, minimize, zoom }

    private func control(_ action: Action) -> some View {
        Button {
            guard let window = handle.window else { return }
            switch action {
            case .close: window.performClose(nil)
            case .minimize: window.performMiniaturize(nil)
            case .zoom:
                // A zoom box zooms; Option goes full screen, like the modern green button in reverse.
                if NSEvent.modifierFlags.contains(.option) || window.styleMask.contains(.fullScreen) { window.toggleFullScreen(nil) } else { window.performZoom(nil) }
            }
        } label: {
            glyph(action).contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(Text(label(action)))
        .help(Text(action == .zoom ? "Ampliar · Opción: pantalla completa" : label(action)))
    }

    private func label(_ action: Action) -> LocalizedStringKey {
        switch action {
        case .close: return "Cerrar ventana"
        case .minimize: return "Minimizar ventana"
        case .zoom: return "Ampliar ventana"
        }
    }

    /// The same symbols as the system's buttons, always visible: ×, − and +.
    private func symbol(_ action: Action) -> String {
        switch action {
        case .close: return "xmark"
        case .minimize: return "minus"
        case .zoom: return "plus"
        }
    }

    @ViewBuilder private func glyph(_ action: Action) -> some View {
        let image = Image(systemName: symbol(action)).font(.system(size: 8, weight: .black))
        switch palette.skin {
        case .terminal:
            // The same three signs, in brackets as a prompt would print them.
            Text(verbatim: action == .close ? "[×]" : action == .minimize ? "[−]" : "[+]")
                .font(.system(size: 12, weight: .bold, design: .monospaced))
                .foregroundStyle(action == .close ? palette.accentText : palette.ink)
        case .classicMac:
            image.foregroundStyle(.black)
                .frame(width: 13, height: 13)
                .background(Color.white)
                .overlay(Rectangle().strokeBorder(.black, lineWidth: 1))
                .padding(.horizontal, 2)
                .background(Color.white)
        case .win9x:
            image.foregroundStyle(.black)
                .frame(width: 17, height: 15)
                .background(Color(.sRGB, red: 192 / 255, green: 192 / 255, blue: 192 / 255))
                .overlay(Win9xBevel())
        default:
            // Gótico: small framed tiles; close carries the accent.
            image.foregroundStyle(action == .close ? palette.onAccent : palette.ink)
                .frame(width: 15, height: 15)
                .background(action == .close ? palette.accent : palette.raised, in: Rectangle())
                .overlay(Rectangle().stroke(palette.line, lineWidth: 1))
        }
    }
}

/// Where to grab a resizable window, as its era drew it. Only a drawing: the edges take the clicks.
struct WindowGrowBox: View {
    let skin: ThemeSkin
    var body: some View {
        Group {
            switch skin {
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
            default:
                EmptyView()
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// Any window's content with its skin's bar when the skin draws its own window.
struct SkinnedWindow<Bar: View, Content: View>: View {
    @Environment(\.creatorPalette) private var palette
    let title: String
    /// Whether nüün's own bar gets a row; the settings window has none under a period caption.
    var barRow: (ThemeSkin) -> Bool = { _ in true }
    var resizable = true
    let bar: Bar
    let content: Content
    init(title: String, barRow: @escaping (ThemeSkin) -> Bool = { _ in true }, resizable: Bool = true, @ViewBuilder bar: () -> Bar, @ViewBuilder content: () -> Content) {
        self.title = title
        self.barRow = barRow
        self.resizable = resizable
        self.bar = bar()
        self.content = content()
    }
    var body: some View {
        if palette.skin.usesSquareWindow {
            VStack(spacing: 0) {
                SkinWindowBar(title: title, showsRow: barRow(palette.skin), resizable: resizable) { bar }
                content.frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            .overlay(alignment: .bottomTrailing) { if resizable { WindowGrowBox(skin: palette.skin) } }
            .overlay { if palette.skin == .classicMac { Rectangle().strokeBorder(.black, lineWidth: 1).allowsHitTesting(false) } }
            .ignoresSafeArea()
        } else {
            content
        }
    }
}
