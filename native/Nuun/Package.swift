// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "Nuun", platforms: [.macOS(.v15)],
    products: [.executable(name: "Nuun", targets: ["Nuun"])],
    dependencies: [.package(path: "../../vendor/nu9ve-core/swift")],
    targets: [
        .target(name: "NuunCore", dependencies: [.product(name: "Nu9veTheme", package: "swift")]),
        // NuMacShell (nu9ve/core): the square window, the bar of the three commons, About and the app's identity.
        // Nu9veAction and Nu9veActionProgress (nu9ve/core): the record of a long action and its common view.
        .executableTarget(name: "Nuun", dependencies: ["NuunCore", .product(name: "Nu9veTheme", package: "swift"),
                                                       .product(name: "NuMacShell", package: "swift"),
                                                       .product(name: "Nu9veAction", package: "swift"),
                                                       .product(name: "Nu9veActionProgress", package: "swift")]),
        .testTarget(name: "NuunCoreTests", dependencies: ["NuunCore", .product(name: "Nu9veTheme", package: "swift"),
                                                          .product(name: "NuMacShell", package: "swift"),
                                                          .product(name: "Nu9veAction", package: "swift")],
                    exclude: ["Fixtures"])
    ]
)
