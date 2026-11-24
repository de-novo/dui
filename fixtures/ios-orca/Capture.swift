import Foundation
import XCTest

final class OrcaCaptureTests: XCTestCase {
    @MainActor
    func testUnpairedHomeAndAbout() throws {
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "com.stably.orca.mobile")
        app.launchArguments = ["-AppleLanguages", "(en)", "-AppleLocale", "en_US"]
        app.launch()

        try record("orca-home", showing: "Connect your desktop", app: app)
        app.open(try XCTUnwrap(URL(string: "orca://about")))
        try record("orca-about", showing: "Open-source agent IDE", app: app)
    }

    @MainActor
    private func record(_ name: String, showing witness: String, app: XCUIApplication) throws {
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let heading = app.descendants(matching: .any)
            .matching(NSPredicate(format: "label CONTAINS[c] %@", witness)).firstMatch
        let deadline = Date().addingTimeInterval(100)
        repeat {
            let alert = springboard.alerts.firstMatch
            if alert.exists {
                // A fresh device can ask about notifications or opening the app's URL.
                let choices = ["Don’t Allow", "Don't Allow", "Not Now", "Open", "Continue"]
                let button = choices.map { alert.buttons[$0] }.first(where: { $0.exists })
                try XCTUnwrap(button, alert.debugDescription).tap()
            } else if app.state == .runningForeground && heading.exists {
                let shot = XCTAttachment(data: XCUIScreen.main.screenshot().pngRepresentation,
                                         uniformTypeIdentifier: "public.png")
                shot.name = name
                shot.lifetime = .keepAlways
                add(shot)
                return
            }
            RunLoop.current.run(until: Date().addingTimeInterval(1))
        } while Date() < deadline
        XCTFail("Missing \(witness). Application tree: \(app.debugDescription); system: \(springboard.debugDescription)")
    }
}
