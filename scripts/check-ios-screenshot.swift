import CoreGraphics
import Foundation
import ImageIO
import Vision

enum CaptureError: Error { case missingPixels, blank, unexpectedHeading }

let path = CommandLine.arguments[1]
let url = URL(fileURLWithPath: path)
guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
      let image = CGImageSourceCreateImageAtIndex(source, 0, nil),
      let context = CGContext(data: nil, width: 64, height: 64, bitsPerComponent: 8,
                              bytesPerRow: 64, space: CGColorSpaceCreateDeviceGray(),
                              bitmapInfo: CGImageAlphaInfo.none.rawValue)
else { throw CaptureError.missingPixels }
context.draw(image, in: CGRect(x: 0, y: 0, width: 64, height: 64))
guard let buffer = context.data,
      Set(UnsafeBufferPointer(start: buffer.assumingMemoryBound(to: UInt8.self), count: 4096)).count > 8
else { throw CaptureError.blank }

// An optional text witness checks the actual image independently of the UI driver.
if CommandLine.arguments.count == 3 {
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.recognitionLanguages = ["en-US"]
    try VNImageRequestHandler(cgImage: image).perform([request])
    let words = (request.results ?? []).compactMap { $0.topCandidates(1).first?.string }
    let text = words.joined().lowercased().filter { $0.isLetter || $0.isNumber }
    let expected = CommandLine.arguments[2].lowercased().filter { $0.isLetter || $0.isNumber }
    guard !expected.isEmpty && text.contains(expected) else { throw CaptureError.unexpectedHeading }
}
print("\(image.width) \(image.height)")
