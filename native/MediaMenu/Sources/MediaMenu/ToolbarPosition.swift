import AppKit
import SwiftUI

/// Toolbar content starts after native window controls. Measure in window coordinates,
/// so adding disks/activity on the right never changes the navigation's absolute center.
struct ToolbarPosition: NSViewRepresentable {
    let changed: (CGFloat) -> Void
    final class Probe: NSView {
        var changed: ((CGFloat) -> Void)?
        private var previous: CGFloat = -1
        override func viewDidMoveToWindow() { super.viewDidMoveToWindow(); measure() }
        override func layout() { super.layout(); measure() }
        private func measure() {
            guard let window else { return }
            let center = window.contentLayoutRect.width / 2 - convert(bounds.origin, to: nil).x
            guard abs(previous - center) > 0.5 else { return }
            previous = center
            DispatchQueue.main.async { [weak self] in self?.changed?(center) }
        }
    }
    func makeNSView(context: Context) -> Probe { let view = Probe(); view.changed = changed; return view }
    func updateNSView(_ view: Probe, context: Context) { view.changed = changed; view.needsLayout = true }
}
