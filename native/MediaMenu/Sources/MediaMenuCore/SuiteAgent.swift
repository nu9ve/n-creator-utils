import Foundation

/// The suite agent as an app sees it (nu9ve-agent-v1 §5), from the `nu9ve` binary nüün carries in its bundle.
///
/// nüün only runs two orders of that binary: `agent state`, a query, at launch and whenever Settings opens, and
/// `agent install --by nuun`, only after the person chose «Instalar el agente» or «Actualizar». It never runs
/// `serve`: without an installed agent there is no agent (§5).
public struct SuiteAgentStatus: Equatable, Sendable {
    /// `absent`, `installed`, `running` or `outdated`; `unavailable` when the bundle has no binary or it failed.
    public var state: String
    public var installedVersion: String
    public var bundledVersion: String
    /// §3 for the bundled binary: `install`, `offer_update` or `keep`.
    public var action: String
    public var problem: String

    public static let unavailable = SuiteAgentStatus(state: "unavailable", installedVersion: "", bundledVersion: "", action: "", problem: "")

    public init(state: String, installedVersion: String, bundledVersion: String, action: String, problem: String) {
        self.state = state; self.installedVersion = installedVersion; self.bundledVersion = bundledVersion
        self.action = action; self.problem = problem
    }

    /// The JSON `nu9ve agent state` prints.
    public init(state value: JSONValue) {
        let known = ["absent", "installed", "running", "outdated"]
        self.init(state: known.contains(value["state"].string) ? value["state"].string : "unavailable",
                  installedVersion: value["installed"]["version"].string, bundledVersion: value["bundled"]["version"].string,
                  action: value["action"].string, problem: "")
    }

    /// Settings offers «Instalar el agente» once, without interrupting (§5).
    public var offersInstall: Bool { state == "absent" && action == "install" }
    /// «Actualizar» when the bundled binary is newer (§3), never in silence.
    public var offersUpdate: Bool { state == "outdated" && action == "offer_update" }
    public var isRunning: Bool { state == "running" }
}

public enum SuiteAgent {
    /// The binary in the bundle (`Resources/backend/nu9ve`), or the one built in the checkout for development.
    public static func binary(resources: URL?, script: String) -> String {
        let candidates: [String?] = [resources?.appendingPathComponent("backend/nu9ve").path,
                                     URL(fileURLWithPath: script).deletingLastPathComponent().appendingPathComponent("engine/target/release/nu9ve").path]
        return candidates.compactMap { $0 }.first { FileManager.default.isExecutableFile(atPath: $0) } ?? ""
    }

    /// `nu9ve agent state`: what the app shows. Never fails: a missing binary or an error is `unavailable`.
    public static func state(binary: String, environment: [String: String] = EngineEnvironment.values) async -> SuiteAgentStatus {
        guard !binary.isEmpty else { return .unavailable }
        do { return SuiteAgentStatus(state: try await run(binary, ["agent", "state"], environment: environment)) }
        catch { var status = SuiteAgentStatus.unavailable; status.problem = error.localizedDescription; return status }
    }

    /// `nu9ve agent install --by nuun`. Only after the person asked for it in Settings.
    public static func install(binary: String, environment: [String: String] = EngineEnvironment.values) async throws -> JSONValue {
        try await run(binary, ["agent", "install", "--by", "nuun"], environment: environment)
    }

    /// One order of the binary: its JSON on stdout (at most 64 KiB), a bounded wait, stderr only for the message.
    static func run(_ binary: String, _ arguments: [String], environment: [String: String], timeout: TimeInterval = 30) async throws -> JSONValue {
        guard FileManager.default.isExecutableFile(atPath: binary) else {
            throw BackendFailure(code: "agent_binary_missing", message: "Esta build no trae el binario nu9ve.")
        }
        return try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .utility).async {
                let process = Process(), output = Pipe(), errors = Pipe()
                process.executableURL = URL(fileURLWithPath: binary)
                process.arguments = arguments
                process.environment = environment
                process.standardInput = FileHandle.nullDevice
                process.standardOutput = output; process.standardError = errors
                let out = CollectedData(), err = CollectedData(), group = DispatchGroup()
                do { try process.run() } catch { continuation.resume(throwing: error); return }
                group.enter()
                DispatchQueue.global(qos: .utility).async { out.store(output.fileHandleForReading.readDataToEndOfFile()); group.leave() }
                group.enter()
                DispatchQueue.global(qos: .utility).async { err.store(errors.fileHandleForReading.readDataToEndOfFile()); group.leave() }
                if group.wait(timeout: .now() + timeout) == .timedOut {
                    process.terminate()
                    continuation.resume(throwing: BackendFailure(code: "agent_timeout", message: "nu9ve no respondió a tiempo."))
                    return
                }
                process.waitUntilExit()
                let data = out.read()
                guard process.terminationStatus == 0, data.count <= 64 * 1024,
                      let value = try? JSONDecoder().decode(JSONValue.self, from: data) else {
                    let message = String(decoding: err.read().prefix(2048), as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
                    continuation.resume(throwing: BackendFailure(code: "agent_failed", message: message.isEmpty ? "nu9ve terminó con error." : message))
                    return
                }
                continuation.resume(returning: value)
            }
        }
    }
}
