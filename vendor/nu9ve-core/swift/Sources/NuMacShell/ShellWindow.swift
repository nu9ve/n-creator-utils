#if os(macOS)
import AppKit
import SwiftUI

/// What a window did, however it was asked: its own controls, a double click on the bar,
/// a menu or a shortcut. Lets the app answer with sound without the shell knowing audio.
public enum ShellWindowEvent: Equatable, Sendable {
    /// `duration` is the frame animation's, in seconds.
    case zoomed(filling: Bool, duration: TimeInterval)
    case miniaturized
    case closed
    case enteredFullScreen
    case exitedFullScreen
}

/// A window the app fully owns. Without `.titled` macOS draws square corners, but a
/// window then refuses keyboard focus and full screen unless its class allows them, and
/// the system runs none of its title bar's behavior. This restores all of it: it must look
/// old and work like any window. SwiftUI's own window class cannot be subclassed.
public final class ShellWindow: NSWindow {
    private var resizeOverlay: ShellWindowResizeOverlay?

    /// Where the size before filling the screen is kept, so it survives a relaunch. Nil
    /// keeps it in memory only.
    public var zoomDefaults: UserDefaults? = .standard

    /// The frame to go back to from a filled screen. Stored beside the window's frame
    /// autosave, under its name.
    var frameBeforeZoom: NSRect? {
        get {
            if let memoryFrameBeforeZoom { return memoryFrameBeforeZoom }
            guard let key = zoomDefaultsKey, let stored = zoomDefaults?.string(forKey: key) else { return nil }
            let frame = NSRectFromString(stored)
            return frame.isEmpty ? nil : frame
        }
        set {
            memoryFrameBeforeZoom = newValue
            guard let key = zoomDefaultsKey else { return }
            if let newValue {
                zoomDefaults?.set(NSStringFromRect(newValue), forKey: key)
            } else {
                zoomDefaults?.removeObject(forKey: key)
            }
        }
    }
    private var memoryFrameBeforeZoom: NSRect?

    private var zoomDefaultsKey: String? {
        frameAutosaveName.isEmpty ? nil : "NuShellWindow Frame Before Zoom \(frameAutosaveName)"
    }

    /// Receives what the window did. `ShellWindowHost` sets it for its windows.
    public var onEvent: (@MainActor (ShellWindowEvent) -> Void)?

    /// Whether people can change its size. Set before the first corner style.
    public var isUserResizable = true

    /// Square corners and no system title bar, for themes that draw their own. The frame stays
    /// where it was: AppKit would keep the content rect instead, moving and resizing the window.
    public var usesSquareCorners = false {
        didSet {
            guard usesSquareCorners != oldValue else { return }
            let frame = frame
            applyCornerStyle()
            if self.frame != frame, !styleMask.contains(.fullScreen) { setFrame(frame, display: true) }
        }
    }

    override public init(
        contentRect: NSRect,
        styleMask style: NSWindow.StyleMask,
        backing backingStoreType: NSWindow.BackingStoreType,
        defer flag: Bool
    ) {
        super.init(contentRect: contentRect, styleMask: style, backing: backingStoreType, defer: flag)
        NotificationCenter.default.addObserver(
            self, selector: #selector(didExitFullScreen), name: NSWindow.didExitFullScreenNotification, object: self
        )
        NotificationCenter.default.addObserver(
            self, selector: #selector(didEnterFullScreen), name: NSWindow.didEnterFullScreenNotification, object: self
        )
    }

    @objc private func didEnterFullScreen() {
        onEvent?(.enteredFullScreen)
    }

    /// AppKit puts back the style it saved on entry, `.titled` included, after this
    /// notification is posted, so the square shape returns on the next turn of the loop.
    @objc private func didExitFullScreen() {
        onEvent?(.exitedFullScreen)
        DispatchQueue.main.async { [weak self] in
            self?.applyCornerStyle()
        }
    }

    override public var canBecomeKey: Bool { true }
    override public var canBecomeMain: Bool { true }

    override public func toggleFullScreen(_ sender: Any?) {
        // Full screen needs a titled window; `applyCornerStyle` runs again on exit.
        if usesSquareCorners { styleMask.insert(.titled) }
        super.toggleFullScreen(sender)
    }

