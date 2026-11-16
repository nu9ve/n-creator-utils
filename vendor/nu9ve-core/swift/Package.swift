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
        .library(name: "Nu9veTheme", targets: ["Nu9veTheme"])
    ],
    targets: [
        // macOS-only code, compiled empty elsewhere. Knows no app: each passes its own names.
        .target(
            name: "NuMacShell",
            swiftSettings: strictSettings
        ),
        .target(name: "Nu9veTheme", swiftSettings: strictSettings),
        .testTarget(name: "Nu9veThemeTests", dependencies: ["Nu9veTheme"], swiftSettings: strictSettings),
        .testTarget(
            name: "NuMacShellTests",
            dependencies: ["NuMacShell"],
            swiftSettings: strictSettings
        )
    ]
)
