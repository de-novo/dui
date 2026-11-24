"""Exercise the showcase OCR gate with temporary PNGs before building the iOS app."""

from pathlib import Path
import runpy
import subprocess
import tempfile


def main():
    visible_fixture = runpy.run_path(str(Path(__file__).with_name("check-ios-t3-showcase.py")))["visible_fixture"]
    cases = [
        ("thread", "Loading messages…", False),
        ("thread", "Terminal state follows you", True),
        ("review", "Changes vs main", False),
        ("review", "RemoteHandoffCard.tsx", False),
        ("review", "Changes vs main\nRemoteHandoffCard.tsx", True),
    ]
    draw = r"""
import AppKit
let directory = URL(fileURLWithPath: CommandLine.arguments[1])
for (index, text) in CommandLine.arguments.dropFirst(2).enumerated() {
    let bounds = NSRect(x: 0, y: 0, width: 720, height: 180)
    let image = NSImage(size: bounds.size, flipped: false) { _ in
        NSColor.white.setFill()
        bounds.fill()
        (text as NSString).draw(in: bounds.insetBy(dx: 24, dy: 20), withAttributes: [
            .font: NSFont.systemFont(ofSize: 36), .foregroundColor: NSColor.black
        ])
        return true
    }
    guard let tiff = image.tiffRepresentation,
          let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:])
    else { fatalError("Could not draw OCR test image") }
    try png.write(to: directory.appendingPathComponent("\(index).png"))
}
"""
    # The simulator harness uses /tmp on macOS, where the filename-based decoder failed.
    with tempfile.TemporaryDirectory(dir="/tmp") as directory:
        subprocess.run(["swift", "-", directory, *(text for _, text, _ in cases)],
                       input=draw, text=True, check=True)
        for index, (scene, text, expected) in enumerate(cases):
            observed = visible_fixture(Path(directory) / f"{index}.png", scene)
            if observed != expected:
                raise AssertionError(f"{scene} OCR for {text!r}: expected {expected}, got {observed}")
    print(f"Passed {len(cases)} showcase OCR checks")


if __name__ == "__main__":
    main()
