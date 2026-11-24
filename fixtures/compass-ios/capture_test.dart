import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:compass_app/data/repositories/itinerary_config/itinerary_config_repository.dart';
import 'package:compass_app/domain/models/itinerary_config/itinerary_config.dart';
import 'package:compass_app/main_development.dart' as development;
import 'package:compass_app/main_staging.dart' as staging;
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:integration_test/integration_test.dart';
import 'package:provider/provider.dart';

void main() {
  final binding = IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  final output = Directory(Platform.environment['COMPASS_CAPTURE_OUTPUT']!)
    ..createSync(recursive: true);
  final screens = <Map<String, Object>>[];
  unawaited(
    binding.allTestsPassed.future.then((passed) {
      final result = File('${output.path}/result.tmp');
      result.writeAsStringSync(
        jsonEncode({
          'passed': passed,
          'screens': screens,
          'failures': binding.failureMethodsDetails
              .map((f) => f.details)
              .toList(),
        }),
        flush: true,
      );
      result.renameSync('${output.path}/result.json');
    }),
  );
  testWidgets('Compass renders its seven screens on iOS', (tester) async {
    expect(Platform.isIOS, isTrue);
    Future<BuildContext> capture(
      String slug,
      String path,
      String witness, {
      String? declaration,
    }) async {
      final label = find.text(witness);
      final timer = Stopwatch()..start();
      while (label.evaluate().isEmpty &&
          timer.elapsed < const Duration(seconds: 30)) {
        await tester.pump(const Duration(milliseconds: 150));
        await Future<void>.delayed(const Duration(milliseconds: 50));
      }
      expect(
        label,
        findsWidgets,
        reason: 'Expected visible Compass text on $path',
      );
      await tester.pump(const Duration(milliseconds: 700));
      final target = label.first;
      final context = tester.element(target);
      final actual = GoRouterState.of(context).uri.path;
      expect(actual, path);
      final view = View.of(context);
      final size = view.physicalSize / view.devicePixelRatio;
      expect((Offset.zero & size).overlaps(tester.getRect(target)), isTrue);
      final bytes = await binding.takeScreenshot(slug);
      expect(bytes.length, greaterThan(1000));
      await File('${output.path}/$slug.png').writeAsBytes(bytes, flush: true);
      screens.add({
        'name': slug,
        'route': declaration ?? path,
        'observedPath': actual,
        'witness': witness,
        'width': size.width.round(),
        'height': size.height.round(),
        'devicePixelRatio': view.devicePixelRatio,
      });
      return context;
    }

    // Start with an empty Keychain/preferences store on the dedicated simulator.
    staging.main();
    await capture('login', '/login', 'Login');
    await tester.pumpWidget(const SizedBox.shrink());
    development.main();
    await capture('home', '/', "Sofie's Trips");
    await tester.tap(find.text('Alaska, North America'));
    final booking = await capture(
      'existing-booking',
      '/booking/0',
      'Alaska',
      declaration: '/booking/:id',
    );
    GoRouter.of(booking).go('/search');
    final search = await capture('search', '/search', 'Europe');
    // Supply the same journey through the sample's local in-memory repository.
    // The screens query their original view models and JSON asset repositories.
    final data = search.read<ItineraryConfigRepository>();
    await data.setItineraryConfig(
      ItineraryConfig(
        continent: 'Europe',
        guests: 1,
        startDate: DateTime(2027, 1, 12),
        endDate: DateTime(2027, 1, 15),
        destination: 'amalfi-coast',
        activities: const ['scenic-boat-tour'],
      ),
    );
    final router = GoRouter.of(search);
    router.go('/results');
    await capture('results', '/results', 'AMALFI COAST');
    router.go('/activities');
    await capture('activities', '/activities', 'Scenic Boat Tour');
    router.go('/booking');
    await capture('new-booking', '/booking', 'Amalfi Coast');
  });
}
