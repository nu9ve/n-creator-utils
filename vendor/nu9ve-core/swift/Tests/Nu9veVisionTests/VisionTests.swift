import CoreGraphics
import Foundation
import Testing
import Nu9veVision

private func near(_ a: Double, _ b: Double, _ tolerance: Double = 1e-9) -> Bool { abs(a - b) <= tolerance }

@Test func visionRectsGoToTopLeft() {
    let rect = VisionGeometry.topLeft(CGRect(x: 0.1, y: 0.6, width: 0.2, height: 0.3))
    #expect(near(rect.minX, 0.1) && near(rect.minY, 0.1) && near(rect.width, 0.2) && near(rect.height, 0.3))
}

@Test func aHeadFromHeadPointsIsCentredOnThem() throws {
    let pose = VisionPose(joints: [
        .leftEye: VisionPoint(x: 0.45, y: 0.30, confidence: 0.9),
        .rightEye: VisionPoint(x: 0.55, y: 0.30, confidence: 0.9),
        .nose: VisionPoint(x: 0.50, y: 0.35, confidence: 0.8)
    ], points: [])
    let head = try #require(VisionGeometry.head(fromPose: pose, width: 1000, height: 1000))
    #expect(head.source == .pose)
    #expect(near(head.box.rect.midX, 0.5, 1e-6))
    #expect(near(head.box.rect.midY, (0.30 + 0.30 + 0.35) / 3, 1e-6))
    // The spread is the eyes' distance (100 px); the radius is 0.8 of it.
    #expect(near(head.box.rect.width * 1000, 2 * 80, 1e-6))
    #expect(near(head.box.confidence, (0.9 + 0.9 + 0.8) / 3, 1e-9))
}

@Test func aHeadFromBehindSitsAboveTheNeck() throws {
    let pose = VisionPose(joints: [
        .neck: VisionPoint(x: 0.5, y: 0.5, confidence: 1),
        .leftShoulder: VisionPoint(x: 0.4, y: 0.5, confidence: 1),
        .rightShoulder: VisionPoint(x: 0.6, y: 0.5, confidence: 1)
    ], points: [])
    let head = try #require(VisionGeometry.head(fromPose: pose, width: 1000, height: 1000))
    #expect(head.box.rect.midY < 0.5)   // up the image
    #expect(near(head.box.rect.midX, 0.5, 1e-6))
    #expect(near(head.box.rect.midY, 0.5 - 0.2 * 0.55, 1e-6))
    #expect(near(head.box.confidence, 0.8))
}

@Test func aPoseWithoutHeadOrShouldersHasNoHead() {
    let weak = VisionPose(joints: [.nose: VisionPoint(x: 0.5, y: 0.5, confidence: 0.1)], points: [])
    #expect(VisionGeometry.head(fromPose: weak, width: 100, height: 100) == nil)
}

@Test func aHeadFromABodyIsAtItsTop() throws {
    let full = VisionBox(rect: CGRect(x: 0.4, y: 0.2, width: 0.1, height: 0.5), confidence: 1)
    let head = try #require(VisionGeometry.head(fromBody: full, width: 1000, height: 1000))
    #expect(head.source == .body)
    #expect(near(head.box.rect.midY, 0.2 + 0.5 * 0.09, 1e-6))
    #expect(near(head.box.confidence, 0.85))
    let upper = VisionBox(rect: CGRect(x: 0.4, y: 0.2, width: 0.3, height: 0.3), confidence: 1)
    let upperHead = try #require(VisionGeometry.head(fromBody: upper, width: 1000, height: 1000))
    #expect(near(upperHead.box.rect.midY, 0.2 + 0.3 * 0.22, 1e-6))
    let tiny = VisionBox(rect: CGRect(x: 0, y: 0, width: 0.01, height: 0.01), confidence: 1)
    #expect(VisionGeometry.head(fromBody: tiny, width: 1000, height: 1000) == nil)
}

@Test func aNonSquareImageKeepsTheHeadSquareInPixels() throws {
    let body = VisionBox(rect: CGRect(x: 0.4, y: 0.2, width: 0.05, height: 0.5), confidence: 1)
    let head = try #require(VisionGeometry.head(fromBody: body, width: 1920, height: 1080))
    #expect(near(head.box.rect.width * 1920, head.box.rect.height * 1080, 1e-6))
}

@Test func personRegionsGrowBodiesAndBoundPoses() {
    let body = VisionBox(rect: CGRect(x: 0.4, y: 0.4, width: 0.2, height: 0.2), confidence: 1)
    let pose = VisionPose(joints: [:], points: [
        VisionPoint(x: 0.1, y: 0.1, confidence: 0.9), VisionPoint(x: 0.2, y: 0.3, confidence: 0.9), VisionPoint(x: 0.15, y: 0.2, confidence: 0.9)
    ])
    let few = VisionPose(joints: [:], points: [VisionPoint(x: 0.9, y: 0.9, confidence: 0.9)])
    let regions = VisionGeometry.personRegions(bodies: [body], poses: [pose, few])
    #expect(regions.count == 2)
    #expect(near(regions[0].width, 0.2 * 1.4, 1e-9))
    #expect(regions[1].contains(CGPoint(x: 0.15, y: 0.2)))
}

@Test func theMaskOnlyCountsWherePeopleAre() {
    let mask = VisionMask(width: 4, height: 2, bytes: [255, 255, 255, 255, 255, 255, 255, 255])
    let gated = VisionGeometry.gated(mask, to: [CGRect(x: 0, y: 0, width: 0.5, height: 1)])
    #expect(gated.bytes == [255, 255, 0, 0, 255, 255, 0, 0])
    #expect(VisionGeometry.gated(mask, to: []).bytes == [UInt8](repeating: 0, count: 8))
}

#if canImport(Vision)
private func blankImage(width: Int = 256, height: Int = 256) throws -> CGImage {
    let context = try #require(CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
                                         space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
    context.setFillColor(CGColor(red: 0.5, green: 0.5, blue: 0.5, alpha: 1))
    context.fill(CGRect(x: 0, y: 0, width: width, height: height))
    return try #require(context.makeImage())
}

@Test func aBlankImageHasNobody() throws {
    let observation = try Nu9veVisionDetector.observe(try blankImage(), options: VisionOptions(faces: true, bodies: .full, poses: true, segmentation: .fast))
    #expect(observation.faces.isEmpty && observation.bodies.isEmpty && observation.heads.isEmpty)
    // Nobody found, so the gated segmentation is not even run.
    #expect(observation.mask == nil)
}

@Test func onlyWhatIsAskedForRuns() throws {
    let observation = try Nu9veVisionDetector.observe(try blankImage(), options: VisionOptions(faces: false))
    #expect(observation == VisionObservation())
}
#endif
