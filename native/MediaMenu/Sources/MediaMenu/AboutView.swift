import AppKit
import SwiftUI
import MediaMenuCore

extension AppIdentity {
    static let nuun = AppIdentity(name: "nüün", tagline: String(localized: "Una multiherramienta para creadores."),
                                  info: Bundle.main.infoDictionary)
}

// macos-app-v1 §5 · origen: ios-fractos Packages/FractosKit/Sources/NuMacShell/AboutView.swift.
// Divergencias: lee el tema y el skin de nüün (`creatorPalette`) en vez de `ShellColors`, y el nombre
// va en la letra display del skin; el logo es el icono de la app.
/// About: icon, name and line, version (a click copies it for a report), build, links and copyright.
struct AboutView: View {
    @Environment(\.creatorPalette) private var palette
    let identity: AppIdentity
    @State private var copied = false
    var body: some View {
        VStack(spacing: 12) {
            Image(nsImage: NSApp.applicationIconImage).resizable().interpolation(.high)
                .frame(width: 72, height: 72).accessibilityHidden(true)
            Text(verbatim: palette.skin == .terminal ? "~$ \(identity.name)" : identity.name).font(palette.display(26))
            Text(verbatim: identity.tagline).font(.callout).foregroundStyle(palette.muted).multilineTextAlignment(.center)
            Button {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(identity.diagnostics(), forType: .string)
                copied = true
            } label: {
                Text(verbatim: copied ? String(localized: "Copiado") : identity.versionLine).font(.system(size: 11, design: .monospaced))
            }
            .buttonStyle(.plain)
            .foregroundStyle(copied ? palette.accentText : palette.muted)
            .help(Text("Copiar versión, sistema y arquitectura para reportar un problema"))
            .accessibilityLabel(Text(verbatim: identity.versionLine))
            .accessibilityHint(Text("Copiar versión, sistema y arquitectura para reportar un problema"))
            if let revision = identity.revisionLine {
                Text(verbatim: revision).font(.system(size: 10, design: .monospaced)).foregroundStyle(palette.muted)
                    .textSelection(.enabled)
            }
            HStack(spacing: 8) {
                Button { NSWorkspace.shared.open(identity.website) } label: { Label(identity.website.host() ?? "nu9ve.xyz", systemImage: "globe") }
                Button { NSWorkspace.shared.open(identity.source) } label: { Label("GitHub", systemImage: "chevron.left.forwardslash.chevron.right") }
            }
            .controlSize(.small)
            Text(verbatim: identity.copyright).font(.caption2).foregroundStyle(palette.muted)
        }
        .padding(.horizontal, 28).padding(.vertical, 24)
        .frame(width: 340)
    }
}
