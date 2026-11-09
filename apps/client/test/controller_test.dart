import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:engine_bridge/engine_bridge.dart';
import 'package:deskharbor/controller.dart';

import 'support/fixture_engine.dart';

void main() {
  late FixtureEngine engine;
  late DeskController controller;
  setUp(() async {
    engine = FixtureEngine();
    controller = DeskController(engine);
    await controller.start('/test-only');
    await controller.connect('192.168.1.9:47832', 'fixture-only');
  });
  tearDown(() async {
    controller.dispose();
    await engine.events.close();
  });

  test(
    'a successful connection retry clears the previous authentication error',
    () async {
      engine.rejectConnect = true;
      await controller.connect('192.168.1.9:47832', 'fixture-only');
      expect(controller.error, contains('Authentication rejected'));
      engine.rejectConnect = false;
      await controller.connect('192.168.1.9:47832', 'fixture-only');
      expect(controller.error, isNull);
    },
  );

  test(
    'key up and focus release wait for an earlier key down to finish',
    () async {
      final gate = Completer<void>();
      engine.inputGate = gate;
      addTearDown(() {
        if (!gate.isCompleted) gate.complete();
      });
      controller.focus(7, true);
      final down = controller.input(7, 'key', usage: 4, pressed: true);
      await Future<void>.delayed(Duration.zero);
      final up = controller.input(7, 'key', usage: 4, pressed: false);
      await Future<void>.delayed(Duration.zero);
      expect(engine.inputs.length, 1);
      controller.focus(7, false);
      gate.complete();
      await Future.wait([down, up]);
      await Future<void>.delayed(Duration.zero);
      expect(engine.inputs.last.kind, 'release_all');
    },
  );

  test(
    'epoch and layout changes discard old frame readiness before refocusing',
    () {
      engine.emit(kind: 'frame', sessionId: 7, sequence: 12);
      expect(controller.frames[7], 12);
      engine.emit(session: engine.session(epoch: 5, layout: 3));
      expect(controller.frames.containsKey(7), isFalse);
    },
  );

  test('keyboard is rejected without focus, then carries current monitor epoch and HID usage', () async {
    await controller.input(7, 'key', usage: 4, pressed: true);
    expect(engine.inputs, isEmpty);
    controller.focus(7, true);
    await controller.input(7, 'key', usage: 4, pressed: true);
    final input = engine.inputs.single;
    expect(input.kind, 'key');
    expect(input.usage, 4);
    expect(input.epoch, 4);
    expect(input.layout, 2);
    expect(input.displayId, 'main');
    expect(input.pressed, isTrue);
    controller.focus(7, false);
    await Future<void>.delayed(Duration.zero);
    expect(engine.inputs.last.kind, 'release_all');
    final count = engine.inputs.length;
    await controller.input(7, 'key', usage: 4, pressed: false);
    expect(engine.inputs.length, count);
  });
  test('epoch transition releases previous input and requires deliberate canvas refocus', () async {
    controller.focus(7, true);
    engine.emit(session: engine.session(epoch: 5));
    await Future<void>.delayed(Duration.zero);
    expect(engine.inputs.single.kind, 'release_all');
    expect(engine.inputs.single.epoch, 4);
    expect(controller.canInput(7), isFalse);
    controller.focus(7, true);
    await controller.input(7, 'key', usage: 5, pressed: true);
    expect(engine.inputs.last.epoch, 5);
  });
  test('reconnecting and stream gaps disable stale input', () async {
    controller.focus(7, true);
    engine.emit(session: engine.session(state: 'reconnecting'));
    expect(controller.canInput(7), isFalse);
    engine.emit(session: engine.session());
    controller.focus(7, true);
    engine.emit(kind: 'error', code: 'event_gap', message: 'Updates missed');
    expect(controller.canInput(7), isFalse);
    expect(controller.error, 'Updates missed');
  });
  test(
    'monitor selection disables input until a session metadata update',
    () async {
      controller.focus(7, true);
      await controller.selectMonitor(7, 'second');
      expect(engine.inputs.last.kind, 'release_all');
      expect(controller.canInput(7), isFalse);
      engine.emit(session: engine.session(layout: 3));
      expect(controller.canInput(7), isFalse);
    },
  );
  test(
    'native permission failure stays visible and does not claim host started',
    () async {
      engine.rejectHost = true;
      await controller.configure(
        HostSettings(
          enabled: true,
          bindAddress: '0.0.0.0:47832',
          allowInput: true,
          clipboard: false,
          files: false,
        ),
      );
      expect(controller.host!.running, isFalse);
      expect(controller.error, contains('permission denied'));
    },
  );
}
