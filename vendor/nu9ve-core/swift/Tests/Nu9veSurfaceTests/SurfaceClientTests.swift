import Darwin
import Foundation
import Nu9veMIDI
import Testing
@testable import Nu9veSurface

/// A fake agent: a Unix socket in `<folder>/runtime/surface` that records the frames it gets and sends
/// what the test asks. One client at a time.
final class FakeAgent: @unchecked Sendable {
    let path: String
    private let listener: Int32
    private let lock = NSLock()
    private var client: Int32 = -1
    private var frames: [[String: Any]] = []

    init(folder: URL, mode: mode_t = 0o600) throws {
        let runtime = folder.appendingPathComponent("runtime")
        try FileManager.default.createDirectory(at: runtime, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        path = runtime.appendingPathComponent("surface").path
        unlink(path)
        listener = socket(AF_UNIX, SOCK_STREAM, 0)
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let bytes = Array(path.utf8)
        withUnsafeMutableBytes(of: &address.sun_path) { $0.copyBytes(from: bytes); $0[bytes.count] = 0 }
        let bound = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(listener, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) }
        }
        precondition(bound == 0 && listen(listener, 4) == 0)
        chmod(path, mode)
        Thread.detachNewThread { [self] in accept() }
    }

    private func accept() {
        while true {
            let fd = Darwin.accept(listener, nil, nil)
            if fd < 0 { return }
            lock.withLock { client = fd }
            var header = [UInt8](repeating: 0, count: 4)
            while recv(fd, &header, 4, MSG_WAITALL) == 4 {
                let size = header.reduce(0) { $0 << 8 | Int($1) }
                var payload = [UInt8](repeating: 0, count: size)
                guard recv(fd, &payload, size, MSG_WAITALL) == size else { break }
                if let m = try? JSONSerialization.jsonObject(with: Data(payload)) as? [String: Any] { lock.withLock { frames.append(m) } }
            }
        }
    }

    func send(_ message: [String: Any]) {
        let payload = try! JSONSerialization.data(withJSONObject: message)
        let n = payload.count
        let frame = [UInt8(n >> 24), UInt8(n >> 16 & 0xff), UInt8(n >> 8 & 0xff), UInt8(n & 0xff)] + [UInt8](payload)
        let fd = lock.withLock { client }
        _ = frame.withUnsafeBytes { write(fd, $0.baseAddress, $0.count) }
    }

    var received: [[String: Any]] { lock.withLock { frames } }
    func types() -> [String] { received.compactMap { $0["type"] as? String } }

    /// The agent goes away: its socket closes and its file is removed.
    func stop() {
        let fd = lock.withLock { client }
        if fd >= 0 { shutdown(fd, SHUT_RDWR); close(fd) }
        shutdown(listener, SHUT_RDWR); close(listener)
        unlink(path)
    }
}

/// A short folder: Unix socket paths end at 104 bytes.
private func folder() -> URL {
    URL(fileURLWithPath: "/tmp/nu9ve-surface-\(UUID().uuidString.prefix(8))", isDirectory: true)
}

@MainActor
private func eventually(_ seconds: Double = 5, _ check: () -> Bool) async -> Bool {
    let deadline = Date().addingTimeInterval(seconds)
    while Date() < deadline {
        if check() { return true }
        try? await Task.sleep(for: .milliseconds(20))
    }
    return check()
}

@MainActor
private func client(_ dir: URL) -> SurfaceClient {
    SurfaceClient(app: "player", profileID: nil,
                  catalog: [.init(id: "record.toggle", title: "Grabar", group: "Grabación", color: .red),
                            .init(id: "mix.level", title: "Nivel", group: "Mezcla", kind: .continuous)],
                  states: [.init("record.toggle", .available)], agentFolder: dir)
}

