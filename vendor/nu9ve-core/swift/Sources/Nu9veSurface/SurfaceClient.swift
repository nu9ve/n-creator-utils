import Darwin
import Foundation
import Nu9veMIDI

/// The app side of nu9ve-surface-v1: it talks to the agent, the only owner of the controller, over
/// `<agent folder>/runtime/surface` (frames of 4 bytes big-endian plus JSON, at most 64 KiB).
///
/// The app keeps its catalog and states; the client sends only what changed and hands it each action.
/// `mode` says who owns the controller:
/// - `.agent`: connected. The app must not open the controller.
/// - `.waiting`: the agent went away or released it (§7). For up to 2 s the client retries; the app waits.
/// - `.direct`: no agent. The app opens the controller itself, as without the suite. The client watches the
///   runtime folder (by events, no timer) and, when the agent comes back, switches to `.agent`: `onMode`
///   runs first, so the app lets go of the controller before the client connects (§1).
@MainActor
public final class SurfaceClient {
    public enum Mode: Equatable, Sendable { case agent, waiting, direct }

    public struct Item: Equatable, Sendable {
        public enum Kind: String, Sendable { case trigger, continuous }
        public var id: String, title: String, group: String, kind: Kind
        public var color: MIDIColor?
        public var arguments: [(value: String, title: String)]
        public init(id: String, title: String, group: String, kind: Kind = .trigger, color: MIDIColor? = nil,
                    arguments: [(value: String, title: String)] = []) {
            self.id = id; self.title = title; self.group = group; self.kind = kind; self.color = color; self.arguments = arguments
        }
        public static func == (a: Item, b: Item) -> Bool {
            a.id == b.id && a.title == b.title && a.group == b.group && a.kind == b.kind && a.color == b.color
                && a.arguments.map(\.value) == b.arguments.map(\.value) && a.arguments.map(\.title) == b.arguments.map(\.title)
        }
    }

    /// The state of one action (by argument), with the light the app asks for, if any (§3).
    public struct State: Hashable, Sendable {
        public var action: String, argument: String?
        public var state: MIDIActionState
        public var light: MIDILight?
        public init(_ action: String, argument: String? = nil, _ state: MIDIActionState, light: MIDILight? = nil) {
            self.action = action; self.argument = argument; self.state = state; self.light = light
        }
        var key: String { action + "\u{0}" + (argument ?? "") }
    }

    public struct Device: Equatable, Sendable {
        public var device: String, profile: String?, port: String
    }

    /// An action from the controller, to run as if it came from the app's menu. Continuous actions bring
    /// `value` (0–1) or `delta`; `hostNS` is the host clock of the MIDI message (recording §5).
    public struct Action: Equatable, Sendable {
        public var action: String, argument: String?
        public var value: Double?, delta: Int?
        public var device: String, control: String, via: String
        public var hostNS: UInt64
    }

    /// `.waiting` until `start`, which settles it with a callback.
    public private(set) var mode: Mode = .waiting
    public private(set) var devices: [Device] = []
    /// The devices whose page this app has.
    public private(set) var pages: Set<String> = []

    /// Called before the mode changes to `.agent` (let go of the controller) and after any other change.
    public var onMode: ((Mode) -> Void)?
    public var onAction: ((Action) -> Void)?
    public var onPage: ((String, Bool) -> Void)?
    public var onDevices: (([Device]) -> Void)?

    public static let contract = "nu9ve-surface/1"
    public static let maxFrame = 64 * 1024
    /// §7: how long the app waits for the agent before opening the controller itself.
    public static let grace: Duration = .seconds(2)

    public let socket: URL
    private let app: String, profileID: String?
    private var catalog: [Item]
    private var states: [String: State] = [:]
    private var connection: Connection?
    private var retry: Task<Void, Never>?
    private var folderWatch: (any DispatchSourceFileSystemObject)?
    private var stopped = true

    /// `agentFolder` is nu9ve-agent-v1 §1 (`SurfaceClient.agentFolder()`); the socket is `runtime/surface`.
    public init(app: String, profileID: String?, catalog: [Item], states: [State] = [], agentFolder: URL) {
        self.app = app; self.profileID = profileID; self.catalog = catalog
        socket = agentFolder.appendingPathComponent("runtime/surface")
        for s in states { self.states[s.key] = s }
    }

    /// nu9ve-agent-v1 §1: `NU9VE_AGENT_DIR` (absolute) or `~/Library/Application Support/nu9ve/agent`.
    public static func agentFolder(environment: [String: String] = ProcessInfo.processInfo.environment,
                                   home: URL = FileManager.default.homeDirectoryForCurrentUser) -> URL? {
        if let over = environment["NU9VE_AGENT_DIR"] {
            return over.hasPrefix("/") ? URL(fileURLWithPath: over, isDirectory: true) : nil
        }
        return home.appendingPathComponent("Library/Application Support/nu9ve/agent", isDirectory: true)
    }

