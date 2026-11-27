import AppKit
import SwiftUI
import NuMacShell
import NuunCore

// macos-app-v1 §5 · NuMacShell `AboutView` y `AppIdentity` desde la copia fijada de nu9ve/core (F3, 2026-11-26).
// nüün sólo pone su logo (el icono y el nombre en la letra display del skin) y sus colores.

extension AppIdentity {
    /// Version, build and copyright come from the bundle the installer stamped (`build_media_app.py`).
    static let nuun = AppIdentity(name: "nüün", tagline: String(localized: "Una multiherramienta para creadores."))
}

/// About: the suite's view with nüün's palette. The window draws the background, so the view does not.
struct NuunAboutView: View {
    @Environment(\.creatorPalette) private var palette
    var body: some View {
        AboutView(identity: .nuun,
                  colors: ShellColors(background: palette.canvas, text: palette.ink, secondaryText: palette.muted,
                                      accent: palette.accentText, isDark: palette.dark),
                  drawsBackground: false) {
            VStack(spacing: 10) {
                Image(nsImage: NSApp.applicationIconImage).resizable().interpolation(.high)
                    .frame(width: 72, height: 72).accessibilityHidden(true)
                Text(verbatim: palette.skin == .terminal ? "~$ \(AppIdentity.nuun.name)" : AppIdentity.nuun.name)
                    .font(palette.display(26)).foregroundStyle(palette.ink)
            }
        }
    }
}
