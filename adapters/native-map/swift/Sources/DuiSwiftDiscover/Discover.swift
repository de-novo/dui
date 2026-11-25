// Find SwiftUI screen declarations. Types and callees come only from the compiler's index
// store; swift-syntax supplies the shape of each call (which argument is the destination).
// Rules: docs/en/roadmap.md#swiftui-screen-discovery.
import Foundation
import IndexStoreDB
import SwiftParser
import SwiftSyntax

public struct Screen: Encodable, Hashable {
    public var name: String
    public var path: String
    public var source: String
    public var line: Int
    public var dynamic: Bool?
    public var condition: String?
}

public struct Report: Encodable {
    public var screens: [Screen] = []
    public var limitations: [String] = []
}

enum USR {
    static let view = "s:7SwiftUI4ViewP"
    static let viewController = "c:objc(cs)UIViewController"
}

/// A SwiftUI API whose argument holds what the user navigates to.
struct Rule: Sendable {
    /// Matches the compiler-resolved callee.
    let callee: @Sendable (Symbol) -> Bool
    /// Argument labels that hold the destination, in the resolved signature.
    let destinations: Set<String>
    /// Each top-level child is its own screen (tab containers).
    var eachChild = false
}

let swiftUI = "s:7SwiftUI"
func constructor(of type: String) -> @Sendable (Symbol) -> Bool {
    { $0.kind == .constructor && $0.usr.hasPrefix(swiftUI + type) }
}
func viewModifier(_ names: [String]) -> @Sendable (Symbol) -> Bool {
    { symbol in
        symbol.usr.hasPrefix(swiftUI) && names.contains { symbol.name.hasPrefix($0 + "(") }
    }
}

let rules: [Rule] = [
    Rule(callee: constructor(of: "11WindowGroupV"), destinations: ["content", "makeContent"]),
    Rule(callee: constructor(of: "13DocumentGroupV"), destinations: ["editor", "viewer"]),
    Rule(callee: constructor(of: "14NavigationLinkV"), destinations: ["destination"]),
    Rule(callee: constructor(of: "7TabViewV"), destinations: ["content"], eachChild: true),
    Rule(callee: constructor(of: "3TabV"), destinations: ["content"]),
    Rule(
        callee: viewModifier(["navigationDestination", "sheet", "fullScreenCover", "popover"]),
        destinations: ["destination", "content"]),
    Rule(callee: constructor(of: "19UIHostingControllerC"), destinations: ["rootView"]),
]

/// SwiftUI wrappers whose content is the screen itself.
let transparent = [
    "15NavigationStackV", "14NavigationViewV", "19NavigationSplitViewV", "5GroupV",
].map { swiftUI + $0 }

public func discover(root: String, storePath: String, libIndexStore: String) throws -> Report {
    let index = try Index(storePath: storePath, libIndexStore: libIndexStore)
    let rootURL = URL(fileURLWithPath: root).resolvingSymlinksInPath()
    var scan = Scan(index: index, root: rootURL.path)
    for file in swiftFiles(rootURL) where index.isIndexed(file) {
        scan.visit(file: file)
    }
    for usr in index.conformers(of: USR.viewController) {
        scan.add(usr, dynamic: false, condition: nil)
    }
    return scan.report()
}

struct Target {
    var usr: String
    var dynamic: Bool
    var condition: String?
}

struct Scan {
    let index: Index
    let root: String
    lazy var views = index.conformers(of: USR.view)
    var found: [String: Screen] = [:]
    var limitations: Set<String> = []

    init(index: Index, root: String) {
        self.index = index
        self.root = root
    }

    mutating func visit(file: String) {
        guard let text = try? String(contentsOfFile: file, encoding: .utf8) else { return }
        let tree = Parser.parse(source: text)
        let site = Site(file: file, converter: SourceLocationConverter(fileName: file, tree: tree), index: index)
        for call in Calls(viewMode: .sourceAccurate).collect(tree) {
            guard let symbol = site.callee(of: call), let rule = rules.first(where: { $0.callee(symbol) })
            else { continue }
            guard let argument = destination(of: call, resolvedName: symbol.name, labels: rule.destinations)
            else { continue }
            let children: [ExprSyntax]
            if rule.eachChild, let closure = argument.as(ClosureExprSyntax.self) {
                children = closure.statements.compactMap { $0.item.as(ExprSyntax.self) }
            } else {
                children = [argument]
            }
            for child in children {
                // A Tab inside a TabView is matched by its own rule.
                if rule.eachChild, let tab = child.as(FunctionCallExprSyntax.self),
                    let callee = site.callee(of: tab), rules.contains(where: { $0.callee(callee) })
                {
                    continue
                }
                for target in targets(of: child, site: site, dynamic: false, condition: nil) {
                    add(target.usr, dynamic: target.dynamic, condition: target.condition)
                }
            }
        }
    }

