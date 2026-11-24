import 'dart:convert';
import 'dart:io';

import 'package:analyzer/dart/analysis/analysis_context_collection.dart';
import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/dart/ast/visitor.dart';
import 'package:analyzer/dart/element/element.dart';
import 'package:analyzer/error/error.dart';
import 'package:analyzer/file_system/overlay_file_system.dart';
import 'package:analyzer/file_system/physical_file_system.dart';
import 'package:path/path.dart' as p;
import 'package:yaml/yaml.dart';

Future<void> main(List<String> args) async {
  if (args.length != 1) {
    stderr.writeln('Usage: discover.dart <Flutter project root>');
    exitCode = 64;
    return;
  }
  try {
    stdout.writeln(jsonEncode(await discover(args.single)));
  } catch (error) {
    stderr.writeln('Flutter route discovery failed: $error');
    exitCode = 1;
  }
}

Future<Map<String, Object>> discover(String directory) async {
  final root = p.normalize(p.absolute(directory));
  final manifest = loadYaml(
    await File(p.join(root, 'pubspec.yaml')).readAsString(),
  );
  if (manifest is! Map || manifest['name'] is! String) {
    throw FormatException('Expected a package name in pubspec.yaml.');
  }
  final dependencies = manifest['dependencies'];
  final flutter = dependencies is Map ? dependencies['flutter'] : null;
  if (flutter is! Map || flutter['sdk'] != 'flutter') {
    throw FormatException(
      'pubspec.yaml does not declare a Flutter SDK dependency.',
    );
  }
  final lib = Directory(p.join(root, 'lib'));
  final scan = _Scan(root);
  if (!await lib.exists()) {
    scan.limitations.add('No lib directory found.');
    return scan.report();
  }
  final files = await lib
      .list(recursive: true, followLinks: false)
      .where((file) => file is File && file.path.endsWith('.dart'))
      .map((file) => file.path)
      .toList();
  files.sort();
  final resources = OverlayResourceProvider(PhysicalResourceProvider.INSTANCE);
  // A fresh checkout can reference its own libraries by package URI before pub get.
  if (!_hasPackageConfig(root)) {
    resources.setOverlay(
      p.join(root, '.dart_tool', 'package_config.json'),
      content: jsonEncode({
        'configVersion': 2,
        'packages': [
          {'name': manifest['name'], 'rootUri': '../', 'packageUri': 'lib/'},
        ],
      }),
      modificationStamp: 1,
    );
  }
  final contexts = AnalysisContextCollection(
    includedPaths: [lib.path],
    resourceProvider: resources,
  );
  try {
    var excluded = 0;
    for (final path in files) {
      final context = contexts.contexts
          .where((context) => context.contextRoot.isAnalyzed(path))
          .firstOrNull;
      if (context == null) {
        excluded++;
        continue;
      }
      final result = await context.currentSession.getResolvedUnit(path);
      if (result is! ResolvedUnitResult) {
        scan.limitations.add(
          '${p.relative(path, from: root)}: source could not be analyzed.',
        );
        continue;
      }
      final syntaxError = result.diagnostics
          .where((d) => d.diagnosticCode.type == DiagnosticType.SYNTACTIC_ERROR)
          .firstOrNull;
      if (syntaxError != null) {
        final line = result.lineInfo.getLocation(syntaxError.offset).lineNumber;
        scan.limitations.add(
          '${p.relative(path, from: root)}:$line: Invalid Dart syntax.',
        );
        continue;
      }
      scan.units[result.unit] = result;
      result.unit.accept(_Index(scan));
    }
    if (excluded > 0) {
      scan.limitations.add(
        'Dart analysis configuration excludes $excluded source file(s).',
      );
    }
    for (final root in scan.roots) {
      scan.readRoot(root);
    }
    if (scan.roots.isEmpty) {
      scan.limitations.add(
        'No supported Flutter route table was found under lib/.',
      );
    }
    return scan.report();
  } finally {
    await contexts.dispose();
  }
}

bool _hasPackageConfig(String root) {
  var directory = Directory(root);
  while (true) {
    if (File(p.join(directory.path, '.dart_tool', 'package_config.json'))
        .existsSync())
      return true;
    if (directory.parent.path == directory.path) return false;
    directory = directory.parent;
  }
}

typedef _Call = ({String type, Map<String, Expression> args, Expression node});

class _Index extends RecursiveAstVisitor<void> {
  _Index(this.scan);
  final _Scan scan;

  @override
  void visitVariableDeclaration(VariableDeclaration node) {
    final element = node.declaredFragment?.element;
    if (element != null &&
        (node.isFinal || node.isConst) &&
        node.initializer != null) {
      scan.initializers[element] = node.initializer!;
    }
    super.visitVariableDeclaration(node);
  }

