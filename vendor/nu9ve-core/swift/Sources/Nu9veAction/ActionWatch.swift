import Foundation

// nu9ve-action-v1 §4: what the common view shows from the records it reads, one read at a time. It is
// `client_view` of the reference (scripts/action-examples.py), plus the time left, which the vectors do
// not pin because it is smoothed.

/// What the view shows for an action.
public enum ActionDisplay: String, Sendable, Equatable {
    /// No record yet: «Preparando».
    case preparing
    case running
    /// The heartbeat is more than 60 s old: «Sin señal de la acción desde hace N s».
    case noSignal = "no_signal"
    /// The engine forgot an action whose record the view had seen.
    case gone
    case complete
    case completeWithWarnings = "complete_with_warnings"
    case failed
    case cancelled

    public var isFinished: Bool { [.gone, .complete, .completeWithWarnings, .failed, .cancelled].contains(self) }
}

/// The state of the time left.
public enum ActionETA: String, Sendable, Equatable {
    /// Before 5 s and 2 %, without a total, or finished.
    case none
    case shown
    /// More than 15 s without progress: «ETA pendiente».
    case pending
}

/// What the view shows after a read.
public struct ActionSnapshot: Sendable, Equatable {
    public var display: ActionDisplay
    /// Truncated, never lower than one already shown; nil without a total.
    public var percent: Int?
    /// The highest fraction seen, for the bar; nil without a total.
    public var fraction: Double?
    public var eta: ActionETA
    /// Seconds left while `eta` is `shown`.
    public var secondsLeft: Double?
    /// Seconds since the last heartbeat, for «Sin señal».
    public var silentSeconds: Double?
    /// Seconds since the action started.
    public var elapsedSeconds: Double?
    public var resumable: Bool?
    /// The problem of the last read when it was not valid: the view kept the previous one.
    public var ignored: ActionRecordProblem?
    /// The last valid record, for the names, the log and the closing.
    public var record: ActionRecord?

    public static let preparing = Self(display: .preparing, percent: nil, fraction: nil, eta: .none)

    public init(display: ActionDisplay, percent: Int?, fraction: Double?, eta: ActionETA, secondsLeft: Double? = nil,
                silentSeconds: Double? = nil, elapsedSeconds: Double? = nil, resumable: Bool? = nil,
                ignored: ActionRecordProblem? = nil, record: ActionRecord? = nil) {
        self.display = display
        self.percent = percent
        self.fraction = fraction
        self.eta = eta
        self.secondsLeft = secondsLeft
        self.silentSeconds = silentSeconds
        self.elapsedSeconds = elapsedSeconds
        self.resumable = resumable
        self.ignored = ignored
        self.record = record
    }
}

/// The reads of one action, in order. Feed it each reply; it keeps what §4 needs between them.
public struct ActionWatch: Sendable {
    public static let etaAfterSeconds = 5.0
    public static let etaAfterFraction = 0.02
    public static let etaPendingSeconds = 15.0
    public static let noSignalSeconds = 60.0
    /// The weight of a new observation in the smoothed time left.
    public static let smoothing = 0.3

    public let progressID: String
    public private(set) var snapshot = ActionSnapshot.preparing
    private var best: Double?
    private var seenRecord = false
    private var smoothedLeft: Double?
    private var lastProgressed: Double?

    public init(progressID: String) {
        self.progressID = progressID
    }

    /// The data of an `action_progress` reply, read at `now` (seconds since 1970).
    @discardableResult
    public mutating func read(_ data: Data, at now: Double) -> ActionSnapshot {
        switch ActionRecord.read(data, progressID: progressID) {
        case .failure(let problem):
            snapshot.ignored = problem
        case .success(let record):
            snapshot = view(record, now: now)
        }
        return snapshot
    }

    /// The same for the current time.
    @discardableResult
    public mutating func read(_ data: Data) -> ActionSnapshot {
        read(data, at: Date().timeIntervalSince1970)
    }

    private mutating func view(_ record: ActionRecord, now: Double) -> ActionSnapshot {
        if record.state == .unknown {
            return ActionSnapshot(display: seenRecord ? .gone : .preparing, percent: nil, fraction: nil, eta: .none)
        }
        seenRecord = true
        if let fraction = record.fraction { best = max(best ?? fraction, fraction) }
        let percent = record.fraction == nil ? nil : best.map { Int($0 * 100) }
        let shownFraction = record.fraction == nil ? nil : best
        switch record.state {
        case .complete:
            let display: ActionDisplay = record.outcome?.warnings.isEmpty == false ? .completeWithWarnings : .complete
            return ActionSnapshot(display: display, percent: record.fraction == nil ? nil : 100,
                                  fraction: record.fraction == nil ? nil : 1, eta: .none, record: record)
        case .failed, .cancelled:
            return ActionSnapshot(display: record.state == .failed ? .failed : .cancelled, percent: percent,
                                  fraction: shownFraction, eta: .none, resumable: record.resumable, record: record)
        case .running, .unknown:
            break
        }
        let elapsed = now - record.startedAt
        let silent = now - record.updatedAt
        let display: ActionDisplay = silent > Self.noSignalSeconds ? .noSignal : .running
        var eta = ActionETA.none
        if let best, elapsed >= Self.etaAfterSeconds, best >= Self.etaAfterFraction {
            eta = now - record.progressedAt > Self.etaPendingSeconds ? .pending : .shown
        }
        // Smoothed only on a new observation of progress: a repaint without progress does not move it.
        if eta == .shown, let best, best > 0, record.progressedAt != lastProgressed {
            let pace = (record.progressedAt - record.startedAt) / best
            let raw = max(0, pace * (1 - best) - (now - record.progressedAt))
            smoothedLeft = smoothedLeft.map { Self.smoothing * raw + (1 - Self.smoothing) * $0 } ?? raw
            lastProgressed = record.progressedAt
        }
        return ActionSnapshot(display: display, percent: percent, fraction: shownFraction, eta: eta,
                              secondsLeft: eta == .shown ? smoothedLeft : nil, silentSeconds: silent, elapsedSeconds: elapsed,
                              record: record)
    }
}
