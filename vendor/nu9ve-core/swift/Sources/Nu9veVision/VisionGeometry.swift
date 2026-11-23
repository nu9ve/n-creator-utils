import CoreGraphics
import Foundation

/// The pure geometry of Nu9veVision: no Vision calls, so it is tested on its own.
/// Generalized from square views to any width and height (origin: swift/README.md, Nu9veVision).
public enum VisionGeometry {
    /// A Vision rectangle (normalized, bottom-left origin) with the origin at the top left.
    public static func topLeft(_ visionRect: CGRect) -> CGRect {
        CGRect(x: visionRect.minX, y: 1 - visionRect.maxY, width: visionRect.width, height: visionRect.height)
    }

    /// A square of radius `radius` pixels around (`cx`, `cy`) pixels, normalized.
    static func headBox(cx: Double, cy: Double, radius: Double, width: Double, height: Double, confidence: Double) -> VisionBox {
        VisionBox(rect: CGRect(x: (cx - radius) / width, y: (cy - radius) / height, width: 2 * radius / width, height: 2 * radius / height),
                  confidence: confidence)
    }

    /// A head from body pose: the head points (nose, eyes, ears) when visible, else above the neck, sized by
    /// the head points' spread or the shoulder width. Nil when the pose says nothing about the head.
    public static func head(fromPose pose: VisionPose, width: Double, height: Double) -> VisionHead? {
        func at(_ joint: VisionJoint, _ minimum: Double = 0.3) -> (x: Double, y: Double, c: Double)? {
            guard let p = pose.joints[joint], p.confidence >= minimum else { return nil }
            return (p.x * width, p.y * height, p.confidence)
        }
        let headPoints = [VisionJoint.nose, .leftEye, .rightEye, .leftEar, .rightEar].compactMap { at($0) }
        let left = at(.leftShoulder), right = at(.rightShoulder), neck = at(.neck)
        var shoulders: Double?
        if let left, let right { shoulders = hypot(left.x - right.x, left.y - right.y) }
        if !headPoints.isEmpty {
            let count = Double(headPoints.count)
            let cx = headPoints.map(\.x).reduce(0, +) / count, cy = headPoints.map(\.y).reduce(0, +) / count
            var spread = 0.0
            for a in headPoints { for b in headPoints { spread = max(spread, hypot(a.x - b.x, a.y - b.y)) } }
            let fromNeck = neck.map { hypot($0.x - cx, $0.y - cy) * 0.75 } ?? 0
            let floor = headPoints.count == 1 ? min(width, height) * 0.012 : 0
            let radius = max(spread * 0.8, (shoulders ?? 0) * 0.32, fromNeck, floor)
            guard radius > 0 else { return nil }
            let confidence = headPoints.map(\.c).reduce(0, +) / count
            return VisionHead(box: headBox(cx: cx, cy: cy, radius: radius, width: width, height: height, confidence: confidence), source: .pose)
        }
        // No head points: a head sits above the neck, about half a shoulder width up.
        guard let n = neck, let w = shoulders, let left, let right, w > 4 else { return nil }
        let ux = -(right.y - left.y) / w, uy = (right.x - left.x) / w   // perpendicular to the shoulders
        let up = uy < 0 ? (ux, uy) : (-ux, -uy)                       // pointing up the image
        return VisionHead(box: headBox(cx: n.x + up.0 * w * 0.55, cy: n.y + up.1 * w * 0.55, radius: w * 0.38,
                                       width: width, height: height, confidence: n.c * 0.8), source: .pose)
    }

    /// A head estimated from a person rectangle (normalized, top-left): the top of a full body, or of an upper
    /// body. Nil for a box shorter than 12 pixels.
    public static func head(fromBody body: VisionBox, width: Double, height: Double) -> VisionHead? {
        let w = body.rect.width * width, h = body.rect.height * height, top = body.rect.minY * height
        guard h > 12 else { return nil }
        let fullBody = h > w * 1.6
        let radius = fullBody ? max(h * 0.085, w * 0.22) : max(h * 0.2, w * 0.25)
        return VisionHead(box: headBox(cx: body.rect.midX * width, cy: top + (fullBody ? h * 0.09 : h * 0.22), radius: radius,
                                       width: width, height: height, confidence: body.confidence * 0.85), source: .body)
    }

    static func grown(_ r: CGRect) -> CGRect { r.insetBy(dx: -r.width * 0.2, dy: -r.height * 0.2) }

    /// Where people are, for gating the segmentation: every body grown 20%, and every pose with at least three
    /// points above 0.2 confidence, bounded, padded and grown the same.
    public static func personRegions(bodies: [VisionBox], poses: [VisionPose]) -> [CGRect] {
        var regions = bodies.map { grown($0.rect) }
        for pose in poses {
            let seen = pose.points.filter { $0.confidence > 0.2 }
            guard seen.count >= 3 else { continue }
            let xs = seen.map(\.x), ys = seen.map(\.y)
            let box = CGRect(x: xs.min()!, y: ys.min()!, width: xs.max()! - xs.min()!, height: ys.max()! - ys.min()!)
            regions.append(grown(box.insetBy(dx: -max(0.02, box.width * 0.25), dy: -max(0.03, box.height * 0.2))))
        }
        return regions
    }

    /// The mask with every pixel outside `regions` set to 0. A pixel counts by its centre.
    public static func gated(_ mask: VisionMask, to regions: [CGRect]) -> VisionMask {
        var bytes = mask.bytes
        for y in 0..<mask.height {
            let v = (Double(y) + 0.5) / Double(mask.height)
            for x in 0..<mask.width {
                let p = CGPoint(x: (Double(x) + 0.5) / Double(mask.width), y: v)
                if !regions.contains(where: { $0.contains(p) }) { bytes[y * mask.width + x] = 0 }
            }
        }
        return VisionMask(width: mask.width, height: mask.height, bytes: bytes)
    }
}