    /// The type of each view a destination expression can produce.
    mutating func targets(of expr: ExprSyntax, site: Site, dynamic: Bool, condition: String?) -> [Target] {
        if let closure = expr.as(ClosureExprSyntax.self) {
            return statements(closure.statements, site: site, dynamic: dynamic, condition: condition)
        }
        if let branch = expr.as(IfExprSyntax.self) {
            return branches(of: branch, site: site, condition: condition)
        }
        if let choice = expr.as(SwitchExprSyntax.self) {
            let subject = choice.subject.trimmedDescription
            return choice.cases.flatMap { item -> [Target] in
                guard let switchCase = item.as(SwitchCaseSyntax.self) else { return [] }
                let label = switchCase.label.trimmedDescription.trimmingCharacters(in: CharacterSet(charactersIn: ":"))
                return statements(switchCase.statements, site: site, dynamic: true,
                                  condition: join(condition, "\(subject) \(label)"))
            }
        }
        if let call = expr.as(FunctionCallExprSyntax.self) {
            // A modifier chain (`X().navigationTitle(...)`) presents its receiver.
            if let member = call.calledExpression.as(MemberAccessExprSyntax.self), let base = member.base {
                return targets(of: base, site: site, dynamic: dynamic, condition: condition)
            }
            guard let type = site.type(at: call.calledExpression) else {
                limit(site, call, "is produced by `\(call.calledExpression.trimmedDescription)` at runtime")
                return []
            }
            if views.contains(type.usr) {
                return [Target(usr: type.usr, dynamic: dynamic, condition: condition)]
            }
            if transparent.contains(where: type.usr.hasPrefix) {
                let inner = [call.trailingClosure.map(ExprSyntax.init)]
                    + call.arguments.map { Optional($0.expression) }
                    + call.additionalTrailingClosures.map { Optional(ExprSyntax($0.closure)) }
                return inner.compactMap { $0 }.flatMap {
                    targets(of: $0, site: site, dynamic: dynamic, condition: condition)
                }
            }
            if type.usr == swiftUI + "7AnyViewV" {
                limit(site, call, "is type-erased with AnyView")
            } else {
                limit(site, call, "is inline `\(type.name)` content, not a screen type")
            }
            return []
        }
        if let reference = expr.as(DeclReferenceExprSyntax.self), let type = site.type(at: ExprSyntax(reference)),
            views.contains(type.usr)
        {
            return [Target(usr: type.usr, dynamic: dynamic, condition: condition)]
        }
        limit(site, expr, "is chosen at runtime (`\(expr.trimmedDescription.prefix(60))`)")
        return []
    }

    mutating func statements(_ items: CodeBlockItemListSyntax, site: Site, dynamic: Bool, condition: String?) -> [Target] {
        items.flatMap { item -> [Target] in
            if let expr = item.item.as(ExprSyntax.self) {
                return targets(of: expr, site: site, dynamic: dynamic, condition: condition)
            }
            if let statement = item.item.as(ExpressionStmtSyntax.self) {
                return targets(of: statement.expression, site: site, dynamic: dynamic, condition: condition)
            }
            return []
        }
    }

    mutating func branches(of branch: IfExprSyntax, site: Site, condition: String?) -> [Target] {
        let test = branch.conditions.trimmedDescription
        var out = statements(branch.body.statements, site: site, dynamic: true, condition: join(condition, test))
        let negated = join(condition, "!(\(test))")
        switch branch.elseBody {
        case .codeBlock(let block):
            out += statements(block.statements, site: site, dynamic: true, condition: negated)
        case .ifExpr(let next):
            out += branches(of: next, site: site, condition: negated)
        case nil:
            break
        }
        return out
    }

