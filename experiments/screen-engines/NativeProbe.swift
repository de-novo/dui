import SwiftUI
import UIKit

@main struct NativeProbe: App {
    var body: some Scene { WindowGroup { ProbeView() } }
}

struct ProbeView: View {
    @State private var model: SceneText?
    @State private var frames = 0
    let dir = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
    var body: some View {
        Text(verbatim: model?.text ?? "Waiting")
            .onReceive(Timer.publish(every: 1.0 / 60, on: .main, in: .common).autoconnect()) { _ in
                guard let bytes = try? Data(contentsOf: dir.appendingPathComponent("model.json")),
                      let next = try? JSONDecoder().decode(SceneText.self, from: bytes) else { return }
                if model != next {
                    model = next
                    frames = 0
                } else if frames < 2 {
                    frames += 1
                    if frames == 2 {
                        if next.revision == 0 {
                            for width in [390,1440] {
                                if next.screen == "DetailView" {
                                    save(DetailView(), next.screen, "reference", width)
                                } else { save(ProfileView(), next.screen, "reference", width) }
                                save(Text(verbatim: next.text), next.screen, "model", width)
                            }
                        }
                        try? Data(String(next.revision).utf8).write(to: dir.appendingPathComponent("ack"), options: .atomic)
                    }
                }
            }
    }
    @MainActor func save<V: View>(_ view: V, _ screen: String, _ mode: String, _ width: Int) {
        let surface = view.frame(width: CGFloat(width), height: width == 390 ? 844 : 920)
            .foregroundStyle(.black).background(.white).environment(\.colorScheme,.light)
        let renderer = ImageRenderer(content: surface)
        renderer.scale = 1
        if let data = renderer.uiImage?.pngData() {
            try? data.write(to: dir.appendingPathComponent("\(screen)-\(mode)-\(width).png"),options:.atomic)
        }
    }
}
