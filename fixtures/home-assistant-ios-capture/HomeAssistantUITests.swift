// dui capture flow for home-assistant/iOS. scripts/capture-home-assistant-ios.py copies this over
// Tests/UI/HomeAssistantUITests.swift in a disposable checkout (the file is already in the Tests-UI
// target) and runs only `testDuiCaptures`. Each screen is captured once an element with the app's
// own accessibility identifier for that screen exists; no server is needed.
import XCTest

final class HomeAssistantUITests: XCTestCase {
    func testDuiCaptures() {
        continueAfterFailure = false
        let app = XCUIApplication()
        // FASTLANE_SNAPSHOT is HA's own capture mode (its E2E test uses it too). Without it, a Debug
        // build adds a fake discovered server every 1.5 s, so the servers list would depend on timing.
        app.launchArguments = ["-FASTLANE_SNAPSHOT", "YES", "-AppleLanguages", "(en)", "-AppleLocale", "en_US"]
        app.launch()

        let welcome = app.buttons["onboarding.welcome.continue"]
        XCTAssertTrue(welcome.waitForExistence(timeout: 90), "welcome screen")
        capture("OnboardingWelcomeView")
        welcome.tap()

        let manualEntry = app.buttons["onboarding.servers.manualEntry"]
        XCTAssertTrue(manualEntry.waitForExistence(timeout: 30), "servers list")
        // Past the point where a Debug build would have listed two fake servers.
        capture("OnboardingServersListView", settle: 5)
        manualEntry.tap()

        let connect = app.buttons["onboarding.manualEntry.connect"]
        XCTAssertTrue(connect.waitForExistence(timeout: 30), "manual entry")
        capture("ManualURLEntryView")
    }

    /// Waits for navigation animations to finish, then attaches the screen as a PNG. The name has
    /// no dots: xcresulttool treats the text after a dot as a file extension.
    private func capture(_ name: String, settle: TimeInterval = 1.5) {
        Thread.sleep(forTimeInterval: settle)
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = "dui-capture-\(name)"
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