    /// Connects if the agent is there; otherwise the app is in `.direct` and the client waits for the agent.
    public func start() {
        guard stopped else { return }
        stopped = false
        if !attempt(fallback: .direct) { enterDirect() }
    }

    /// `bye`, then nothing: the app quits or turns its controller off.
    public func stop() {
        stopped = true
        retry?.cancel(); retry = nil
        folderWatch?.cancel(); folderWatch = nil
        connection?.send(["type": "bye"])
        connection?.close(); connection = nil
    }

    /// The app's states now: only those that changed since the last call are sent.
    public func update(_ next: [State]) {
        var changed: [State] = []
        for s in next where states[s.key] != s {
            states[s.key] = s; changed.append(s)
        }
        guard !changed.isEmpty else { return }
        connection?.send(["type": "states", "states": changed.map(Self.json)])
    }

    /// The whole catalog again, for example when presets change.
    public func setCatalog(_ items: [Item]) {
        guard items != catalog else { return }
        catalog = items
        connection?.send(["type": "catalog", "catalog": items.map(Self.json)])
    }

    /// Call when the app comes to the front (`applicationDidBecomeActive`): the page follows it (§5).
    public func focus() {
        connection?.send(["type": "focus", "active": true])
    }

    // MARK: connection

    private func setMode(_ next: Mode) {
        guard next != mode else { return }
        mode = next
        onMode?(next)
    }

    /// Checks the socket (§2: a socket of this user, `0600` or tighter, never through a link) and connects.
    /// On a failed connection the mode goes back to `fallback`, with its callback, so an app that let go
    /// of the controller opens it again.
    private func attempt(fallback: Mode) -> Bool {
        guard Self.trusted(socket.path) else { return false }
        // The app lets go of the controller before the agent opens it (§1).
        setMode(.agent)
        guard let c = Connection(path: socket.path, onFrame: { [weak self] in self?.received($0) },
                                 onClose: { [weak self] in self?.lost() }) else {
            setMode(fallback)
            return false
        }
        connection = c
        folderWatch?.cancel(); folderWatch = nil
        let hello: [String: Any] = [
            "type": "hello", "contract": Self.contract, "app": app, "pid": Int(getpid()),
            "process_start_us": Self.processStartUS() ?? 0, "profile_id": profileID ?? NSNull(),
            "catalog": catalog.map(Self.json), "states": states.values.sorted { $0.key < $1.key }.map(Self.json)
        ]
        c.send(hello)
        return true
    }

    static func trusted(_ path: String) -> Bool {
        var info = stat()
        return lstat(path, &info) == 0 && (info.st_mode & S_IFMT) == S_IFSOCK && info.st_uid == getuid()
            && info.st_mode & 0o077 == 0
    }

    private func lost() {
        connection = nil
        devices = []; pages = []
        guard !stopped else { return }
        setMode(.waiting)
        retry?.cancel()
        retry = Task { [weak self] in
            let clock = ContinuousClock()
            let deadline = clock.now.advanced(by: Self.grace)
            while clock.now < deadline {
                try? await Task.sleep(for: .milliseconds(200))
                guard let self, !Task.isCancelled, !self.stopped else { return }
                if self.attempt(fallback: .waiting) { return }
            }
            guard let self, !Task.isCancelled, !self.stopped else { return }
            self.enterDirect()
        }
    }

    /// Opens the controller itself and waits, by events on the runtime folder, for the agent to come back.
    private func enterDirect() {
        setMode(.direct)
        let folder = socket.deletingLastPathComponent()
        let watched = FileManager.default.fileExists(atPath: folder.path) ? folder : folder.deletingLastPathComponent()
        let fd = open(watched.path, O_EVTONLY)
        guard fd >= 0 else { return }
        let source = DispatchSource.makeFileSystemObjectSource(fileDescriptor: fd, eventMask: [.write, .rename, .delete], queue: .main)
        source.setEventHandler { [weak self] in
            MainActor.assumeIsolated {
                guard let self, self.mode == .direct, !self.stopped else { return }
                if self.attempt(fallback: .direct) { return }
                // The runtime folder may have just appeared: watch it instead of its parent.
                if watched != folder, FileManager.default.fileExists(atPath: folder.path) { self.enterDirect() }
            }
        }
        source.setCancelHandler { close(fd) }
        folderWatch?.cancel()
        folderWatch = source
        source.resume()
    }

    private func received(_ message: [String: Any]) {
        switch message["type"] as? String {
        case "welcome", "devices":
            devices = (message["devices"] as? [[String: Any]] ?? []).compactMap {
                guard let d = $0["device"] as? String, let port = $0["port"] as? String else { return nil }
                return Device(device: d, profile: $0["profile"] as? String, port: port)
            }
            onDevices?(devices)
        case "page":
            guard let device = message["device"] as? String, let has = message["page"] as? Bool else { return }
            if has { pages.insert(device) } else { pages.remove(device) }
            onPage?(device, has)
        case "action":
            guard let action = message["action"] as? String else { return }
            onAction?(Action(action: action, argument: message["argument"] as? String,
                             value: (message["value"] as? NSNumber)?.doubleValue, delta: (message["delta"] as? NSNumber)?.intValue,
                             device: message["device"] as? String ?? "", control: message["control"] as? String ?? "",
                             via: message["via"] as? String ?? "page", hostNS: (message["host_ns"] as? NSNumber)?.uint64Value ?? 0))
        case "released":
            // §7: the agent lets go (it updates or quits). The connection closes next.
            connection?.close()
        default:
            break   // §4: unknown types are ignored
        }
    }