    // The `perform` versions press a title bar button, and here there is none: without
    // these, the menu and its shortcuts only beep.
    override public func performClose(_ sender: Any?) {
        guard usesSquareCorners, !styleMask.contains(.titled) else {
            super.performClose(sender)
            return
        }
        if delegate?.windowShouldClose?(self) == false { return }
        close()
    }

    override public func performMiniaturize(_ sender: Any?) {
        guard usesSquareCorners, !styleMask.contains(.titled) else {
            super.performMiniaturize(sender)
            return
        }
        miniaturize(sender)
    }

    override public func performZoom(_ sender: Any?) {
        guard usesSquareCorners, !styleMask.contains(.titled) else {
            super.performZoom(sender)
            return
        }
        zoom(sender)
    }

    /// Every material zooms the same way, with or without the system's title bar: between
    /// the window's own size and filling the screen beside the Dock and the menu bar. Not
    /// full screen, and not AppKit's zoom, which goes to the content's ideal size and
    /// would differ between materials.
    override public func zoom(_ sender: Any?) {
        if styleMask.contains(.fullScreen) {
            toggleFullScreen(sender)
            return
        }
        guard isUserResizable, let visible = screen?.visibleFrame else {
            super.zoom(sender)
            return
        }
        let step = ShellWindowGeometry.zoom(current: frame, visible: visible, previous: frameBeforeZoom, minimum: smallestFrameSize)
        frameBeforeZoom = step.previous
        onEvent?(.zoomed(filling: step.previous != nil, duration: animationResizeTime(step.frame)))
        setFrame(step.frame, display: true, animate: true)
    }

    override public func miniaturize(_ sender: Any?) {
        if !isMiniaturized { onEvent?(.miniaturized) }
        super.miniaturize(sender)
    }

    override public func close() {
        if isVisible { onEvent?(.closed) }
        super.close()
    }

    override public var isZoomed: Bool {
        guard let visible = screen?.visibleFrame else { return super.isZoomed }
        return ShellWindowGeometry.fills(frame, visible: visible)
    }

    /// The smallest this window may be, content's own minimum included.
    var smallestFrameSize: NSSize {
        let content = frameRect(forContentRect: NSRect(origin: .zero, size: contentMinSize)).size
        return NSSize(width: max(minSize.width, content.width), height: max(minSize.height, content.height))
    }

    func applyCornerStyle() {
        guard !styleMask.contains(.fullScreen) else { return }
        var mask: NSWindow.StyleMask = [.miniaturizable, .closable, .fullSizeContentView]
        if isUserResizable { mask.insert(.resizable) }
        if usesSquareCorners {
            styleMask.remove(.titled)
            styleMask.formUnion(mask)
        } else {
            styleMask.formUnion(mask.union(.titled))
        }
        if !isUserResizable { styleMask.remove(.resizable) }
        updateResizeOverlay()
        hasShadow = true
        invalidateShadow()
    }

    /// Without a title bar AppKit offers no resizable edges, so an overlay provides them.
    private func updateResizeOverlay() {
        guard let contentView else { return }
        let isNeeded = usesSquareCorners && isUserResizable
        if isNeeded, resizeOverlay == nil {
            let overlay = ShellWindowResizeOverlay(frame: contentView.bounds)
            overlay.autoresizingMask = [.width, .height]
            contentView.addSubview(overlay)
            resizeOverlay = overlay
        }
        resizeOverlay?.isHidden = !isNeeded
        invalidateCursorRects(for: resizeOverlay ?? contentView)
    }
}

/// The arithmetic of a window without a title bar.
enum ShellWindowGeometry {
    struct Edges: OptionSet {
        let rawValue: Int
        static let left = Edges(rawValue: 1)
        static let right = Edges(rawValue: 2)
        static let bottom = Edges(rawValue: 4)
        static let top = Edges(rawValue: 8)
    }

