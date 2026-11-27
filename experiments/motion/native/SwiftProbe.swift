import SwiftUI
import AppKit

struct MotionTrack: Decodable {
    struct Frame: Decodable { let x: Double }
    let durationMs: Double
    let frames: [Frame]
}
struct MotionDocument: Decodable { let tracks: [MotionTrack] }
struct Probe: View {
    let track: MotionTrack
    @State private var moving = false
    var body: some View {
        ZStack(alignment: .topLeading) {
            Color.white
            Rectangle().fill(Color(red: 19/255, green: 92/255, blue: 82/255))
                .frame(width: 40, height: 30)
                .offset(x: 20 + (moving ? track.frames[1].x : track.frames[0].x), y: 40)
        }.frame(width: 320, height: 120).onAppear {
            withAnimation(.linear(duration: track.durationMs/1000).repeatForever(autoreverses: false)) {
                moving = true
            }
        }
    }
}
let track = try JSONDecoder().decode(MotionDocument.self, from: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))).tracks[0]
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 320, height: 120), styleMask: [.titled, .closable], backing: .buffered, defer: false)
window.title = "dui research SwiftUI"
window.contentView = NSHostingView(rootView: Probe(track: track))
window.center()
window.makeKeyAndOrderFront(nil)
print(window.windowNumber)
fflush(stdout)
app.activate(ignoringOtherApps: true)
app.run()
