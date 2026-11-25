#if canImport(UIKit)
import SwiftUI
import UIKit

final class LegacyViewController: UIViewController {
    func showHosted() {
        present(UIHostingController(rootView: HostedView()), animated: true)
    }
}
#endif
