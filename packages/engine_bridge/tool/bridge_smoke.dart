import 'dart:convert';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:engine_bridge/src/rust/api.dart' as api;
import 'package:engine_bridge/src/rust/frb_generated.dart';

/// Exercises the real generated bridge without connecting, sharing or input.
/// Supply an absolute native-library path and a NEW task-owned data directory.
Future<void> main(List<String> arguments) async {
  if (arguments.length != 2) {
    throw ArgumentError(
      'Usage: dart run tool/bridge_smoke.dart <native-library> <new-data-directory>',
    );
  }
  final library = File(arguments[0]).absolute;
  final directory = Directory(arguments[1]).absolute;
  if (!library.existsSync()) throw ArgumentError('Native library is missing');
  if (directory.existsSync()) {
    throw ArgumentError(
      'Data directory must be new; existing user data is never modified',
    );
  }
  await directory.create(recursive: true);
  await RustLib.init(externalLibrary: ExternalLibrary.open(library.path));
  await api.initialize(dataDir: directory.path);
  final host = await api.hostStatus();
  if (host.state.isEmpty || host.detail.isEmpty) {
    throw StateError('Native host status lacks a useful capability result');
  }
  final initial = await api.devices();
  if (initial.isNotEmpty)
    throw StateError('New isolated device store was not empty');
  await api.saveDevice(
    device: const api.SavedDevice(
      name: 'Bridge smoke fixture',
      address: '127.0.0.1:47832',
    ),
  );
  final saved = await api.devices();
  if (saved.length != 1 ||
      saved.single.name != 'Bridge smoke fixture' ||
      saved.single.address != '127.0.0.1:47832' ||
      saved.single.identity != null) {
    throw StateError('Native device round trip failed');
  }
  await api.forgetDevice(address: '127.0.0.1:47832');
  if ((await api.devices()).isNotEmpty) {
    throw StateError('Native device removal failed');
  }
  stdout.writeln(
    jsonEncode({
      'state': host.state,
      'running': host.running,
      'unattended_available': host.unattendedAvailable,
      'device_round_trip': true,
    }),
  );
  RustLib.dispose();
}
