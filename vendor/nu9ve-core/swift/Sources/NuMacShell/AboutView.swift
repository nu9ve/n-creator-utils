#if os(macOS)
import AppKit
import SwiftUI

/// Colors for a window that does not belong to the app's own theme type.
public struct ShellColors: Equatable, Sendable {
    public let background: Color
    public let text: Color
    public let secondaryText: Color
    public let accent: Color
    public let isDark: Bool

    public init(background: Color, text: Color, secondaryText: Color, accent: Color, isDark: Bool) {
        self.background = background
        self.text = text
        self.secondaryText = secondaryText
        self.accent = accent
        self.isDark = isDark
    }
}

/// The content of the "About" window required by the nu9ve macOS app contract.
/// Use it in a `Window` with `.windowResizability(.contentSize)`.
public struct AboutView<Logo: View>: View {
    private let identity: AppIdentity
    private let colors: ShellColors
    private let drawsBackground: Bool
    private let systemButtonStyle: Bool
    private let logo: Logo
    @State private var copied = false

    /// Pass `drawsBackground` false when the window already draws its own. `systemButtonStyle` false leaves the
    /// links' button style to the environment, so an app with skins gives them its own (by default, the system's
    /// bordered buttons, as before).
    public init(
        identity: AppIdentity,
        colors: ShellColors,
        drawsBackground: Bool = true,
        systemButtonStyle: Bool = true,
        @ViewBuilder logo: () -> Logo
    ) {
        self.logo = logo()
        self.identity = identity
        self.colors = colors
        self.drawsBackground = drawsBackground
        self.systemButtonStyle = systemButtonStyle
    }

    public var body: some View {
        VStack(spacing: 14) {
            logo

            Text(identity.tagline)
                .font(.system(size: 12))
                .foregroundStyle(colors.secondaryText)
                .multilineTextAlignment(.center)

            Button {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(identity.diagnostics(), forType: .string)
                copied = true
            } label: {
                Text(copied ? "Copiado" : identity.versionLine)
                    .font(.system(size: 11, design: .monospaced))
            }
            .buttonStyle(.plain)
            .foregroundStyle(copied ? colors.accent : colors.secondaryText)
            .help("Copiar versión, sistema y arquitectura para reportar un problema")
            .accessibilityLabel("\(identity.versionLine). Copiar información de versión")

            // Which source is running, so a stale binary is obvious at a glance.
            if let revision = identity.revisionLine {
                Text(revision)
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundStyle(colors.secondaryText)
                    .textSelection(.enabled)
                    .accessibilityLabel("Compilación \(revision)")
            }

            links
                .controlSize(.small)

            Text(identity.copyright)
                .font(.system(size: 10))
                .foregroundStyle(colors.secondaryText)
        }
        .padding(.horizontal, 28)
        .padding(.vertical, 22)
        .frame(width: 340)
        .foregroundStyle(colors.text)
        .tint(colors.accent)
        .background(drawsBackground ? colors.background : .clear)
    }

    @ViewBuilder private var links: some View {
        let row = HStack(spacing: 8) {
            Link(destination: identity.website) {
                Label(identity.website.host() ?? "Sitio", systemImage: "globe")
            }
            Link(destination: identity.source) {
                Label("GitHub", systemImage: "chevron.left.forwardslash.chevron.right")
            }
        }
        if systemButtonStyle {
            row.buttonStyle(.bordered)
        } else {
            row
        }
    }
}
#endif
