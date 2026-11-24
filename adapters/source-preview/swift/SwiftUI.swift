// Portable preview primitives. The target's component supplies the content and bindings.
import Foundation

private final class BooleanSlot {
    var value: Bool
    init(_ value: Bool) { self.value = value }
}

@propertyWrapper public struct State {
    private let slot: BooleanSlot
    public init(wrappedValue: Bool) {
        slot = BooleanSlot(wrappedValue)
        Preview.slots.append(slot)
    }
    public var wrappedValue: Bool {
        get { slot.value }
        nonmutating set { slot.value = newValue }
    }
    public var projectedValue: Binding<Bool> {
        Binding(get: { slot.value }, set: { slot.value = $0 })
    }
}

public struct Binding<Value> {
    let get: () -> Value
    let set: (Value) -> Void
}

public protocol View {
    associatedtype Body: View
    @ViewBuilder var body: Body { get }
    func draw()
}

public extension View {
    func draw() { body.draw() }
}

public struct Element: View {
    let emit: () -> Void
    public var body: Element { self }
    public func draw() { emit() }
}

@resultBuilder public enum ViewBuilder {
    public static func buildBlock(_ items: any View...) -> Element {
        Element { for item in items { item.draw() } }
    }
}

private func stack(_ kind: String, _ content: @escaping () -> Element) -> Element {
    Element {
        Preview.lines.append(kind)
        content().draw()
        Preview.lines.append("end")
    }
}

public func VStack(@ViewBuilder content: @escaping () -> Element) -> Element { stack("column", content) }
public func HStack(@ViewBuilder content: @escaping () -> Element) -> Element { stack("row", content) }

public func Text(_ value: String) -> Element {
    Element { Preview.lines.append("text\t" + Data(value.utf8).base64EncodedString()) }
}

public func Toggle(_ label: String, isOn: Binding<Bool>) -> Element {
    Element {
        Preview.lines.append("switch\t" + (isOn.get() ? "1" : "0") + "\t" + Data(label.utf8).base64EncodedString())
        Preview.actions.append { isOn.set(!isOn.get()) }
    }
}

public enum Preview {
    fileprivate static var slots: [BooleanSlot] = []
    fileprivate static var actions: [() -> Void] = []
    fileprivate static var lines: [String] = []

    public static func run<V: View>(_ root: V) {
        var expectedSlots: Int?
        func draw() {
            actions.removeAll()
            lines.removeAll()
            root.draw()
            if let expectedSlots { precondition(slots.count == expectedSlots, "Dynamic state allocation is unsupported") }
            expectedSlots = slots.count
        }
        draw()
        for input in CommandLine.arguments.dropFirst() {
            guard let i = Int(input), actions.indices.contains(i) else { fatalError("Unknown control") }
            actions[i]()
            draw()
        }
        print((["state\t" + slots.map { $0.value ? "1" : "0" }.joined()] + lines).joined(separator: "\n"))
    }
}
