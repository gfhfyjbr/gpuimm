import XCTest

final class MobileEmbeddingTests: XCTestCase {
    private func waitForLayout(_ app: XCUIApplication, landscape: Bool) {
        let button = app.buttons["native-next"]
        var previous = CGRect.null
        var stableSince = Date()
        let settled = NSPredicate { _, _ in
            let frame = button.frame
            let screen = app.windows.firstMatch.frame
            guard (screen.width > screen.height) == landscape,
                  screen.contains(frame), button.isHittable else { return false }
            if frame != previous {
                previous = frame
                stableSince = Date()
                return false
            }
            return Date().timeIntervalSince(stableSince) > 0.7
        }
        XCTAssertEqual(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: settled, object: app)], timeout: 10), .completed)
    }

    func testNativeControlsDriveRustAndSurviveRotation() {
        continueAfterFailure = false
        XCUIDevice.shared.orientation = .portrait
        let app = XCUIApplication()
        app.launch()
        let next = app.buttons["native-next"]
        XCTAssertTrue(next.waitForExistence(timeout: 30))
        let count = app.staticTexts["native-count"]
        XCTAssertTrue(count.waitForExistence(timeout: 10))
        next.tap()
        XCTAssertTrue(count.wait(for: \.label, toEqual: "Scene 1", timeout: 10))
        let title = app.textFields["scene-title"]
        title.tap()
        title.typeText("From UIKit\n")
        XCTAssertTrue(app.keyboards.firstMatch.wait(for: \.exists, toEqual: false, timeout: 10))
        XCUIDevice.shared.orientation = .landscapeLeft
        waitForLayout(app, landscape: true)
        next.tap()
        XCTAssertTrue(count.wait(for: \.label, toEqual: "Scene 2", timeout: 10))
        XCUIDevice.shared.orientation = .portrait
        waitForLayout(app, landscape: false)
        XCUIDevice.shared.press(.home)
        app.activate()
        waitForLayout(app, landscape: false)
        next.tap()
        XCTAssertTrue(count.wait(for: \.label, toEqual: "Scene 3", timeout: 10))
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
