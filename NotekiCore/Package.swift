// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "NotekiCore",
    platforms: [.macOS(.v15)],
    products: [
        .library(name: "NotekiCore", targets: ["NotekiCore"]),
    ],
    targets: [
        .target(name: "NotekiCore"),
        .testTarget(name: "NotekiCoreTests", dependencies: ["NotekiCore"]),
    ]
)
