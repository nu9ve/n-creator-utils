// CoreMIDI for one controller at a time: a client that only listens for notifications, an input
// port that receives by events (never a poll) and an output port for the lights.

import CoreMIDI
import Foundation

@MainActor
public final class MIDIConnection {
    public struct Port: Equatable, Sendable {
        public var name: String
        public var source: MIDIEndpointRef
        public var destination: MIDIEndpointRef?
    }

    /// Called on the main actor with each channel message and its host time in milliseconds.
    public var onMessage: ((MIDIMessage, UInt64) -> Void)?
    /// Called when the connected port appears or goes away.
    public var onChange: ((Port?) -> Void)?
    /// Chooses which source to use among those present.
    public var choose: (([String]) -> String?)?

    public private(set) var port: Port?
    private var client = MIDIClientRef()
    private var input = MIDIPortRef()
    private var output = MIDIPortRef()
    private let name: String

    public init(name: String) { self.name = name }

    public var isOpen: Bool { client != 0 }

    public func open() {
        guard client == 0 else { return }
        let owner = ObjectIdentifier(self)
        // @Sendable: CoreMIDI calls these on its own threads; without it Swift 6 checks for the main actor and traps.
        let status = MIDIClientCreateWithBlock(name as CFString, &client) { @Sendable notification in
            let id = notification.pointee.messageID
            guard id == .msgSetupChanged || id == .msgObjectAdded || id == .msgObjectRemoved else { return }
            DispatchQueue.main.async { MainActor.assumeIsolated { MIDIConnection.live[owner]?.rescan() } }
        }
        guard status == noErr else { client = 0; return }
        Self.live[ObjectIdentifier(self)] = self
        let deliver = Delivery(owner: ObjectIdentifier(self))
        MIDIInputPortCreateWithProtocol(client, "\(name) in" as CFString, ._1_0, &input) { @Sendable list, _ in
            deliver.receive(list)
        }
        MIDIOutputPortCreate(client, "\(name) out" as CFString, &output)
        rescan()
    }

    public func close() {
        guard client != 0 else { return }
        if let port { MIDIPortDisconnectSource(input, port.source) }
        MIDIClientDispose(client)
        client = 0; input = 0; output = 0
        Self.live[ObjectIdentifier(self)] = nil
        if port != nil { port = nil; onChange?(nil) }
    }

    /// Names of the sources present now, online or not.
    public static func sourceNames() -> [String] {
        (0..<MIDIGetNumberOfSources()).map { displayName(MIDIGetSource($0)) }
    }

    public func rescan() {
        guard client != 0 else { return }
        let sources = (0..<MIDIGetNumberOfSources()).map { MIDIGetSource($0) }
        let names = sources.map(Self.displayName)
        let wanted = choose?(names)
        let found = zip(names, sources).first { $0.0 == wanted }
        if let port, found?.1 == port.source { return }
        if let port { MIDIPortDisconnectSource(input, port.source) }
        guard let (name, source) = found else {
            if port != nil { port = nil; onChange?(nil) }
            return
        }
        MIDIPortConnectSource(input, source, nil)
        let destination = (0..<MIDIGetNumberOfDestinations()).map { MIDIGetDestination($0) }.first { Self.displayName($0) == name }
        port = Port(name: name, source: source, destination: destination)
        onChange?(port)
    }

    /// Short messages and SysEx, in order.
    public func send(_ messages: [[UInt8]]) {
        guard let destination = port?.destination, output != 0, !messages.isEmpty else { return }
        let size = 1024
        let buffer = UnsafeMutableRawPointer.allocate(byteCount: size, alignment: MemoryLayout<MIDIPacketList>.alignment)
        defer { buffer.deallocate() }
        // One packet list per message: a handful of lights at a time, and SysEx stays whole.
        for message in messages where !message.isEmpty && message.count <= size - 64 {
            let list = buffer.assumingMemoryBound(to: MIDIPacketList.self)
            let packet = MIDIPacketListInit(list)
            _ = message.withUnsafeBufferPointer { MIDIPacketListAdd(list, size, packet, 0, $0.count, $0.baseAddress!) }
            MIDISend(output, destination, list)
        }
    }

    static func displayName(_ object: MIDIObjectRef) -> String {
        var value: Unmanaged<CFString>?
        MIDIObjectGetStringProperty(object, kMIDIPropertyDisplayName, &value)
        return (value?.takeRetainedValue() as String?) ?? ""
    }

    fileprivate static var live: [ObjectIdentifier: MIDIConnection] = [:]

    fileprivate func deliver(_ messages: [(MIDIMessage, UInt64)]) {
        for (message, ms) in messages { onMessage?(message, ms) }
    }

    /// Host time now, in milliseconds: the clock of incoming messages and of a session's epoch.
    public static func nowMS() -> UInt64 { hostMS(mach_absolute_time()) }

    nonisolated static func hostMS(_ ticks: UInt64) -> UInt64 {
        var info = mach_timebase_info_data_t()
        mach_timebase_info(&info)
        return ticks / 1_000_000 * UInt64(info.numer) / UInt64(info.denom)
            + (ticks % 1_000_000) * UInt64(info.numer) / UInt64(info.denom) / 1_000_000
    }
}

/// Runs on CoreMIDI's thread: turns MIDI 1.0 UMP words into messages, then hops to the main actor.
private struct Delivery: Sendable {
    let owner: ObjectIdentifier

    func receive(_ list: UnsafePointer<MIDIEventList>) {
        var out: [(MIDIMessage, UInt64)] = []
        for packet in list.unsafeSequence() {
            let stamp = packet.pointee.timeStamp
            let ms = MIDIConnection.hostMS(stamp == 0 ? mach_absolute_time() : stamp)
            for word in MIDIEventPacket.WordCollection(packet) where word >> 28 == 0x2 {
                let bytes = [UInt8((word >> 16) & 0xff), UInt8((word >> 8) & 0xff), UInt8(word & 0xff)]
                if let message = MIDIMessage.parse(bytes) { out.append((message, ms)) }
            }
        }
        guard !out.isEmpty else { return }
        let messages = out.map { Item(message: $0.0, ms: $0.1) }
        let owner = owner
        DispatchQueue.main.async {
            MainActor.assumeIsolated { MIDIConnection.live[owner]?.deliver(messages.map { ($0.message, $0.ms) }) }
        }
    }

    struct Item: Sendable { let message: MIDIMessage; let ms: UInt64 }
}
