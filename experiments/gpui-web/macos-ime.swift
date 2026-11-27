// swiftc macos-ime.swift -o /tmp/dui-ime && /tmp/dui-ime http://127.0.0.1:18763/
import AppKit
import Carbon
import WebKit

@MainActor
func test() async throws {
  let app = NSApplication.shared
  let web = WKWebView(frame: NSRect(x: 0, y: 0, width: 900, height: 600))
  let window = NSWindow(
    contentRect: web.frame, styleMask: [.titled], backing: .buffered, defer: false)
  window.contentView = web
  window.makeKeyAndOrderFront(nil)
  app.activate(ignoringOtherApps: true)
  window.makeFirstResponder(web)
  guard CommandLine.arguments.count == 2, let url = URL(string: CommandLine.arguments[1]),
    url.scheme == "http", url.host == "127.0.0.1"
  else {
    throw NSError(domain: "Expected a loopback probe URL", code: 1)
  }
  web.load(URLRequest(url: url))
  for _ in 0..<100 {
    if (try? await web.evaluateJavaScript(
      "!!document.querySelector('canvas') && !!document.querySelector('textarea')")) as? Bool
      == true
    {
      break
    }
    try await Task.sleep(for: .milliseconds(150))
  }
  guard
    (try? await web.evaluateJavaScript(
      "!!document.querySelector('canvas') && !!document.querySelector('textarea')")) as? Bool
      == true
  else {
    throw NSError(domain: "GPUI failed to initialize", code: 5)
  }
  // GPUI hit testing needs a window click to focus its own input state.
  for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
    let event = NSEvent.mouseEvent(
      with: type, location: NSPoint(x: 90, y: 550),
      modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
      windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: 1)!
    window.sendEvent(event)
  }
  try await Task.sleep(for: .milliseconds(200))
  let original = TISCopyCurrentKeyboardInputSource().takeRetainedValue()
  let filter = [kTISPropertyInputSourceID as String: "com.apple.inputmethod.Korean.2SetKorean"]
  let list = TISCreateInputSourceList(filter as CFDictionary, false).takeRetainedValue() as NSArray
  guard list.count == 1 else {
    throw NSError(domain: "Two-set Korean input source unavailable", code: 2)
  }
  let korean = list[0] as! TISInputSource
  guard TISSelectInputSource(korean) == noErr else {
    throw NSError(domain: "Input source selection failed", code: 3)
  }
  defer { TISSelectInputSource(original) }
  try await Task.sleep(for: .milliseconds(350))
  let selected = TISCopyCurrentKeyboardInputSource().takeRetainedValue()
  let sourceID = Unmanaged<CFString>.fromOpaque(
    TISGetInputSourceProperty(selected, kTISPropertyInputSourceID)
  ).takeUnretainedValue()
  print("Input source", sourceID, "key window", window.isKeyWindow)
  func send(_ codes: [Int]) async throws {
    for code in codes {
      for type in [NSEvent.EventType.keyDown, .keyUp] {
        let cg = CGEvent(
          keyboardEventSource: nil, virtualKey: CGKeyCode(code), keyDown: type == .keyDown)!
        let event = NSEvent(cgEvent: cg)!
        window.sendEvent(event)
      }
      try await Task.sleep(for: .milliseconds(120))
    }
  }
  try await send([5, 40, 1, 15, 46, 3, 49])
  let committed = try await web.evaluateJavaScript(
    "window.probe.values.at(-1) === '한글 ' && document.activeElement.value === '한글 '")
  try await send([51, 51])
  let result = try await web.evaluateJavaScript(
    "JSON.stringify({probe:window.probe,focus:document.activeElement.tagName,mirror:document.activeElement.value})"
  )
  print(result as? String ?? "missing observation")
  let passed = try await web.evaluateJavaScript(
    "window.probe.values.at(-1)==='한' && document.activeElement.value==='한'")
  guard committed as? Bool == true, passed as? Bool == true else {
    throw NSError(domain: "GPUI composition unverified", code: 4)
  }
}
let app = NSApplication.shared
app.setActivationPolicy(.regular)
Task { @MainActor in
  do {
    try await test()
    exit(0)
  } catch {
    print(error)
    exit(1)
  }
}
app.run()
