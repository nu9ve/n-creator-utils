import Foundation

// nu9ve-action-v1 §3: the record of a long action, read from the data of an `action_progress` reply.
// `ActionRecord.read(_:progressID:)` validates with the same checks, in the same order, as the reference
// (scripts/action-examples.py), so a record gives the same problem in Swift and in Python.

/// The contract string of every record.
public let actionContract = "nu9ve-action/1"

/// The common stages of §3.2, in the order they usually come.
public enum ActionStage: String, CaseIterable, Sendable {
    case preparing, listing, scanning, reviewing, copying, verifying, rendering, analyzing, releasing, writing, finishing
}

/// The state of a record. `unknown` is the reply for an id the engine does not know (§2).
public enum ActionState: String, Sendable, Equatable {
    case running, complete, failed, cancelled, unknown
    public var isFinished: Bool { self == .complete || self == .failed || self == .cancelled }
}

/// The unit of the overall measure (§3.1).
public enum ActionUnit: String, Sendable, Equatable {
    case bytes, files, items, seconds, work
}

/// A phase or a step: `{index, count, label}`, and `fraction` for a step.
public struct ActionCounter: Sendable, Equatable {
    public var index: Int
    public var count: Int
    public var label: String
    public var fraction: Double?
}

/// The file in hand.
public struct ActionCurrent: Sendable, Equatable {
    public var name: String
    public var from: String
    public var to: String
    public var action: String
    public var fileDone: Int?
    public var fileTotal: Int?
    /// `file_done / file_total`, or nil when the file is not measured.
    public var fraction: Double? {
        guard let fileDone, let fileTotal, fileTotal > 0 else { return nil }
        return min(1, Double(fileDone) / Double(fileTotal))
    }
}

/// One line of the short log the engine writes.
public struct ActionLogEntry: Sendable, Equatable, Identifiable {
    /// Its position in the log, stable while the log only grows at the end.
    public var id: Int
    public var at: String
    public var kind: String
    public var name: String
    public var target: String
    public var text: String?
}

/// A warning of a finished action, or its error.
public struct ActionMessage: Sendable, Equatable {
    public var code: String
    public var message: String
}

/// What a finished action did (§3.4).
public struct ActionOutcome: Sendable, Equatable {
    public var summary: String
    public var warnings: [ActionMessage]
    public var receipt: String?
}

/// A valid record. Unknown fields are kept out: an app's own fields do not reach the common view.
public struct ActionRecord: Sendable, Equatable {
    public var progressID: String
    public var state: ActionState
    public var action: String
    public var label: String?
    /// A common stage, or `<app>.<stage>`.
    public var stage: String
    public var phase: ActionCounter?
    public var step: ActionCounter?
    public var overallDone: Int
    public var overallTotal: Int?
    public var unit: ActionUnit
    public var filesDone: Int?
    public var filesTotal: Int?
    public var current: ActionCurrent?
    public var log: [ActionLogEntry]
    public var cancellable: Bool
    /// Seconds since 1970, from the three instants.
    public var startedAt: Double
    public var progressedAt: Double
    public var updatedAt: Double
    /// The instants as written, so two records of one action compare exactly.
    public var startedAtText: String
    public var updatedAtText: String
    public var outcome: ActionOutcome?
    public var error: ActionMessage?
    public var resumable: Bool?

    /// The common stage, or nil for an app's own one.
    public var commonStage: ActionStage? { ActionStage(rawValue: stage) }

    /// `overall_done / overall_total`, or nil when the total is not known (an indeterminate bar).
    public var fraction: Double? {
        guard let overallTotal, overallTotal > 0 else { return nil }
        return Double(overallDone) / Double(overallTotal)
    }

    /// The record of an `unknown` reply.
    static func unknown(_ progressID: String) -> Self {
        Self(progressID: progressID, state: .unknown, action: "", label: nil, stage: "preparing", phase: nil, step: nil,
             overallDone: 0, overallTotal: nil, unit: .items, filesDone: nil, filesTotal: nil, current: nil, log: [],
             cancellable: false, startedAt: 0, progressedAt: 0, updatedAt: 0, startedAtText: "", updatedAtText: "",
             outcome: nil, error: nil, resumable: nil)
    }
}

