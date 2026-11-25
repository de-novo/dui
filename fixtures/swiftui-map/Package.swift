// swift-tools-version:6.0
// Fixture for SwiftUI screen discovery (docs/en/roadmap.md#swiftui-screen-discovery).
import PackageDescription

let package = Package(
    name: "SwiftUIMapFixture",
    platforms: [.iOS(.v18), .macOS(.v15)],
    products: [.library(name: "SwiftUIMapFixture", targets: ["SwiftUIMapFixture"])],
    targets: [.target(name: "SwiftUIMapFixture")]
)
