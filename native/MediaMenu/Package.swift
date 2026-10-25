// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "MediaMenu", platforms: [.macOS(.v14)],
    products: [.executable(name: "MediaMenu", targets: ["MediaMenu"])],
    targets: [
        .target(name: "MediaMenuCore"),
        .executableTarget(name: "MediaMenu", dependencies: ["MediaMenuCore"]),
        .testTarget(name: "MediaMenuCoreTests", dependencies: ["MediaMenuCore"])
    ]
)
