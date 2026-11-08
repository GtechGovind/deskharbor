import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:native_surfaces/native_surfaces.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('deskharbor/native_surfaces');
  final calls = <MethodCall>[];
  setUp(() {
    calls.clear();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          calls.add(call);
          return call.method == 'createTexture' ? 42 : null;
        });
  });
  tearDown(
    () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null),
  );
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
}