    /// `delta` is the pointer's travel, in screen coordinates (y grows upward).
    static func resized(start: NSRect, edges: Edges, delta: NSPoint, minimum: NSSize) -> NSRect {
        var frame = start
        if edges.contains(.right) {
            frame.size.width = max(minimum.width, start.width + delta.x)
        }
        if edges.contains(.left) {
            frame.size.width = max(minimum.width, start.width - delta.x)
            frame.origin.x = start.maxX - frame.width
        }
        if edges.contains(.top) {
            frame.size.height = max(minimum.height, start.height + delta.y)
        }
        if edges.contains(.bottom) {
            frame.size.height = max(minimum.height, start.height - delta.y)
            frame.origin.y = start.maxY - frame.height
        }
        return frame
    }

    /// A window that already fills the screen, as one remembered from a previous launch,
    /// has nothing to go back to, so it gets a size of its own instead of staying stuck.
    static func zoom(current: NSRect, visible: NSRect, previous: NSRect?, minimum: NSSize) -> (frame: NSRect, previous: NSRect?) {
        guard fills(current, visible: visible) else { return (visible, current) }
        // A size kept from another launch may belong to a screen that is gone.
        if let previous, let fitted = fitted(previous, in: visible, minimum: minimum) { return (fitted, nil) }
        let width = min(visible.width, max(minimum.width, visible.width * 0.75))
        let height = min(visible.height, max(minimum.height, visible.height * 0.75))
        return (NSRect(x: visible.midX - width / 2, y: visible.midY - height / 2, width: width, height: height), nil)
    }
}

extension ShellWindowGeometry {
    static func fills(_ frame: NSRect, visible: NSRect) -> Bool {
        abs(frame.width - visible.width) < 2 && abs(frame.height - visible.height) < 2
    }

    /// `frame` moved fully onto `visible`, or nil when it no longer means anything there:
    /// as large as the screen itself, or smaller than the window may be.
    static func fitted(_ frame: NSRect, in visible: NSRect, minimum: NSSize) -> NSRect? {
        guard frame.width >= minimum.width, frame.height >= minimum.height,
              frame.width < visible.width - 1 || frame.height < visible.height - 1 else { return nil }
        var fitted = frame
        fitted.size.width = min(frame.width, visible.width)
        fitted.size.height = min(frame.height, visible.height)
        fitted.origin.x = min(max(frame.minX, visible.minX), visible.maxX - fitted.width)
        fitted.origin.y = min(max(frame.minY, visible.minY), visible.maxY - fitted.height)
        return fitted
    }
}

/// The window's edges and corners. Everything else passes through to the content.
final class ShellWindowResizeOverlay: NSView {
    private static let side: CGFloat = 5
    /// Thin, so the bar's own controls keep their clicks.
    private static let top: CGFloat = 3
    private static let corner: CGFloat = 14

    private var drag: (start: NSRect, mouse: NSPoint, edges: ShellWindowGeometry.Edges)?

    override var isFlipped: Bool { false }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    private func edges(at point: NSPoint) -> ShellWindowGeometry.Edges {
        var edges: ShellWindowGeometry.Edges = []
        if point.x < Self.side { edges.insert(.left) }
        if point.x > bounds.width - Self.side { edges.insert(.right) }
        if point.y < Self.side { edges.insert(.bottom) }
        if point.y > bounds.height - Self.top { edges.insert(.top) }
        guard !edges.isEmpty else { return [] }
        // Near a corner an edge counts as both, which makes corners easy to grab.
        if point.x < Self.corner { edges.insert(.left) }
        if point.x > bounds.width - Self.corner { edges.insert(.right) }
        if point.y < Self.corner, !edges.contains(.top) { edges.insert(.bottom) }
        if edges.contains(.top), edges.contains(.bottom) { edges.remove(.bottom) }
        return edges
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        guard !isHidden, let superview else { return nil }
        return edges(at: convert(point, from: superview)).isEmpty ? nil : self
    }