  void record(Expression node) {
    final call = scan.call(node);
    if (call == null) return;
    if (call.type == 'GoRouter' || _Scan.appTypes.contains(call.type)) {
      scan.roots.add(call);
    } else if (_Scan.pageRouteTypes.contains(call.type)) {
      scan.limit(node, 'Imperative page route requires runtime navigation.');
    } else if (call.type == 'Navigator' && call.args.containsKey('pages')) {
      scan.limit(node, 'Navigator.pages depends on runtime page state.');
    }
  }

  @override
  void visitMethodInvocation(MethodInvocation node) {
    record(node);
    super.visitMethodInvocation(node);
  }

  @override
  void visitInstanceCreationExpression(InstanceCreationExpression node) {
    record(node);
    super.visitInstanceCreationExpression(node);
  }
}

class _Scan {
  _Scan(this.root);
  final String root;
  final units = <AstNode, ResolvedUnitResult>{};
  final initializers = <VariableElement, Expression>{};
  final roots = <_Call>[];
  final screens = <String, Map<String, Object>>{};
  final limitations = <String>{};

  static const routerTypes = {
    'GoRouter',
    'GoRoute',
    'ShellRoute',
    'StatefulShellRoute',
    'StatefulShellBranch',
  };
  static const appTypes = {'WidgetsApp', 'CupertinoApp', 'MaterialApp'};

  static const pageRouteTypes = {
    'MaterialPageRoute',
    'CupertinoPageRoute',
    'PageRouteBuilder',
  };

  _Call? call(Expression node) {
    final List<String> parts;
    final ArgumentList args;
    final Element? element;
    if (node is InstanceCreationExpression) {
      final constructor = node.constructorName;
      parts = [
        if (constructor.type.importPrefix case final prefix?)
          prefix.name.lexeme,
        constructor.type.name.lexeme,
        if (constructor.name case final name?) name.name,
      ];
      element = node.constructorName.element;
      args = node.argumentList;
    } else if (node is MethodInvocation) {
      final target = node.target;
      if (target != null && target is! Identifier) return null;
      parts = [
        if (target is Identifier) ...target.name.split('.'),
        node.methodName.name,
      ];
      element = node.methodName.element;
      args = node.argumentList;
    } else {
      return null;
    }
    final at = parts.indexWhere(
      (name) =>
          routerTypes.contains(name) ||
          appTypes.contains(name) ||
          pageRouteTypes.contains(name) ||
          name == 'Navigator',
    );
    if (at < 0 || at > 1 || parts.length > at + 2) return null;
    final type = parts[at];
    if (parts.length > at + 1) {
      final namedConstructor = parts.last;
      final supported = switch (type) {
        'GoRouter' => namedConstructor == 'routingConfig',
        'StatefulShellRoute' => namedConstructor == 'indexedStack',
        'MaterialApp' ||
        'CupertinoApp' ||
        'WidgetsApp' => namedConstructor == 'router',
        _ => false,
      };
      if (!supported) return null;
    }
    final isRouter = routerTypes.contains(type);
    final library = element?.library?.uri.toString();
    if (library != null) {
      if (!library.startsWith(
        isRouter ? 'package:go_router/' : 'package:flutter/',
      ))
        return null;
    } else {
      final prefix = at == 0 ? null : parts.first;
      final imports = (node.root as CompilationUnit).directives
          .whereType<ImportDirective>();
      if (!imports.any((import) {
        final uri = import.uri.stringValue;
        final matches = isRouter
            ? uri == 'package:go_router/go_router.dart'
            : {
                'package:flutter/widgets.dart',
                'package:flutter/material.dart',
                'package:flutter/cupertino.dart',
              }.contains(uri);
        return matches &&
            import.prefix?.name == prefix &&
            import.combinators.every(
              (clause) => switch (clause) {
                ShowCombinator() => clause.shownNames.any(
                  (n) => n.name == type,
                ),
                HideCombinator() => !clause.hiddenNames.any(
                  (n) => n.name == type,
                ),
              },
            );
      }))
        return null;
    }
    return (
      type: type,
      node: node,
      args: {
        for (final a in args.arguments.whereType<NamedArgument>())
          a.name.lexeme: a.argumentExpression,
      },
    );
  }

  VariableElement? variable(Expression? expression) {
    final element = switch (expression) {
      Identifier() => expression.element,
      PropertyAccess() => expression.propertyName.element,
      _ => null,
    };
    return switch (element) {
      GetterElement() => element.variable,
      VariableElement() => element,
      _ => null,
    };
  }

  String? stringValue(Expression? expression) =>
      expression?.computeConstantValue()?.value?.toStringValue();

