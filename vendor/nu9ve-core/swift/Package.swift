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
        .library(name: "Nu9veFormat", targets: ["Nu9veFormat"]),
        .library(name: "Nu9veEngine", targets: ["Nu9veEngine"]),
        .library(name: "Nu9veVision", targets: ["Nu9veVision"]),
        .library(name: "Nu9veScore", targets: ["Nu9veScore"]),
        .library(name: "Nu9veAction", targets: ["Nu9veAction"]),
        .library(name: "Nu9veActionProgress", targets: ["Nu9veActionProgress"]),
        .library(name: "Nu9veMIDI", targets: ["Nu9veMIDI"]),
        .library(name: "Nu9veRecording", targets: ["Nu9veRecording"]),
        .library(name: "Nu9veSurface", targets: ["Nu9veSurface"])
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
        // nu9ve-controls-v1: values, steps, readouts, time codes and instants. Foundation only, no UI.
        .target(name: "Nu9veFormat", swiftSettings: strictSettings),
        // The nu9ve-engine-v1 client: Foundation and Synchronization only.
        .target(name: "Nu9veEngine", swiftSettings: strictSettings),
        // Apple Vision on one image: faces, bodies, heads from pose, person masks. Vision, CoreVideo, CoreGraphics.
        .target(name: "Nu9veVision", swiftSettings: strictSettings),
        // The offline music engine: synths, drums, world percussion, FX, buses and mastering, with revisions.
        .target(name: "Nu9veScore", swiftSettings: strictSettings),
        // nu9ve-action-v1: the record of a long action and what the view shows from its reads. Foundation only.
        .target(name: "Nu9veAction", swiftSettings: strictSettings),
        // The common view of a long action (nu9ve-interaction-v1 §7): screen, compact and indicator, drawn by the skin.
        .target(
            name: "Nu9veActionProgress",
            dependencies: ["Nu9veAction", "Nu9veFormat", "Nu9veTheme", "NuMacShell"],
            swiftSettings: strictSettings
        ),
        // nu9ve-midi in Swift (messages, profiles, detection) plus lights, maps, live reading and CoreMIDI.
        // The profiles of data/midi/ are embedded (scripts/embed-midi-profiles.py), like the crate's include_str!.
        .target(name: "Nu9veMIDI", swiftSettings: strictSettings),
        // nu9ve-recording-v1: the reader, the producer and a watcher of the recording signal. Foundation, Darwin, os.
        .target(name: "Nu9veRecording", swiftSettings: strictSettings),
        // nu9ve-surface-v1, the app side: the client of the agent's controller socket and the maps of every app.
        .target(name: "Nu9veSurface", dependencies: ["Nu9veMIDI"], swiftSettings: strictSettings),
        // A renderer the score tests and the comparison with the apps' engines use. Not a product.
        .executableTarget(name: "Nu9veScoreFixture", dependencies: ["Nu9veScore"], swiftSettings: strictSettings),
        // A test engine the client's tests launch. Not a product.
        .executableTarget(
            name: "Nu9veEngineFixture",
            dependencies: ["Nu9veEngine"],
            swiftSettings: strictSettings
        ),
        .testTarget(name: "Nu9veThemeTests", dependencies: ["Nu9veTheme"], swiftSettings: strictSettings),
        .testTarget(name: "Nu9veFormatTests", dependencies: ["Nu9veFormat"], swiftSettings: strictSettings),
        .testTarget(
            name: "NuMacShellTests",
            dependencies: ["NuMacShell", "Nu9veTheme"],
            swiftSettings: strictSettings
        ),
        .testTarget(name: "Nu9veActionTests", dependencies: ["Nu9veAction"], swiftSettings: strictSettings),
        .testTarget(
            name: "Nu9veActionProgressTests",
            dependencies: ["Nu9veActionProgress", "Nu9veAction", "Nu9veTheme"],
            swiftSettings: strictSettings
        ),
        .testTarget(name: "Nu9veMIDITests", dependencies: ["Nu9veMIDI"], swiftSettings: strictSettings),
        .testTarget(name: "Nu9veRecordingTests", dependencies: ["Nu9veRecording"], swiftSettings: strictSettings),
        .testTarget(name: "Nu9veSurfaceTests", dependencies: ["Nu9veSurface", "Nu9veMIDI"], swiftSettings: strictSettings),
        .testTarget(name: "Nu9veVisionTests", dependencies: ["Nu9veVision"], swiftSettings: strictSettings),
        .testTarget(name: "Nu9veScoreTests", dependencies: ["Nu9veScore"], exclude: ["Fixtures"], swiftSettings: strictSettings),
        .testTarget(
            name: "Nu9veEngineTests",
            dependencies: ["Nu9veEngine", "Nu9veEngineFixture"],
            swiftSettings: strictSettings
        )
    ]
)
