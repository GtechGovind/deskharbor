// P0 evidence harness. Public APIs only; successful Aqua capture is not LoginWindow proof.
import Foundation
import CoreGraphics
import ScreenCaptureKit
import ImageIO
import UniformTypeIdentifiers
import ApplicationServices
import SystemConfiguration
import Darwin

struct ProbeFailure: Error, CustomStringConvertible {
    let description: String
    init(_ description: String) { self.description = description }
}

@main struct Probe {
    static func emit(_ object: [String: Any]) throws {
        let data = try JSONSerialization.data(withJSONObject: object, options: [.prettyPrinted, .sortedKeys])
        FileHandle.standardOutput.write(data)
        FileHandle.standardOutput.write(Data([10]))
    }

    static func displays() throws -> [CGDirectDisplayID] {
        var count: UInt32 = 0
        guard CGGetActiveDisplayList(0, nil, &count) == .success, count <= 64 else {
            throw ProbeFailure("Cannot enumerate active displays")
        }
        if count == 0 { return [] }
        var ids = [CGDirectDisplayID](repeating: 0, count: Int(count))
        guard CGGetActiveDisplayList(count, &ids, &count) == .success else {
            throw ProbeFailure("Display layout changed during enumeration")
        }
        return Array(ids.prefix(Int(count)))
    }

    static func inspection() throws -> [String: Any] {
        var uid: uid_t = 0
        var gid: gid_t = 0
        let console = SCDynamicStoreCopyConsoleUser(nil, &uid, &gid) as String?
        let active = try displays()
        let process = ProcessInfo.processInfo
        let signingProcess = Process()
        signingProcess.executableURL = URL(fileURLWithPath: "/usr/bin/codesign")
        signingProcess.arguments = ["--display", "--verbose=4", CommandLine.arguments[0]]
        let pipe = Pipe()
        signingProcess.standardError = pipe
        signingProcess.standardOutput = FileHandle.nullDevice
        try signingProcess.run()
        let signing = pipe.fileHandleForReading.readDataToEndOfFile()
        signingProcess.waitUntilExit()
        return [
            "schema_version": 1, "platform": "macos", "environment": "native_process_observation",
            "os": process.operatingSystemVersionString, "uid": getuid(), "euid": geteuid(),
            "pid": process.processIdentifier, "executable": Bundle.main.executablePath ?? CommandLine.arguments[0],
            "console_user": console as Any? ?? NSNull(), "console_uid": uid,
            "screen_capture_permission": CGPreflightScreenCaptureAccess(),
            "accessibility_permission": AXIsProcessTrusted(),
            "codesign_exit": signingProcess.terminationStatus,
            "codesign_display": String(data: signing, encoding: .utf8) ?? "",
            "displays": active.map { id -> [String: Any] in
                let bounds = CGDisplayBounds(id)
                return ["id": id, "x": bounds.origin.x, "y": bounds.origin.y,
                        "width": CGDisplayPixelsWide(id), "height": CGDisplayPixelsHigh(id),
                        "rotation": CGDisplayRotation(id), "main": CGDisplayIsMain(id) != 0]
            },
            "p0_status": "not_run",
            "warning": "Signature display is inventory, not validation. This observation does not prove LoginWindow, logout, second-user or permission-revocation support."
        ]
    }

