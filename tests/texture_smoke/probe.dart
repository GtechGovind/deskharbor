// Test entry point only. The normal application never imports this file.
import 'dart:async';
import 'dart:io';

import 'package:engine_bridge/engine_bridge.dart';
import 'package:engine_bridge/src/rust/frb_generated.dart';
import 'package:flutter/material.dart';
import 'package:native_surfaces/native_surfaces.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final endpoint = Platform.environment['DESKHARBOR_SMOKE_ENDPOINT'];
  final finish = Platform.environment['DESKHARBOR_SMOKE_DONE'];
  if (endpoint == null ||
      !endpoint.startsWith('127.0.0.1:') ||
      finish == null) {
    stderr.writeln('Texture smoke requires explicit loopback fixture settings');
    exit(1);
  }
  final directory = await Directory.systemTemp.createTemp(
    'deskharbor-texture-',
  );
  final engine = FrbEngine();
  NativeSurface? surface;
  StreamSubscription<dynamic>? subscription;
  int? sessionId;
  final texture = ValueNotifier<int?>(null);
  try {
    await engine.initialize(directory.path);
    final firstFrame = Completer<void>();
    subscription = engine.subscribe().listen((event) {
      if (event.kind == 'frame' && !firstFrame.isCompleted) {
        firstFrame.complete();
      } else if (event.kind == 'error' && !firstFrame.isCompleted) {
        firstFrame.completeError(StateError(event.code));
      }
    });
    final session = await engine.connect(
      endpoint,
      'DeskHarbor synthetic test only',
    );
    sessionId = session.id;
    if (session.displays.length != 1 ||
        session.displays.single.id != 'synthetic:0' ||
        session.displays.single.width != 64 ||
        session.displays.single.height != 48) {
      throw StateError('Expected the explicit synthetic diagnostic host');
    }
    await firstFrame.future.timeout(const Duration(seconds: 15));
    runApp(
      MaterialApp(
        debugShowCheckedModeBanner: false,
        home: Scaffold(
          backgroundColor: Colors.black,
          body: Center(
            child: Container(
              padding: const EdgeInsets.all(8),
              color: Colors.pink,
              child: SizedBox(
                width: 320,
                height: 240,
                child: ValueListenableBuilder<int?>(
                  valueListenable: texture,
                  builder: (_, id, _) => id == null
                      ? const ColoredBox(color: Colors.black)
                      : Texture(
                          textureId: id,
                          filterQuality: FilterQuality.none,
                        ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
    // These are real native registrations, renderer callbacks and disposal
    // fences in the standard Linux runner, not a mocked method channel.
    for (var cycle = 0; cycle < 8; cycle++) {
      surface = await NativeSurface.create(session.id);
      texture.value = surface.textureId;
      for (var paint = 0; paint < 3; paint++) {
        await surface.present();
        await Future<void>.delayed(const Duration(milliseconds: 150));
        await WidgetsBinding.instance.endOfFrame;
      }
      if (cycle < 7) {
        texture.value = null;
        await WidgetsBinding.instance.endOfFrame;
        await surface.dispose();
        surface = null;
      }
    }
    stdout.writeln('DESKHARBOR_TEXTURE_READY');
    await stdout.flush();
    final deadline = DateTime.now().add(const Duration(seconds: 30));
    while (!await File(finish).exists()) {
      if (DateTime.now().isAfter(deadline)) {
        throw TimeoutException('External pixel check did not finish');
      }
      await surface!.present();
      await Future<void>.delayed(const Duration(milliseconds: 150));
    }
    texture.value = null;
    await WidgetsBinding.instance.endOfFrame;
    await surface!.dispose();
    surface = null;
    await engine.disconnect(session.id);
    sessionId = null;
    await subscription.cancel();
    subscription = null;
    RustLib.dispose();
    stdout.writeln('DESKHARBOR_TEXTURE_CLEAN');
    await stdout.flush();
    exit(0);
  } catch (error, stack) {
    stderr.writeln('Texture smoke failed: $error\n$stack');
    await stderr.flush();
    exitCode = 1;
  } finally {
    texture.value = null;
    if (surface != null) {
      try {
        await surface.dispose();
      } catch (_) {
        exitCode = 1;
      }
    }
    if (sessionId != null) {
      try {
        await engine.disconnect(sessionId);
      } catch (_) {
        exitCode = 1;
      }
    }
    await subscription?.cancel();
  }
  exit(1);
}