    // MARK: JSON

    static func json(_ item: Item) -> [String: Any] {
        var out: [String: Any] = ["id": item.id, "title": item.title, "group": item.group, "kind": item.kind.rawValue]
        if let color = item.color { out["color"] = color.rawValue }
        if !item.arguments.isEmpty { out["arguments"] = item.arguments.map { ["value": $0.value, "title": $0.title] } }
        return out
    }

    static func json(_ state: State) -> [String: Any] {
        var out: [String: Any] = ["action": state.action, "state": state.state.rawValue]
        if let argument = state.argument { out["argument"] = argument }
        if let light = state.light {
            out["light"] = ["color": light.color.map { $0.rawValue as Any } ?? NSNull(), "dim": light.dim, "motion": light.motion.rawValue]
        }
        return out
    }

    /// The start time of this process in µs (registry §4).
    static func processStartUS() -> Int64? {
        var info = proc_bsdinfo()
        let size = Int32(MemoryLayout<proc_bsdinfo>.size)
        guard proc_pidinfo(getpid(), PROC_PIDTBSDINFO, 0, &info, size) == size else { return nil }
        return Int64(info.pbi_start_tvsec) * 1_000_000 + Int64(info.pbi_start_tvusec)
    }
}

/// One framed Unix socket: reads by a dispatch source (no polling), writes on its own queue.
@MainActor
final class Connection {
    private let fd: Int32
    private let source: any DispatchSourceRead
    private let writes = DispatchQueue(label: "app.nu9ve.surface.write")
    private var buffer = Data()
    private var closed = false
    private let onFrame: ([String: Any]) -> Void
    private let onClose: () -> Void

    init?(path: String, onFrame: @escaping ([String: Any]) -> Void, onClose: @escaping () -> Void) {
        let fd = Darwin.socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { return nil }
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let bytes = Array(path.utf8)
        guard bytes.count < MemoryLayout.size(ofValue: address.sun_path) else { Darwin.close(fd); return nil }
        withUnsafeMutableBytes(of: &address.sun_path) { raw in
            raw.copyBytes(from: bytes)
            raw[bytes.count] = 0
        }
        let ok = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) == 0
            }
        }
        guard ok else { Darwin.close(fd); return nil }
        var on: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &on, socklen_t(MemoryLayout<Int32>.size))
        self.fd = fd; self.onFrame = onFrame; self.onClose = onClose
        source = DispatchSource.makeReadSource(fileDescriptor: fd, queue: .main)
        source.setEventHandler { [weak self] in MainActor.assumeIsolated { self?.readable() } }
        source.resume()
    }

    private func readable() {
        var chunk = [UInt8](repeating: 0, count: 16 * 1024)
        let n = chunk.withUnsafeMutableBytes { read(fd, $0.baseAddress, $0.count) }
        if n <= 0 { close(); return }
        buffer.append(contentsOf: chunk.prefix(n))
        while buffer.count >= 4 {
            let size = buffer.prefix(4).reduce(0) { $0 << 8 | Int($1) }
            guard size > 0, size <= SurfaceClient.maxFrame else { close(); return }
            guard buffer.count >= 4 + size else { break }
            let payload = buffer.subdata(in: buffer.startIndex + 4 ..< buffer.startIndex + 4 + size)
            buffer = Data(buffer.dropFirst(4 + size))
            if let message = try? JSONSerialization.jsonObject(with: payload) as? [String: Any] { onFrame(message) }
            if closed { return }
        }
    }

    func send(_ message: [String: Any]) {
        guard !closed, let payload = try? JSONSerialization.data(withJSONObject: message), payload.count <= SurfaceClient.maxFrame else { return }
        let count = UInt32(payload.count)
        let frame = Data([UInt8(count >> 24), UInt8(count >> 16 & 0xff), UInt8(count >> 8 & 0xff), UInt8(count & 0xff)]) + payload
        let fd = fd
        writes.async {
            frame.withUnsafeBytes { raw in
                var offset = 0
                while offset < raw.count {
                    let n = write(fd, raw.baseAddress!.advanced(by: offset), raw.count - offset)
                    if n < 0, errno == EINTR { continue }
                    if n <= 0 { return }
                    offset += n
                }
            }
        }
    }

    func close() {
        guard !closed else { return }
        closed = true
        source.cancel()
        let fd = fd
        // After pending writes (a `bye`), so they are not cut.
        writes.async { Darwin.shutdown(fd, SHUT_RDWR); Darwin.close(fd) }
        onClose()
    }
}
