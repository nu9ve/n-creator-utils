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
}
#endif
