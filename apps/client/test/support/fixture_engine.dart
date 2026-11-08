import 'dart:async';

import 'package:engine_bridge/engine_bridge.dart';

/// Test-only controlled backend. Never reachable from production startup.
class FixtureEngine implements EngineGateway {
  final events = StreamController<EngineEventInfo>.broadcast(sync: true);
  final inputs = <InputRequest>[];
  final saved = <SavedDevice>[];
  bool rejectConnect = false, rejectHost = false;
  int disconnects = 0;
  HostInfo host = HostInfo(
    state: 'permission_needed',
    running: false,
    passwordConfigured: false,
    unattendedAvailable: false,
    addresses: [],
    detail: 'Screen capture permission denied',
  );
  SessionInfo session({
    String state = 'connected',
    int epoch = 4,
    int layout = 2,
  }) => SessionInfo(
    id: 7,
    endpoint: '192.168.1.9:47832',
    state: state,
    transport: 'quic',
    hostIdentity: 'sha256:fixture',
    epoch: epoch,
    layout: layout,
    displays: [
      DisplayInfo(
        id: 'main',
        name: 'Main display',
        width: 1920,
        height: 1080,
        x: 0,
        y: 0,
      ),
      DisplayInfo(
        id: 'second',
        name: 'Second display',
        width: 1280,
        height: 1024,
        x: 1920,
        y: 0,
      ),
    ],
    selectedDisplay: 'main',
  );
  void emit({
    SessionInfo? session,
    String kind = 'session',
    String code = '',
    String message = '',
    int? sessionId,
    int sequence = 0,
  }) => events.add(
    EngineEventInfo(
      kind: kind,
      session: session,
      host: null,
      transfer: null,
      sessionId: sessionId,
      sequence: sequence,
      width: 1920,
      height: 1080,
      code: code,
      message: message,
    ),
  );
  @override
  Future<void> initialize(String dataDirectory) async {}
  @override
  Stream<EngineEventInfo> subscribe() => events.stream;
  @override
  Future<SessionInfo> connect(String endpoint, String password) async {
    if (rejectConnect) throw StateError('Authentication rejected');
    return session();
  }

  @override
  Future<void> disconnect(int sessionId) async {
    disconnects++;
    emit(session: session(state: 'disconnected'));
  }

  @override
  Future<void> sendInput(int sessionId, InputRequest event) async {
    inputs.add(event);
  }

  @override
  Future<void> selectMonitor(int sessionId, String displayId) async {}
  @override
  Future<String> clipboardRequestText(int sessionId) async => 'fixture text';
  @override
  Future<void> clipboardSendText(int sessionId, String text) async {}
  @override
  Future<TransferInfo> startTransfer(int sessionId, String path) async =>
      TransferInfo(
        id: 1,
        sessionId: sessionId,
        filename: 'fixture.txt',
        total: 100,
        transferred: 20,
        state: 'sending',
        checksum: '',
      );
  @override
  Future<void> cancelTransfer(int id) async {}
  @override
  Future<HostInfo> hostStatus() async => host;
  @override
  Future<HostInfo> configureHost(HostSettings settings) async {
    if (rejectHost) throw StateError('Screen capture permission denied');
    return host;
  }

  @override
  Future<void> setHostPassword(String password) async {
    if (rejectHost) throw StateError('Native credential store unavailable');
  }

  @override
  Future<List<SavedDevice>> devices() async => List.of(saved);
  @override
  Future<void> saveDevice(SavedDevice device) async {
    saved.add(device);
  }

  @override
  Future<void> forgetDevice(String address) async {
    saved.removeWhere((d) => d.address == address);
  }

  @override
  Future<List<TransferInfo>> transfers() async => [];
}