/// Why a record is not valid. The raw values are the reference's problems.
public enum ActionRecordProblem: String, Error, Sendable, Equatable {
    case notJSON = "not_json"
    case notAnObject = "not_an_object"
    case tooLarge = "too_large"
    case badContract = "bad_contract"
    case otherProgressID = "other_progress_id"
    case unknownWithFields = "unknown_with_fields"
    case badState = "bad_state"
    case badAction = "bad_action"
    case badLabel = "bad_label"
    case badStage = "bad_stage"
    case badPhase = "bad_phase"
    case badStep = "bad_step"
    case badOverall = "bad_overall"
    case doneOverTotal = "done_over_total"
    case badUnit = "bad_unit"
    case badFiles = "bad_files"
    case badCurrent = "bad_current"
    case badLog = "bad_log"
    case badCancellable = "bad_cancellable"
    case badInstant = "bad_instant"
    case instantsOutOfOrder = "instants_out_of_order"
    case fullWhileRunning = "full_while_running"
    case closingWhileRunning = "closing_while_running"
    case stillWorkingAfterEnd = "still_working_after_end"
    case badOutcome = "bad_outcome"
    case completeWithoutOutcome = "complete_without_outcome"
    case errorOnComplete = "error_on_complete"
    case completeNotFull = "complete_not_full"
    case badResumable = "bad_resumable"
    case badError = "bad_error"
    case errorOnCancelled = "error_on_cancelled"
}

/// The `action_progress` request (§2): nil when the id is valid, otherwise `invalid_params`.
public func actionProgressParamsProblem(_ params: [String: Any]) -> String? {
    guard params.count == 1, let id = params["progress_id"] as? String, ActionPatterns.progressID.matches(id) else {
        return "invalid_params"
    }
    return nil
}

/// Whether `id` can be a `progress_id` (§1).
public func isActionProgressID(_ id: String) -> Bool { ActionPatterns.progressID.matches(id) }

extension ActionRecord {
    static let maxRecordBytes = 256 * 1024
    static let maxLog = 60
    static let maxWarnings = 20
    static let maxSafe = 9_007_199_254_740_991

