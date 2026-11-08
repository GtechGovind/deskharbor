import 'dart:ui' as ui;

import 'package:flutter/foundation.dart';
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
    return _disposal = _disposeNative();
  }

  Future<void> _disposeNative() async {
    if (kIsWeb || defaultTargetPlatform != TargetPlatform.linux) {
      await _channel.invokeMethod<void>('disposeTexture', {
        'textureId': textureId,
      });
      return;
    }
    final ticket = await _channel.invokeMethod<int>('disposeTexture', {
      'textureId': textureId,
    });
    if (ticket == null || ticket <= 0) {
      throw PlatformException(
        code: 'cleanup_unavailable',
        message: 'Native disposal fence ticket is unavailable',
      );
    }
    // Pinned standard Linux runners have a dedicated raster task runner.
    // Picture.toImage posts behind native UnregisterExternalTexture there.
    // This offscreen blank picture synchronizes lifecycle only; it never reads
    // the application canvas, desktop, or remote video pixels.
    final recorder = ui.PictureRecorder();
    ui.Canvas(recorder).drawColor(const ui.Color(0), ui.BlendMode.src);
    final picture = recorder.endRecording();
    try {
      final fenceImage = await picture.toImage(1, 1);
      fenceImage.dispose();
    } finally {
      picture.dispose();
    }
    await _channel.invokeMethod<void>('collectDisposedTextures', {
      'ticket': ticket,
    });
  }
}
