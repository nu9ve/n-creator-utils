#if os(macOS)
import AppKit
@testable import NuMacShell
import Testing

@Suite("Window without a title bar")
@MainActor
struct ShellWindowTests {
    private let start = NSRect(x: 100, y: 100, width: 1_000, height: 700)
    private let minimum = NSSize(width: 800, height: 500)
    private let visible = NSRect(x: 0, y: 40, width: 1_600, height: 960)

    @Test func theRightEdgeMovesOnlyTheWidth() {
        let frame = ShellWindowGeometry.resized(start: start, edges: .right, delta: NSPoint(x: 50, y: 30), minimum: minimum)
        #expect(frame == NSRect(x: 100, y: 100, width: 1_050, height: 700))
    }

    @Test func theLeftAndBottomEdgesKeepTheOppositeSideInPlace() {
        let frame = ShellWindowGeometry.resized(start: start, edges: [.left, .bottom], delta: NSPoint(x: -20, y: -30), minimum: minimum)
        #expect(frame == NSRect(x: 80, y: 70, width: 1_020, height: 730))
    }

    @Test func aWindowStopsAtItsMinimumWithoutSliding() {
        let frame = ShellWindowGeometry.resized(start: start, edges: .left, delta: NSPoint(x: 500, y: 0), minimum: minimum)
        #expect(frame.width == 800)
        #expect(frame.maxX == start.maxX)
    }

    @Test func zoomFillsTheScreenAndComesBack() {
        let zoomed = ShellWindowGeometry.zoom(current: start, visible: visible, previous: nil, minimum: minimum)
        #expect(zoomed.frame == visible)
        let back = ShellWindowGeometry.zoom(current: visible, visible: visible, previous: zoomed.previous, minimum: minimum)
        #expect(back.frame == start)
    }

    /// The frame is remembered across launches and the frame before zoom is not: a window
    /// saved full must still have somewhere to go.
    @Test func aWindowRememberedFullIsNotStuck() {
        let step = ShellWindowGeometry.zoom(current: visible, visible: visible, previous: nil, minimum: minimum)
        #expect(step.frame.width < visible.width)
        #expect(step.frame.height < visible.height)
        #expect(step.frame.width >= minimum.width)
    }

    /// A size kept from a larger screen comes back inside the current one.
    @Test func aRememberedSizeIsFittedToTheScreen() {
        let offScreen = NSRect(x: 2_400, y: 900, width: 1_000, height: 700)
        let back = ShellWindowGeometry.zoom(current: visible, visible: visible, previous: offScreen, minimum: minimum)
        #expect(back.frame.size == offScreen.size)
        #expect(visible.contains(back.frame))
    }

    /// A remembered size that is itself the whole screen would leave the window stuck.
    @Test func aRememberedFullSizeIsIgnored() {
        let back = ShellWindowGeometry.zoom(current: visible, visible: visible, previous: visible, minimum: minimum)
        #expect(back.frame.width < visible.width)
    }

    @Test func theSizeBeforeFillingSurvivesARelaunch() throws {
        let suite = "nu9ve.shell.tests.\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        let saved = NSRect(x: 120, y: 140, width: 1_000, height: 700)

        let first = squareWindow()
        first.zoomDefaults = defaults
        first.setFrameAutosaveName("tests.zoom.\(suite)")
        first.frameBeforeZoom = saved

        let relaunched = squareWindow()
        relaunched.zoomDefaults = defaults
        relaunched.setFrameAutosaveName("tests.zoom.relaunched")
        #expect(relaunched.frameBeforeZoom == nil, "Each window keeps its own size")
        first.close()
        relaunched.setFrameAutosaveName("tests.zoom.\(suite)")
        #expect(relaunched.frameBeforeZoom == saved)

        relaunched.frameBeforeZoom = nil
        #expect(defaults.string(forKey: "NuShellWindow Frame Before Zoom tests.zoom.\(suite)") == nil)
    }

    @Test func doubleClickFollowsTheSystemSetting() throws {
        let suite = "nu9ve.shell.tests.\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }

        // AppKit registers its own default for the key, so the legacy fallback cannot be
        // reached from a test process.
        #expect(TitleBarDoubleClick.fromSystem(defaults) == .zoom, "macOS zooms when nothing is set")
        for (value, action) in [("Fill", TitleBarDoubleClick.zoom), ("Maximize", .zoom), ("Minimize", .minimize), ("None", .none)] {
            defaults.set(value, forKey: "AppleActionOnDoubleClick")
            #expect(TitleBarDoubleClick.fromSystem(defaults) == action, "\(value)")
        }
    }

    private func squareWindow(resizable: Bool = true) -> ShellWindow {
        let window = ShellWindow(contentRect: start, styleMask: [.titled, .closable, .miniaturizable], backing: .buffered, defer: true)
        window.isReleasedWhenClosed = false
        window.isUserResizable = resizable
        window.contentView = NSView(frame: start)
        window.usesSquareCorners = true
        return window
    }

    /// `NSWindow.performClose` needs a title bar button: without this, ⌘W only beeps.
    @Test func closingWorksWithoutATitleBar() {
        let window = squareWindow()
        #expect(!window.styleMask.contains(.titled))
        window.orderFront(nil)
        window.performClose(nil)
        #expect(!window.isVisible)
    }

    @Test func squareWindowsResizeOnlyIfTheyMay() {
        let fixed = squareWindow(resizable: false)
        #expect(!fixed.styleMask.contains(.resizable))
        #expect(!(fixed.contentView?.subviews.contains { $0 is ShellWindowResizeOverlay } ?? true))
        let free = squareWindow()
        #expect(free.styleMask.contains(.resizable))
        #expect(free.contentView?.subviews.contains { $0 is ShellWindowResizeOverlay } == true)
    }

    /// Only the edges take clicks; the content keeps everything else.
    @Test func theOverlayLeavesTheContentAlone() throws {
        let overlay = ShellWindowResizeOverlay(frame: NSRect(x: 0, y: 0, width: 1_000, height: 700))
        let parent = NSView(frame: overlay.frame)
        parent.addSubview(overlay)
        #expect(overlay.hitTest(NSPoint(x: 500, y: 350)) == nil)
        #expect(overlay.hitTest(NSPoint(x: 500, y: 690)) == nil)
        #expect(overlay.hitTest(NSPoint(x: 998, y: 350)) === overlay)
        #expect(overlay.hitTest(NSPoint(x: 500, y: 2)) === overlay)
    }
}

@Suite("Haptics")
@MainActor
struct HapticPlayerTests {
    @MainActor
    private final class Recording {
        var now = 0.0
        var played: [HapticTouch] = []
        var count = 0
    }

    @Test func touchesTooCloseTogetherAreDropped() {
        let recording = Recording()
        let player = HapticPlayer(minimumGap: 0.05, clock: { recording.now }) { recording.played.append($0) }
        player.play([(0, .tick)])
        recording.now = 0.01
        player.play([(0, .tick)])
        recording.now = 0.07
        player.play([(0, .thud)])
        #expect(recording.played == [.tick, .thud])
    }

    @Test func aDeepPressFiresOncePerPress() {
        let monitor = PressureMonitor()
        let recording = Recording()
        monitor.onDeepPress = { recording.count += 1 }
        monitor.update(pressure: 0.4, stage: 1)
        monitor.update(pressure: 0.2, stage: 2)
        monitor.update(pressure: 0.6, stage: 2)
        monitor.update(pressure: 0.1, stage: 1)
        monitor.update(pressure: 0.3, stage: 2)
        #expect(recording.count == 2)
        #expect(monitor.pressure == 0.3)
    }
}
#endif
