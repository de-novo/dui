import Foundation
import XCTest

final class RunnerTests: XCTestCase {
    @MainActor
    func testCompassScreens() async throws {
        // RunnerTests is hosted inside the sample app. Yield the main thread so
        // Flutter can draw frames while XCTest waits for the widget assertions.
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("dui-capture")
        let result = directory.appendingPathComponent("result.json")
        let progress = directory.appendingPathComponent("progress.txt")
        let deadline = Date().addingTimeInterval(300)
        var lastStep = "Waiting for the Flutter entry point"
        while !FileManager.default.fileExists(atPath: result.path) {
            if let step = try? String(contentsOf: progress, encoding: .utf8),
               !step.isEmpty, step != lastStep {
                lastStep = step
                print(step)
            }
            guard Date() < deadline else {
                XCTFail("Compass did not finish: \(lastStep)")
                return
            }
            try await Task.sleep(nanoseconds: 250_000_000)
        }

        let data = try Data(contentsOf: result)
        let report = try JSONDecoder().decode(Report.self, from: data)
        guard report.passed else {
            XCTFail(report.failures.compactMap { $0 }.joined(separator: "\n"))
            return
        }
        attach(data, named: "dui-compass-result", type: "public.json")
        for screen in report.screens {
            let image = directory.appendingPathComponent(screen.name).appendingPathExtension("png")
            attach(try Data(contentsOf: image), named: "dui-compass-frame-" + screen.name, type: "public.png")
        }
    }

    private func attach(_ data: Data, named name: String, type: String) {
        let attachment = XCTAttachment(data: data, uniformTypeIdentifier: type)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private struct Report: Decodable {
        struct Screen: Decodable { let name: String }
        let passed: Bool
        let failures: [String?]
        let screens: [Screen]
    }
}