    mutating func limit(_ site: Site, _ node: some SyntaxProtocol, _ reason: String) {
        let line = site.converter.location(for: node.positionAfterSkippingLeadingTrivia).line
        limitations.insert("\(relative(site.file)):\(line): destination \(reason).")
    }

    mutating func add(_ usr: String, dynamic: Bool, condition: String?) {
        guard let definition = index.definition(of: usr) else { return }
        let file = URL(fileURLWithPath: definition.location.path).resolvingSymlinksInPath().path
        let name = index.qualifiedName(of: definition)
        let screen = Screen(
            name: name,
            path: "\(definition.location.moduleName)/\(URL(fileURLWithPath: file).lastPathComponent)#\(name)",
            source: relative(file),
            line: definition.location.line,
            dynamic: dynamic ? true : nil,
            condition: dynamic ? condition : nil)
        // A screen reached by any static route is not dynamic.
        if let existing = found[usr], existing.dynamic == nil || dynamic { return }
        found[usr] = screen
    }

    func relative(_ file: String) -> String {
        file.hasPrefix(root + "/") ? String(file.dropFirst(root.count + 1)) : file
    }

    mutating func report() -> Report {
        Report(
            screens: found.values.sorted { ($0.path, $0.line) < ($1.path, $1.line) },
            limitations: limitations.sorted())
    }
}

func join(_ outer: String?, _ inner: String) -> String {
    outer.map { "\($0) && \(inner)" } ?? inner
}

/// The argument that holds the destination. A trailing closure binds to the destination when it
/// is the last parameter of the resolved signature that the call does not pass by label.
func destination(of call: FunctionCallExprSyntax, resolvedName: String, labels: Set<String>) -> ExprSyntax? {
    let parameters = argumentLabels(resolvedName)
    guard let label = parameters.last(where: labels.contains) else { return nil }
    if let argument = call.arguments.first(where: { $0.label?.text == label }) {
        return argument.expression
    }
    if let closure = call.additionalTrailingClosures.first(where: { $0.label.text == label }) {
        return ExprSyntax(closure.closure)
    }
    guard let trailing = call.trailingClosure else { return nil }
    var remaining = parameters
    var unlabeled = call.arguments.filter { $0.label == nil }.count
    remaining.removeAll { parameter in
        if parameter == "_", unlabeled > 0 {
            unlabeled -= 1
            return true
        }
        return call.arguments.contains { $0.label?.text == parameter }
            || call.additionalTrailingClosures.contains { $0.label.text == parameter }
    }
    return remaining.last == label ? ExprSyntax(trailing) : nil
}

/// `sheet(isPresented:onDismiss:content:)` → ["isPresented", "onDismiss", "content"].
func argumentLabels(_ name: String) -> [String] {
    guard let open = name.firstIndex(of: "("), let close = name.lastIndex(of: ")") else { return [] }
    return name[name.index(after: open)..<close].split(separator: ":", omittingEmptySubsequences: true).map(String.init)
}

final class Calls: SyntaxVisitor {
    var calls: [FunctionCallExprSyntax] = []
    func collect(_ tree: SourceFileSyntax) -> [FunctionCallExprSyntax] {
        walk(tree)
        return calls
    }
    override func visit(_ node: FunctionCallExprSyntax) -> SyntaxVisitorContinueKind {
        calls.append(node)
        return .visitChildren
    }
}

/// Joins syntax positions in one file with the index occurrences at the same line and column.
struct Site {
    let file: String
    let converter: SourceLocationConverter
    let index: Index

    func occurrences(at token: TokenSyntax) -> [SymbolOccurrence] {
        let location = converter.location(for: token.positionAfterSkippingLeadingTrivia)
        return index.occurrences(in: file)[Key(line: location.line, column: location.column)] ?? []
    }

    func nameToken(of callee: ExprSyntax) -> TokenSyntax? {
        if let reference = callee.as(DeclReferenceExprSyntax.self) { return reference.baseName }
        if let member = callee.as(MemberAccessExprSyntax.self) { return member.declName.baseName }
        if let generic = callee.as(GenericSpecializationExprSyntax.self) { return nameToken(of: generic.expression) }
        return nil
    }

