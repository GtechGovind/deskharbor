import 'dart:async';

import 'package:engine_bridge/engine_bridge.dart';
import 'package:flutter/foundation.dart';

class DeskController extends ChangeNotifier {
  DeskController(this.engine);
  final EngineGateway engine;
  bool initialized = false;
  bool busy = false;
  String? error;
  HostInfo? host;
  final Map<int, SessionInfo> sessions = {};
  List<SavedDevice> devices = [];
  final Map<int, TransferInfo> transfers = {};
  final Map<int, int> frames = {};
  final Set<int> _focused = {};
  final Set<int> _invalid = {};
  final Map<int, Future<void>> _inputQueues = {};
  StreamSubscription<EngineEventInfo>? _subscription;
  bool _disposed = false;

  Future<void> start(String path) async {
    try {
      await engine.initialize(path);
      initialized = true;
      _subscription = engine.subscribe().listen(
        _event,
        onError: (Object e) {
          _invalid.addAll(sessions.keys);
          error = 'Engine updates stopped: $e';
          _notify();
        },
      );
      await refresh();
    } catch (e) {
      error = 'Engine unavailable: $e';
    }
    _notify();
  }

  Future<void> refresh() async {
    if (!initialized) return;
    await run(() async {
      host = await engine.hostStatus();
      devices = await engine.devices();
      for (final transfer in await engine.transfers()) {
        transfers[transfer.id] = transfer;
      }
    });
  }

  Future<T?> run<T>(Future<T> Function() action) async {
    error = null;
    try {
      return await action();
    } catch (e) {
      error = '$e';
      return null;
    } finally {
      _notify();
    }
  }

  Future<SessionInfo?> connect(String address, String password) async {
    if (!initialized || busy) return null;
    if (address.trim().isEmpty || password.isEmpty) {
      error = 'Enter an address and host password';
      _notify();
      return null;
    }
    error = null;
    busy = true;
    _notify();
    try {
      final session = await engine.connect(address.trim(), password);
      sessions[session.id] = session;
      return session;
    } catch (e) {
      error = '$e';
      return null;
    } finally {
      busy = false;
      _notify();
    }
  }

  bool canInput(int id) =>
      initialized &&
      sessions[id]?.state == 'connected' &&
      !_invalid.contains(id) &&
      _focused.contains(id);
  bool sessionReady(int id) =>
      initialized &&
      sessions[id]?.state == 'connected' &&
      !_invalid.contains(id);
  void focus(int id, bool value) {
    bool changed;
    if (value) {
      changed = sessionReady(id) && _focused.add(id);
    } else {
      changed = _focused.remove(id);
      if (changed) unawaited(release(id));
    }
    if (changed) _notify();
  }

  InputRequest request(
    SessionInfo session,
    String kind, {
    int usage = 0,
    bool pressed = false,
    double x = 0,
    double y = 0,
    int buttons = 0,
    int wheelX = 0,
    int wheelY = 0,
    String text = '',
  }) => InputRequest(
    epoch: session.epoch,
    layout: session.layout,
    displayId: session.selectedDisplay,
    kind: kind,
    x: x,
    y: y,
    buttons: buttons,
    wheelX: wheelX,
    wheelY: wheelY,
    usage: usage,
    pressed: pressed,
    text: text,
  );
  Future<void> input(
    int id,
    String kind, {
    int usage = 0,
    bool pressed = false,
    double x = 0,
    double y = 0,
    int buttons = 0,
    int wheelX = 0,
    int wheelY = 0,
    String text = '',
  }) async {
    if (!canInput(id)) return;
    final session = sessions[id]!;
    await _dispatchInput(
      id,
      request(
        session,
        kind,
        usage: usage,
        pressed: pressed,
        x: x,
        y: y,
        buttons: buttons,
        wheelX: wheelX,
        wheelY: wheelY,
        text: text,
      ),
    );
  }

