// swift-tools-version: 5.9
import PackageDescription
let package = Package(name: "DeskHarborPlatformProbe", platforms: [.macOS(.v14)], products: [
    .executable(name: "dh-macos-probe", targets: ["DeskHarborProbe"])
], targets: [.executableTarget(name: "DeskHarborProbe", path: "Sources")])