    static func capture(displayID: CGDirectDisplayID, path: String) async throws {
        guard CGPreflightScreenCaptureAccess() else { throw ProbeFailure("permission_needed: authorize this installed identity in Screen Recording settings") }
        let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: false)
        guard let display = content.displays.first(where: { $0.displayID == displayID }) else { throw ProbeFailure("no_active_display: requested display not shareable in this security session") }
        let width = CGDisplayPixelsWide(displayID), height = CGDisplayPixelsHigh(displayID)
        guard width > 0, height > 0, width <= 16384, height <= 16384,
              width <= (256 * 1024 * 1024) / 4 / height else { throw ProbeFailure("invalid_capture_dimensions") }
        let filter = SCContentFilter(display: display, excludingApplications: [], exceptingWindows: [])
        let config = SCStreamConfiguration()
        config.width = width
        config.height = height
        config.showsCursor = true
        let image = try await SCScreenshotManager.captureImage(contentFilter: filter, configuration: config)
        guard image.width == width, image.height == height else { throw ProbeFailure("capture_dimensions_changed") }
        let data = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(data, UTType.png.identifier as CFString, 1, nil) else { throw ProbeFailure("png_destination_failed") }
        CGImageDestinationAddImage(destination, image, nil)
        guard CGImageDestinationFinalize(destination) else { throw ProbeFailure("png_encode_failed") }
        // Exclusive creation and 0600 prevent overwriting files or following a final symlink.
        let fd = open(path, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0o600)
        guard fd >= 0 else { throw ProbeFailure("output_create_failed: \(String(cString: strerror(errno)))") }
        let handle = FileHandle(fileDescriptor: fd, closeOnDealloc: true)
        try handle.write(contentsOf: data as Data)
        try handle.synchronize()
        try handle.close()
        try emit(["output": path, "width": image.width, "height": image.height,
                  "scope": "native security-session capture only; inspect pixels and installed identities in the lab"])
    }

    static func collect() throws {
        // Each launch identity writes under its own home, not a shared world-writable log.
        let root = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".deskharbor-p0-evidence")
        if mkdir(root.path, 0o700) != 0 && errno != EEXIST { throw ProbeFailure("cannot_create_private_evidence_directory") }
        let directory = open(root.path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
        guard directory >= 0 else { throw ProbeFailure("evidence_directory_unavailable") }
        defer { close(directory) }
        var metadata = stat()
        guard fstat(directory, &metadata) == 0, metadata.st_uid == geteuid(),
              (metadata.st_mode & 0o077) == 0 else { throw ProbeFailure("evidence_directory_must_be_owned_by_current_uid_and_private") }
        var report = try inspection()
        report["collector_role"] = URL(fileURLWithPath: CommandLine.arguments[0]).lastPathComponent
        report["collected_utc"] = ISO8601DateFormatter().string(from: Date())
        let name = "inspection-\(getpid())-\(UUID().uuidString).json"
        let descriptor = openat(directory, name, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0o600)
        guard descriptor >= 0 else { throw ProbeFailure("cannot_create_exclusive_evidence_file") }
        let file = FileHandle(fileDescriptor: descriptor, closeOnDealloc: true)
        try file.write(contentsOf: JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys]))
        try file.synchronize()
        try file.close()
        try emit(["inspection_artifact": root.appendingPathComponent(name).path, "p0_status": "not_run"])
    }

    static func main() async {
        do {
            let args = Array(CommandLine.arguments.dropFirst())
            switch args.first ?? "inspect" {
            case "inspect" where args.count <= 1:
                try emit(inspection())
            case "collect" where args.count == 1:
                try collect()
            case "capture" where args.count == 3:
                guard let id = UInt32(args[1]), args[2].lowercased().hasSuffix(".png") else { throw ProbeFailure("capture requires numeric display ID and .png path") }
                try await capture(displayID: id, path: args[2])
            case "input-test" where args == ["input-test", "--acknowledge-isolated-lab"]:
                guard AXIsProcessTrusted() else { throw ProbeFailure("permission_needed: Accessibility") }
                // Fixed F8 down/up test, never credentials, arbitrary commands or a persistent input listener.
                guard let down = CGEvent(keyboardEventSource: nil, virtualKey: 100, keyDown: true),
                      let up = CGEvent(keyboardEventSource: nil, virtualKey: 100, keyDown: false) else { throw ProbeFailure("input_event_create_failed") }
                down.post(tap: .cghidEventTap)
                up.post(tap: .cghidEventTap)
                try emit(["input": "F8 down/up submitted", "status": "requires_visible_observer_confirmation"])
            default:
                throw ProbeFailure("usage: dh-macos-probe inspect | collect | capture DISPLAY_ID FILE.png | input-test --acknowledge-isolated-lab")
            }
        } catch {
            FileHandle.standardError.write(Data("\(error)\n".utf8))
            exit(1)
        }
    }
}
