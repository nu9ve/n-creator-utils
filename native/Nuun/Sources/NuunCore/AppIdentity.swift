import Foundation

// macos-app-v1 §5 · origen: ios-fractos Packages/FractosKit/Sources/NuMacShell/AppIdentity.swift.
// Divergencias: sin fecha de compilación localizada por `Date.FormatStyle` (se muestra tal cual la
// sella el instalador) y copyright por defecto con año, igual que `NSHumanReadableCopyright` del bundle.
/// Who made the app and which build this is: the same facts for About and bug reports. Version and
/// build always come from the bundle, so they cannot drift from `Info.plist`.
public struct AppIdentity: Sendable, Equatable {
    public static let defaultCopyright = "© 2026 nu9ve"
    public let name: String
    public let tagline: String
    public let version: String
    public let build: String
    /// The source the binary came from, as the installer stamps it. Nil in a plain `swift run`.
    public let buildID: String?
    /// When the installer built it, ISO 8601 as stamped.
    public let buildDate: String?
    public let website: URL
    public let source: URL
    public let copyright: String

    /// Info.plist keys shared by every nu9ve app, so one script can stamp them all.
    public enum InfoKey {
        public static let buildID = "Nu9veBuildID"
        public static let buildDate = "Nu9veBuildDate"
    }

    public init(name: String, tagline: String, info: [String: Any]?,
                website: URL = URL(string: "https://nu9ve.xyz")!, source: URL = URL(string: "https://github.com/nu9ve")!) {
        self.name = name
        self.tagline = tagline
        version = Self.stamped(info?["CFBundleShortVersionString"]) ?? "0.0.0"
        build = Self.stamped(info?["CFBundleVersion"]) ?? "0"
        buildID = Self.stamped(info?[InfoKey.buildID])
        buildDate = Self.stamped(info?[InfoKey.buildDate])
        self.website = website
        self.source = source
        copyright = Self.stamped(info?["NSHumanReadableCopyright"]) ?? Self.defaultCopyright
    }

    /// A `swift run` build has no bundle version; About says so instead of inventing one.
    public var isDevelopmentBuild: Bool { version == "0.0.0" }

    public var versionLine: String {
        isDevelopmentBuild ? String(localized: "Build de desarrollo") : String(localized: "Versión \(version) (\(build))")
    }

    /// The build's origin, e.g. `d220fa121c80 · 2026-11-07T07:49`, or nil when nothing stamped it.
    public var revisionLine: String? {
        guard let buildID else { return nil }
        guard let buildDate else { return buildID }
        return "\(buildID) · \(buildDate.prefix(16))"
    }

    /// One line to paste into a bug report.
    public func diagnostics(system: OperatingSystemVersion = ProcessInfo.processInfo.operatingSystemVersion) -> String {
        let revision = buildID.map { " \($0)" } ?? ""
        return "\(name) \(version) (\(build))\(revision) · macOS \(system.majorVersion).\(system.minorVersion).\(system.patchVersion) · \(Self.architecture)"
    }

    /// An unexpanded `$(VAR)` or an empty value means nobody stamped the bundle.
    private static func stamped(_ value: Any?) -> String? {
        guard let text = (value as? String)?.trimmingCharacters(in: .whitespaces), !text.isEmpty, !text.hasPrefix("$(") else { return nil }
        return text
    }

    private static var architecture: String {
        #if arch(arm64)
        "arm64"
        #else
        "x86_64"
        #endif
    }
}
