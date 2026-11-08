import 'package:flutter/services.dart';

/// Native texture handle. Only IDs/metadata cross the Dart channel.
class NativeSurface {
  NativeSurface._(this.textureId);
  final int textureId;
  static const _channel = MethodChannel('deskharbor/native_surfaces');
  bool _disposed = false;
  Future<void>? _disposal;

  static Future<NativeSurface> create(int sessionId) async {
    if (sessionId < 0 || sessionId > 0xffffffff) {
      throw ArgumentError.value(sessionId, 'sessionId', 'Expected uint32');
    }
    final id = await _channel.invokeMethod<int>('createTexture', {
      'sessionId': sessionId,
    });
    if (id == null || id < 0) {
      throw PlatformException(
        code: 'texture_unavailable',
        message: 'Native texture registration failed',
      );
    }
    return NativeSurface._(id);
  }

  Future<void> present() async {
    if (_disposed) throw StateError('Native surface is disposed');
    await _channel.invokeMethod<void>('present', {'textureId': textureId});
  }

  Future<void> dispose() {
    if (_disposal != null) return _disposal!;
    _disposed = true;
    return _disposal = _channel.invokeMethod<void>('disposeTexture', {
      'textureId': textureId,
    });
  }
}