    /// Reads and validates the data of an `action_progress` reply for `progressID`.
    public static func read(_ data: Data, progressID: String) -> Result<Self, ActionRecordProblem> {
        guard let any = try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed]) else {
            return .failure(.notJSON)
        }
        guard let object = any as? [String: Any] else { return .failure(.notAnObject) }
        if data.count > maxRecordBytes { return .failure(.tooLarge) }
        do { return .success(try parse(object, progressID: progressID)) } catch let problem as ActionRecordProblem {
            return .failure(problem)
        } catch {
            return .failure(.notAnObject)
        }
    }

    static func parse(_ record: [String: Any], progressID: String) throws -> Self {
        guard record["contract"] as? String == actionContract else { throw ActionRecordProblem.badContract }
        guard record["progress_id"] as? String == progressID else { throw ActionRecordProblem.otherProgressID }
        let stateText = record["state"] as? String
        if stateText == "unknown" {
            guard Set(record.keys) == ["contract", "progress_id", "state"] else { throw ActionRecordProblem.unknownWithFields }
            return .unknown(progressID)
        }
        guard let stateText, let state = ActionState(rawValue: stateText), state != .unknown else { throw ActionRecordProblem.badState }
        guard let action = record["action"] as? String, ActionPatterns.word.matches(action) else { throw ActionRecordProblem.badAction }
        var label: String?
        if record.keys.contains("label") {
            guard let text = record["label"] as? String, shortText(text, 200) else { throw ActionRecordProblem.badLabel }
            label = text
        }
        guard let stage = record["stage"] as? String,
              ActionStage(rawValue: stage) != nil || ActionPatterns.appStage.matches(stage) else { throw ActionRecordProblem.badStage }
        let phase = try record.keys.contains("phase") ? counter(record["phase"], step: false) : nil
        let step = try record.keys.contains("step") ? counter(record["step"], step: true) : nil
        guard let done = count(record["overall_done"]) else { throw ActionRecordProblem.badOverall }
        guard record.keys.contains("overall_total") else { throw ActionRecordProblem.badOverall }
        var total: Int?
        if !(record["overall_total"] is NSNull) {
            guard let value = count(record["overall_total"]) else { throw ActionRecordProblem.badOverall }
            total = value
        }
        if let total, total > 0, done > total { throw ActionRecordProblem.doneOverTotal }
        guard let unitText = record["unit"] as? String, let unit = ActionUnit(rawValue: unitText) else { throw ActionRecordProblem.badUnit }
        var filesDone: Int?, filesTotal: Int?
        for key in ["files_done", "files_total"] where record.keys.contains(key) {
            guard let value = count(record[key]) else { throw ActionRecordProblem.badFiles }
            if key == "files_done" { filesDone = value } else { filesTotal = value }
        }
        if let filesDone, let filesTotal, filesDone > filesTotal { throw ActionRecordProblem.badFiles }
        let current = try record.keys.contains("current") ? currentFile(record["current"]) : nil
        let log = try record.keys.contains("log") ? logEntries(record["log"]) : []
        guard let cancellable = boolean(record["cancellable"]) else { throw ActionRecordProblem.badCancellable }
        guard let startedText = record["started_at"] as? String, let started = actionInstant(startedText),
              let progressed = actionInstant(record["progressed_at"] as? String ?? ""),
              let updatedText = record["updated_at"] as? String, let updated = actionInstant(updatedText) else {
            throw ActionRecordProblem.badInstant
        }
        guard started <= progressed, progressed <= updated else { throw ActionRecordProblem.instantsOutOfOrder }
        var parsed = Self(progressID: progressID, state: state, action: action, label: label, stage: stage, phase: phase, step: step,
                          overallDone: done, overallTotal: total, unit: unit, filesDone: filesDone, filesTotal: filesTotal,
                          current: current, log: log, cancellable: cancellable, startedAt: started, progressedAt: progressed,
                          updatedAt: updated, startedAtText: startedText, updatedAtText: updatedText,
                          outcome: nil, error: nil, resumable: nil)
        let full = total.map { $0 > 0 } ?? false
        if state == .running {
            if full, done == total { throw ActionRecordProblem.fullWhileRunning }
            if ["outcome", "error", "resumable"].contains(where: record.keys.contains) { throw ActionRecordProblem.closingWhileRunning }
            return parsed
        }
        // §3.4: a finished record
        if cancellable || truthy(record["current"]) { throw ActionRecordProblem.stillWorkingAfterEnd }
        if record.keys.contains("outcome") { parsed.outcome = try outcome(record["outcome"]) }
        if state == .complete {
            guard parsed.outcome != nil else { throw ActionRecordProblem.completeWithoutOutcome }
            if record.keys.contains("error") || record.keys.contains("resumable") { throw ActionRecordProblem.errorOnComplete }
            if full, done != total { throw ActionRecordProblem.completeNotFull }
            return parsed
        }
        guard let resumable = boolean(record["resumable"]) else { throw ActionRecordProblem.badResumable }
        parsed.resumable = resumable
        if state == .failed {
            guard let error = record["error"] as? [String: Any], Set(error.keys) == ["code", "message"],
                  let code = error["code"] as? String, ActionPatterns.word.matches(code),
                  let message = error["message"] as? String, shortText(message, 500) else { throw ActionRecordProblem.badError }
            parsed.error = ActionMessage(code: code, message: message)
        } else if record.keys.contains("error") {
            throw ActionRecordProblem.errorOnCancelled
        }
        return parsed
    }

    private static func counter(_ any: Any?, step: Bool) throws -> ActionCounter {
        let problem: ActionRecordProblem = step ? .badStep : .badPhase
        guard let value = any as? [String: Any] else { throw problem }
        let allowed: Set<String> = step ? ["index", "count", "label", "fraction"] : ["index", "count", "label"]
        guard Set(value.keys).isSubset(of: allowed), Set(["index", "count", "label"]).isSubset(of: Set(value.keys)) else { throw problem }
        guard let index = integer(value["index"]), let total = integer(value["count"]), 1 <= index, index <= total, total <= maxSafe else {
            throw problem
        }
        guard let label = value["label"] as? String, shortText(label, 200) else { throw problem }
        var fraction: Double?
        if let raw = value["fraction"], !(raw is NSNull) {
            guard let number = raw as? NSNumber, !isBoolean(number), number.doubleValue >= 0, number.doubleValue <= 1 else { throw problem }
            fraction = number.doubleValue
        }
        return ActionCounter(index: index, count: total, label: label, fraction: fraction)
    }

    private static func currentFile(_ any: Any?) throws -> ActionCurrent {
        guard let value = any as? [String: Any], let name = value["name"] as? String, shortText(name, 255), !name.contains("/") else {
            throw ActionRecordProblem.badCurrent
        }
        var texts: [String: String] = [:]
        for key in ["from", "to", "action"] where value.keys.contains(key) {
            guard let text = value[key] as? String else { throw ActionRecordProblem.badCurrent }
            texts[key] = text
        }
        var sizes: [String: Int] = [:]
        for key in ["file_done", "file_total"] where value.keys.contains(key) {
            guard let size = count(value[key]) else { throw ActionRecordProblem.badCurrent }
            sizes[key] = size
        }
        if let done = sizes["file_done"], let total = sizes["file_total"], done > total { throw ActionRecordProblem.badCurrent }
        return ActionCurrent(name: name, from: texts["from"] ?? "", to: texts["to"] ?? "", action: texts["action"] ?? "",
                             fileDone: sizes["file_done"], fileTotal: sizes["file_total"])
    }

    private static func logEntries(_ any: Any?) throws -> [ActionLogEntry] {
        guard let lines = any as? [Any], lines.count <= maxLog else { throw ActionRecordProblem.badLog }
        return try lines.enumerated().map { index, line in
            guard let entry = line as? [String: Any], let at = entry["at"] as? String, actionInstant(at) != nil,
                  let kind = entry["kind"] as? String, ActionPatterns.word.matches(kind),
                  let name = entry["name"] as? String else { throw ActionRecordProblem.badLog }
            for key in ["target", "text"] where entry.keys.contains(key) && !(entry[key] is String) {
                throw ActionRecordProblem.badLog
            }
            return ActionLogEntry(id: index, at: at, kind: kind, name: name, target: entry["target"] as? String ?? "",
                                  text: entry["text"] as? String)
        }
    }

    private static func outcome(_ any: Any?) throws -> ActionOutcome {
        guard let value = any as? [String: Any], let summary = value["summary"] as? String, shortText(summary, 500) else {
            throw ActionRecordProblem.badOutcome
        }
        let rawWarnings = value["warnings"] ?? [Any]()
        guard let list = rawWarnings as? [Any], list.count <= maxWarnings else { throw ActionRecordProblem.badOutcome }
        let warnings = try list.map { item -> ActionMessage in
            guard let warning = item as? [String: Any], Set(warning.keys) == ["code", "message"],
                  let code = warning["code"] as? String, ActionPatterns.word.matches(code),
                  let message = warning["message"] as? String, shortText(message, 500) else { throw ActionRecordProblem.badOutcome }
            return ActionMessage(code: code, message: message)
        }
        var receipt: String?
        if let raw = value["receipt"], !(raw is NSNull) {
            guard let path = raw as? String, isAbsolutePath(path) else { throw ActionRecordProblem.badOutcome }
            receipt = path
        }
        return ActionOutcome(summary: summary, warnings: warnings, receipt: receipt)
    }
}

