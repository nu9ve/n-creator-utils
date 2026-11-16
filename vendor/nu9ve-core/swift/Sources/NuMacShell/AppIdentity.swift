import Foundation

/// Who made the app and which build this is: the same facts for About, Help and
/// bug reports. Version and build always come from the bundle, so they cannot
/// drift from `Info.plist`.
public struct AppIdentity: Sendable {
    public let name: String
    public let tagline: String
    public let version: String
    public let build: String
    /// The source the binary came from, as the install script stamps it: a commit, plus
    /// `+dirty.<hash>` when the working tree had changes. Nil in a plain Xcode build.
    public let buildID: String?
    /// When the install script built it, ISO 8601.
    public let buildDate: Date?
    public let website: URL
    public let source: URL
    public let copyright: String

    /// Info.plist keys shared by every nu9ve app, so one script can stamp them all.
    public enum InfoKey {
        public static let buildID = "Nu9veBuildID"
        public static let buildDate = "Nu9veBuildDate"
    }

    public init(
        name: String,
        tagline: String,
        bundle: Bundle = .main,
        info: [String: Any]? = nil,
        website: URL = URL(string: "https://nu9ve.xyz")!,
        source: URL = URL(string: "https://github.com/nu9ve")!,
        copyright: String? = nil
    ) {
        let info = info ?? bundle.infoDictionary
        self.name = name
        self.tagline = tagline
        self.version = info?["CFBundleShortVersionString"] as? String ?? "0.0.0"
        self.build = info?["CFBundleVersion"] as? String ?? "0"
        self.buildID = Self.stamped(info?[InfoKey.buildID])
        self.buildDate = Self.stamped(info?[InfoKey.buildDate]).flatMap { try? Date($0, strategy: .iso8601) }
        self.website = website
        self.source = source
        self.copyright = copyright
            ?? info?["NSHumanReadableCopyright"] as? String
            ?? "© nu9ve"
    }

    public var versionLine: String {
        "Versión \(version) (\(build))"
    }

    /// The build's origin, e.g. `83fcace+dirty.1a2b3c · 5 nov 2026, 20:14`, or nil when
    /// nothing stamped it.
    public var revisionLine: String? {
        guard let buildID else { return nil }
        guard let buildDate else { return buildID }
        return "\(buildID) · \(buildDate.formatted(date: .abbreviated, time: .shortened))"
    }

    /// One line to paste into a bug report.
    public func diagnostics(
        system: OperatingSystemVersion = ProcessInfo.processInfo.operatingSystemVersion
    ) -> String {
        let revision = buildID.map { " \($0)" } ?? ""
        return "\(name) \(version) (\(build))\(revision) · macOS \(system.majorVersion).\(system.minorVersion).\(system.patchVersion) · \(Self.architecture)"
    }

    /// An unexpanded `$(VAR)` or an empty value means nobody stamped the bundle.
    private static func stamped(_ value: Any?) -> String? {
        guard let text = (value as? String)?.trimmingCharacters(in: .whitespaces),
              !text.isEmpty, !text.hasPrefix("$(") else { return nil }
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
