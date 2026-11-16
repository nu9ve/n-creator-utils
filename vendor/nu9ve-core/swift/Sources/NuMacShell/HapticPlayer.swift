#if os(macOS)
import AppKit

/// One touch of the trackpad's motor. The system offers only these three.
public enum HapticTouch: Sendable {
    /// The lightest: crossing something.
    case tick
    /// A step: a value changed.
    case notch
    /// The fullest: something landed.
    case thud
}

/// A rhythm of touches, timed in seconds from its start.
public typealias HapticPhrase = [(at: Double, touch: HapticTouch)]

/// Plays phrases on a Force Touch trackpad. On anything else the system does nothing, so
/// callers never need to ask. A motor cannot keep up with a pointer crossing a list, so
/// touches closer than `minimumGap` are dropped.
@MainActor
public final class HapticPlayer {
    private let perform: @MainActor (HapticTouch) -> Void
    private let minimumGap: Double
    private let clock: @MainActor () -> Double
    private var lastTouch = -Double.infinity

    public convenience init() {
        self.init(minimumGap: 0.05, clock: { ProcessInfo.processInfo.systemUptime }) { touch in
            let pattern: NSHapticFeedbackManager.FeedbackPattern
            switch touch {
            case .tick: pattern = .alignment
            case .notch: pattern = .levelChange
            case .thud: pattern = .generic
            }
            NSHapticFeedbackManager.defaultPerformer.perform(pattern, performanceTime: .now)
        }
    }

    init(minimumGap: Double, clock: @escaping @MainActor () -> Double, perform: @escaping @MainActor (HapticTouch) -> Void) {
        self.minimumGap = minimumGap
        self.clock = clock
        self.perform = perform
    }

    public func play(_ phrase: HapticPhrase) {
        for step in phrase {
            if step.at <= 0 {
                touch(step.touch)
            } else {
                Task { [weak self] in
                    try? await Task.sleep(for: .seconds(step.at))
                    self?.touch(step.touch)
                }
            }
        }
    }

    func touch(_ kind: HapticTouch) {
        let now = clock()
        guard now - lastTouch >= minimumGap else { return }
        lastTouch = now
        perform(kind)
    }
}

/// Reads how hard the trackpad is being pressed, wherever the pointer is.
@MainActor
public final class PressureMonitor {
    /// 0...1 inside the current stage, or nil on a device that has not reported any.
    public private(set) var pressure: Double?
    private var stage = 0
    private var monitor: Any?
    public var onDeepPress: (@MainActor () -> Void)?

    public init() {}

    public func start() {
        guard monitor == nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .pressure) { [weak self] event in
            let reading = (pressure: Double(event.pressure), stage: event.stage)
            MainActor.assumeIsolated { self?.update(pressure: reading.pressure, stage: reading.stage) }
            return event
        }
    }

    func update(pressure: Double, stage: Int) {
        self.pressure = pressure
        if stage >= 2, self.stage < 2 { onDeepPress?() }
        self.stage = stage
    }
}
#endif
