import 'package:flutter/services.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:native_surfaces/native_surfaces.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('deskharbor/native_surfaces');
  final calls = <MethodCall>[];
  setUp(() {
    debugDefaultTargetPlatformOverride = TargetPlatform.windows;
    calls.clear();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          calls.add(call);
          return call.method == 'createTexture' ? 42 : null;
        });
  });
  tearDown(() {
    debugDefaultTargetPlatformOverride = null;
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null);
  });
  test('texture lifecycle sends identifiers and disposes once', () async {
    final s = await NativeSurface.create(7);
    expect(s.textureId, 42);
    await s.present();
    await s.dispose();
    await s.dispose();
    expect(calls.map((c) => c.method), [
      'createTexture',
      'present',
      'disposeTexture',
    ]);
    expect(calls.first.arguments, {'sessionId': 7});
    expect(calls[1].arguments, {'textureId': 42});
    await expectLater(s.present(), throwsStateError);
  });
  test('invalid session cannot cross platform boundary', () async {
    await expectLater(NativeSurface.create(-1), throwsArgumentError);
    expect(calls, isEmpty);
  });
  test('unavailable native texture propagates error', () async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          channel,
          (call) async => throw PlatformException(code: 'frame_unavailable'),
        );
    await expectLater(
      NativeSurface.create(3),
      throwsA(isA<PlatformException>()),
    );
  });
  testWidgets('Linux disposal collects only after its raster fence', (
    tester,
  ) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.linux;
    try {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, (call) async {
            calls.add(call);
            if (call.method == 'createTexture') return 42;
            if (call.method == 'disposeTexture') return 17;
            return null;
          });
      final surface = await NativeSurface.create(7);
      await tester.runAsync(surface.dispose);
      expect(calls.map((call) => call.method), [
        'createTexture',
        'disposeTexture',
        'collectDisposedTextures',
      ]);
      expect(calls.last.arguments, {'ticket': 17});
    } finally {
      debugDefaultTargetPlatformOverride = null;
    }
  });

  test('Linux invalid fence metadata never authorizes collection', () async {
    debugDefaultTargetPlatformOverride = TargetPlatform.linux;
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          calls.add(call);
          return call.method == 'createTexture' ? 42 : null;
        });
    final surface = await NativeSurface.create(7);
    await expectLater(surface.dispose(), throwsA(isA<PlatformException>()));
    expect(calls.map((call) => call.method), [
      'createTexture',
      'disposeTexture',
    ]);
  });

  testWidgets('Linux cleanup context failure remains an actionable error', (
    tester,
  ) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.linux;
    try {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, (call) async {
            calls.add(call);
            if (call.method == 'createTexture') return 42;
            if (call.method == 'disposeTexture') return 18;
            throw PlatformException(
              code: 'cleanup_unavailable',
              message: 'Restart application',
            );
          });
      final surface = await NativeSurface.create(7);
      await tester.runAsync(() async {
        await expectLater(
          surface.dispose(),
          throwsA(
            isA<PlatformException>().having(
              (error) => error.code,
              'code',
              'cleanup_unavailable',
            ),
          ),
        );
      });
      expect(calls.last.arguments, {'ticket': 18});
    } finally {
      debugDefaultTargetPlatformOverride = null;
    }
  });
}
