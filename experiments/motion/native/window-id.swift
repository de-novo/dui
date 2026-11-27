import CoreGraphics
import Foundation
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
guard let item = windows.first(where: { ($0[kCGWindowName as String] as? String) == CommandLine.arguments[1] }), let id = item[kCGWindowNumber as String] as? Int else { exit(1) }
print(id)