    /// The compiler-resolved function or initializer a call invokes.
    func callee(of call: FunctionCallExprSyntax) -> Symbol? {
        guard let token = nameToken(of: call.calledExpression) else { return nil }
        let found = occurrences(at: token).filter { $0.roles.contains(.call) || $0.symbol.kind == .constructor }
        return found.first(where: { $0.symbol.kind == .constructor })?.symbol ?? found.first?.symbol
    }

    /// The nominal type a call constructs or a name refers to.
    func type(at callee: ExprSyntax) -> Symbol? {
        guard let token = nameToken(of: callee) else { return nil }
        return occurrences(at: token).first { [.struct, .class, .enum].contains($0.symbol.kind) }?.symbol
    }
}

struct Key: Hashable {
    let line: Int
    let column: Int
}

final class Index {
    let db: IndexStoreDB
    var byFile: [String: [Key: [SymbolOccurrence]]] = [:]

    init(storePath: String, libIndexStore: String) throws {
        let library = try IndexStoreLibrary(dylibPath: libIndexStore)
        let database = FileManager.default.temporaryDirectory
            .appendingPathComponent("dui-swift-index-\(UUID().uuidString)").path
        db = try IndexStoreDB(
            storePath: storePath, databasePath: database, library: library,
            waitUntilDoneInitializing: true, listenToUnitEvents: false)
        db.pollForUnitChangesAndWait(isInitialScan: true)
    }

    func isIndexed(_ file: String) -> Bool { !occurrences(in: file).isEmpty }

    func occurrences(in file: String) -> [Key: [SymbolOccurrence]] {
        if let cached = byFile[file] { return cached }
        var map: [Key: [SymbolOccurrence]] = [:]
        for occurrence in db.symbolOccurrences(inFilePath: file) {
            map[Key(line: occurrence.location.line, column: occurrence.location.utf8Column), default: []]
                .append(occurrence)
        }
        byFile[file] = map
        return map
    }

    /// Project types that conform to a protocol or inherit from a class, directly, through refining
    /// protocols, superclasses or extensions.
    func conformers(of root: String) -> Set<String> {
        var pending = [root]
        var seen: Set<String> = [root]
        var types: Set<String> = []
        while let usr = pending.popLast() {
            for occurrence in db.occurrences(ofUSR: usr, roles: .reference) {
                for relation in occurrence.relations where relation.roles.contains(.baseOf) {
                    for symbol in extended(relation.symbol) where seen.insert(symbol.usr).inserted {
                        pending.append(symbol.usr)
                        if symbol.kind != .protocol, definition(of: symbol.usr) != nil {
                            types.insert(symbol.usr)
                        }
                    }
                }
            }
        }
        return types
    }

    /// An extension stands for the type it extends.
    func extended(_ symbol: Symbol) -> [Symbol] {
        guard symbol.kind == .extension else { return [symbol] }
        return db.occurrences(relatedToUSR: symbol.usr, roles: .extendedBy).map(\.symbol)
    }

    func definition(of usr: String) -> SymbolOccurrence? {
        db.occurrences(ofUSR: usr, roles: .definition)
            .filter { !$0.location.isSystem }
            .min { ($0.location.path, $0.location.line) < ($1.location.path, $1.location.line) }
    }

    /// `Outer.Inner` for nested types.
    func qualifiedName(of occurrence: SymbolOccurrence) -> String {
        guard let parent = occurrence.relations.first(where: { $0.roles.contains(.childOf) })?.symbol,
            [.struct, .class, .enum, .extension].contains(parent.kind),
            let parentDefinition = definition(of: extended(parent).first?.usr ?? parent.usr)
        else { return occurrence.symbol.name }
        return qualifiedName(of: parentDefinition) + "." + occurrence.symbol.name
    }
}

func swiftFiles(_ root: URL) -> [String] {
    let skip: Set<String> = [".build", "DerivedData", "Pods", "Carthage", ".git", "node_modules"]
    var out: [String] = []
    guard let walker = FileManager.default.enumerator(at: root, includingPropertiesForKeys: nil) else { return [] }
    for case let url as URL in walker {
        if skip.contains(url.lastPathComponent) {
            walker.skipDescendants()
            continue
        }
        if url.pathExtension == "swift" { out.append(url.resolvingSymlinksInPath().path) }
    }
    return out.sorted()
}
