import Foundation
import NuMacShell
import Testing

@Suite("AppIdentity")
struct AppIdentityTests {
    @Test func diagnosticsNameTheBuildAndSystem() {
        let identity = AppIdentity(name: "Demo", tagline: "x", copyright: "© 2026 nu9ve")
        let line = identity.diagnostics(
            system: OperatingSystemVersion(majorVersion: 26, minorVersion: 1, patchVersion: 2)
        )

        #expect(line.hasPrefix("Demo \(identity.version) (\(identity.build))"))
        #expect(line.contains("macOS 26.1.2"))
        #expect(identity.versionLine == "Versión \(identity.version) (\(identity.build))")
        #expect(identity.copyright == "© 2026 nu9ve")
    }

    @Test func stampedBuildIDReachesAboutAndDiagnostics() {
        let identity = AppIdentity(name: "Demo", tagline: "x", info: [
            "CFBundleShortVersionString": "0.1.0",
            "CFBundleVersion": "42",
            AppIdentity.InfoKey.buildID: "83fcace+dirty.1a2b3c",
            AppIdentity.InfoKey.buildDate: "2026-11-05T20:14:00Z"
        ])

        #expect(identity.buildID == "83fcace+dirty.1a2b3c")
        #expect(identity.revisionLine?.hasPrefix("83fcace+dirty.1a2b3c · ") == true)
        #expect(identity.diagnostics().hasPrefix("Demo 0.1.0 (42) 83fcace+dirty.1a2b3c · macOS"))
    }

    @Test func anUnstampedBuildHasNoRevision() {
        let identity = AppIdentity(name: "Demo", tagline: "x", info: [
            AppIdentity.InfoKey.buildID: "$(NU9VE_BUILD_ID)",
            AppIdentity.InfoKey.buildDate: ""
        ])

        #expect(identity.buildID == nil)
        #expect(identity.revisionLine == nil)
    }

    @Test func linksPointToTheNu9vePlacesByDefault() {
        let identity = AppIdentity(name: "Demo", tagline: "x")

        #expect(identity.website.absoluteString == "https://nu9ve.xyz")
        #expect(identity.source.absoluteString == "https://github.com/nu9ve")
    }
}