@MainActor
@Suite(.serialized) struct SurfaceClientTests {
    @Test func helloStatesFocusActionsAndBye() async throws {
        let dir = folder()
        defer { try? FileManager.default.removeItem(at: dir) }
        let agent = try FakeAgent(folder: dir)
        defer { agent.stop() }
        let surface = client(dir)
        var modes: [SurfaceClient.Mode] = []
        var actions: [SurfaceClient.Action] = []
        var pages: [(String, Bool)] = []
        surface.onMode = { modes.append($0) }
        surface.onAction = { actions.append($0) }
        surface.onPage = { pages.append(($0, $1)) }
        surface.start()
        #expect(surface.mode == .agent && modes == [.agent])

        #expect(await eventually { agent.types() == ["hello"] })
        let hello = agent.received[0]
        #expect(hello["contract"] as? String == "nu9ve-surface/1" && hello["app"] as? String == "player")
        #expect(hello["profile_id"] is NSNull && (hello["pid"] as? Int) == Int(getpid()))
        #expect((hello["catalog"] as? [[String: Any]])?.first?["color"] as? String == "red")
        #expect((hello["states"] as? [[String: Any]])?.first?["state"] as? String == "available")

        agent.send(["type": "welcome", "contract": "nu9ve-surface/1", "agent": "0.3.0",
                    "devices": [["device": "pad-1", "profile": "novation-launchpad-pro", "port": "Launchpad Pro Standalone Port"]]])
        agent.send(["type": "page", "page": true, "device": "pad-1"])
        agent.send(["type": "action", "action": "record.toggle", "device": "pad-1", "control": "pad-1-1", "via": "page", "host_ns": 123_000_000])
        agent.send(["type": "action", "action": "mix.level", "value": 0.42, "device": "pad-1", "control": "pad-2-1", "via": "zone", "host_ns": 124_000_000])
        agent.send(["type": "something-later"])
        #expect(await eventually { actions.count == 2 })
        #expect(surface.devices.first?.port == "Launchpad Pro Standalone Port")
        #expect(surface.pages == ["pad-1"] && pages.first! == ("pad-1", true))
        #expect(actions[0].action == "record.toggle" && actions[0].hostNS == 123_000_000 && actions[0].via == "page")
        #expect(actions[1].value == 0.42 && actions[1].via == "zone")

        // Only what changed: the same state twice is sent once, with its light.
        surface.update([.init("record.toggle", .active, light: MIDILight(.red, motion: .pulse))])
        surface.update([.init("record.toggle", .active, light: MIDILight(.red, motion: .pulse))])
        surface.focus()
        surface.stop()
        #expect(await eventually { agent.types() == ["hello", "states", "focus", "bye"] })
        let states = agent.received[1]["states"] as? [[String: Any]]
        #expect(states?.count == 1 && (states?[0]["light"] as? [String: Any])?["motion"] as? String == "pulse")
    }

    @Test func withoutAnAgentTheAppIsDirectAndTheAgentIsTakenWhenItComes() async throws {
        let dir = folder()
        defer { try? FileManager.default.removeItem(at: dir) }
        try FileManager.default.createDirectory(at: dir.appendingPathComponent("runtime"), withIntermediateDirectories: true)
        let surface = client(dir)
        var modes: [SurfaceClient.Mode] = []
        surface.onMode = { modes.append($0) }
        surface.start()
        #expect(surface.mode == .direct && modes == [.direct])

        // The agent starts: the folder changes, the app lets go and the client connects.
        let agent = try FakeAgent(folder: dir)
        defer { agent.stop() }
        #expect(await eventually { surface.mode == .agent && agent.types() == ["hello"] })
        #expect(modes == [.direct, .agent])
        surface.stop()
    }

    @Test func aFallenAgentGetsTwoSecondsThenTheAppGoesDirect() async throws {
        let dir = folder()
        defer { try? FileManager.default.removeItem(at: dir) }
        var agent: FakeAgent? = try FakeAgent(folder: dir)
        let surface = client(dir)
        var modes: [SurfaceClient.Mode] = []
        surface.onMode = { modes.append($0) }
        surface.start()
        #expect(await eventually { agent!.types() == ["hello"] })

        // It comes back within the gap (an update): connected again, with a new hello.
        agent!.send(["type": "released"])
        #expect(await eventually { surface.mode == .waiting })
        agent!.stop()
        agent = try FakeAgent(folder: dir)
        #expect(await eventually { surface.mode == .agent && agent!.types() == ["hello"] })

        // It does not come back: after 2 s, direct.
        let lost = Date()
        agent!.stop()
        #expect(await eventually(4) { surface.mode == .direct })
        #expect(Date().timeIntervalSince(lost) >= 1.9)
        #expect(Array(modes.suffix(2)) == [.waiting, .direct])
        surface.stop()
    }

    @Test func aSocketOthersCanReachIsNotTrusted() async throws {
        let dir = folder()
        defer { try? FileManager.default.removeItem(at: dir) }
        let agent = try FakeAgent(folder: dir, mode: 0o666)
        defer { agent.stop() }
        #expect(!SurfaceClient.trusted(agent.path))
        let surface = client(dir)
        surface.start()
        #expect(surface.mode == .direct)
        try? await Task.sleep(for: .milliseconds(100))
        #expect(agent.received.isEmpty)
        surface.stop()
    }

    @Test func theAgentFolderFollowsTheContract() {
        let home = URL(fileURLWithPath: "/Users/ana", isDirectory: true)
        #expect(SurfaceClient.agentFolder(environment: [:], home: home)?.path == "/Users/ana/Library/Application Support/nu9ve/agent")
        #expect(SurfaceClient.agentFolder(environment: ["NU9VE_AGENT_DIR": "/tmp/qa/agent"], home: home)?.path == "/tmp/qa/agent")
        #expect(SurfaceClient.agentFolder(environment: ["NU9VE_AGENT_DIR": "qa/agent"], home: home) == nil)
    }
}
