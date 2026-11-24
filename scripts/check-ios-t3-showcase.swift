import Foundation
import Vision

// Report allowlisted launch states. The diagnostic screenshot stays on the runner.
let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.recognitionLanguages = ["en-US"]
try VNImageRequestHandler(url: URL(fileURLWithPath: CommandLine.arguments[1])).perform([request])
let visible = (request.results ?? []).compactMap { $0.topCandidates(1).first?.string }
    .joined(separator: " ").lowercased()
let witnesses = [
    "development servers", "enter url manually", "no development build", "bundling",
    "allow", "local network", "open from qr", "add environment", "add a connection",
    "remote coding feel local", "remote-first command center", "review changes", "uncaught error", "failed to load",
]
let matched = witnesses.filter { visible.contains($0) }
print(matched.isEmpty ? "No startup witness recognized" : matched.joined(separator: ", "))
