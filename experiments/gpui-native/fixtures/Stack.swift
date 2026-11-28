import SwiftUI
import UIKit

struct StackExample: View {
  var body: some View {
    VStack(alignment: .leading, spacing: 8) {
      Text("Settings").font(.system(size: 20))
      HStack(spacing: 12) {
        Text("Alerts")
        Text("Enabled")
      }
      Text("Notifications appear on this device when a reminder is due.")
        .frame(width: 180, alignment: .leading)
    }
    .font(.system(size: 16))
    .frame(width: 204, alignment: .leading)
  }
}

@main struct Capture {
  @MainActor static func main() throws {
    for width in [390, 1440] {
      let view = StackExample().frame(width: CGFloat(width), height: width == 390 ? 844 : 920)
        .foregroundStyle(.black).background(.white).environment(\.colorScheme, .light)
      let renderer = ImageRenderer(content: view)
      renderer.scale = 1
      guard let data = renderer.uiImage?.pngData() else { throw CocoaError(.fileWriteUnknown) }
      try data.write(to: URL(fileURLWithPath: CommandLine.arguments[1] + "/ios-stack-\(width).png"))
    }
  }
}