/// §3.3 and §3.4 between two valid records of one action read in this order: nil, or the rule it breaks.
public func actionStepProblem(_ before: ActionRecord, _ after: ActionRecord) -> String? {
    if before.state == .unknown || after.state == .unknown { return nil }
    if after.startedAtText != before.startedAtText { return nil }  // another action with the same id (§1)
    if before.state != .running, after.state == .running { return "running_after_end" }
    if after.updatedAt < before.updatedAt { return "updated_went_back" }
    if let a = before.fraction, let b = after.fraction, b < a { return "fraction_went_back" }
    return nil
}

/// Seconds since 1970 of an RFC 3339 UTC instant with `Z` and an optional fraction, or nil.
public func actionInstant(_ text: String) -> Double? {
    let scalars = Array(text.unicodeScalars)
    guard scalars.count >= 20, scalars.last == "Z" else { return nil }
    func digits(_ range: Range<Int>) -> Int? {
        var value = 0
        for index in range {
            guard index < scalars.count, ("0"..."9").contains(scalars[index]) else { return nil }
            value = value * 10 + Int(scalars[index].value - 48)
        }
        return value
    }
    guard scalars[4] == "-", scalars[7] == "-", scalars[10] == "T", scalars[13] == ":", scalars[16] == ":",
          let year = digits(0..<4), let month = digits(5..<7), let day = digits(8..<10),
          let hour = digits(11..<13), let minute = digits(14..<16), let second = digits(17..<19) else { return nil }
    var fraction = 0.0
    if scalars.count > 20 {
        let digitsCount = scalars.count - 21
        guard scalars[19] == ".", (1...9).contains(digitsCount), let value = digits(20..<(20 + digitsCount)) else { return nil }
        fraction = Double(value) / pow(10, Double(digitsCount))
    }
    guard (1...12).contains(month), hour < 24, minute < 60, second < 60 else { return nil }
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(identifier: "UTC")!
    let components = DateComponents(year: year, month: month, day: day, hour: hour, minute: minute, second: second)
    guard let date = calendar.date(from: components),
          calendar.component(.day, from: date) == day, calendar.component(.month, from: date) == month else { return nil }
    return date.timeIntervalSince1970 + fraction
}

