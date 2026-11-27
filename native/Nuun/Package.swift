// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "Nuun", platforms: [.macOS(.v15)],
    products: [.executable(name: "Nuun", targets: ["Nuun"])],
    dependencies: [.package(path: "../../vendor/nu9ve-core/swift")],
    targets: [
        .target(name: "NuunCore", dependencies: [.product(name: "Nu9veTheme", package: "swift")]),
        .executableTarget(name: "Nuun", dependencies: ["NuunCore", .product(name: "Nu9veTheme", package: "swift")]),
        .testTarget(name: "NuunCoreTests", dependencies: ["NuunCore", .product(name: "Nu9veTheme", package: "swift")])
    ]
)
