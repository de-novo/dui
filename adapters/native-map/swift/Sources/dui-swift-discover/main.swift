import DuiSwiftDiscover
import Foundation

let args = CommandLine.arguments
guard args.count == 4 else {
    FileHandle.standardError.write(Data("Usage: dui-swift-discover <project root> <index store> <libIndexStore.dylib>\n".utf8))
    exit(64)
}
do {
    let report = try discover(root: args[1], storePath: args[2], libIndexStore: args[3])
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.sortedKeys, .prettyPrinted, .withoutEscapingSlashes]
    FileHandle.standardOutput.write(try encoder.encode(report))
    FileHandle.standardOutput.write(Data("\n".utf8))
} catch {
    FileHandle.standardError.write(Data("SwiftUI screen discovery failed: \(error)\n".utf8))
    exit(1)
}
