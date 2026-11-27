#if os(macOS)
import AppKit
import Nu9veTheme
@testable import NuMacShell
import SwiftUI
import Testing

@Suite("Period window bar")
@MainActor
struct PeriodWindowBarTests {
    private let colors = PeriodBarColors(ThemeRoles(skin: .win9x, dark: false, base: ThemeColor(0.2, 0.4, 0.9)))

    @Test func onlyTheThreeCommonsAreEras() {
        #expect(ThemeSkin.commons.compactMap(PeriodEra.init) == PeriodEra.allCases)
        #expect(PeriodEra(.glass) == nil)
        #expect(PeriodEra(.gothic) == nil)
        for era in PeriodEra.allCases { #expect(PeriodEra(era.skin) == era) }
    }

    /// The catalog's measures (nu9ve-theme-v1 §5): Terminal carries its controls in the row.
    @Test func heightsComeFromTheCatalog() {
        #expect(PeriodEra.classicMac.height == 50)
        #expect(PeriodEra.win9x.height == 52)
        #expect(PeriodEra.terminal.captionHeight == 0)
        #expect(PeriodEra.terminal.height == 30)
    }

    @Test func controlsKeepTheOrderAndSignsOfMacOS() {
        #expect(PeriodWindowControl.allCases == [.close, .minimize, .zoom])
        #expect(PeriodWindowControl.allCases.map(\.sign) == ["×", "−", "+"])
        #expect(PeriodWindowControl.allCases.map(\.label) == ["Cerrar ventana", "Minimizar ventana", "Ampliar ventana"])
    }

    /// The Windows 9x caption keeps white text readable, whatever the seed.
    @Test func theWin9xCaptionReadsWhiteText() {
        for seed in [ThemeColor.white, ThemeColor(1, 1, 0), ThemeColor.gray(0.5), ThemeColor(0.02, 0.02, 0.02)] {
            let ends = [seed.mixed(with: .black, amount: 0.55), seed.mixed(with: .black, amount: 0.15)]
                .map { $0.readable(on: [.white], minimum: 4.5) }
            for end in ends { #expect(end.contrast(with: .white) >= 4.5, "\(seed.hex)") }
            #expect(PeriodBarColors.win9xCaption(seed: seed).count == 2)
        }
    }

    private func squareWindow() -> ShellWindow {
        let window = ShellWindow(
            contentRect: NSRect(x: 200, y: 200, width: 900, height: 600),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: true
        )
        window.isReleasedWhenClosed = false
        window.contentView = NSView(frame: NSRect(x: 0, y: 0, width: 900, height: 600))
        return window
    }

    /// The window must not move when the title bar comes and goes. In an app AppKit kept the
    /// content rect instead; this process does not reproduce that drift, so the test only guards
    /// the result, not the cause.
    @Test func changingTheTitleBarKeepsTheFrame() {
        let window = squareWindow()
        let frame = window.frame
        window.usesSquareCorners = true
        #expect(window.frame == frame)
        window.usesSquareCorners = false
        #expect(window.frame == frame)
    }

    @Test func theControlsActOnTheirWindow() {
        let window = squareWindow()
        window.usesSquareCorners = true
        window.orderFront(nil)
        PeriodWindowControl.close.perform(on: window, option: false)
        #expect(!window.isVisible)
    }

    /// Each era lays out with and without its row, and the bar takes the height it announces.
    @Test func everyEraLaysOut() {
        for era in PeriodEra.allCases {
            for showsRow in [true, false] {
                let bar = PeriodWindowBar(title: "Documento", era: era, colors: colors, zoomEnabled: showsRow, showsRow: showsRow) {
                    Text(verbatim: "fila").frame(maxWidth: .infinity)
                }
                let host = NSHostingView(rootView: bar.frame(width: 800))
                host.layoutSubtreeIfNeeded()
                let expected = showsRow || era.captionHeight == 0 ? era.height : era.captionHeight
                #expect(host.fittingSize.height == expected, "\(era) showsRow \(showsRow)")
            }
        }
    }

    /// With `singleRow`, Macintosh and Windows 9x take one row on the caption; Terminal's bar is one row already.
    @Test func singleRowLaysOutAtItsHeight() {
        #expect(PeriodEra.classicMac.singleRowHeight == 34)
        #expect(PeriodEra.win9x.singleRowHeight == 34)
        #expect(PeriodEra.terminal.singleRowHeight == PeriodEra.terminal.height)
        for era in PeriodEra.allCases {
            let bar = PeriodWindowBar(title: "Documento", era: era, colors: colors, singleRow: true) {
                HStack {
                    Text(verbatim: "app").periodClearing()
                    Spacer()
                    Text(verbatim: "acciones").periodPlate()
                }
            }
            let host = NSHostingView(rootView: bar.frame(width: 800))
            host.layoutSubtreeIfNeeded()
            #expect(host.fittingSize.height == era.singleRowHeight, "\(era)")
        }
        // Without its row, the caption alone, as before: singleRow needs a row to put on it.
        let captionOnly = PeriodWindowBar(title: "Acerca de", era: .win9x, colors: colors, showsRow: false, singleRow: true) {
            EmptyView()
        }
        let host = NSHostingView(rootView: captionOnly.frame(width: 400))
        host.layoutSubtreeIfNeeded()
        #expect(host.fittingSize.height == PeriodEra.win9x.captionHeight)
    }

    /// Windows 9x's face is the era's canvas: silver and black in the original, the user's tint with «Con tu color».
    @Test func win9xFaceFollowsTheEra() {
        let seed = ThemeColor(0.2, 0.4, 0.9)
        let original = PeriodBarColors(ThemeRoles(skin: .win9x, dark: false, base: seed))
        #expect(original.face == PeriodBarColors.silver)
        #expect(original.faceInk == PeriodBarColors.color(.black))
        let tinted = PeriodBarColors(ThemeRoles(skin: .win9x, dark: true, base: seed, era: .tinted))
        #expect(tinted.face != PeriodBarColors.silver)
        // An app that builds its colors by hand keeps the silver face and draws Macintosh's lines with its ink.
        let manual = PeriodBarColors(canvas: .white, surface: .white, ink: .black, accent: .blue, caption: [.blue, .blue])
        #expect(manual.face == PeriodBarColors.silver)
        #expect(manual.rule == .black)
    }

    /// Macintosh's lines are the roles' `line`: black in the original era, the user's color with «Con tu color».
    @Test func macintoshLinesCarryTheColor() {
        let seed = ThemeColor(0.85, 0.35, 0.2)
        let original = PeriodBarColors(ThemeRoles(skin: .classicMac, dark: false, base: seed))
        #expect(original.rule == PeriodBarColors.color(.black))
        let tintedRoles = ThemeRoles(skin: .classicMac, dark: false, base: seed, era: .tinted)
        #expect(PeriodBarColors(tintedRoles).rule == PeriodBarColors.color(tintedRoles.line))
        #expect(tintedRoles.line != tintedRoles.ink)
        for dark in [false, true] {
            let roles = ThemeRoles(skin: .classicMac, dark: dark, base: seed, era: .tinted)
            #expect(min(roles.line.contrast(with: roles.canvas), roles.line.contrast(with: roles.surface)) >= 3)
        }
        // Macintosh with your color starts light, the opposite of Terminal.
        #expect(ThemeSkin.classicMac.defaultDark(era: .tinted) == false)
        #expect(ThemeSkin.classicMac.defaultDark(era: .original) == false)
        #expect(ThemeSkin.win9x.defaultDark(era: .tinted) == nil)
        #expect(ThemeSkin.terminal.defaultDark(era: .tinted) == true)
        #expect(ThemeSkin.glass.defaultDark(era: .tinted) == nil)
    }

    /// Outside a bar's row the modifiers change nothing.
    @Test func platesOutsideABarAreNoOps() {
        let plain = NSHostingView(rootView: Text(verbatim: "x").fixedSize())
        let plated = NSHostingView(rootView: Text(verbatim: "x").periodPlate().periodClearing().fixedSize())
        plain.layoutSubtreeIfNeeded()
        plated.layoutSubtreeIfNeeded()
        #expect(plain.fittingSize == plated.fittingSize)
    }
}
#endif
