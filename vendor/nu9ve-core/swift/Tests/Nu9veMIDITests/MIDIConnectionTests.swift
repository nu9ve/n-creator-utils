import CoreMIDI
import Foundation
import Testing
@testable import Nu9veMIDI

/// CoreMIDI end to end with a virtual source: the input arrives on CoreMIDI's thread and must
/// reach the main actor without tripping Swift 6's isolation checks (a crash found in QA).
@MainActor
@Suite struct MIDIConnectionLive {
    @Test func aVirtualSourceReachesTheMainActorAndGoesAway() async throws {
        var client = MIDIClientRef()
        // The MIDI server may still be starting (kMIDIServerStartErr, -304): give it a moment.
        var status = MIDIClientCreateWithBlock("Nu9veMIDITests" as CFString, &client, nil)
        for _ in 0..<10 where status == kMIDIServerStartErr {
            try await Task.sleep(for: .milliseconds(300))
            status = MIDIClientCreateWithBlock("Nu9veMIDITests" as CFString, &client, nil)
        }
        try #require(status == noErr)
        defer { MIDIClientDispose(client) }
        let name = "Nu9veMIDI test \(UUID().uuidString.prefix(8))"
        var source = MIDIEndpointRef()
        try #require(MIDISourceCreateWithProtocol(client, name as CFString, ._1_0, &source) == noErr)

        let connection = MIDIConnection(name: "Nu9veMIDITests listener")
        var received: [MIDIMessage] = []
        var ports: [String?] = []
        connection.choose = { names in names.first { $0 == name } }
        connection.onChange = { ports.append($0?.name) }
        connection.onMessage = { message, _ in
            MainActor.preconditionIsolated()
            received.append(message)
        }
        connection.open()
        #expect(connection.port?.name == name)

        var list = MIDIEventList()
        let packet = MIDIEventListInit(&list, ._1_0)
        var word: UInt32 = 0x2090_0B7F   // Note On, channel 1, note 11, velocity 127
        _ = MIDIEventListAdd(&list, MemoryLayout<MIDIEventList>.size, packet, 0, 1, &word)
        MIDIReceivedEventList(source, &list)
        for _ in 0..<50 where received.isEmpty { try await Task.sleep(for: .milliseconds(20)) }
        #expect(received == [MIDIMessage.parse([0x90, 0x0b, 0x7f])!])

        MIDIEndpointDispose(source)
        for _ in 0..<50 where connection.port != nil { try await Task.sleep(for: .milliseconds(20)) }
        #expect(connection.port == nil, "a source that goes away disconnects")
        #expect(ports.first == name && ports.last == .some(nil))
        connection.close()
    }
}
