// dui capture flow for home-assistant/iOS. scripts/capture-home-assistant-ios.py copies this over
// Tests/UI/HomeAssistantUITests.swift in a disposable checkout (the file is already in the Tests-UI
// target) and runs only `testDuiCaptures`. Each screen is captured once an element with the app's
// own accessibility identifier for that screen exists; no server is needed.
import XCTest

final class HomeAssistantUITests: XCTestCase {
    func testDuiCaptures() {
        continueAfterFailure = false
        let app = XCUIApplication()
        app.launchArguments = ["-AppleLanguages", "(en)", "-AppleLocale", "en_US"]
        app.launch()

        let welcome = app.buttons["onboarding.welcome.continue"]
        XCTAssertTrue(welcome.waitForExistence(timeout: 90), "welcome screen")
        capture("OnboardingWelcomeView")
        welcome.tap()

        let manualEntry = app.buttons["onboarding.servers.manualEntry"]
        XCTAssertTrue(manualEntry.waitForExistence(timeout: 30), "servers list")
        capture("OnboardingServersListView")
        manualEntry.tap()

        let connect = app.buttons["onboarding.manualEntry.connect"]
        XCTAssertTrue(connect.waitForExistence(timeout: 30), "manual entry")
        capture("ManualURLEntryView")
    }

    /// Waits for navigation animations to finish, then attaches the screen as a PNG. The name has
    /// no dots: xcresulttool treats the text after a dot as a file extension.
    private func capture(_ name: String) {
        Thread.sleep(forTimeInterval: 1.5)
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = "dui-capture-\(name)"
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