  void readRoot(_Call call) {
    if (call.type == 'GoRouter') {
      if (call.args.containsKey('routingConfig')) {
        limit(call.node, 'Runtime routing configuration cannot be enumerated.');
        return;
      }
      collection(call.args['routes'], '', null, true, call.node, {});
      return;
    }
    final home = call.args['home'];
    if (home != null) addScreen('/', home, null);
    final routes = call.args['routes'];
    if (routes != null) collection(routes, '', null, false, call.node, {});
    if (call.args.containsKey('onGenerateRoute') ||
        call.args.containsKey('onUnknownRoute')) {
      limit(call.node, 'Runtime route callbacks cannot be enumerated.');
    }
  }

  void limit(AstNode node, String message) {
    final unit = units[node.root]!;
    limitations.add(
      '${p.relative(unit.path, from: root)}:${unit.lineInfo.getLocation(node.offset).lineNumber}: $message',
    );
  }

  void addScreen(
    String path,
    AstNode node,
    String? condition, {
    String? name,
    bool dynamicPath = false,
  }) {
    final unit = units[node.root]!;
    final screen = <String, Object>{
      'name': name ?? path,
      'path': path,
      'source': p.relative(unit.path, from: root).split(p.separator).join('/'),
      'line': unit.lineInfo.getLocation(node.offset).lineNumber,
      if (condition != null) 'condition': condition,
      if (dynamicPath) 'dynamic': true,
    };
    screens[jsonEncode(screen)] = screen;
  }

  void collection(
    Expression? value,
    String parent,
    String? condition,
    bool go,
    AstNode anchor,
    Set<AstNode> active,
  ) {
    if (value == null || !active.add(value)) {
      limit(anchor, 'Missing or cyclic route collection.');
      return;
    }
    final elements = switch (value) {
      ListLiteral() when go => value.elements,
      SetOrMapLiteral() when !go => value.elements,
      _ => null,
    };
    if (elements != null) {
      for (final entry in elements) {
        entryAt(entry, parent, condition, go, active);
      }
    } else {
      final init = initializers[variable(value)];
      if (init != null) {
        collection(init, parent, condition, go, value, active);
      } else {
        limit(value, 'Route collection is runtime-computed or unsupported.');
      }
    }
    active.remove(value);
  }

  void entryAt(
    CollectionElement entry,
    String parent,
    String? condition,
    bool go,
    Set<AstNode> active,
  ) {
    if (entry is SpreadElement) {
      collection(entry.expression, parent, condition, go, entry, active);
      return;
    }
    if (entry is IfElement) {
      final predicate = [
        entry.expression.toSource(),
        if (entry.caseClause case final clause?) clause.toSource(),
      ].join(' ');
      String combine(String branch) =>
          condition == null ? branch : '($condition) && ($branch)';
      entryAt(entry.thenElement, parent, combine(predicate), go, active);
      if (entry.elseElement case final alternate?) {
        entryAt(alternate, parent, combine('!($predicate)'), go, active);
      }
      return;
    }
    if (!go && entry is MapLiteralEntry) {
      final path = stringValue(entry.key);
      if (path == null || !path.startsWith('/')) {
        limit(entry, 'Named route does not have a static absolute path.');
      } else {
        addScreen(path, entry.key, condition);
      }
      return;
    }
    final route = entry is Expression ? call(entry) : null;
    if (route == null || !go) {
      limit(entry, 'Route entry is runtime-computed or unsupported.');
      return;
    }
    var path = parent;
    if (route.type == 'GoRoute') {
      final segment = stringValue(route.args['path']);
      if (segment == null ||
          segment.isEmpty ||
          (parent.isEmpty && !segment.startsWith('/'))) {
        limit(entry, 'Route path cannot be statically determined.');
        return;
      }
      path =
          '/${'$parent/$segment'.split('/').where((piece) => piece.isNotEmpty).join('/')}';
      if (route.args.containsKey('builder') ||
          route.args.containsKey('pageBuilder')) {
        addScreen(
          path,
          entry,
          condition,
          name: stringValue(route.args['name']),
          dynamicPath: path.split('/').any((s) => s.startsWith(':')),
        );
      } else if (!route.args.containsKey('redirect')) {
        limit(entry, 'No page builder found for route.');
      }
    } else if (!{
      'ShellRoute',
      'StatefulShellBranch',
      'StatefulShellRoute',
    }.contains(route.type)) {
      limit(entry, 'Unsupported route declaration.');
      return;
    }
    final children = route.args['routes'] ?? route.args['branches'];
    if (children != null)
      collection(children, path, condition, true, entry, active);
  }

  Map<String, Object> report() {
    final ordered = screens.values.toList();
    ordered.sort((a, b) {
      for (final field in ['path', 'source']) {
        final order = (a[field] as String).compareTo(b[field] as String);
        if (order != 0) return order;
      }
      return (a['line'] as int).compareTo(b['line'] as int);
    });
    return {'screens': ordered, 'limitations': limitations.toList()..sort()};
  }
}
