import SwiftUI
import Nu9veTheme

/// Marks the edges of a horizontal scroll row that still hide content, drawn the way each skin would:
/// a fade on glass and gothic, a dither strip on the period skins (no gradients in 1990 chrome) and a
/// text caret on the terminal. Purely visual: the row scrolls as before and the hint never takes clicks.
struct HorizontalScrollHints: ViewModifier {
    @State private var edges = Edges()

    struct Edges: Equatable {
        var leading = false
        var trailing = false
    }

    func body(content: Content) -> some View {
        content
            .onScrollGeometryChange(for: Edges.self) { geometry in
                let x = geometry.contentOffset.x + geometry.contentInsets.leading
                return Edges(leading: x > 2, trailing: x + geometry.containerSize.width < geometry.contentSize.width - 2)
            } action: { _, new in
                edges = new
            }
            .overlay(alignment: .leading) { if edges.leading { ScrollEdgeHint(edge: .leading) } }
            .overlay(alignment: .trailing) { if edges.trailing { ScrollEdgeHint(edge: .trailing) } }
            .animation(.easeOut(duration: 0.15), value: edges)
    }
}

extension View {
    func horizontalScrollHints() -> some View { modifier(HorizontalScrollHints()) }
}

struct ScrollEdgeHint: View {
    let edge: HorizontalEdge
    @Environment(\.creatorPalette) private var palette

    var body: some View {
        Group {
            switch palette.skin {
            case .terminal:
                Text(edge == .leading ? "‹" : "›")
                    .font(.system(.title3, design: .monospaced).weight(.bold))
                    .foregroundStyle(palette.accentText)
                    .padding(.horizontal, 4)
                    .frame(maxHeight: .infinity)
                    .background(palette.canvas)
            case .classicMac, .win9x:
                HStack(spacing: 0) {
                    if edge == .trailing { Dither(color: palette.canvas).frame(width: 10) }
                    Image(systemName: edge == .leading ? "arrowtriangle.left.fill" : "arrowtriangle.right.fill")
                        .font(.system(size: 8))
                        .foregroundStyle(palette.ink)
                        .frame(width: 14)
                        .frame(maxHeight: .infinity)
                        .background(palette.canvas)
                    if edge == .leading { Dither(color: palette.canvas).frame(width: 10) }
                }
            default:
                let stops = [palette.canvas.opacity(0), palette.canvas.opacity(0.95)]
                ZStack(alignment: edge == .leading ? .leading : .trailing) {
                    LinearGradient(colors: edge == .leading ? stops.reversed() : stops,
                                   startPoint: .leading, endPoint: .trailing)
                    Image(systemName: edge == .leading ? "chevron.left" : "chevron.right")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(palette.muted)
                        .padding(.horizontal, 4)
                }
                .frame(width: 36)
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// A 50 % checkerboard of `color` on 1-point cells: the period skins' way to fade.
private struct Dither: View {
    let color: Color
    var body: some View {
        Canvas { context, size in
            var path = Path()
            for y in 0..<Int(size.height.rounded(.up)) {
                for x in stride(from: y % 2, to: Int(size.width.rounded(.up)), by: 2) {
                    path.addRect(CGRect(x: x, y: y, width: 1, height: 1))
                }
            }
            context.fill(path, with: .color(color))
        }
    }
}
