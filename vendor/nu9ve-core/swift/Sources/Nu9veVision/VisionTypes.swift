import CoreGraphics

// Nu9veVision: Apple Vision on one image, the part two apps did the same way (swift/README.md). Every box is normalized with the origin at the top left
// (y grows down), whatever the app does with it after (a sphere direction, a framing crop).

/// A detected region: normalized, top-left origin, and Vision's confidence (0…1).
public struct VisionBox: Sendable, Equatable {
    public var rect: CGRect
    public var confidence: Double

    public init(rect: CGRect, confidence: Double) {
        self.rect = rect
        self.confidence = confidence
    }
}

/// A face from `VNDetectFaceRectanglesRequest`, with its roll in radians when Vision gives one.
public struct VisionFace: Sendable, Equatable {
    public var box: VisionBox
    public var roll: Double?

    public init(box: VisionBox, roll: Double? = nil) {
        self.box = box
        self.roll = roll
    }
}

/// A head estimated where a face may not be visible: in profile, from behind, far away.
public struct VisionHead: Sendable, Equatable {
    public enum Source: String, Sendable, Equatable {
        /// From body pose: nose, eyes and ears, or above the neck.
        case pose
        /// From a person rectangle: the top of the body.
        case body
    }

    /// A square around the head, in pixels, normalized by the image's width and height.
    public var box: VisionBox
    public var source: Source

    public init(box: VisionBox, source: Source) {
        self.box = box
        self.source = source
    }
}

/// A point of a body pose: normalized, top-left origin.
public struct VisionPoint: Sendable, Equatable {
    public var x: Double
    public var y: Double
    public var confidence: Double

    public init(x: Double, y: Double, confidence: Double) {
        self.x = x
        self.y = y
        self.confidence = confidence
    }
}

/// The pose joints a head estimate uses. The rest of a pose only bounds the person (`VisionPose.points`).
public enum VisionJoint: String, CaseIterable, Sendable {
    case nose, leftEye, rightEye, leftEar, rightEar, neck, leftShoulder, rightShoulder
}

/// One person's body pose.
public struct VisionPose: Sendable, Equatable {
    public var joints: [VisionJoint: VisionPoint]
    /// Every recognized point, named or not.
    public var points: [VisionPoint]

    public init(joints: [VisionJoint: VisionPoint], points: [VisionPoint]) {
        self.joints = joints
        self.points = points
    }
}

/// A one-byte-per-pixel mask, rows top to bottom with no padding. 255 is a person.
public struct VisionMask: Sendable, Equatable {
    public var width: Int
    public var height: Int
    public var bytes: [UInt8]

    public init(width: Int, height: Int, bytes: [UInt8]) {
        precondition(bytes.count == width * height, "a mask has width × height bytes")
        self.width = width
        self.height = height
        self.bytes = bytes
    }
}

/// What to look for. Each request costs time, so an app asks only for what it uses.
public struct VisionOptions: Sendable, Equatable {
    public enum Bodies: Sendable, Equatable {
        case none
        /// Head and shoulders: a framing fallback when the face turns away.
        case upper
        /// The whole person: a fallback for small or far people.
        case full
    }

    public enum Segmentation: Sendable, Equatable {
        case fast, balanced, accurate
    }

    public var faces: Bool
    public var bodies: Bodies
    /// Body pose, for heads in profile or from behind.
    public var poses: Bool
    /// Person segmentation, the slow part; nil to skip it.
    public var segmentation: Segmentation?
    /// Segmentation also paints walls and fabric where nobody is: with this, it only counts inside
    /// the people found in the same image (bodies and poses), and is skipped when there are none.
    /// Without bodies or poses requested there is nothing to gate with, and the mask is kept whole.
    public var gateSegmentationToPeople: Bool

    public init(faces: Bool = true, bodies: Bodies = .none, poses: Bool = false,
                segmentation: Segmentation? = nil, gateSegmentationToPeople: Bool = true) {
        self.faces = faces
        self.bodies = bodies
        self.poses = poses
        self.segmentation = segmentation
        self.gateSegmentationToPeople = gateSegmentationToPeople
    }
}

/// What Vision found in one image.
public struct VisionObservation: Sendable, Equatable {
    public var faces: [VisionFace]
    public var bodies: [VisionBox]
    public var poses: [VisionPose]
    /// From poses first, then from bodies. Several can be the same person: merging is the app's.
    public var heads: [VisionHead]
    /// Where people are (bodies and poses, grown), the gate of the mask.
    public var personRegions: [CGRect]
    public var mask: VisionMask?

    public init(faces: [VisionFace] = [], bodies: [VisionBox] = [], poses: [VisionPose] = [],
                heads: [VisionHead] = [], personRegions: [CGRect] = [], mask: VisionMask? = nil) {
        self.faces = faces
        self.bodies = bodies
        self.poses = poses
        self.heads = heads
        self.personRegions = personRegions
        self.mask = mask
    }
}
