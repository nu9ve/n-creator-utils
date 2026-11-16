// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "MediaMenu", platforms: [.macOS(.v15)],
    products: [.executable(name: "MediaMenu", targets: ["MediaMenu"])],
    dependencies: [.package(path: "../../vendor/nu9ve-core/swift")],
    targets: [
        .target(name: "MediaMenuCore"),
        .executableTarget(name: "MediaMenu", dependencies: ["MediaMenuCore", .product(name: "Nu9veTheme", package: "swift")]),
        .testTarget(name: "MediaMenuCoreTests", dependencies: ["MediaMenuCore", .product(name: "Nu9veTheme", package: "swift")])
    ]
)
