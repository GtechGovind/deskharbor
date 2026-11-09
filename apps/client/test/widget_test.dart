import 'dart:async';

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

  testWidgets('a device save may finish after its page is closed', (
    tester,
  ) async {
    final gate = Completer<void>();
    engine.saveGate = gate;
    addTearDown(() {
      if (!gate.isCompleted) gate.complete();
    });
    await tester.pumpWidget(MyApp(controller: controller));
    await tester.tap(find.text('Devices').first);
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField).first, 'Office');
    await tester.enterText(find.byType(TextField).last, '192.168.1.9:47832');
    await tester.ensureVisible(find.text('Save device'));
    await tester.tap(find.text('Save device'));
    await tester.tap(find.text('Connect').first);
    await tester.pumpAndSettle();
    gate.complete();
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    expect(find.text('Connect to a computer'), findsOneWidget);
  });

  testWidgets('a clipboard request may finish after its dialog is closed', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1400, 1100);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final gate = Completer<String>();
    engine.clipboardGate = gate;
    addTearDown(() {
      if (!gate.isCompleted) gate.complete('fixture text');
    });
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    const channel = MethodChannel('deskharbor/native_surfaces');
    messenger.setMockMethodCallHandler(
      channel,
      (call) async => call.method == 'createTexture' ? 20 : null,
    );
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    await controller.connect('192.168.1.9:47832', 'fixture-only');
    await tester.pumpWidget(MyApp(controller: controller));
    await tester.tap(find.text('192.168.1.9:47832').first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Clipboard'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Get from host'));
    await tester.tap(find.text('Close'));
    await tester.pumpAndSettle();
    gate.complete('fixture text');
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    expect(find.text('Session clipboard'), findsNothing);
  });
  testWidgets('Android retains bottom navigation on a wide tablet', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1200, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(MyApp(controller: controller));
    expect(find.byType(NavigationBar), findsOneWidget);
  }, variant: TargetPlatformVariant.only(TargetPlatform.android));

  testWidgets('phone session controls remain usable with the keyboard open', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(390, 844);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    const channel = MethodChannel('deskharbor/native_surfaces');
    messenger.setMockMethodCallHandler(
      channel,
      (call) async => call.method == 'createTexture' ? 20 : null,
    );
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    await controller.connect('192.168.1.9:47832', 'fixture-only');
    engine.emit(kind: 'frame', sessionId: 7, sequence: 1);
    await tester.pumpWidget(MyApp(controller: controller));
    await tester.ensureVisible(find.text('192.168.1.9:47832').first);
    await tester.tap(find.text('192.168.1.9:47832').first);
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Keyboard'));
    await tester.tap(find.text('Keyboard'));
    await tester.pumpAndSettle();
    tester.view.viewInsets = const FakeViewPadding(bottom: 300);
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    await tester.ensureVisible(find.text('Send text'));
    expect(find.text('Send text').hitTestable(), findsOneWidget);
  }, variant: TargetPlatformVariant.only(TargetPlatform.android));
  testWidgets(
    'physical keys forward only on the current focused native canvas',
    (tester) async {
      tester.view.physicalSize = const Size(1400, 1100);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      const channel = MethodChannel('deskharbor/native_surfaces');
      messenger.setMockMethodCallHandler(
        channel,
        (call) async => switch (call.method) {
          'createTexture' => 19,
          'disposeTexture' => 1,
          'present' || 'collectDisposedTextures' => null,
          _ => throw StateError('Unexpected native method ${call.method}'),
        },
      );
      addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
      await controller.connect('192.168.1.9:47832', 'fixture-only');
      engine.emit(kind: 'frame', sessionId: 7, sequence: 1);
      await tester.pumpWidget(MyApp(controller: controller));
      await tester.tap(find.text('192.168.1.9:47832').first);
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(
        LogicalKeyboardKey.keyA,
        physicalKey: PhysicalKeyboardKey.keyA,
      );
      expect(engine.inputs.where((e) => e.kind == 'key'), isEmpty);
      await tester.tap(find.byKey(const Key('remote-canvas')));
      await tester.sendKeyEvent(
        LogicalKeyboardKey.keyA,
        physicalKey: PhysicalKeyboardKey.keyA,
      );
      final keys = engine.inputs.where((e) => e.kind == 'key').toList();
      expect(keys.map((e) => e.usage), [4, 4]);
      expect(keys.map((e) => e.pressed), [true, false]);
      expect(keys.map((e) => e.epoch), [4, 4]);
      await tester.tap(find.byType(TextField).last);
      await tester.sendKeyEvent(
        LogicalKeyboardKey.keyB,
        physicalKey: PhysicalKeyboardKey.keyB,
      );
      expect(engine.inputs.where((e) => e.kind == 'key').length, 2);
      expect(engine.inputs.any((e) => e.kind == 'release_all'), isTrue);
      engine.emit(session: engine.session(epoch: 5, layout: 3));
      await tester.pump();
      expect(find.byType(Texture), findsNothing);
      await tester.tap(find.byKey(const Key('remote-canvas')));
      await tester.sendKeyEvent(
        LogicalKeyboardKey.keyC,
        physicalKey: PhysicalKeyboardKey.keyC,
      );
      expect(engine.inputs.where((e) => e.kind == 'key').length, 2);
      engine.emit(kind: 'frame', sessionId: 7, sequence: 2);
      await tester.pumpAndSettle();
      expect(find.byType(Texture), findsOneWidget);
    },
  );
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
      await tester.ensureVisible(find.byKey(const Key('connect-submit')));
      await tester.tap(find.byKey(const Key('connect-submit')));
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
      await tester.ensureVisible(
        find.widgetWithText(FilledButton, 'Set password'),
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
      await tester.ensureVisible(
        find.text('Permissions and native capabilities'),
      );
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
