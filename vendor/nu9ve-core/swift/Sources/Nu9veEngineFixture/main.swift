// A test engine for the Swift client: nu9ve-engine-v1 over NDJSON, with methods that are slow,
// crash, break the protocol or hold a lease. Only the package's tests run it.
//
//   nu9ve-engine-fixture serve [--contract <c>] [--protocol <p>] [--info-delay-ms <n>] [--crash-on-start]

import Foundation
import Nu9veEngine

let maxLine = 1 << 20
var contract = "nu9ve-engine/1"
var proto = "fixture-engine/1"
var infoDelay: UInt32 = 0
var arguments = Array(CommandLine.arguments.dropFirst())
guard arguments.first == "serve" else {
    FileHandle.standardError.write(Data("usage: nu9ve-engine-fixture serve [options]\n".utf8))
    exit(2)
}
arguments.removeFirst()
while !arguments.isEmpty {
    let flag = arguments.removeFirst()
    switch flag {
    case "--contract": contract = arguments.removeFirst()
    case "--protocol": proto = arguments.removeFirst()
    case "--info-delay-ms": infoDelay = UInt32(arguments.removeFirst()) ?? 0
    case "--crash-on-start": exit(4)
    default: exit(2)
    }
}

var leasePath: String?

enum Answer {
    case success(EngineValue)
    case failure(EngineObject)
}

func reply(_ id: EngineValue, _ result: Answer) -> EngineValue {
    switch result {
    case .success(let data): ["id": id, "ok": true, "data": data]
    case .failure(let error): ["id": id, "ok": false, "error": .object(error)]
    }
}

func failure(_ code: String, _ message: String, reason: String? = nil) -> Answer {
    var error: EngineObject = ["code": .string(code), "message": .string(message)]
    if let reason { error["reason"] = .string(reason) }
    return .failure(error)
}

func write(_ value: EngineValue) {
    FileHandle.standardOutput.write(value.jsonData + [0x0A])
}

func writeRaw(_ text: String) {
    FileHandle.standardOutput.write(Data((text + "\n").utf8))
}

let ported = ["engine_info", "echo"]
let native = ["sleep", "crash", "not_ported", "big", "fail", "lease", "lease_release", "garbage", "wrong_id", "pid"]

@MainActor func dispatch(_ method: String, _ params: EngineObject, id: EngineValue) -> Answer? {
    switch method {
    case "engine_info":
        if infoDelay > 0 { usleep(infoDelay * 1000) }
        let limits: EngineValue = ["request_bytes": .number(EngineNumber(maxLine)), "response_bytes": .number(EngineNumber(16 << 20))]
        let info: EngineObject = [
            "contract": .string(contract), "protocol": .string(proto), "engine": "swift", "version": "0.1.0",
            "capabilities": [], "ported": .array(ported.map { EngineValue.string($0) }),
            "native": .array(native.map { EngineValue.string($0) }), "limits": limits
        ]
        return .success(.object(info))
    case "echo":
        return .success(.object(params))
    case "sleep":
        let ms = params["ms"]?.numberValue?.int ?? 0
        usleep(UInt32(ms) * 1000)
        return .success(["slept": .number(EngineNumber(ms))])
    case "crash":
        exit(3)
    case "not_ported":
        return failure("not_ported", "This method is available in the oracle.", reason: "method_unavailable")
    case "big":
        let count = params["bytes"]?.numberValue?.int ?? 0
        return .success(.string(String(repeating: "a", count: count)))
    case "fail":
        return failure(params["code"]?.stringValue ?? "project_not_found", "Failed on purpose.")
    case "lease":
        if leasePath != nil { return failure("lease_active", "Release the active lease first.") }
        guard let path = params["path"]?.stringValue else { return failure("invalid_params", "Expected path.") }
        FileManager.default.createFile(atPath: path, contents: Data())
        leasePath = path
        return .success(["phase": "ready"])
    case "lease_release":
        guard let path = leasePath else { return failure("lease_missing", "No active lease.") }
        try? FileManager.default.removeItem(atPath: path)
        leasePath = nil
        return .success(["phase": "released"])
    case "garbage":
        writeRaw("this is not json")
        return nil
    case "wrong_id":
        write(reply(.number(EngineNumber(987_654_321)), .success(true)))
        return nil
    case "pid":
        return .success(.number(EngineNumber(Int(getpid()))))
    default:
        return failure("unknown_method", "No such method: \(method)")
    }
}

@MainActor func answer(_ line: [UInt8]) {
    guard let request = try? EngineValue(parsing: Data(line)) else {
        write(reply(.null, failure("invalid_request", "Not JSON."))); return
    }
    guard case .object(let object) = request else {
        write(reply(.null, failure("invalid_request", "Not an object."))); return
    }
    let rawID = object["id"] ?? .null
    guard EngineID(rawID) != nil else {
        write(reply(.null, failure("invalid_request", "Invalid id."))); return
    }
    guard case .string(let method)? = object["method"] else {
        write(reply(rawID, failure("invalid_request", "Expected a method."))); return
    }
    let params: EngineObject
    switch object["params"] {
    case nil: params = [:]
    case .object(let given)?: params = given
    default: write(reply(rawID, failure("invalid_params", "params must be an object."))); return
    }
    if let result = dispatch(method, params, id: rawID) { write(reply(rawID, result)) }
}

// Bounded reading: a line over 1 MiB is answered request_too_large and discarded to its newline.
var line = [UInt8]()
var discarding = false
let chunk = UnsafeMutablePointer<UInt8>.allocate(capacity: 1 << 16)
while true {
    let count = read(0, chunk, 1 << 16)
    if count < 0, errno == EINTR { continue }
    if count <= 0 { break }
    for byte in UnsafeBufferPointer(start: chunk, count: count) {
        if byte == 0x0A {
            if !discarding, line.contains(where: { ![0x20, 0x09, 0x0D, 0x0B, 0x0C].contains($0) }) { answer(line) }
            line.removeAll(keepingCapacity: true)
            discarding = false
        } else if !discarding {
            line.append(byte)
            if line.count > maxLine {
                write(reply(.null, failure("request_too_large", "Request exceeds 1 MiB.")))
                line.removeAll()
                discarding = true
            }
        }
    }
}
// EOF releases what a lease held.
if let leasePath { try? FileManager.default.removeItem(atPath: leasePath) }
exit(0)
