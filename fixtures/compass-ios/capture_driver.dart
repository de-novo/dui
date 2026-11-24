import 'dart:convert';
import 'dart:io';

import 'package:integration_test/integration_test_driver_extended.dart';

Future<void> main() async {
  final output = Directory(Platform.environment['COMPASS_CAPTURE_OUTPUT']!);
  await integrationDriver(
    onScreenshot: (name, bytes, [args]) async {
      if (!RegExp(r'^[a-z-]+$').hasMatch(name))
        throw FormatException('Invalid screenshot name');
      await File('${output.path}/$name.png').writeAsBytes(bytes);
      return true;
    },
    responseDataCallback: (data) async {
      final screens = data?['compassScreens'];
      if (screens is! List || screens.length != 7)
        throw StateError('Expected seven iOS screens');
      await File('${output.path}/screens.json')
          .writeAsString(jsonEncode(screens));
    },
  );
}
