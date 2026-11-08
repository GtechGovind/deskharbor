import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;

import 'src/rust/api.dart' as api;
import 'src/rust/streams.dart';
import 'src/rust/frb_generated.dart';
export 'src/rust/api.dart'
    show
        DisplayInfo,
        SessionInfo,
        HostInfo,
        HostSettings,
        SavedDevice,
        TransferInfo,
        InputRequest;
export 'src/rust/streams.dart' show EngineEventInfo;

/// Injectable engine boundary. Production always uses the generated FRB implementation.
abstract interface class EngineGateway {
  Future<void> initialize(String dataDirectory);
  Stream<EngineEventInfo> subscribe();
  Future<api.SessionInfo> connect(String endpoint, String password);
  Future<void> disconnect(int sessionId);
  Future<void> sendInput(int sessionId, api.InputRequest event);
  Future<void> selectMonitor(int sessionId, String displayId);
  Future<String> clipboardRequestText(int sessionId);
  Future<void> clipboardSendText(int sessionId, String text);
  Future<api.TransferInfo> startTransfer(int sessionId, String path);
  Future<void> cancelTransfer(int id);
  Future<api.HostInfo> hostStatus();
  Future<api.HostInfo> configureHost(api.HostSettings settings);
  Future<void> setHostPassword(String password);
  Future<List<api.SavedDevice>> devices();
  Future<void> saveDevice(api.SavedDevice device);
  Future<void> forgetDevice(String address);
  Future<List<api.TransferInfo>> transfers();
}

class FrbEngine implements EngineGateway {
  @override
  Future<void> initialize(String dataDirectory) async {
    // Resolve the packaged module explicitly. FRB's development-directory
    // fallback can otherwise load a different copy than the texture plugin.
    final directory = File(Platform.resolvedExecutable).parent.uri;
    final libraryPath = switch (Platform.operatingSystem) {
      'windows' => directory.resolve('dh_ffi.dll').toFilePath(),
      'linux' => directory.resolve('lib/libdh_ffi.so').toFilePath(),
      'macos' =>
        directory.resolve('../Frameworks/libdh_ffi.dylib').toFilePath(),
      'android' => 'libdh_ffi.so',
      _ => throw UnsupportedError('DeskHarbor is unavailable on this platform'),
    };
    await RustLib.init(externalLibrary: ExternalLibrary.open(libraryPath));
    await api.initialize(dataDir: dataDirectory);
  }

  @override
  Stream<EngineEventInfo> subscribe() => subscribeEvents();
  @override
  Future<api.SessionInfo> connect(String endpoint, String password) =>
      api.connect(endpoint: endpoint, password: password);
  @override
  Future<void> disconnect(int sessionId) =>
      api.disconnect(sessionId: sessionId);
  @override
  Future<void> sendInput(int sessionId, api.InputRequest event) =>
      api.sendInput(sessionId: sessionId, event: event);
  @override
  Future<void> selectMonitor(int sessionId, String displayId) =>
      api.selectMonitor(sessionId: sessionId, displayId: displayId);
  @override
  Future<String> clipboardRequestText(int sessionId) =>
      api.clipboardRequestText(sessionId: sessionId);
  @override
  Future<void> clipboardSendText(int sessionId, String text) =>
      api.clipboardSendText(sessionId: sessionId, text: text);
  @override
  Future<api.TransferInfo> startTransfer(int sessionId, String path) =>
      api.startTransfer(sessionId: sessionId, localPath: path);
  @override
  Future<void> cancelTransfer(int id) => api.cancelTransfer(transferId: id);
  @override
  Future<api.HostInfo> hostStatus() => api.hostStatus();
  @override
  Future<api.HostInfo> configureHost(api.HostSettings settings) =>
      api.configureHost(settings: settings);
  @override
  Future<void> setHostPassword(String password) =>
      api.setHostPassword(password: password);
  @override
  Future<List<api.SavedDevice>> devices() => api.devices();
  @override
  Future<void> saveDevice(api.SavedDevice device) =>
      api.saveDevice(device: device);
  @override
  Future<void> forgetDevice(String address) =>
      api.forgetDevice(address: address);
  @override
  Future<List<api.TransferInfo>> transfers() => api.transfers();
}

Stream<EngineEventInfo> subscribeEvents() =>
    RustLib.instance.api.crateStreamsSubscribe();
