// swift-tools-version:6.0
// Discover SwiftUI screens from an official build's index store and swift-syntax call shapes.
// See docs/en/roadmap.md#swiftui-screen-discovery.
import PackageDescription

let package = Package(
    name: "dui-swift-discover",
    platforms: [.macOS(.v14)],
    products: [.executable(name: "dui-swift-discover", targets: ["dui-swift-discover"])],
    dependencies: [
        .package(url: "https://github.com/swiftlang/swift-syntax.git", exact: "603.0.2"),
        // Tag swift-6.3.3-RELEASE.
        .package(url: "https://github.com/swiftlang/indexstore-db.git", revision: "003ac41513ba291f10ff1a0147ae68588914668d"),
    ],
    targets: [
        .target(
            name: "DuiSwiftDiscover",
            dependencies: [
                .product(name: "SwiftSyntax", package: "swift-syntax"),
                .product(name: "SwiftParser", package: "swift-syntax"),
                .product(name: "IndexStoreDB", package: "indexstore-db"),
            ]
        ),
        .executableTarget(name: "dui-swift-discover", dependencies: ["DuiSwiftDiscover"]),
    ]
)
