import SwiftUI
import UIKit

// Run inside the iOS Simulator runtime. Renders into a UIImage even if
// SpringBoard cannot present an application window.
@main struct SurfaceProbe {
    @MainActor static func main() throws {
        let dir = URL(fileURLWithPath: CommandLine.arguments[1])
        let input = dir.appendingPathComponent("model.json")
        var previous = Data()
        let deadline = Date().addingTimeInterval(180)
        while Date() < deadline {
            if let bytes = try? Data(contentsOf: input), bytes != previous {
                let model = try JSONDecoder().decode(SceneText.self, from: bytes)
                for width in [390,1440] {
                    if model.revision == 0 {
                        if model.screen == "DetailView" {
                            try save(DetailView(),dir,model.screen,"reference",width)
                        } else { try save(ProfileView(),dir,model.screen,"reference",width) }
                    }
                    try save(Text(verbatim:model.text),dir,model.screen,"model",width)
                }
                previous = bytes
                try Data(String(model.revision).utf8).write(to:dir.appendingPathComponent("ack"),options:.atomic)
                if model.screen == "ProfileView" && model.revision == 8 {return}
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.005))
        }
    }
    @MainActor static func save<V: View>(_ view:V,_ dir:URL,_ screen:String,_ mode:String,_ width:Int) throws {
        let surface=view.frame(width:CGFloat(width),height:width==390 ? 844 : 920).foregroundStyle(.black).background(.white).environment(\.colorScheme,.light)
        let renderer=ImageRenderer(content:surface);renderer.scale=1
        guard let data=renderer.uiImage?.pngData() else {throw CocoaError(.fileWriteUnknown)}
        try data.write(to:dir.appendingPathComponent("\(screen)-\(mode)-\(width).png"),options:.atomic)
    }
}
