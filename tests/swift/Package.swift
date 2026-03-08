// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "ZettelDBTests",
    platforms: [.macOS(.v13), .iOS(.v16)],
    targets: [
        .binaryTarget(
            name: "ZettelDBFFI",
            path: "../../out/swift/ZettelDB.xcframework"
        ),
        .target(
            name: "ZettelDB",
            dependencies: ["ZettelDBFFI"],
            path: "Sources/ZettelDB"
        ),
        .testTarget(
            name: "ZettelDBTests",
            dependencies: ["ZettelDB"],
            path: "Tests/ZettelDBTests"
        ),
    ]
)
