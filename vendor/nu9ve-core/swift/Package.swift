// swift-tools-version: 6.2
import Foundation
import PackageDescription

// scripts/check-swift.sh turns warnings into errors. It cannot be permanent: Xcode builds
// local packages with -suppress-warnings, and the compiler rejects both flags together.
let warningSettings: [SwiftSetting] = ProcessInfo.processInfo.environment["NU9VE_STRICT_WARNINGS"] == "1"
    ? [.treatAllWarnings(as: .error)]
    : []

let strictSettings: [SwiftSetting] = [
    .enableUpcomingFeature("ExistentialAny"),
    .enableUpcomingFeature("MemberImportVisibility")
] + warningSettings

let package = Package(
    name: "nu9ve-swift",
    platforms: [
        .macOS(.v15)
    ],
    products: [
        .library(name: "NuMacShell", targets: ["NuMacShell"]),
        .library(name: "Nu9veTheme", targets: ["Nu9veTheme"]),
        .library(name: "Nu9veEngine", targets: ["Nu9veEngine"])
    ],
    targets: [
        // macOS-only code, compiled empty elsewhere. Knows no app: each passes its own names.
        // Nu9veTheme gives the period bar the three commons and their measures.
        .target(
            name: "NuMacShell",
            dependencies: ["Nu9veTheme"],
            swiftSettings: strictSettings
        ),
        .target(name: "Nu9veTheme", swiftSettings: strictSettings),
        // The nu9ve-engine-v1 client: Foundation and Synchronization only.
        .target(name: "Nu9veEngine", swiftSettings: strictSettings),
        // A test engine the client's tests launch. Not a product.
        .executableTarget(
            name: "Nu9veEngineFixture",
            dependencies: ["Nu9veEngine"],
            swiftSettings: strictSettings
        ),
        .testTarget(name: "Nu9veThemeTests", dependencies: ["Nu9veTheme"], swiftSettings: strictSettings),
        .testTarget(
            name: "NuMacShellTests",
            dependencies: ["NuMacShell", "Nu9veTheme"],
            swiftSettings: strictSettings
        ),
        .testTarget(
            name: "Nu9veEngineTests",
            dependencies: ["Nu9veEngine", "Nu9veEngineFixture"],
            swiftSettings: strictSettings
        )
    ]
)