// MARK: - JSON helpers

enum ActionPatterns {
    /// `^[a-z0-9][a-z0-9._:-]{0,127}$`
    static let progressID = Pattern(first: "abcdefghijklmnopqrstuvwxyz0123456789", rest: "abcdefghijklmnopqrstuvwxyz0123456789._:-", max: 128)
    /// `^[a-z][a-z0-9_]{0,63}$`: actions, kinds and codes.
    static let word = Pattern(first: "abcdefghijklmnopqrstuvwxyz", rest: "abcdefghijklmnopqrstuvwxyz0123456789_", max: 64)
    /// `^[a-z][a-z0-9-]{0,31}\.[a-z][a-z0-9_]{0,63}$`
    static let appStage = DottedPattern()

    struct Pattern {
        let first: Set<Character>
        let rest: Set<Character>
        let max: Int
        init(first: String, rest: String, max: Int) {
            self.first = Set(first)
            self.rest = Set(rest)
            self.max = max
        }
        func matches(_ text: String) -> Bool {
            guard let head = text.first, first.contains(head), text.count <= max, text.unicodeScalars.count == text.count else { return false }
            return text.dropFirst().allSatisfy(rest.contains)
        }
    }

    struct DottedPattern {
        private let app = Pattern(first: "abcdefghijklmnopqrstuvwxyz", rest: "abcdefghijklmnopqrstuvwxyz0123456789-", max: 32)
        func matches(_ text: String) -> Bool {
            let parts = text.split(separator: ".", omittingEmptySubsequences: false)
            return parts.count == 2 && app.matches(String(parts[0])) && ActionPatterns.word.matches(String(parts[1]))
        }
    }
}

func isBoolean(_ number: NSNumber) -> Bool { CFGetTypeID(number) == CFBooleanGetTypeID() }

/// An integer that is not a boolean nor written with a fraction or exponent.
func integer(_ any: Any?) -> Int? {
    guard let number = any as? NSNumber, !isBoolean(number) else { return nil }
    let type = String(cString: number.objCType)
    guard !["f", "d"].contains(type) else { return nil }
    return number.intValue
}

/// A count of §3.1: an integer from 0 to 2^53 − 1.
func count(_ any: Any?) -> Int? {
    guard let value = integer(any), value >= 0, value <= ActionRecord.maxSafe else { return nil }
    return value
}

func boolean(_ any: Any?) -> Bool? {
    guard let number = any as? NSNumber, isBoolean(number) else { return nil }
    return number.boolValue
}

/// Like Python's truthiness, for the `current` of a finished record.
func truthy(_ any: Any?) -> Bool {
    switch any {
    case nil, is NSNull: return false
    case let object as [String: Any]: return !object.isEmpty
    case let list as [Any]: return !list.isEmpty
    case let text as String: return !text.isEmpty
    case let number as NSNumber: return number.doubleValue != 0
    default: return true
    }
}

/// Between 1 and `limit` code points, without NUL.
func shortText(_ text: String, _ limit: Int) -> Bool {
    let count = text.unicodeScalars.count
    return count > 0 && count <= limit && !text.unicodeScalars.contains("\0")
}

func isAbsolutePath(_ path: String) -> Bool {
    if path.hasPrefix("/") { return true }
    let scalars = Array(path.unicodeScalars)
    guard scalars.count >= 3, CharacterSet.letters.contains(scalars[0]), scalars[0].isASCII else { return false }
    return scalars[1] == ":" && (scalars[2] == "\\" || scalars[2] == "/")
}
