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

/// Prints the logical screen size once the component is on screen, for the capture script.
private func reportScreen() {
    DispatchQueue.main.async {
        guard let screen = UIApplication.shared.connectedScenes
            .compactMap({ ($0 as? UIWindowScene)?.screen }).first
        else { return }
        print(
            "DUI_SCREEN {\"width\":\(screen.bounds.width),\"height\":\(screen.bounds.height),\"scale\":\(screen.scale)}"
        )
        fflush(stdout)
    }
}