    override func resetCursorRects() {
        let width = bounds.width, height = bounds.height
        let side = Self.side, top = Self.top, corner = Self.corner
        let regions: [(NSRect, NSCursor.FrameResizePosition)] = [
            (NSRect(x: 0, y: corner, width: side, height: height - corner * 2), .left),
            (NSRect(x: width - side, y: corner, width: side, height: height - corner * 2), .right),
            (NSRect(x: corner, y: 0, width: width - corner * 2, height: side), .bottom),
            (NSRect(x: corner, y: height - top, width: width - corner * 2, height: top), .top),
            (NSRect(x: 0, y: 0, width: corner, height: side), .bottomLeft),
            (NSRect(x: 0, y: 0, width: side, height: corner), .bottomLeft),
            (NSRect(x: width - corner, y: 0, width: corner, height: side), .bottomRight),
            (NSRect(x: width - side, y: 0, width: side, height: corner), .bottomRight),
            (NSRect(x: 0, y: height - top, width: corner, height: top), .topLeft),
            (NSRect(x: width - corner, y: height - top, width: corner, height: top), .topRight)
        ]
        for (rect, position) in regions {
            addCursorRect(rect, cursor: .frameResize(position: position, directions: .all))
        }
    }

    override func mouseDown(with event: NSEvent) {
        guard let window else { return }
        let edges = edges(at: convert(event.locationInWindow, from: nil))
        drag = (window.frame, NSEvent.mouseLocation, edges)
    }

    override func mouseDragged(with event: NSEvent) {
        guard let window = window as? ShellWindow, let drag else { return }
        let mouse = NSEvent.mouseLocation
        let delta = NSPoint(x: mouse.x - drag.mouse.x, y: mouse.y - drag.mouse.y)
        window.setFrame(
            ShellWindowGeometry.resized(start: drag.start, edges: drag.edges, delta: delta, minimum: window.smallestFrameSize),
            display: true
        )
    }

    override func mouseUp(with event: NSEvent) {
        drag = nil
    }
}

/// Creates each window once and brings it back later, so a scene of the app has one
/// window and its frame is remembered.
@MainActor
public final class ShellWindowHost {
    private var windows: [String: ShellWindow] = [:]
    private let frameAutosavePrefix: String

    /// What any of its windows did, with the window's id.
    public var onWindowEvent: (@MainActor (_ id: String, _ event: ShellWindowEvent) -> Void)?

    /// `frameAutosavePrefix` names the app's windows in its defaults, as `<prefix>.<id>`.
    /// Keep it once chosen: a new one forgets every remembered frame.
    public init(frameAutosavePrefix: String) {
        self.frameAutosavePrefix = frameAutosavePrefix
    }

    public func window(id: String) -> ShellWindow? {
        windows[id]
    }

    /// Shows the window `id`, creating it the first time with `content` as its root.
    ///
    /// When the window later gains or loses its title bar (`usesSquareCorners`, a bar change
    /// through `WindowChromeBridge`) or enters fullscreen, AppKit rebuilds its frame view and the
    /// root appears again: its `.task` and `.onAppear` run again and the running `.task` is
    /// cancelled. Start work that must run once per launch outside the view, guarded by a flag.
    public func present<Content: View>(
        id: String,
        title: String,
        size: NSSize,
        isResizable: Bool = true,
        remembersFrame: Bool = true,
        @ViewBuilder content: () -> Content
    ) {
        if let existing = windows[id] {
            existing.makeKeyAndOrderFront(nil)
            return
        }
        var mask: NSWindow.StyleMask = [.titled, .closable, .miniaturizable, .fullSizeContentView]
        if isResizable { mask.insert(.resizable) }
        let window = ShellWindow(
            contentRect: NSRect(origin: .zero, size: size),
            styleMask: mask,
            backing: .buffered,
            defer: false
        )
        window.title = title
        window.isUserResizable = isResizable
        window.isReleasedWhenClosed = false
        // The host creates its windows itself; system restoration would duplicate them.
        window.isRestorable = false
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = true
        window.titlebarSeparatorStyle = .none
        window.tabbingMode = .disallowed
        window.onEvent = { [weak self] event in self?.onWindowEvent?(id, event) }
        window.contentView = NSHostingView(rootView: content())
        window.center()
        if remembersFrame {
            window.setFrameAutosaveName("\(frameAutosavePrefix).\(id)")
        }
        windows[id] = window
        window.makeKeyAndOrderFront(nil)
    }
}
#endif
