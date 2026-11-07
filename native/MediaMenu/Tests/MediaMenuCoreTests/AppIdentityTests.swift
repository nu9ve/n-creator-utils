import Foundation
import Testing
@testable import MediaMenuCore

// macos-app-v1 §5: About reads its facts from the bundle, and the build script stamps them.

@Test func identityReadsTheStampedBundle() {
    let info: [String: Any] = ["CFBundleShortVersionString": "0.2.0", "CFBundleVersion": "7",
                               "Nu9veBuildID": "d220fa121c80", "Nu9veBuildDate": "2026-11-07T07:49:05+00:00",
                               "NSHumanReadableCopyright": "© 2026 nu9ve"]
    let identity = AppIdentity(name: "nüün", tagline: "x", info: info)
    #expect(!identity.isDevelopmentBuild)
    #expect(identity.revisionLine == "d220fa121c80 · 2026-11-07T07:49")
    let line = identity.diagnostics(system: OperatingSystemVersion(majorVersion: 26, minorVersion: 1, patchVersion: 0))
    #expect(line.hasPrefix("nüün 0.2.0 (7) d220fa121c80 · macOS 26.1.0 · "))
}

@Test func unstampedBundleIsADevelopmentBuild() {
    let identity = AppIdentity(name: "nüün", tagline: "x", info: ["CFBundleShortVersionString": "$(MARKETING_VERSION)", "Nu9veBuildID": " "])
    #expect(identity.isDevelopmentBuild)
    #expect(identity.buildID == nil && identity.revisionLine == nil)
    #expect(identity.copyright == AppIdentity.defaultCopyright)
}

@Test func buildScriptStampsTheSameCopyright() throws {
    // The installer's Info.plist and About must agree (macos-app-v1 §5).
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let script = try String(contentsOf: repo.appendingPathComponent("scripts/build_media_app.py"), encoding: .utf8)
    #expect(script.contains("'NSHumanReadableCopyright': '\(AppIdentity.defaultCopyright)'"))
    #expect(script.contains("'Nu9veBuildID'") && script.contains("'Nu9veBuildDate'"))
}
