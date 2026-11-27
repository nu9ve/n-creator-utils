#if os(macOS)
import AppKit
@testable import NuMacShell
import SwiftUI
import Testing

@Suite("Window chrome bridge")
@MainActor
struct WindowChromeTests {
    private static let barItem = NSToolbarItem.Identifier("xyz.nu9ve.shell.bar")

    private func chrome(isDark: Bool?) -> WindowChromeView<EmptyView>.Chrome {
        .init(title: "QA", background: .gray, isDark: isDark, usesSystemTitleBar: true,
              usesSquareCorners: false, showsToolbar: true)
    }

    /// The chrome is applied on the next turn of the main queue, after SwiftUI's layout.
    private func settle() async {
        await withCheckedContinuation { done in DispatchQueue.main.async { done.resume() } }
    }

    private func window(with view: NSView) -> NSWindow {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 900, height: 600),
                              styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: true)
        window.isReleasedWhenClosed = false
        window.contentView = view
        return window
    }

    private func minimumWidth(of item: NSToolbarItem?) -> CGFloat? {
        item?.view?.constraints.first {
            $0.firstAttribute == .width && $0.relation == .greaterThanOrEqual
        }?.constant
    }

    @Test func darkAndLightSetTheWindowAppearance() async {
        let view = WindowChromeView(bar: EmptyView(), barHeight: 30, shell: WindowShellState())
        let window = window(with: view)
        view.chrome = chrome(isDark: true)
        await settle()
        #expect(window.appearance?.name == .darkAqua)
        view.chrome = chrome(isDark: false)
        await settle()
        #expect(window.appearance?.name == .aqua)
    }

    /// `nil` leaves the window without its own appearance, so it follows `NSApp.appearance`
    /// or the system's, also after it had one.
    @Test func nilInheritsTheAppAppearance() async {
        let view = WindowChromeView(bar: EmptyView(), barHeight: 30, shell: WindowShellState())
        let window = window(with: view)
        view.chrome = chrome(isDark: true)
        await settle()
        view.chrome = chrome(isDark: nil)
        await settle()
        #expect(window.appearance == nil)
    }

    @Test func theBarKeepsItsDefaultMinimumWidth() {
        let view = WindowChromeView(bar: EmptyView(), barHeight: 30, shell: WindowShellState())
        let item = view.toolbar(NSToolbar(identifier: "qa"), itemForItemIdentifier: Self.barItem,
                                willBeInsertedIntoToolbar: true)
        #expect(minimumWidth(of: item) == 260)
    }

    @Test func theMinimumWidthIsAParameterAndCanChange() {
        let view = WindowChromeView(bar: EmptyView(), barHeight: 30, minimumBarWidth: 540, shell: WindowShellState())
        let item = view.toolbar(NSToolbar(identifier: "qa"), itemForItemIdentifier: Self.barItem,
                                willBeInsertedIntoToolbar: true)
        #expect(minimumWidth(of: item) == 540)
        view.minimumBarWidth = 320
        #expect(minimumWidth(of: item) == 320)
    }
}
#endif
