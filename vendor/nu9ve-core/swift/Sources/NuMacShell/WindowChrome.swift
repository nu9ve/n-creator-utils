#if os(macOS)
import AppKit
import Combine
import SwiftUI

/// Window state the content reacts to. In fullscreen a toolbar moves to a system
/// window that ignores the theme, so the content draws the bar itself while this
/// is true.
@MainActor
public final class WindowShellState: ObservableObject {
    @Published public internal(set) var isFullScreen = false
    weak var window: NSWindow?

    public init() {}

    // For apps that draw their own window controls.
    public func close() { window?.performClose(nil) }
    public func minimize() { window?.miniaturize(nil) }
    public func zoom() { window?.zoom(nil) }
    public func toggleFullScreen() { window?.toggleFullScreen(nil) }
}

/// Installs one flexible bar beside the window controls, following section 1 of
/// the nu9ve macOS app contract, and applies the theme to the window itself.
///
/// The bar lives in its own hosting view and does not inherit the SwiftUI
/// environment: theme, stores and actions must arrive through its initializer.
///
/// Changing `usesSystemTitleBar` or `usesSquareCorners`, like entering fullscreen, makes
/// AppKit rebuild the window's frame view and SwiftUI moves the content into it: the root
/// view appears again and its `.task` and `.onAppear` run again, cancelling the running
/// ones. Work that must happen once per launch starts outside the view, guarded by a flag.
public struct WindowChromeBridge<Bar: View>: NSViewRepresentable {
    private let bar: Bar
    private let barHeight: CGFloat
    private let title: String
    private let background: Color
    private let isDark: Bool
    private let usesSystemTitleBar: Bool
    private let usesSquareCorners: Bool
    private let showsToolbar: Bool
    private let shell: WindowShellState

    /// With `usesSystemTitleBar` false the toolbar and the system window buttons are
    /// hidden: the content draws its own bar, using `WindowShellState` for its window
    /// controls. Square corners need a `ShellWindow`. Windows without global actions
    /// pass `showsToolbar` false and keep the system buttons over their content.
    public init(
        title: String,
        background: Color,
        isDark: Bool,
        usesSystemTitleBar: Bool = true,
        usesSquareCorners: Bool = false,
        showsToolbar: Bool = true,
        shell: WindowShellState,
        barHeight: CGFloat = 30,
        @ViewBuilder bar: () -> Bar
    ) {
        self.title = title
        self.background = background
        self.isDark = isDark
        self.usesSystemTitleBar = usesSystemTitleBar
        self.usesSquareCorners = usesSquareCorners
        self.showsToolbar = showsToolbar
        self.shell = shell
        self.barHeight = barHeight
        self.bar = bar()
    }

    public func makeNSView(context: Context) -> WindowChromeView<Bar> {
        WindowChromeView(bar: bar, barHeight: barHeight, shell: shell)
    }

    public func updateNSView(_ view: WindowChromeView<Bar>, context: Context) {
        view.update(bar: bar)
        view.chrome = WindowChromeView<Bar>.Chrome(
            title: title,
            background: background,
            isDark: isDark,
            usesSystemTitleBar: usesSystemTitleBar,
            usesSquareCorners: usesSquareCorners,
            showsToolbar: showsToolbar
        )
    }
}

public final class WindowChromeView<Bar: View>: NSView, NSToolbarDelegate {
    struct Chrome: Equatable {
        var title: String
        var background: Color
        var isDark: Bool
        var usesSystemTitleBar: Bool
        var usesSquareCorners: Bool
        var showsToolbar: Bool
    }

    private static var barItem: NSToolbarItem.Identifier { NSToolbarItem.Identifier("xyz.nu9ve.shell.bar") }

    var chrome: Chrome? {
        didSet {
            if chrome != oldValue { scheduleChrome() }
        }
    }
    private var isChromeScheduled = false

    private lazy var toolbar: NSToolbar = {
        let toolbar = NSToolbar(identifier: "xyz.nu9ve.shell.main")
        toolbar.delegate = self
        toolbar.displayMode = .iconOnly
        toolbar.allowsUserCustomization = false
        return toolbar
    }()
    private let host: NSHostingView<Bar>
    private let barHeight: CGFloat
    private let shell: WindowShellState
    private var observers: [any NSObjectProtocol] = []
    private weak var installed: NSWindow?

    init(bar: Bar, barHeight: CGFloat, shell: WindowShellState) {
        host = NSHostingView(rootView: bar)
        self.barHeight = barHeight
        self.shell = shell
        super.init(frame: .zero)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not supported")
    }

    func update(bar: Bar) {
        host.rootView = bar
    }

    override public func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        guard let window, window !== installed else { return }
        installed = window
        shell.window = window
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = true
        window.titlebarSeparatorStyle = .none
        window.tabbingMode = .disallowed
        window.collectionBehavior.insert(.fullScreenPrimary)

