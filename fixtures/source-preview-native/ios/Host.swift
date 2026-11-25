// Hosts fixtures/source-preview/Settings.swift unchanged in a plain SwiftUI app so the
// simulator renders it with Apple's SwiftUI. Only the window and the size report are added.
import SwiftUI
import UIKit

@main
struct DuiSourcePreviewHost: App {
    var body: some Scene {
        WindowGroup {
            Settings().onAppear(perform: reportScreen)
        }
    }
}

/// Writes the logical screen size to Documents/dui-screen.json once the component is on screen.
private func reportScreen() {
    DispatchQueue.main.async {
        guard let screen = UIApplication.shared.connectedScenes
            .compactMap({ ($0 as? UIWindowScene)?.screen }).first,
            let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first
        else { return }
        let report = ["width": screen.bounds.width, "height": screen.bounds.height, "scale": screen.scale]
        if let data = try? JSONSerialization.data(withJSONObject: report) {
            try? data.write(to: documents.appendingPathComponent("dui-screen.json"))
        }
    }
}
