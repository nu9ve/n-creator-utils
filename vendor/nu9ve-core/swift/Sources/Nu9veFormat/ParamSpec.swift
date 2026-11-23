import Foundation

// nu9ve-controls-v1 §3: a value is declared once, and every control (knob, slider, XY pad, number field)
// moves it the same way. Pure: no UI, Foundation only. The same rules live in Nu9ve.Core (Format/ParamSpec.cs),
// and both are checked against data/ui-format-vectors-v1.json.

/// How a value maps to the travel of a control.
public enum ValueScale: String, Sendable, Codable, Equatable {
    /// Equal travel, equal change.
    case linear
    /// Equal travel, equal ratio (Hz, Kelvin, effect times). Needs `min > 0`.
    case log
}

/// One step of a control: the arrows give `normal`, ⇧ (or Page Up/Down) `large`, ⌥ (Alt on Windows) `fine`.
public enum ParamStep: String, Sendable, Codable, Equatable, CaseIterable {
    case fine, normal, large
}

/// A value a person can adjust, as an app or its engine declares it.
///
/// `step` is in the value's units with a linear scale. With a log scale the value has no even step, so
/// `step` is a fraction of the travel (0.01 is a hundredth of it). `large` is `largeSteps` steps and
/// `fine` a tenth of one.
public struct ParamSpec: Sendable, Equatable, Codable {
    public var id: String
    public var label: String
    public var min: Double
    public var max: Double
    public var step: Double
    public var defaultValue: Double
    /// Where the filled part of the track starts: the centre of an EQ. `min` when not given.
    public var origin: Double
    public var scale: ValueScale
    public var unit: String
    public var fractionDigits: Int
    public var largeSteps: Double

    public init(
        id: String,
        label: String,
        min: Double,
        max: Double,
        step: Double,
        defaultValue: Double,
        origin: Double? = nil,
        scale: ValueScale = .linear,
        unit: String = "",
        fractionDigits: Int = 0,
        largeSteps: Double = 4
    ) {
        self.id = id
        self.label = label
        self.min = min
        self.max = max
        self.step = step
        self.defaultValue = defaultValue
        self.origin = origin ?? min
        self.scale = scale
        self.unit = unit
        self.fractionDigits = fractionDigits
        self.largeSteps = largeSteps
    }

    /// Whether the declaration can be used: finite bounds in order, a positive step, the default and the
    /// origin inside, and a positive minimum for a log scale.
    public var isValid: Bool {
        guard min.isFinite, max.isFinite, min < max, step.isFinite, step > 0, largeSteps >= 1,
              fractionDigits >= 0, fractionDigits <= 9 else { return false }
        guard (min...max).contains(defaultValue), (min...max).contains(origin) else { return false }
        if scale == .log { return min > 0 && step <= 1 }
        return true
    }

    /// The value kept inside the bounds. Not a number becomes the default.
    public func clamped(_ value: Double) -> Double {
        guard !value.isNaN else { return defaultValue }
        return Swift.min(max, Swift.max(min, value))
    }

    /// Where the value sits on the travel, from 0 to 1.
    public func position(of value: Double) -> Double {
        let v = clamped(value)
        switch scale {
        case .linear: return (v - min) / (max - min)
        case .log: return Foundation.log(v / min) / Foundation.log(max / min)
        }
    }

    /// The value at a point of the travel, from 0 to 1 (clamped).
    public func value(at position: Double) -> Double {
        let t = position.isNaN ? self.position(of: defaultValue) : Swift.min(1, Swift.max(0, position))
        switch scale {
        case .linear: return clamped(min + t * (max - min))
        case .log: return clamped(min * Foundation.pow(max / min, t))
        }
    }

    /// The value on the step grid that starts at `min`, for a linear scale; with a log scale, the value
    /// rounded to its `fractionDigits`.
    public func snapped(_ value: Double) -> Double {
        let v = clamped(value)
        switch scale {
        case .linear:
            let steps = ((v - min) / step).rounded(.toNearestOrAwayFromZero)
            return clamped(min + steps * step)
        case .log:
            return clamped(NumberText.rounded(v, fractionDigits: fractionDigits))
        }
    }

    /// The value after `count` steps of one kind (negative goes down), clamped. A fine step is not snapped,
    /// so it can reach between the marks; a normal or large one lands on the grid.
    public func stepped(_ value: Double, _ kind: ParamStep = .normal, count: Int = 1) -> Double {
        let size: Double
        switch kind {
        case .fine: size = step / 10
        case .normal: size = step
        case .large: size = step * largeSteps
        }
        let delta = size * Double(count)
        switch scale {
        case .linear:
            let next = clamped(value) + delta
            return kind == .fine ? clamped(next) : snapped(next)
        case .log:
            return self.value(at: position(of: value) + delta)
        }
    }

    /// Whether the value is the default one: the readout shows it in `muted`, and in `ink` once it changed.
    public func isDefault(_ value: Double) -> Bool {
        abs(clamped(value) - defaultValue) <= Swift.max(1e-9, step * 1e-6)
    }

    /// The readout: the value, inside its bounds, with its digits, the decimal separator of the language and its
    /// unit. A value that is not a number reads `—`.
    public func text(_ value: Double, decimalSeparator: String = ",") -> String {
        guard !value.isNaN else { return NumberText.notANumber }
        return NumberText.format(clamped(value), fractionDigits: fractionDigits, unit: unit, decimalSeparator: decimalSeparator)
    }
}