        observeFullScreen(of: window)
        scheduleChrome()
    }

    /// `window.delegate` belongs to SwiftUI here, so this listens to notifications.
    private func observeFullScreen(of window: NSWindow) {
        removeObservers()
        let center = NotificationCenter.default
        let shell = shell
        observers = [
            center.addObserver(forName: NSWindow.willEnterFullScreenNotification, object: window, queue: .main) { [weak window] _ in
                MainActor.assumeIsolated {
                    window?.toolbar?.isVisible = false
                    shell.isFullScreen = true
                }
            },
            // The toolbar comes back the moment the exit starts: waiting for the next turn of
            // the loop leaves the row empty while the window shrinks.
            center.addObserver(forName: NSWindow.willExitFullScreenNotification, object: window, queue: .main) { [weak self, weak window] _ in
                MainActor.assumeIsolated {
                    shell.isFullScreen = false
                    if let toolbar = window?.toolbar, self?.wantsToolbar == true { toolbar.isVisible = true }
                    self?.scheduleChrome()
                }
            },
            // AppKit restores the state it saved on entry, so the bar comes back
            // once more when the transition settles.
            center.addObserver(forName: NSWindow.didExitFullScreenNotification, object: window, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated {
                    self?.scheduleChrome()
                }
            }
        ]
    }

    private func removeObservers() {
        observers.forEach(NotificationCenter.default.removeObserver)
        observers = []
    }

    override public func viewWillMove(toWindow newWindow: NSWindow?) {
        if newWindow == nil { removeObservers() }
        super.viewWillMove(toWindow: newWindow)
    }

    /// SwiftUI updates this view during layout. Changing the style mask rebuilds the
    /// window's frame view, which would free views AppKit is still laying out.
    private func scheduleChrome() {
        guard !isChromeScheduled else { return }
        isChromeScheduled = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            isChromeScheduled = false
            applyChrome()
        }
    }

    private func applyChrome() {
        guard let window, let chrome else { return }
        window.title = chrome.title
        window.backgroundColor = NSColor(chrome.background)
        window.appearance = NSAppearance(named: chrome.isDark ? .darkAqua : .aqua)
        guard !window.styleMask.contains(.fullScreen) else { return }
        // Changing the title bar or the toolbar keeps the content rect, which would move and
        // resize the window: the frame people left is the one that stays.
        let frame = window.frame
        // A hidden toolbar would still take the clicks of the row, so it comes out. A toolbar
        // only sticks on a titled window: it leaves before the title bar and comes back after it.
        let toolbarForChrome = wantsToolbar ? toolbar : nil
        if toolbarForChrome == nil, window.toolbar != nil { window.toolbar = nil }
        (window as? ShellWindow)?.usesSquareCorners = chrome.usesSquareCorners
        if window.toolbar !== toolbarForChrome { window.toolbar = toolbarForChrome }
        if wantsToolbar {
            window.toolbarStyle = .unifiedCompact
            toolbar.isVisible = true
        }
        for button in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
            window.standardWindowButton(button)?.isHidden = !chrome.usesSystemTitleBar
        }
        if window.frame != frame { window.setFrame(frame, display: true) }
    }

    private var wantsToolbar: Bool {
        guard let chrome else { return false }
        return chrome.usesSystemTitleBar && chrome.showsToolbar
    }

    public func toolbarAllowedItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        [Self.barItem]
    }

    public func toolbarDefaultItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        [Self.barItem]
    }

    public func toolbar(
        _ toolbar: NSToolbar,
        itemForItemIdentifier itemIdentifier: NSToolbarItem.Identifier,
        willBeInsertedIntoToolbar flag: Bool
    ) -> NSToolbarItem? {
        let item = NSToolbarItem(itemIdentifier: itemIdentifier)
        host.sizingOptions = []
        host.translatesAutoresizingMaskIntoConstraints = false
        // A flexible item: the toolbar sizes it between these bounds to fill the row.
        NSLayoutConstraint.activate([
            host.heightAnchor.constraint(equalToConstant: barHeight),
            host.widthAnchor.constraint(greaterThanOrEqualToConstant: 260),
            host.widthAnchor.constraint(lessThanOrEqualToConstant: 10_000)
        ])
        item.view = host
        item.visibilityPriority = .high
        // macOS 26 wraps custom view items in one large glass platter; the bar
        // draws its own surfaces.
        item.isBordered = false
        return item
    }
}

/// What a double click on a title bar does, as System Settings › Desktop & Dock sets it.
public enum TitleBarDoubleClick: Equatable, Sendable {
    /// Between the window's size and filling the screen: "Fill" and "Zoom" alike, since a
    /// `ShellWindow` zooms by filling.
    case zoom
    case minimize
    case none

    /// `AppleActionOnDoubleClick` wins; older systems only wrote
    /// `AppleMiniaturizeOnDoubleClick`. With neither, macOS zooms.
    public static func fromSystem(_ defaults: UserDefaults = .standard) -> TitleBarDoubleClick {
        switch defaults.string(forKey: "AppleActionOnDoubleClick") {
        case "None": return .none
        case "Minimize": return .minimize
        case "Maximize", "Fill", "Zoom": return .zoom
        default: return defaults.bool(forKey: "AppleMiniaturizeOnDoubleClick") ? .minimize : .zoom
        }
    }

    @MainActor
    public func perform(on window: NSWindow) {
        guard !window.styleMask.contains(.fullScreen) else { return }
        switch self {
        case .zoom: window.zoom(nil)
        case .minimize: window.miniaturize(nil)
        case .none: break
        }
    }
}

/// The bar fills the toolbar row, so AppKit's titlebar never sees clicks there.
/// Its empty space behaves like one again: drag moves the window, and a double
/// click follows the system setting.
///
/// Put it behind the bar, and keep the bar's decoration out of hit testing
/// (`allowsHitTesting(false)`): an opaque color or gradient above this view takes the
/// click first and the window never moves.
public struct TitleBarDragArea: NSViewRepresentable {
    public final class Area: NSView {
        override public var mouseDownCanMoveWindow: Bool { false }
        override public func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

        override public func mouseDown(with event: NSEvent) {
            guard let window else { return }
            if event.clickCount == 2 {
                TitleBarDoubleClick.fromSystem().perform(on: window)
            } else {
                window.performDrag(with: event)
            }
        }
    }

    public init() {}

    public func makeNSView(context: Context) -> Area { Area() }
    public func updateNSView(_ view: Area, context: Context) {}
}
#endif
