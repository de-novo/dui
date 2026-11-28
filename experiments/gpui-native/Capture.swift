import CoreGraphics
import Foundation

@main struct Capture {
  static func main() throws {
    let args = CommandLine.arguments
    let pid = Int(args[1])!
    let width = CGFloat(Int(args[2])!)
    let height = CGFloat(Int(args[3])!)
    for _ in 0..<100 {
      let windows =
        CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as? [[String: Any]] ?? []
      for window in windows {
        guard window[kCGWindowOwnerPID as String] as? Int == pid,
          let bounds = window[kCGWindowBounds as String] as? NSDictionary,
          let rect = CGRect(dictionaryRepresentation: bounds),
          rect.width == width, rect.height == height,
          let id = window[kCGWindowNumber as String] as? Int
        else { continue }
        print(id)
        return
      }
      Thread.sleep(forTimeInterval: 0.1)
    }
    throw CocoaError(.fileNoSuchFile)
  }
}
