import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:go_router/go_router.dart';
import 'package:native_surfaces/native_surfaces.dart';

import 'controller.dart';
import 'main.dart' show desk;

class SessionPage extends StatefulWidget {
  const SessionPage({super.key, required this.sessionId});
  final int sessionId;
  @override
  State<SessionPage> createState() => _SessionPageState();
}

class _SessionPageState extends State<SessionPage> with WidgetsBindingObserver {
  final canvasFocus = FocusNode(debugLabel: 'Remote canvas');
  final text = TextEditingController(), path = TextEditingController();
  NativeSurface? surface;
  String? surfaceError;
  DeskController? controller;
  int? lastSequence;
  bool trackpad = false, showKeyboard = false;
  Offset cursor = const Offset(.5, .5), down = Offset.zero;
  double movement = 0;
  final Set<int> modifiers = {};

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (controller == null) {
      controller = desk(context);
      controller!.addListener(updated);
      unawaited(createSurface());
    }
  }

  Future<void> createSurface() async {
    try {
      final created = await NativeSurface.create(widget.sessionId);
      if (!mounted) {
        await created.dispose();
        return;
      }
      setState(() => surface = created);
      if (controller!.frames.containsKey(widget.sessionId)) {
        await present();
      }
    } catch (e) {
      if (mounted) {
        setState(() => surfaceError = 'Native video unavailable: $e');
      }
    }
  }

  void updated() {
    if (!controller!.canInput(widget.sessionId)) modifiers.clear();
    final sequence = controller!.frames[widget.sessionId];
    if (sequence != null && sequence != lastSequence) {
      lastSequence = sequence;
      unawaited(present());
    }
  }

  Future<void> present() async {
    if (surface == null) return;
    try {
      await surface!.present();
      if (mounted && surfaceError != null) setState(() => surfaceError = null);
    } catch (e) {
      controller?.focus(widget.sessionId, false);
      if (mounted) {
        setState(() => surfaceError = 'Native frame unavailable: $e');
      }
    }
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state != AppLifecycleState.resumed) {
      canvasFocus.unfocus();
      controller?.focus(widget.sessionId, false);
      modifiers.clear();
    }
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    controller?.removeListener(updated);
    controller?.focus(widget.sessionId, false);
    canvasFocus.dispose();
    text.dispose();
    path.dispose();
    if (surface != null) {
      unawaited(surface!.dispose().catchError((Object _) {}));
    }
    super.dispose();
  }

  bool get ready =>
      controller!.sessionReady(widget.sessionId) &&
      surface != null &&
      surfaceError == null &&
      controller!.frames.containsKey(widget.sessionId);
  void focusCanvas() {
    if (ready) {
      canvasFocus.requestFocus();
      controller!.focus(widget.sessionId, true);
    }
  }

  KeyEventResult key(FocusNode node, KeyEvent event) {
    if (!ready || !controller!.canInput(widget.sessionId)) {
      return KeyEventResult.ignored;
    }
    final usage = event.physicalKey.usbHidUsage;
    if (usage >> 16 != 7) return KeyEventResult.ignored;
    unawaited(
      controller!.input(
        widget.sessionId,
        'key',
        usage: usage & 0xffff,
        pressed: event is! KeyUpEvent,
      ),
    );
    return KeyEventResult.handled;
  }

  void pointer(PointerEvent event, Size size, {required bool up}) {
    if (!ready) return;
    if (event is PointerDownEvent) {
      focusCanvas();
      down = event.localPosition;
      movement = 0;
    }
    if (trackpad && event.kind == PointerDeviceKind.touch) {
      if (event is PointerMoveEvent) {
        movement += (event.localPosition - down).distance;
        down = event.localPosition;
        cursor = Offset(
          (cursor.dx + event.delta.dx / size.width).clamp(0, 1),
          (cursor.dy + event.delta.dy / size.height).clamp(0, 1),
        );
        unawaited(
          controller!.input(
            widget.sessionId,
            'pointer',
            x: cursor.dx,
            y: cursor.dy,
          ),
        );
      }
      if (up && movement < 8) unawaited(click());
      return;
    }
    cursor = Offset(
      (event.localPosition.dx / size.width).clamp(0, 1),
      (event.localPosition.dy / size.height).clamp(0, 1),
    );
    unawaited(
      controller!.input(
        widget.sessionId,
        'pointer',
        x: cursor.dx,
        y: cursor.dy,
        buttons: up ? 0 : event.buttons,
      ),
    );
  }

  Future<void> click() async {
    await controller!.input(
      widget.sessionId,
      'pointer',
      x: cursor.dx,
      y: cursor.dy,
      buttons: 1,
    );
    await controller!.input(
      widget.sessionId,
      'pointer',
      x: cursor.dx,
      y: cursor.dy,
    );
  }

  Future<void> sendText() async {
    focusCanvas();
    final value = text.text;
    text.clear();
    await controller!.input(widget.sessionId, 'text', text: value);
  }

  Future<void> clipboard() async {
    canvasFocus.unfocus();
    final field = TextEditingController();
    var active = true;
    try {
      await showDialog<void>(
        context: context,
        builder: (dialogContext) => AlertDialog(
          title: const Text('Session clipboard'),
          content: SizedBox(
            width: 500,
            child: TextField(
              controller: field,
              minLines: 3,
              maxLines: 8,
              decoration: const InputDecoration(labelText: 'Plain text'),
            ),
          ),
          actions: [
            TextButton(
              onPressed: () async {
                final value = await controller!.readClipboard(widget.sessionId);
                if (active && dialogContext.mounted && value != null) {
                  field.text = value;
                }
              },
              child: const Text('Get from host'),
            ),
            TextButton(
              onPressed: () async {
                await controller!.sendClipboard(widget.sessionId, field.text);
                if (active && dialogContext.mounted) {
                  Navigator.pop(dialogContext);
                }
              },
              child: const Text('Send to host'),
            ),
            TextButton(
              onPressed: () => Navigator.pop(dialogContext),
              child: const Text('Close'),
            ),
          ],
        ),
      );
    } finally {
      active = false;
      field.dispose();
    }
  }

  Widget canvas(double ratio) => LayoutBuilder(
    builder: (context, constraints) {
      final width = math.min(
            constraints.maxWidth,
            constraints.maxHeight * ratio,
          ),
          height = width / ratio;
      final c = controller!;
      return Center(
        child: SizedBox(
          width: width,
          height: height,
          child: Focus(
            focusNode: canvasFocus,
            onFocusChange: (value) {
              c.focus(widget.sessionId, value && ready);
              if (!value) modifiers.clear();
            },
            onKeyEvent: key,
            child: Listener(
              key: const Key('remote-canvas'),
              behavior: HitTestBehavior.opaque,
              onPointerDown: (e) => pointer(e, Size(width, height), up: false),
              onPointerMove: (e) => pointer(e, Size(width, height), up: false),
              onPointerUp: (e) => pointer(e, Size(width, height), up: true),
              onPointerCancel: (_) => unawaited(c.release(widget.sessionId)),
              onPointerHover: (e) {
                if (canvasFocus.hasFocus) {
                  pointer(e, Size(width, height), up: false);
                }
              },
              onPointerSignal: (e) {
                if (e is PointerScrollEvent && ready) {
                  unawaited(
                    c.input(
                      widget.sessionId,
                      'pointer',
                      x: cursor.dx,
                      y: cursor.dy,
                      wheelX: e.scrollDelta.dx.round(),
                      wheelY: e.scrollDelta.dy.round(),
                    ),
                  );
                }
              },
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: const Color(0xff060c14),
                  border: Border.all(
                    color: c.canInput(widget.sessionId)
                        ? Theme.of(context).colorScheme.primary
                        : Colors.grey,
                    width: 2,
                  ),
                  borderRadius: BorderRadius.circular(8),
                ),
                child: ClipRRect(
                  borderRadius: BorderRadius.circular(6),
                  child: Stack(
                    fit: StackFit.expand,
                    children: [
                      if (ready) Texture(textureId: surface!.textureId),
                      if (!ready)
                        Center(
                          child: Padding(
                            padding: const EdgeInsets.all(24),
                            child: Text(
                              surfaceError ??
                                  (c.sessionReady(widget.sessionId)
                                      ? 'Waiting for the first native frame…'
                                      : '${c.sessions[widget.sessionId]?.state ?? 'disconnected'} — remote input disabled'),
                              textAlign: TextAlign.center,
                              style: const TextStyle(
                                color: Colors.white,
                                fontSize: 16,
                              ),
                            ),
                          ),
                        ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ),
      );
    },
  );
  @override
  Widget build(BuildContext context) {
    final c = controller!, session = c.sessions[widget.sessionId];
    if (session == null) {
      return const Center(child: Text('Session not found. Return to Connect.'));
    }
    final display = session.displays
        .where((d) => d.id == session.selectedDisplay)
        .firstOrNull;
    final ratio = display != null && display.height > 0
        ? display.width / display.height
        : 16 / 9;
    return LayoutBuilder(
      builder: (context, constraints) {
        final compact =
            constraints.maxWidth < 600 || constraints.maxHeight < 600;
        final content = Padding(
          padding: const EdgeInsets.all(16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Wrap(
                spacing: 12,
                runSpacing: 8,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: [
                  Text(
                    session.endpoint,
                    style: Theme.of(context).textTheme.titleLarge,
                  ),
                  Chip(label: Text(session.state.replaceAll('_', ' '))),
                  Text(session.transport),
                  if (session.displays.isNotEmpty)
                    SizedBox(
                      width: 200,
                      child: DropdownButtonFormField<String>(
                        isExpanded: true,
                        key: ValueKey(
                          '${session.id}:${session.selectedDisplay}',
                        ),
                        initialValue:
                            session.displays.any(
                              (d) => d.id == session.selectedDisplay,
                            )
                            ? session.selectedDisplay
                            : null,
                        decoration: const InputDecoration(labelText: 'Monitor'),
                        items: [
                          for (final d in session.displays)
                            DropdownMenuItem(
                              value: d.id,
                              child: Text(
                                d.name,
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                        ],
                        onChanged: c.sessionReady(session.id)
                            ? (id) {
                                if (id != null) {
                                  unawaited(c.selectMonitor(session.id, id));
                                }
                              }
                            : null,
                      ),
                    ),
                  OutlinedButton.icon(
                    onPressed: c.sessionReady(session.id) ? clipboard : null,
                    icon: const Icon(Icons.content_paste),
                    label: const Text('Clipboard'),
                  ),
                  OutlinedButton(
                    onPressed: () async {
                      await c.disconnect(session.id);
                      if (context.mounted) context.go('/connect');
                    },
                    child: const Text('Disconnect'),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              SelectableText(
                'Host identity: ${session.hostIdentity.isEmpty ? 'Unavailable' : session.hostIdentity}',
                style: Theme.of(context).textTheme.bodySmall,
              ),
              const SizedBox(height: 8),
              if (compact)
                SizedBox(
                  height: math.max(
                    160,
                    math.min(480, constraints.maxHeight * .45),
                  ),
                  child: canvas(ratio),
                )
              else
                Expanded(child: canvas(ratio)),
              const SizedBox(height: 8),
              Text(
                c.canInput(session.id)
                    ? 'Remote canvas focused · physical HID keys forwarded'
                    : 'Click the remote canvas to control it. Input stops when focus or session state changes.',
                style: Theme.of(context).textTheme.bodySmall,
              ),
              const SizedBox(height: 8),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: [
                  SegmentedButton<bool>(
                    segments: const [
                      ButtonSegment(value: false, label: Text('Direct touch')),
                      ButtonSegment(value: true, label: Text('Trackpad')),
                    ],
                    selected: {trackpad},
                    onSelectionChanged: (value) =>
                        setState(() => trackpad = value.first),
                  ),
                  for (final m in const [
                    (224, 'Ctrl'),
                    (225, 'Shift'),
                    (226, 'Alt'),
                    (227, 'Meta'),
                  ])
                    FilterChip(
                      label: Text(m.$2),
                      selected: modifiers.contains(m.$1),
                      onSelected: ready
                          ? (value) {
                              focusCanvas();
                              setState(() {
                                if (value) {
                                  modifiers.add(m.$1);
                                } else {
                                  modifiers.remove(m.$1);
                                }
                              });
                              unawaited(
                                c.input(
                                  session.id,
                                  'key',
                                  usage: m.$1,
                                  pressed: value,
                                ),
                              );
                            }
                          : null,
                    ),
                  OutlinedButton.icon(
                    onPressed: ready
                        ? () => setState(() => showKeyboard = !showKeyboard)
                        : null,
                    icon: const Icon(Icons.keyboard),
                    label: const Text('Keyboard'),
                  ),
                ],
              ),
              if (showKeyboard) ...[
                const SizedBox(height: 8),
                Row(
                  children: [
                    Expanded(
                      child: TextField(
                        key: const Key('remote-text'),
                        controller: text,
                        decoration: const InputDecoration(
                          labelText: 'Send Unicode text',
                        ),
                        onSubmitted: (_) => sendText(),
                      ),
                    ),
                    const SizedBox(width: 8),
                    FilledButton(
                      onPressed: ready ? sendText : null,
                      child: const Text('Send text'),
                    ),
                  ],
                ),
              ],
              const SizedBox(height: 8),
              Row(
                children: [
                  Expanded(
                    child: TextField(
                      controller: path,
                      decoration: const InputDecoration(
                        labelText: 'Local file path',
                        isDense: true,
                      ),
                    ),
                  ),
                  const SizedBox(width: 8),
                  OutlinedButton(
                    onPressed: c.sessionReady(session.id)
                        ? () => c.sendFile(session.id, path.text)
                        : null,
                    child: const Text('Send file'),
                  ),
                ],
              ),
            ],
          ),
        );
        return compact ? SingleChildScrollView(child: content) : content;
      },
    );
  }
}
