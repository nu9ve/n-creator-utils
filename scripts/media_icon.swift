import AppKit

let output = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
for (name, pixels) in [("icon_16x16",16),("icon_16x16@2x",32),("icon_32x32",32),("icon_32x32@2x",64),("icon_128x128",128),("icon_128x128@2x",256),("icon_256x256",256),("icon_256x256@2x",512),("icon_512x512",512),("icon_512x512@2x",1024)] {
    let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
    NSGraphicsContext.current!.cgContext.scaleBy(x: CGFloat(pixels)/1024, y: CGFloat(pixels)/1024)
    let background = NSBezierPath(roundedRect: NSRect(x: 45, y: 45, width: 934, height: 934), xRadius: 212, yRadius: 212)
    NSGradient(starting: NSColor(srgbRed: 0.17, green: 0.13, blue: 0.42, alpha: 1), ending: NSColor(srgbRed: 0.29, green: 0.43, blue: 0.95, alpha: 1))!.draw(in: background, angle: 55)
    let card = NSBezierPath()
    card.move(to: NSPoint(x: 295, y: 230)); card.line(to: NSPoint(x: 295, y: 680)); card.line(to: NSPoint(x: 420, y: 805)); card.line(to: NSPoint(x: 730, y: 805)); card.line(to: NSPoint(x: 730, y: 230)); card.close()
    NSColor.white.withAlphaComponent(0.19).setFill(); card.fill()
    NSColor.white.withAlphaComponent(0.65).setStroke(); card.lineWidth = 18; card.stroke()
    ("n" as NSString).draw(in: NSRect(x: 378, y: 300, width: 315, height: 410), withAttributes: [.font: NSFont.systemFont(ofSize: 370, weight: .semibold), .foregroundColor: NSColor.white])
    for x in [470, 540, 610] { NSColor.white.withAlphaComponent(0.7).setFill(); NSBezierPath(roundedRect: NSRect(x: CGFloat(x), y: 704, width: 35, height: 58), xRadius: 9, yRadius: 9).fill() }
    NSGraphicsContext.restoreGraphicsState()
    try bitmap.representation(using: .png, properties: [:])!.write(to: output.appendingPathComponent(name + ".png"))
}
