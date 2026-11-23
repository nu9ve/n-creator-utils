import Foundation
import Testing
@testable import Nu9veEngine

/// The client rules of data/engine-examples-v1.json: envelopes, ids and engine_info, read with
/// this package's own JSON reader (so an id beyond 64 bits is checked too).
struct EngineExamples {
    static let url = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("data/engine-examples-v1.json")

    static let root: EngineValue = {
        // The file itself is read with the reader under test.
        try! EngineValue(parsing: Data(contentsOf: url))
    }()

    static func list(_ path: String...) -> [EngineValue] {
        var value: EngineValue? = root
        for key in path { value = value?[key] }
        return value?.arrayValue ?? []
    }
}

extension EngineValue {
    func text(_ key: String) -> String { self[key]?.stringValue ?? "" }
}

@Test func examplesAreTheContractsOwn() {
    #expect(EngineExamples.root.text("contract") == "nu9ve-engine-v1")
    #expect(EngineExamples.list("server").count == 20)
}

@Test func validEnvelopesAreAccepted() throws {
    let cases = EngineExamples.list("envelopes", "valid")
    #expect(cases.count == 9)
    for item in cases {
        let result = EngineEnvelope.parse(Data(item.text("text").utf8))
        guard case .success = result else {
            Issue.record("\(item.text("name")): \(result)")
            continue
        }
    }
}

@Test func invalidEnvelopesSayWhy() {
    let cases = EngineExamples.list("envelopes", "invalid")
    #expect(cases.count == 16)
    for item in cases {
        switch EngineEnvelope.parse(Data(item.text("text").utf8)) {
        case .success: Issue.record("\(item.text("name")) was accepted")
        case .failure(let problem): #expect(problem.rawValue == item.text("why"), "\(item.text("name"))")
        }
    }
}

@Test func bigIntegerIdKeepsItsDigits() throws {
    let envelope = try EngineEnvelope.parse(Data(#"{"id":18446744073709551616,"ok":true,"data":1}"#.utf8)).get()
    #expect(envelope.id == .integer("18446744073709551616"))
    #expect(envelope.id.description == "18446744073709551616")
    guard case .data(_, let bytes) = envelope.body else { Issue.record("not data"); return }
    #expect(String(decoding: bytes, as: UTF8.self) == "1")
}

@Test func dataKeepsItsBytes() throws {
    let text = #"{"id":7,"ok":true,"data":{"title":"Día 1","n":1.0,"big":18446744073709551616}}"#
    let envelope = try EngineEnvelope.parse(Data(text.utf8)).get()
    guard case .data(let value, let bytes) = envelope.body else { Issue.record("not data"); return }
    #expect(String(decoding: bytes, as: UTF8.self) == #"{"title":"Día 1","n":1.0,"big":18446744073709551616}"#)
    #expect(value.jsonText == String(decoding: bytes, as: UTF8.self))
    #expect(value["n"] != value["big"])
}

@Test func idsMatchByTypeAndValue() throws {
    let cases = EngineExamples.list("id_match")
    #expect(cases.count == 7)
    for item in cases {
        let sent = try EngineValue(parsing: item.text("sent"))
        let received = try EngineValue(parsing: item.text("received"))
        let same = EngineID(sent).flatMap { a in EngineID(received).map { a == $0 } } ?? false
        #expect(same == (item["match"]?.boolValue ?? false), "\(item.text("name"))")
    }
}

@Test func engineInfoExamples() throws {
    for item in EngineExamples.list("engine_info", "valid") {
        let result = EngineInfo.parse(try EngineValue(parsing: item.text("data")))
        guard case .success = result else { Issue.record("\(item.text("name")): \(result)"); continue }
    }
    let invalid = EngineExamples.list("engine_info", "invalid")
    #expect(invalid.count == 5)
    for item in invalid {
        switch EngineInfo.parse(try EngineValue(parsing: item.text("data"))) {
        case .success: Issue.record("\(item.text("name")) was accepted")
        case .failure(let problem): #expect(problem.rawValue == item.text("why"), "\(item.text("name"))")
        }
    }
}

@Test func readerIsStrict() {
    for text in ["", "{", "NaN", "[1,]", "{\"a\":1}x", "\u{FEFF}{}", "\"\u{01}\"", "01", "1.", "-", "{'a':1}", "[1e]"] {
        #expect(throws: EngineJSONError.self, "\(text.debugDescription)") { try EngineValue(parsing: text) }
    }
    func nested(_ depth: Int) -> String { String(repeating: "[", count: depth) + String(repeating: "]", count: depth) }
    #expect(throws: EngineJSONError.self) { try EngineValue(parsing: nested(10_000)) }
    #expect((try? EngineValue(parsing: nested(128))) != nil)
}

@Test func writerRoundTrips() throws {
    let text = #"{"b":[1,-2.5e3,true,false,null],"a":"line\nbreak \"q\" \\ \u0001 ñ 😀","z":{}}"#
    let value = try EngineValue(parsing: text)
    #expect(value.jsonText == text)
    #expect(try EngineValue(parsing: #""😀""#) == .string("😀"))
    #expect(!value.jsonText.contains("\n"))
}

@Test func codableBridges() throws {
    struct Item: Codable, Equatable { let title: String; let count: Int }
    let value = try EngineValue(encoding: Item(title: "Día", count: 3))
    #expect(value["title"] == "Día")
    #expect(try value.decode(Item.self) == Item(title: "Día", count: 3))
}
