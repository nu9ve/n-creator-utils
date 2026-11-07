import AppKit
import SwiftUI

// nu9ve-theme-v1 §5 · origen: ios-fractos Packages/FractosKit/Sources/NuMacShell/ShellWindow.swift.
// Divergencias: el zoom propio (llenar la pantalla) solo aplica con esquinas cuadradas, para que
// Cristal conserve el zoom de AppKit que nüün ya tenía; sin `onEvent` (nüün no tiene sonido) y el
// marco previo al zoom vive en memoria. `.fullSizeContentView` solo se añade sin título.

/// A window nüün fully owns. Without `.titled` macOS draws square corners, but a window then
/// refuses keyboard focus and full screen unless its class allows them, and the system runs none
/// of its title bar's behavior. This restores all of it: it must look old and work like any window.
final class ShellWindow: NSWindow {
    private var resizeOverlay: ShellWindowResizeOverlay?
    private var frameBeforeZoom: NSRect?
    /// Whether people can change its size (About cannot). Set before the first corner style.
    var isUserResizable = true

    /// Square corners and no system title bar, for skins that draw their own.
    var usesSquareCorners = false {
        didSet { if usesSquareCorners != oldValue { applyCornerStyle() } }
    }

    override init(contentRect: NSRect, styleMask style: NSWindow.StyleMask, backing backingStoreType: NSWindow.BackingStoreType, defer flag: Bool) {
        super.init(contentRect: contentRect, styleMask: style, backing: backingStoreType, defer: flag)
        NotificationCenter.default.addObserver(self, selector: #selector(didExitFullScreen), name: NSWindow.didExitFullScreenNotification, object: self)
    }

    /// AppKit puts back the style it saved on entry, `.titled` included, after this notification
    /// is posted, so the square shape returns on the next turn of the loop.
    @objc private func didExitFullScreen() {
        DispatchQueue.main.async { [weak self] in self?.applyCornerStyle() }
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }

    override func toggleFullScreen(_ sender: Any?) {
        // Full screen needs a titled window; `applyCornerStyle` runs again on exit.
        if usesSquareCorners { styleMask.insert(.titled) }
        super.toggleFullScreen(sender)
    }

    private var drawsOwnTitleBar: Bool { usesSquareCorners && !styleMask.contains(.titled) }

    // The `perform` versions press a title bar button, and here there is none: without these,
    // the menu and its shortcuts only beep.
    override func performClose(_ sender: Any?) {
        guard drawsOwnTitleBar else { super.performClose(sender); return }
        if delegate?.windowShouldClose?(self) == false { return }
        close()
    }
    override func performMiniaturize(_ sender: Any?) {
        guard drawsOwnTitleBar else { super.performMiniaturize(sender); return }
        miniaturize(sender)
    }
    override func performZoom(_ sender: Any?) {
        guard drawsOwnTitleBar else { super.performZoom(sender); return }
        zoom(sender)
    }

    /// Between the window's own size and filling the screen beside the Dock and the menu bar.
    override func zoom(_ sender: Any?) {
        if styleMask.contains(.fullScreen) { toggleFullScreen(sender); return }
        guard usesSquareCorners, isUserResizable, let visible = screen?.visibleFrame else { super.zoom(sender); return }
        let step = ShellWindowGeometry.zoom(current: frame, visible: visible, previous: frameBeforeZoom, minimum: minSize)
        frameBeforeZoom = step.previous
        setFrame(step.frame, display: true, animate: !NSWorkspace.shared.accessibilityDisplayShouldReduceMotion)
    }

    override var isZoomed: Bool {
        guard usesSquareCorners, let visible = screen?.visibleFrame else { return super.isZoomed }
        return ShellWindowGeometry.fills(frame, visible: visible)
    }

    func applyCornerStyle() {
        guard !styleMask.contains(.fullScreen) else { return }
        if usesSquareCorners {
            styleMask.remove(.titled)
            styleMask.formUnion([.miniaturizable, .closable, .fullSizeContentView])
        } else {
            styleMask.formUnion([.titled, .miniaturizable, .closable])
            styleMask.remove(.fullSizeContentView)
        }
        if isUserResizable { styleMask.insert(.resizable) } else { styleMask.remove(.resizable) }
        updateResizeOverlay()
        hasShadow = true
        invalidateShadow()
    }

    /// Without a title bar AppKit offers no resizable edges, so an overlay provides them.
    private func updateResizeOverlay() {
        guard let contentView else { return }
        let wanted = usesSquareCorners && isUserResizable
        if wanted, resizeOverlay == nil || resizeOverlay?.superview !== contentView {
            resizeOverlay?.removeFromSuperview()
            let overlay = ShellWindowResizeOverlay(frame: contentView.bounds)
            overlay.autoresizingMask = [.width, .height]
            contentView.addSubview(overlay)
            resizeOverlay = overlay
        }
        resizeOverlay?.isHidden = !wanted
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
        if edges.contains(.right) { frame.size.width = max(minimum.width, start.width + delta.x) }
        if edges.contains(.left) {
            frame.size.width = max(minimum.width, start.width - delta.x)
            frame.origin.x = start.maxX - frame.width
        }
        if edges.contains(.top) { frame.size.height = max(minimum.height, start.height + delta.y) }
        if edges.contains(.bottom) {
            frame.size.height = max(minimum.height, start.height - delta.y)
            frame.origin.y = start.maxY - frame.height
        }
        return frame
    }

    /// A window that already fills the screen has nothing to go back to, so it gets a size of its own.
    static func zoom(current: NSRect, visible: NSRect, previous: NSRect?, minimum: NSSize) -> (frame: NSRect, previous: NSRect?) {
        guard fills(current, visible: visible) else { return (visible, current) }
        if let previous, let fitted = fitted(previous, in: visible, minimum: minimum) { return (fitted, nil) }
        let width = min(visible.width, max(minimum.width, visible.width * 0.75))
        let height = min(visible.height, max(minimum.height, visible.height * 0.75))
        return (NSRect(x: visible.midX - width / 2, y: visible.midY - height / 2, width: width, height: height), nil)
    }

    static func fills(_ frame: NSRect, visible: NSRect) -> Bool {
        abs(frame.width - visible.width) < 2 && abs(frame.height - visible.height) < 2
    }

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
        let left = NSRect(x: 0, y: corner, width: side, height: height - corner * 2)
        let right = NSRect(x: width - side, y: corner, width: side, height: height - corner * 2)
        let bottom = NSRect(x: corner, y: 0, width: width - corner * 2, height: side)
        let upper = NSRect(x: corner, y: height - top, width: width - corner * 2, height: top)
        let corners = [NSRect(x: 0, y: 0, width: corner, height: side), NSRect(x: 0, y: 0, width: side, height: corner),
                       NSRect(x: width - corner, y: 0, width: corner, height: side), NSRect(x: width - side, y: 0, width: side, height: corner),
                       NSRect(x: 0, y: height - top, width: corner, height: top), NSRect(x: width - corner, y: height - top, width: corner, height: top)]
        if #available(macOS 15.0, *) {
            let positions: [NSCursor.FrameResizePosition] = [.bottomLeft, .bottomLeft, .bottomRight, .bottomRight, .topLeft, .topRight]
            addCursorRect(left, cursor: .frameResize(position: .left, directions: .all))
            addCursorRect(right, cursor: .frameResize(position: .right, directions: .all))
            addCursorRect(bottom, cursor: .frameResize(position: .bottom, directions: .all))
            addCursorRect(upper, cursor: .frameResize(position: .top, directions: .all))
            for (rect, position) in zip(corners, positions) { addCursorRect(rect, cursor: .frameResize(position: position, directions: .all)) }
        } else {
            // macOS 14 has no diagonal resize cursors: corners show the crosshair.
            for rect in [left, right] { addCursorRect(rect, cursor: .resizeLeftRight) }
            for rect in [bottom, upper] { addCursorRect(rect, cursor: .resizeUpDown) }
            for rect in corners { addCursorRect(rect, cursor: .crosshair) }
        }
    }

    override func mouseDown(with event: NSEvent) {
        guard let window else { return }
        drag = (window.frame, NSEvent.mouseLocation, edges(at: convert(event.locationInWindow, from: nil)))
    }
    override func mouseDragged(with event: NSEvent) {
        guard let window, let drag else { return }
        let mouse = NSEvent.mouseLocation
        let delta = NSPoint(x: mouse.x - drag.mouse.x, y: mouse.y - drag.mouse.y)
        window.setFrame(ShellWindowGeometry.resized(start: drag.start, edges: drag.edges, delta: delta, minimum: window.minSize), display: true)
    }
    override func mouseUp(with event: NSEvent) {
        drag = nil
        // nüün remembers each mode's frame when a live resize ends; this drag is one too.
        if let window { window.delegate?.windowDidEndLiveResize?(Notification(name: NSWindow.didEndLiveResizeNotification, object: window)) }
    }
}

/// Lets a SwiftUI bar reach the window it sits in, for its period close/minimize/zoom controls.
@MainActor final class WindowHandle {
    weak var window: NSWindow?
}

struct WindowHandleReader: NSViewRepresentable {
    let handle: WindowHandle
    final class Probe: NSView {
        var handle: WindowHandle?
        override func viewDidMoveToWindow() { super.viewDidMoveToWindow(); handle?.window = window }
    }
    func makeNSView(context: Context) -> Probe { let view = Probe(); view.handle = handle; return view }
    func updateNSView(_ view: Probe, context: Context) { view.handle = handle; handle.window = view.window }
}
