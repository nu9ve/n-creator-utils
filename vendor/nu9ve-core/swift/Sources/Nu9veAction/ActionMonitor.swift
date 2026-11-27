import Foundation
import Observation

/// How the action is seen, which sets how often it is read (nu9ve-action-v1 §4).
public enum ActionViewSize: Sendable, Equatable {
    /// On screen or in the compact panel: every 0.75 s, within the contract's 0.5 to 1 s.
    case screen
    /// Only the indicator: every 5 s, the slowest the contract allows.
    case indicator

    public var interval: Duration {
        switch self {
        case .screen: return .milliseconds(750)
        case .indicator: return .seconds(5)
        }
    }
}

/// Reads one action until it ends. The app gives it how to ask its engine: `fetch` sends `action_progress`
/// with the id and returns the reply's `data` as bytes (for example `EngineReply.bytes` of `Nu9veEngine`).
/// A failed fetch is not the end: the view keeps the last snapshot and the heartbeat says when it is silent.
@MainActor
@Observable
public final class ActionMonitor {
    public let progressID: String
    public private(set) var snapshot = ActionSnapshot.preparing
    /// The last fetch that failed, for the app's log; cleared by the next reply.
    public private(set) var lastFetchError: String?
    public var size: ActionViewSize
    @ObservationIgnored private var watch: ActionWatch
    @ObservationIgnored private var task: Task<Void, Never>?
    @ObservationIgnored private let fetch: @Sendable (String) async throws -> Data
    @ObservationIgnored private let clock: @Sendable () -> Double

    /// `clock` gives seconds since 1970; a test passes a fixed one.
    public init(progressID: String, size: ActionViewSize = .screen,
                clock: @escaping @Sendable () -> Double = { Date().timeIntervalSince1970 },
                fetch: @escaping @Sendable (String) async throws -> Data) {
        self.progressID = progressID
        self.size = size
        self.clock = clock
        self.fetch = fetch
        watch = ActionWatch(progressID: progressID)
    }

    /// Starts reading; a second call does nothing while it reads.
    public func start() {
        guard task == nil else { return }
        task = Task { [weak self] in
            while let self, !Task.isCancelled {
                await self.readOnce()
                if self.snapshot.display.isFinished { break }
                try? await Task.sleep(for: self.size.interval)
            }
            self?.task = nil
        }
    }

    public func stop() {
        task?.cancel()
        task = nil
    }

    /// One read now, for a test or for a refresh the person asked for.
    public func readOnce() async {
        do {
            let data = try await fetch(progressID)
            snapshot = watch.read(data, at: clock())
            lastFetchError = nil
        } catch {
            lastFetchError = String(describing: error)
        }
    }

    public var isReading: Bool { task != nil }
}
