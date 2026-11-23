import Foundation
import Testing
@testable import MediaMenuCore

/// The `nu9ve` binary built from the pinned core (`cargo build --release -p nu9ve-mcp --bin nu9ve` in engine/).
private let agentBinary = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
    .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("engine/target/release/nu9ve").path

/// Always QA (nu9ve-agent-v1 §1): every folder the agent touches is temporary, so nothing reaches this Mac's agent,
/// its device.json, its keychain or its LaunchAgents.
private func qaEnvironment(_ root: URL) -> [String: String] {
    var env = ProcessInfo.processInfo.environment
    env["NU9VE_AGENT_DIR"] = root.appendingPathComponent("agent").path
    env["NU9VE_DEVICE_FILE"] = root.appendingPathComponent("device.json").path
    env["NU9VE_ENGINES_DIR"] = root.appendingPathComponent("engines").path
    env["NU9VE_VOLUMES_DIR"] = root.appendingPathComponent("volumes").path
    env["HOME"] = root.appendingPathComponent("home").path
    return env
}

private func json(_ text: String) throws -> JSONValue { try JSONDecoder().decode(JSONValue.self, from: Data(text.utf8)) }

@Test func agentStateReadsWhatTheBinaryPrints() throws {
    let absent = SuiteAgentStatus(state: try json(#"{"state":"absent","installed":null,"bundled":{"version":"0.3.0","build":"public"},"action":"install"}"#))
    #expect(absent.offersInstall && !absent.offersUpdate && !absent.isRunning)
    let outdated = SuiteAgentStatus(state: try json(#"{"state":"outdated","installed":{"version":"0.2.2","build":"public"},"bundled":{"version":"0.3.0","build":"public"},"action":"offer_update"}"#))
    #expect(outdated.offersUpdate && outdated.installedVersion == "0.2.2" && outdated.bundledVersion == "0.3.0")
    // An installed agent that is newer than what nüün carries is kept: nothing is offered (§3).
    let newer = SuiteAgentStatus(state: try json(#"{"state":"running","installed":{"version":"0.9.0","build":"cloud"},"bundled":{"version":"0.3.0","build":"public"},"action":"keep"}"#))
    #expect(newer.isRunning && !newer.offersInstall && !newer.offersUpdate)
    #expect(SuiteAgentStatus(state: try json(#"{"state":"other"}"#)).state == "unavailable")
}

@Test func noBinaryIsUnavailableNotAnError() async {
    #expect(await SuiteAgent.state(binary: "").state == "unavailable")
    #expect(await SuiteAgent.state(binary: "/nonexistent/nu9ve").state == "unavailable")
}

@Test(.enabled(if: FileManager.default.isExecutableFile(atPath: agentBinary)))
func installOnlyWhenAskedThenItIsKept() async throws {
    let root = URL(fileURLWithPath: "/private/tmp").appendingPathComponent("nuun-agent-swift-" + UUID().uuidString)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let env = qaEnvironment(root)
    let before = await SuiteAgent.state(binary: agentBinary, environment: env)
    #expect(before.state == "absent" && before.offersInstall)
    // Asking the state twice installs nothing.
    #expect(!FileManager.default.fileExists(atPath: root.appendingPathComponent("agent/agent.json").path))
    let installed = try await SuiteAgent.install(binary: agentBinary, environment: env)
    #expect(installed["action"].string == "install")
    #expect(installed["startup"].string == "none")  // QA never registers a LaunchAgent (§1).
    let record = try json(String(contentsOf: root.appendingPathComponent("agent/agent.json"), encoding: .utf8))
    #expect(record["installed_by"].string == "nuun")
    // Installed, not running: nüün never starts `serve` itself.
    let after = await SuiteAgent.state(binary: agentBinary, environment: env)
    #expect(after.state == "installed" && after.action == "keep" && !after.offersUpdate)
    #expect(try await SuiteAgent.install(binary: agentBinary, environment: env)["action"].string == "keep")
}
