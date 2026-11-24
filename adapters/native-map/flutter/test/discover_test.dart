import 'dart:io';

import 'package:path/path.dart' as p;

import '../bin/discover.dart' as adapter;

void expect(bool pass, String message) {
  if (!pass) throw StateError(message);
}

List<Map<String, Object>> screens(Map<String, Object> result) =>
    result['screens'] as List<Map<String, Object>>;

Future<void> main(List<String> args) async {
  final root = await Directory.systemTemp.createTemp('dui-flutter-routes-');
  try {
    await File(p.join(root.path, 'pubspec.yaml')).writeAsString('''
name: travel_maps
dependencies:
  flutter:
    sdk: flutter
  go_router: any
''');
    await File(p.join(root.path, 'analysis_options.yaml'))
        .writeAsString('analyzer:\n  exclude:\n    - lib/omitted.dart\n');
    final lib = await Directory(p.join(root.path, 'lib')).create();
    await File(p.join(lib.path, 'omitted.dart')).writeAsString(
      "import 'package:flutter/material.dart'; final ignored = MaterialApp(home: page());",
    );
    await File(p.join(lib.path, 'routes.dart')).writeAsString('''
import 'package:go_router/go_router.dart' as nav;
class Names { static const detail = 'item/:id'; }
final children = [
  nav.GoRoute(path: Names.detail, name: 'item', builder: (_, __) => page()),
];
''');
    await File(p.join(lib.path, 'main.dart')).writeAsString('''
import 'package:go_router/go_router.dart' as nav;
import 'package:flutter/material.dart' show MaterialApp, MaterialPageRoute, Navigator;
import 'package:travel_maps/routes.dart';
const places = '/places';
final router = nav.GoRouter(routes: [
  nav.ShellRoute(routes: [
    nav.GoRoute(path: places, builder: (_, __) => page(), routes: [...children]),
    nav.GoRoute(path: '/old', redirect: (_, __) => places),
    nav.GoRoute(path: runtimePath(), builder: (_, __) => page()),
  ]),
  nav.StatefulShellRoute.indexedStack(branches: [
    nav.StatefulShellBranch(routes: [
      nav.GoRoute(path: '/branch', builder: (_, __) => page()),
    ]),
  ]),
]);
final imperial = MaterialPageRoute(builder: (_) => page());
final nestedNavigator = Navigator(pages: runtimePages());
final unused = nav.GoRoute(path: '/orphan', builder: (_, __) => page());
final widget = MaterialApp(home: page(), routes: {
  if (enabled) '/extra': (_) => page() else '/basic': (_) => page(),
}, onGenerateRoute: dynamicRoute);
''');
    await File(p.join(lib.path, 'unrelated.dart')).writeAsString('''
class MaterialApp { MaterialApp({Object? routes}); }
final other = MaterialApp(routes: {'/unrelated': 0});
''');
    final result = await adapter.discover(root.path);
    final list = screens(result);
    expect(
      list.map((s) => s['path']).join(',') ==
          '/,/basic,/branch,/extra,/places,/places/item/:id',
      '$result',
    );
    final item = list.last;
    expect(
      item['dynamic'] == true &&
          item['source'] == 'lib/routes.dart' &&
          item['line'] == 4 &&
          item['name'] == 'item',
      '$item',
    );
    expect(
      list[1]['condition'] == '!(enabled)' && list[3]['condition'] == 'enabled',
      '$list',
    );
    final limits = result['limitations'] as List<String>;
    expect(
      limits.any((l) => l.contains('Runtime route callbacks')) &&
          limits.any((l) => l.contains('excludes 1 source')) &&
          limits.any((l) => l.contains('Imperative page route')) &&
          limits.any((l) => l.contains('Navigator.pages')) &&
          limits.any(
            (l) => l.contains('Route path cannot be statically determined'),
          ),
      '$limits',
    );
    await File(p.join(root.path, 'pubspec.yaml'))
        .writeAsString('name: plain_dart');
    try {
      await adapter.discover(root.path);
      throw StateError('Non-Flutter Dart package passed.');
    } on FormatException catch (error) {
      expect(error.message.contains('Flutter SDK dependency'), error.message);
    }
    print(
      'PASS Flutter app route maps, conditional entries, nested shells, cross-file source evidence and unresolved routes.',
    );
  } finally {
    await root.delete(recursive: true);
  }
  if (args.isNotEmpty) {
    // flutter/samples @ a5bccd247163b1017d2eb9320cf3b94a95ccfb5e, compass_app/app.
    final real = await adapter.discover(args.single);
    final list = screens(real);
    const expected =
        '/,/activities,/booking,/booking/:id,/login,/results,/search';
    expect(list.map((s) => s['path']).join(',') == expected, '$real');
    final source = await File(
      p.join(args.single, 'lib', 'routing', 'router.dart'),
    ).readAsLines();
    for (final route in list) {
      expect(
        route['source'] == 'lib/routing/router.dart' &&
            source[(route['line'] as int) - 1].contains('GoRoute('),
        '$route',
      );
    }
    expect(
      list.singleWhere((s) => s['path'] == '/booking/:id')['dynamic'] == true,
      '$real',
    );
    print(
      'PASS Flutter Compass OSS app: ${list.length} routes with source evidence.',
    );
  }
}
