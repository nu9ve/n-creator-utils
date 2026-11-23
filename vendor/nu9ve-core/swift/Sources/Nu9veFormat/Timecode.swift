import Foundation

// nu9ve-controls-v1 §4: one time code for the suite, in digits of one width.

/// Time code of a position in seconds.
public enum Timecode {
    /// `m:ss.cc`, or `h:mm:ss.cc` from an hour on: hundredths, because the frame rate changes from clock to clock.
    /// Negative times carry U+2212; a time that is not finite is `--:--.--`.
    public static func format(_ seconds: Double) -> String {
        guard seconds.isFinite else { return "--:--.--" }
        let hundredths = (abs(seconds) * 100).rounded(.toNearestOrAwayFromZero)
        guard hundredths < 9e15 else { return "--:--.--" }
        let total = UInt64(hundredths)
        let cc = total % 100, s = (total / 100) % 60, m = (total / 6000) % 60, h = total / 360_000
        let body = h > 0
            ? "\(h):\(two(m)):\(two(s)).\(two(cc))"
            : "\(m):\(two(s)).\(two(cc))"
        return (seconds < 0 && total > 0 ? NumberText.minus : "") + body
    }

    /// `h:mm:ss:ff` where the frames matter and the rate is known (a whole number of frames per second).
    /// The frame is the one the time falls in; a time that is not finite, or a rate under 1, is `--:--:--:--`.
    public static func format(_ seconds: Double, framesPerSecond: Int) -> String {
        guard seconds.isFinite, framesPerSecond >= 1 else { return "--:--:--:--" }
        let frames = (abs(seconds) * Double(framesPerSecond) + 1e-9).rounded(.down)
        guard frames < 9e15 else { return "--:--:--:--" }
        let total = UInt64(frames), fps = UInt64(framesPerSecond)
        let ff = total % fps, whole = total / fps
        let s = whole % 60, m = (whole / 60) % 60, h = whole / 3600
        let body = "\(h):\(two(m)):\(two(s)):\(pad(ff, to: String(fps - 1).count))"
        return (seconds < 0 && total > 0 ? NumberText.minus : "") + body
    }

    private static func two(_ value: UInt64) -> String { pad(value, to: 2) }

    private static func pad(_ value: UInt64, to width: Int) -> String {
        let text = String(value)
        return text.count >= width ? text : String(repeating: "0", count: width - text.count) + text
    }
}
