import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:deskharbor/controller.dart';
import 'package:deskharbor/main.dart';

import 'support/fixture_engine.dart';

void main() {
  late FixtureEngine engine;
  late DeskController controller;
  setUp(() async {
    engine = FixtureEngine();
    controller = DeskController(engine);
    await controller.start('/test-only');
  });
  tearDown(() async {
    controller.dispose();
    await engine.events.close();
  });
  testWidgets('connect form gives a private address and password entry', (
    tester,
  ) async {
    await tester.pumpWidget(MyApp(controller: controller));
    expect(find.text('Connect to a computer'), findsOneWidget);
    expect(find.text('Address'), findsOneWidget);
    expect(find.text('Host password'), findsOneWidget);
  });
  testWidgets(
    'authentication failure clears secret field and shows useful error',
    (tester) async {
      engine.rejectConnect = true;
      await tester.pumpWidget(MyApp(controller: controller));
      await tester.enterText(find.byType(TextField).first, '192.168.1.9:47832');
      await tester.enterText(
        find.byKey(const Key('connect-password')),
        'test-only-secret',
      );
      await tester.tap(find.widgetWithText(FilledButton, 'Connect'));
      await tester.pumpAndSettle();
      expect(
        tester
            .widget<TextField>(find.byKey(const Key('connect-password')))
            .controller!
            .text,
        isEmpty,
      );
      expect(find.textContaining('Authentication rejected'), findsOneWidget);
      expect(find.text('test-only-secret'), findsNothing);
    },
  );
  testWidgets(
    'host password clears even when native credential store rejects request',
    (tester) async {
      engine.rejectHost = true;
      await tester.pumpWidget(MyApp(controller: controller));
      await tester.tap(find.text('This Computer').first);
      await tester.pumpAndSettle();
      await tester.enterText(
        find.byKey(const Key('host-password')),
        'test-only-secret',
      );
      await tester.tap(find.widgetWithText(FilledButton, 'Set password'));
      await tester.pumpAndSettle();
      expect(
        tester
            .widget<TextField>(find.byKey(const Key('host-password')))
            .controller!
            .text,
        isEmpty,
      );
      expect(
        find.textContaining('credential store unavailable'),
        findsOneWidget,
      );
      expect(
        find.textContaining('Unattended hosting is unavailable'),
        findsOneWidget,
      );
    },
  );
  testWidgets(
    'navigation exposes saved devices, transfers, settings and readiness',
    (tester) async {
      await tester.pumpWidget(MyApp(controller: controller));
      await tester.tap(find.text('Devices').first);
      await tester.pumpAndSettle();
      expect(find.text('No saved devices'), findsOneWidget);
      await tester.tap(find.text('Transfers').first);
      await tester.pumpAndSettle();
      expect(find.text('No transfers'), findsOneWidget);
      await tester.tap(find.text('Settings').first);
      await tester.pumpAndSettle();
      await tester.tap(find.text('Permissions and native capabilities'));
      await tester.pumpAndSettle();
      expect(find.text('Screen capture permission denied'), findsOneWidget);
    },
  );
  testWidgets(
    'native texture failure disables remote input with an explicit reason',
    (tester) async {
      tester.view.reset();
      tester.view.physicalSize = const Size(1400, 1100);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(
            const MethodChannel('deskharbor/native_surfaces'),
            (call) async {
              throw PlatformException(
                code: 'library_unavailable',
                message: 'Rust library not loaded',
              );
            },
          );
      addTearDown(
        () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(
              const MethodChannel('deskharbor/native_surfaces'),
              null,
            ),
      );
      await controller.connect('192.168.1.9:47832', 'fixture-only');
      await tester.pumpWidget(MyApp(controller: controller));
      await tester.tap(find.text('192.168.1.9:47832').first);
      await tester.pumpAndSettle();
      expect(find.textContaining('Native video unavailable'), findsOneWidget);
      await tester.tap(find.byKey(const Key('remote-canvas')));
      await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
      await tester.pump();
      expect(engine.inputs.where((event) => event.kind == 'key'), isEmpty);
    },
  );
}