  Future<void> _dispatchInput(
    int id,
    InputRequest event, {
    bool release = false,
  }) async {
    final preceding = _inputQueues[id] ?? Future<void>.value();
    final pending = preceding.then((_) async {
      final current = sessions[id];
      // A queued event must still belong to the focused, current context when
      // it reaches the native boundary. Release always follows earlier sends.
      if (!release &&
          (!canInput(id) ||
              current?.epoch != event.epoch ||
              current?.layout != event.layout ||
              current?.selectedDisplay != event.displayId)) {
        return;
      }
      try {
        await engine.sendInput(id, event);
      } catch (e) {
        // Cleanup must not erase a more useful permission/event-gap diagnostic.
        if (!release || error == null) error = '$e';
        _notify();
      }
    });
    _inputQueues[id] = pending;
    await pending;
    if (identical(_inputQueues[id], pending)) _inputQueues.remove(id);
  }

  Future<void> release(int id, [SessionInfo? previous]) async {
    final session = previous ?? sessions[id];
    if (session != null && initialized) {
      await _dispatchInput(id, request(session, 'release_all'), release: true);
    }
  }

  void releaseAll() {
    for (final id in _focused.toList()) {
      focus(id, false);
    }
  }

  Future<void> disconnect(int id) async {
    _focused.remove(id);
    _notify();
    await release(id);
    await run(() => engine.disconnect(id));
  }

  Future<void> selectMonitor(int id, String display) async {
    if (!sessionReady(id)) return;
    await release(id);
    _invalid.add(id);
    frames.remove(id);
    _notify();
    await run(() => engine.selectMonitor(id, display));
    // Input resumes only after a current SessionChanged event arrives.
  }

  Future<void> configure(HostSettings settings) async {
    if (!initialized) return;
    await run(() async {
      host = await engine.configureHost(settings);
    });
  }

  Future<void> hostPassword(String password) async {
    if (!initialized) return;
    if (password.isEmpty) {
      error = 'A host password is required';
      _notify();
      return;
    }
    await run(() async {
      await engine.setHostPassword(password);
      host = await engine.hostStatus();
    });
  }

  Future<void> save(String name, String address, [String? identity]) async {
    if (name.trim().isEmpty || address.trim().isEmpty) {
      error = 'Device name and address are required';
      _notify();
      return;
    }
    await run(() async {
      await engine.saveDevice(
        SavedDevice(
          name: name.trim(),
          address: address.trim(),
          identity: identity,
        ),
      );
      devices = await engine.devices();
    });
  }

  Future<void> forget(String address) async {
    await run(() async {
      await engine.forgetDevice(address);
      devices = await engine.devices();
    });
  }

  Future<void> sendFile(int id, String path) async {
    if (!sessionReady(id) || path.trim().isEmpty) return;
    await run(() async {
      final transfer = await engine.startTransfer(id, path.trim());
      transfers[transfer.id] = transfer;
    });
  }

  Future<void> cancelTransfer(int id) async {
    await run(() => engine.cancelTransfer(id));
  }

  Future<String?> readClipboard(int id) async {
    if (!sessionReady(id)) return null;
    return run(() => engine.clipboardRequestText(id));
  }

  Future<void> sendClipboard(int id, String text) async {
    if (sessionReady(id)) {
      await run(() => engine.clipboardSendText(id, text));
    }
  }

  void _event(EngineEventInfo event) {
    if (event.session case final session?) {
      final previous = sessions[session.id];
      if (previous != null &&
          (previous.epoch != session.epoch ||
              previous.layout != session.layout ||
              previous.state != session.state)) {
        unawaited(release(session.id, previous));
        _focused.remove(session.id);
        frames.remove(session.id);
      }
      sessions[session.id] = session;
      if (session.state == 'connected') {
        _invalid.remove(session.id);
      } else {
        _invalid.add(session.id);
      }
    }
    if (event.host case final value?) {
      host = value;
    }
    if (event.transfer case final value?) {
      transfers[value.id] = value;
    }
    if (event.kind == 'frame' && event.sessionId != null) {
      frames[event.sessionId!] = event.sequence;
    }
    if (event.kind == 'error') {
      error = event.message.isEmpty
          ? 'Engine error: ${event.code}'
          : event.message;
      if (event.code == 'event_gap') {
        _invalid.addAll(sessions.keys);
        frames.clear();
        releaseAll();
      }
    }
    _notify();
  }

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    releaseAll();
    _disposed = true;
    unawaited(_subscription?.cancel());
    super.dispose();
  }
}
