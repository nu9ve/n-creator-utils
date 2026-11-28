import Foundation
import Nu9veMIDI
import Testing
@testable import Nu9veSurface

private func write(_ text: String, _ path: String, in root: URL) throws {
    let url = root.appendingPathComponent(path)
    try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
    try Data(text.utf8).write(to: url)
}

private func map(_ device: String) -> String {
    #"{"schemaVersion":1,"enabled":true,"devices":[\#(device)]}"#
}

@Suite struct SurfaceMapsTests {
    @Test func theFolderFollowsTheContract() {
        let home = URL(fileURLWithPath: "/Users/ana", isDirectory: true)
        #expect(SurfaceMaps.folder(environment: [:], home: home)?.path == "/Users/ana/Library/Application Support/nu9ve/controllers")
        #expect(SurfaceMaps.folder(environment: ["NU9VE_CONTROLLERS_DIR": "/tmp/c"], home: home)?.path == "/tmp/c")
        #expect(SurfaceMaps.folder(environment: ["NU9VE_CONTROLLERS_DIR": "c"], home: home) == nil)
    }

    @Test func zonesReservedRepeatedAndUnreadableMapsAreConflicts() throws {
        let root = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("nu9ve-maps-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: root) }
        // Two apps want left-8 as a zone on the Launchpad; the second also maps Session, the page control.
        try write(map(#"{"match":"Launchpad Pro","profile":"novation-launchpad-pro","mappings":[{"control":"left-8","action":"a"}],"zones":[{"when":"recording","controls":["left-8"]}]}"#),
                  "_/recorder.json", in: root)
        try write(map(#"{"match":"launchpad pro standalone","profile":"novation-launchpad-pro","mappings":[{"control":"top-5","action":"b"},{"control":"pad-1-1","action":"c"},{"control":"pad-1-1","action":"d"}],"zones":[{"when":"always","controls":["left-8"]}]}"#),
                  "_/player.json", in: root)
        // Another person's profile: never active with the two above.
        try write(map(#"{"match":"Launchpad Pro","profile":"novation-launchpad-pro","mappings":[],"zones":[{"when":"always","controls":["left-8"]}]}"#),
                  "e64/editor.json", in: root)
        try write("{", "_/broken.json", in: root)
        try write("{}", "_/.partial.tmp", in: root)

        let maps = SurfaceMaps.read(root)
        #expect(maps.entries.map(\.app).sorted() == ["broken", "editor", "player", "recorder"])
        #expect(maps.entries.first { $0.app == "recorder" }?.file?.devices.first?.zones?.first?.when == .recording)
        let conflicts = maps.conflicts()
        #expect(conflicts.contains(.init(kind: .unreadable, profile: "_", device: nil, control: nil, apps: ["broken"])))
        #expect(conflicts.contains(.init(kind: .repeated, profile: "_", device: "launchpad pro standalone", control: "pad-1-1", apps: ["player"])))
        #expect(conflicts.contains(.init(kind: .reserved, profile: "_", device: "launchpad pro standalone", control: "top-5", apps: ["player"])))
        let zones = conflicts.filter { $0.kind == .zone }
        #expect(zones.count == 1 && zones[0].control == "left-8" && zones[0].apps == ["player", "recorder"] && zones[0].profile == "_")

        // suite.json moves the page control: Session is no longer reserved, bottom-8 is.
        try write(#"{"schemaVersion":1,"devices":[{"match":"Launchpad Pro","pageControl":null}]}"#, "suite.json", in: root)
        #expect(!SurfaceMaps.read(root).conflicts().contains { $0.kind == .reserved })
    }

    @Test func aMissingFolderHasNoMaps() {
        let maps = SurfaceMaps.read(URL(fileURLWithPath: "/tmp/nu9ve-no-such-\(UUID().uuidString)"))
        #expect(maps.entries.isEmpty && maps.conflicts().isEmpty)
    }
}
