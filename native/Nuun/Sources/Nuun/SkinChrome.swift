import AppKit
import SwiftUI
import NuMacShell
import NuunCore
import Nu9veTheme

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

// nu9ve-theme-v1 §5 y §8 · NuMacShell (F3, 2026-11-26): los tres comunes (Macintosh, Windows 9x y Terminal) usan
// `PeriodWindowBar`, `PeriodGrowBox`, `Win9xBevel` y `ClassicPinstripes` del core, iguales en toda la suite. Gótico,
// el skin propio de nüün, conserva su barra con las acciones y textos de `PeriodWindowControl`.

extension CreatorPalette {
    /// What the period bar takes from nüün's palette: canvas, surface, ink, `accentText` and the seed's caption.
    var periodColors: PeriodBarColors { PeriodBarColors(roles) }
}

/// The period caption's background: the core's for the commons, the lintel for Gótico.
/// Also the skin card's preview strip in Settings.
struct SkinBarBackground: View {
    @Environment(\.creatorPalette) private var palette
    let skin: ThemeSkin
    var body: some View {
        Group {
            if let era = PeriodEra(skin) {
                PeriodBarBackground(era: era, colors: palette.periodColors)
            } else if skin == .gothic {
                LinearGradient(colors: [palette.surface, palette.canvas], startPoint: .top, endPoint: .bottom)
                    .overlay(alignment: .bottom) {
                        VStack(spacing: 2) {
                            Rectangle().fill(palette.line).frame(height: 1)
                            Rectangle().fill(palette.accent.opacity(0.5)).frame(height: 1)
                        }
                    }
            } else {
                palette.canvas
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// Gótico's own bar: its lintel with × − + as small framed tiles (close carries the accent), and nüün's
/// bar in the row below. Actions, labels and help are the suite's (`PeriodWindowControl`).
struct GothicWindowBar<Content: View>: View {
    @Environment(\.creatorPalette) private var palette
    let title: String
    var showsRow = true
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
                ZStack {
                    SkinBarBackground(skin: skin)
                    HStack(spacing: 6) { controls; Spacer(minLength: 0) }.padding(.horizontal, PeriodEra.inset)
                }
                .background(TitleBarDragArea())
                .frame(height: CGFloat(skin.captionHeight))
            }
            HStack(spacing: 6) {
                if skin.captionHeight == 0 { controls.padding(.leading, PeriodEra.inset) }
                content
            }
            .padding(.trailing, PeriodEra.inset)
            .frame(height: CGFloat(skin.barHeight))
            .background(SkinBarBackground(skin: skin))
            .background(TitleBarDragArea())
            .opacity(showsRow ? 1 : 0)
            .frame(height: showsRow ? nil : 0)
            .clipped()
        }
        .background(WindowHandleReader(handle: handle))
    }

    private var controls: some View {
        HStack(spacing: 5) {
            ForEach(PeriodWindowControl.allCases, id: \.self) { control in
                let enabled = control != .zoom || resizable
                Button {
                    guard let window = handle.window else { return }
                    control.perform(on: window, option: NSEvent.modifierFlags.contains(.option))
                } label: {
                    Text(verbatim: control.sign).font(.system(size: 11, weight: .black))
                        .foregroundStyle(control == .close ? palette.onAccent : palette.ink)
                        .frame(width: 15, height: 15)
                        .background(control == .close ? palette.accent : palette.raised, in: Rectangle())
                        .overlay(Rectangle().stroke(palette.line, lineWidth: 1))
                        .contentShape(Rectangle())
                        .opacity(enabled ? 1 : 0.4)
                }
                .buttonStyle(.plain)
                .disabled(!enabled)
                .accessibilityLabel(Text(LocalizedStringKey(control.label)))
                .help(Text(LocalizedStringKey(control.help)))
            }
        }
    }
}

/// Over Macintosh's pinstripes, each group of the bar sits on a margin of paper, as the title did in System 7,
/// so it reads; on the Windows 9x caption, on a raised silver panel. Elsewhere it changes nothing.
struct PeriodPlate: ViewModifier {
    @Environment(\.creatorPalette) private var palette
    @Environment(\.barOnCaption) private var onCaption
    func body(content: Content) -> some View {
        if palette.skin == .classicMac {
            // Only the paper margin: like the title in System 7, no box of its own.
            content.modifier(PeriodClearing())
        } else if palette.skin == .win9x && onCaption {
            // On the caption's gradient: a raised silver panel, like a toolbar band in Windows 98.
            content.padding(.horizontal, 6).frame(height: 22)
                .background(palette.canvas).overlay(Win9xBevel())
        } else {
            content
        }
    }
}

/// Macintosh: a margin of paper around what sits on the pinstripes, as the title box had, so the rules stop
/// short of it and never touch it. The bar's bottom rule stays visible.
struct PeriodClearing: ViewModifier {
    @Environment(\.creatorPalette) private var palette
    func body(content: Content) -> some View {
        if palette.skin == .classicMac {
            content.padding(.horizontal, 7).frame(maxHeight: .infinity)
                .background(palette.surface).padding(.top, 2).padding(.bottom, 1)
        } else {
            content
        }
    }
}

private struct BarOnCaptionKey: EnvironmentKey { static let defaultValue = false }
extension EnvironmentValues {
    /// True while nüün's bar is drawn on a period caption instead of in its own row.
    var barOnCaption: Bool {
        get { self[BarOnCaptionKey.self] }
        set { self[BarOnCaptionKey.self] = newValue }
    }
}

/// Any window's content with its skin's bar when the skin draws its own window.
struct SkinnedWindow<Bar: View, Content: View>: View {
    @Environment(\.creatorPalette) private var palette
    let title: String
    /// Whether nüün's own bar gets a row; the settings window has none under a period caption.
    var barRow: (ThemeSkin) -> Bool = { _ in true }
    var resizable = true
    /// Macintosh: one bar instead of a caption over nüün's row, so the name is not said twice (2026-11-26).
    var singleBar = false
    let bar: Bar
    let content: Content
    init(title: String, barRow: @escaping (ThemeSkin) -> Bool = { _ in true }, resizable: Bool = true, singleBar: Bool = false,
         @ViewBuilder bar: () -> Bar, @ViewBuilder content: () -> Content) {
        self.title = title
        self.barRow = barRow
        self.resizable = resizable
        self.singleBar = singleBar
        self.bar = bar()
        self.content = content()
    }
    var body: some View {
        if palette.skin == .classicMac && singleBar && barRow(palette.skin) {
            // Pinstripes across, × − + at the left on their own boxes, and nüün's bar (brand, navigation in the
            // middle, actions) on plates over them: the same order as Cristal, in the era's materials.
            VStack(spacing: 0) {
                ZStack {
                    PeriodBarBackground(era: .classicMac, colors: palette.periodColors)
                    HStack(spacing: 6) {
                        // × − + sit on the pinstripes, each on its own box, as the era drew them.
                        PeriodWindowControls(era: .classicMac, colors: palette.periodColors, zoomEnabled: resizable)
                            .padding(.leading, PeriodEra.inset)
                        bar.padding(.trailing, PeriodEra.inset)
                    }
                }
                .frame(height: PeriodEra.classicMac.rowHeight + 4)
                .background(TitleBarDragArea())
                .accessibilityElement(children: .contain)
                .accessibilityLabel(Text(verbatim: title))
                content.frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            .overlay(alignment: .bottomTrailing) { if resizable { PeriodGrowBox(era: .classicMac) } }
            .overlay { Rectangle().strokeBorder(palette.ink, lineWidth: 1).allowsHitTesting(false) }
            .ignoresSafeArea()
        } else if palette.skin == .win9x && singleBar && barRow(palette.skin) {
            // Windows 9x (user's choice, 2026-11-26): the whole bar on the caption's gradient, with the title in white
            // after × − + and each group on a raised silver panel, so the caption holds navigation, disks and Settings.
            VStack(spacing: 0) {
                ZStack {
                    PeriodBarBackground(era: .win9x, colors: palette.periodColors)
                    HStack(spacing: 6) {
                        PeriodWindowControls(era: .win9x, colors: palette.periodColors, zoomEnabled: resizable)
                            .padding(.leading, PeriodEra.inset)
                        Text(verbatim: "nüün").font(.custom("Tahoma Bold", size: 12)).foregroundStyle(.white)
                            .padding(.leading, 2).allowsHitTesting(false)
                        bar.padding(.trailing, PeriodEra.inset).environment(\.barOnCaption, true)
                    }
                }
                .frame(height: PeriodEra.win9x.rowHeight + 4)
                .background(TitleBarDragArea())
                .accessibilityElement(children: .contain)
                .accessibilityLabel(Text(verbatim: title))
                content.frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            .overlay(alignment: .bottomTrailing) { if resizable { PeriodGrowBox(era: .win9x) } }
            .ignoresSafeArea()
        } else if let era = PeriodEra(palette.skin) {
            VStack(spacing: 0) {
                // The row gets no margins from the core; nüün's bar brings its own on the left and keeps 8 pt on the
                // right, as before. In Terminal it follows the controls.
                PeriodWindowBar(title: title, era: era, colors: palette.periodColors, zoomEnabled: resizable,
                                showsRow: barRow(palette.skin)) {
                    bar.padding(.trailing, PeriodEra.inset)
                }
                content.frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            .overlay(alignment: .bottomTrailing) { if resizable { PeriodGrowBox(era: era) } }
            .overlay { if era == .classicMac { Rectangle().strokeBorder(palette.ink, lineWidth: 1).allowsHitTesting(false) } }
            .ignoresSafeArea()
        } else if palette.skin.usesSquareWindow {
            VStack(spacing: 0) {
                GothicWindowBar(title: title, showsRow: barRow(palette.skin), resizable: resizable) { bar }
                content.frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            .ignoresSafeArea()
        } else {
            content
        }
    }
}

// MARK: - Dialogs

/// Closes the dialog a view is shown in, whether it is a system sheet or a skin's in-window dialog.
private struct CloseSheetKey: EnvironmentKey {
    static let defaultValue: @MainActor @Sendable () -> Void = {}
}
extension EnvironmentValues {
    var closeSheet: @MainActor @Sendable () -> Void {
        get { self[CloseSheetKey.self] }
        set { self[CloseSheetKey.self] = newValue }
    }
}

/// nu9ve-theme-v1 §8: every window and dialog follows the skin. Cristal keeps the system sheet; skins
/// that draw their own window show the dialog inside it, square and with the era's surface, because
/// macOS always rounds a sheet's corners.
struct SkinSheet<Sheet: View>: ViewModifier {
    @Environment(\.creatorPalette) private var palette
    @Binding var isPresented: Bool
    let sheet: () -> Sheet
    func body(content: Content) -> some View {
        let inWindow = palette.skin.usesSquareWindow
        let close: @MainActor @Sendable () -> Void = { [binding = $isPresented] in binding.wrappedValue = false }
        content
            .sheet(isPresented: Binding(get: { isPresented && !inWindow }, set: { if !$0 { isPresented = false } })) {
                sheet().environment(\.closeSheet, close).modifier(CreatorAppearance())
            }
            .overlay {
                if inWindow && isPresented {
                    ZStack {
                        // The backdrop takes the clicks, as a sheet blocks its window.
                        Color.black.opacity(palette.dark ? 0.5 : 0.3).contentShape(Rectangle()).onTapGesture {}
                            .accessibilityHidden(true)
                        GeometryReader { geometry in
                            // The surface hugs the dialog's own size; the window only caps what it is offered.
                            sheet().environment(\.closeSheet, close)
                                .background(palette.surface)
                                .modifier(GlassSurface(shape: palette.panel))
                                .overlay { if palette.skin == .classicMac { Rectangle().inset(by: 3).stroke(palette.ink, lineWidth: 2).allowsHitTesting(false) } }
                                .frame(maxWidth: geometry.size.width - 40, maxHeight: geometry.size.height - 40)
                                .position(x: geometry.size.width / 2, y: geometry.size.height / 2)
                        }
                    }
                    .accessibilityAddTraits(.isModal)
                    .transition(.opacity)
                }
            }
    }
}

extension View {
    func skinSheet<Sheet: View>(isPresented: Binding<Bool>, @ViewBuilder content: @escaping () -> Sheet) -> some View {
        modifier(SkinSheet(isPresented: isPresented, sheet: content))
    }
    func skinSheet<Item: Identifiable, Sheet: View>(item: Binding<Item?>, @ViewBuilder content: @escaping (Item) -> Sheet) -> some View {
        let shown = Binding(get: { item.wrappedValue != nil }, set: { if !$0 { item.wrappedValue = nil } })
        return modifier(SkinSheet(isPresented: shown, sheet: { ItemSheet(item: item.wrappedValue, content: content) }))
    }
}

/// The item a dialog was opened for; empty while it closes.
private struct ItemSheet<Item, Sheet: View>: View {
    let item: Item?
    let content: (Item) -> Sheet
    var body: some View { if let item { content(item) } }
}
