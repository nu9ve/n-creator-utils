#if canImport(Vision)
import CoreGraphics
import CoreVideo
import Foundation
import Vision

/// Apple Vision on one image, with the options an app asks for. Synchronous: call it off the main thread.
/// Where it comes from: swift/README.md, Nu9veVision.
public enum Nu9veVisionDetector {
    public static func observe(_ pixelBuffer: CVPixelBuffer, options: VisionOptions = VisionOptions()) throws -> VisionObservation {
        let handler = VNImageRequestHandler(cvPixelBuffer: pixelBuffer, orientation: .up, options: [:])
        return try observe(handler, width: Double(CVPixelBufferGetWidth(pixelBuffer)), height: Double(CVPixelBufferGetHeight(pixelBuffer)), options: options)
    }

    public static func observe(_ image: CGImage, options: VisionOptions = VisionOptions()) throws -> VisionObservation {
        let handler = VNImageRequestHandler(cgImage: image, orientation: .up, options: [:])
        return try observe(handler, width: Double(image.width), height: Double(image.height), options: options)
    }

    static func observe(_ handler: VNImageRequestHandler, width: Double, height: Double, options: VisionOptions) throws -> VisionObservation {
        let faceRequest = VNDetectFaceRectanglesRequest()
        let bodyRequest = VNDetectHumanRectanglesRequest()
        bodyRequest.upperBodyOnly = options.bodies == .upper
        let poseRequest = VNDetectHumanBodyPoseRequest()
        var requests: [VNRequest] = []
        if options.faces { requests.append(faceRequest) }
        if options.bodies != .none { requests.append(bodyRequest) }
        if options.poses { requests.append(poseRequest) }
        if !requests.isEmpty { try handler.perform(requests) }

        var observation = VisionObservation()
        if options.faces {
            observation.faces = (faceRequest.results ?? []).map {
                VisionFace(box: VisionBox(rect: VisionGeometry.topLeft($0.boundingBox), confidence: Double($0.confidence)), roll: $0.roll?.doubleValue)
            }
        }
        if options.bodies != .none {
            observation.bodies = (bodyRequest.results ?? []).map {
                VisionBox(rect: VisionGeometry.topLeft($0.boundingBox), confidence: Double($0.confidence))
            }
        }
        if options.poses {
            observation.poses = (poseRequest.results ?? []).compactMap(pose)
        }
        observation.heads = observation.poses.compactMap { VisionGeometry.head(fromPose: $0, width: width, height: height) }
            + observation.bodies.compactMap { VisionGeometry.head(fromBody: $0, width: width, height: height) }
        let gating = options.gateSegmentationToPeople && (options.bodies != .none || options.poses)
        if gating { observation.personRegions = VisionGeometry.personRegions(bodies: observation.bodies, poses: observation.poses) }

        if let level = options.segmentation, !(gating && observation.personRegions.isEmpty) {
            let segmentation = VNGeneratePersonSegmentationRequest()
            segmentation.qualityLevel = switch level {
            case .fast: .fast
            case .balanced: .balanced
            case .accurate: .accurate
            }
            segmentation.outputPixelFormat = kCVPixelFormatType_OneComponent8
            try handler.perform([segmentation])
            if let buffer = segmentation.results?.first?.pixelBuffer {
                let mask = copy(buffer)
                observation.mask = gating ? VisionGeometry.gated(mask, to: observation.personRegions) : mask
            }
        }
        return observation
    }

    static func pose(_ observation: VNHumanBodyPoseObservation) -> VisionPose? {
        guard let recognized = try? observation.recognizedPoints(.all) else { return nil }
        func point(_ p: VNRecognizedPoint) -> VisionPoint {
            VisionPoint(x: Double(p.location.x), y: 1 - Double(p.location.y), confidence: Double(p.confidence))
        }
        let names: [VisionJoint: VNHumanBodyPoseObservation.JointName] = [
            .nose: .nose, .leftEye: .leftEye, .rightEye: .rightEye, .leftEar: .leftEar, .rightEar: .rightEar,
            .neck: .neck, .leftShoulder: .leftShoulder, .rightShoulder: .rightShoulder
        ]
        var joints: [VisionJoint: VisionPoint] = [:]
        for (joint, name) in names { if let p = recognized[name] { joints[joint] = point(p) } }
        return VisionPose(joints: joints, points: recognized.values.map(point))
    }

    static func copy(_ buffer: CVPixelBuffer) -> VisionMask {
        CVPixelBufferLockBaseAddress(buffer, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(buffer, .readOnly) }
        let width = CVPixelBufferGetWidth(buffer), height = CVPixelBufferGetHeight(buffer), row = CVPixelBufferGetBytesPerRow(buffer)
        var bytes = [UInt8](repeating: 0, count: width * height)
        if let base = CVPixelBufferGetBaseAddress(buffer)?.assumingMemoryBound(to: UInt8.self) {
            for y in 0..<height { for x in 0..<width { bytes[y * width + x] = base[y * row + x] } }
        }
        return VisionMask(width: width, height: height, bytes: bytes)
    }
}
#endif
