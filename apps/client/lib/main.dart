import 'dart:async';

import 'package:engine_bridge/engine_bridge.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:path_provider/path_provider.dart';

import 'controller.dart';
import 'session_page.dart';

void main() => runApp(const MyApp());
final deskControllerProvider = Provider<DeskController>(
  (ref) => throw StateError('DeskController must be provided'),
);
DeskController desk(BuildContext context) {
  context.dependOnInheritedWidgetOfExactType<_ControllerScope>();
  return ProviderScope.containerOf(context).read(deskControllerProvider);
}

class _ControllerScope extends InheritedNotifier<DeskController> {
  const _ControllerScope({
    required DeskController controller,
    required super.child,
  }) : super(notifier: controller);
}

class MyApp extends StatefulWidget {
  const MyApp({super.key, this.controller});
  final DeskController? controller;
  @override
  State<MyApp> createState() => _MyAppState();
}

class _MyAppState extends State<MyApp> {
  late final DeskController controller =
      widget.controller ?? DeskController(FrbEngine());
  ThemeMode mode = ThemeMode.system;
  late final GoRouter router = GoRouter(
    initialLocation: '/connect',
    routes: [
      ShellRoute(
        builder: (context, state, child) =>
            DeskShell(location: state.uri.path, child: child),
        routes: [
          GoRoute(
            path: '/connect',
            builder: (context, state) =>
                ConnectPage(address: state.uri.queryParameters['address']),
          ),
          GoRoute(path: '/host', builder: (context, state) => const HostPage()),
          GoRoute(
            path: '/devices',
            builder: (context, state) => const DevicesPage(),
          ),
          GoRoute(
            path: '/transfers',
            builder: (context, state) => const TransfersPage(),
          ),
          GoRoute(
            path: '/permissions',
            builder: (context, state) => const PermissionsPage(),
          ),
          GoRoute(
            path: '/settings',
            builder: (context, state) => SettingsPage(
              mode: mode,
              onMode: (value) => setState(() => mode = value),
            ),
          ),
          GoRoute(
            path: '/session/:id',
            builder: (context, state) => SessionPage(
              sessionId: int.tryParse(state.pathParameters['id']!) ?? -1,
            ),
          ),
        ],
      ),
    ],
  );
  @override
  void initState() {
    super.initState();
    if (widget.controller == null) unawaited(_start());
  }

  Future<void> _start() async {
    try {
      final path = (await getApplicationSupportDirectory()).path;
      await controller.start(path);
    } catch (e) {
      controller.error = 'Application storage unavailable: $e';
      if (mounted) setState(() {});
    }
  }

  @override
  void dispose() {
    router.dispose();
    if (widget.controller == null) controller.dispose();
    super.dispose();
  }

  ThemeData theme(Brightness brightness) => ThemeData(
    useMaterial3: true,
    brightness: brightness,
    colorScheme: ColorScheme.fromSeed(
      seedColor: const Color(0xff087f8c),
      brightness: brightness,
      primary: brightness == Brightness.dark
          ? const Color(0xff6dd5d0)
          : const Color(0xff006a70),
    ),
    scaffoldBackgroundColor: brightness == Brightness.dark
        ? const Color(0xff0c1827)
        : const Color(0xfff3f7fa),
    visualDensity: VisualDensity.standard,
    filledButtonTheme: FilledButtonThemeData(
      style: FilledButton.styleFrom(minimumSize: const Size(48, 48)),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(minimumSize: const Size(48, 48)),
    ),
    inputDecorationTheme: const InputDecorationTheme(
      border: OutlineInputBorder(),
    ),
  );
  @override
  Widget build(BuildContext context) => ProviderScope(
    overrides: [deskControllerProvider.overrideWithValue(controller)],
    child: _ControllerScope(
      controller: controller,
      child: MaterialApp.router(
        title: 'DeskHarbor',
        debugShowCheckedModeBanner: false,
        theme: theme(Brightness.light),
        darkTheme: theme(Brightness.dark),
        themeMode: mode,
        routerConfig: router,
      ),
    ),
  );
}

const destinations = [
  ('Connect', Icons.computer_outlined, '/connect'),
  ('This Computer', Icons.screen_share_outlined, '/host'),
  ('Devices', Icons.devices_outlined, '/devices'),
  ('Transfers', Icons.swap_vert, '/transfers'),
  ('Settings', Icons.settings_outlined, '/settings'),
];

class DeskShell extends StatelessWidget {
  const DeskShell({super.key, required this.location, required this.child});
  final String location;
  final Widget child;
  @override
  Widget build(BuildContext context) {
    final controller = desk(context);
    final index = destinations.indexWhere((d) => d.$3 == location);
    final selected = location == '/permissions' ? 4 : (index < 0 ? 0 : index);
    final wide =
        defaultTargetPlatform != TargetPlatform.android &&
        MediaQuery.sizeOf(context).width >= 760;
    return Scaffold(
      appBar: AppBar(
        title: const Text('DeskHarbor'),
        actions: [
          IconButton(
            tooltip: 'Refresh engine status',
            onPressed: controller.initialized ? controller.refresh : null,
            icon: const Icon(Icons.refresh),
          ),
        ],
      ),
      bottomNavigationBar: wide
          ? null
          : NavigationBar(
              selectedIndex: selected,
              onDestinationSelected: (i) => context.go(destinations[i].$3),
              destinations: [
                for (final d in destinations)
                  NavigationDestination(icon: Icon(d.$2), label: d.$1),
              ],
            ),
      body: Row(
        children: [
          if (wide)
            SizedBox(
              width: 224,
              child: ListView(
                children: [
                  const Padding(
                    padding: EdgeInsets.fromLTRB(24, 16, 16, 24),
                    child: Text(
                      'Your computers.\nYour connection.',
                      style: TextStyle(
                        fontSize: 18,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                  ),
                  for (var i = 0; i < destinations.length; i++)
                    Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 8,
                        vertical: 2,
                      ),
                      child: ListTile(
                        minVerticalPadding: 12,
                        selected: selected == i,
                        shape: RoundedRectangleBorder(
                          borderRadius: BorderRadius.circular(16),
                        ),
                        leading: Icon(destinations[i].$2),
                        title: Text(destinations[i].$1),
                        onTap: () => context.go(destinations[i].$3),
                      ),
                    ),
                  const Divider(),
                  for (final session in controller.sessions.values)
                    ListTile(
                      leading: const Icon(Icons.desktop_windows),
                      title: Text(
                        session.endpoint,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                      subtitle: Text(session.state),
                      onTap: () => context.go('/session/${session.id}'),
                    ),
                  const Padding(
                    padding: EdgeInsets.all(20),
                    child: Text(
                      'Direct LAN / VPN\nNo account or relay',
                      style: TextStyle(fontSize: 12),
                    ),
                  ),
                ],
              ),
            ),
          if (wide) const VerticalDivider(width: 1),
          Expanded(
            child: Column(
              children: [
                if (controller.error != null)
                  MaterialBanner(
                    content: Text(controller.error!),
                    actions: [
                      TextButton(
                        onPressed: () => context.go('/permissions'),
                        child: const Text('Details'),
                      ),
                    ],
                  ),
                Expanded(child: child),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class PageBody extends StatelessWidget {
  const PageBody({
    super.key,
    required this.title,
    required this.subtitle,
    required this.children,
  });
  final String title, subtitle;
  final List<Widget> children;
  @override
  Widget build(BuildContext context) => SingleChildScrollView(
    padding: const EdgeInsets.all(24),
    child: Align(
      alignment: Alignment.topLeft,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 880),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(title, style: Theme.of(context).textTheme.headlineMedium),
            const SizedBox(height: 8),
            Text(subtitle),
            const SizedBox(height: 24),
            ...children,
          ],
        ),
      ),
    ),
  );
}

class StatusCard extends StatelessWidget {
  const StatusCard({
    super.key,
    required this.title,
    required this.detail,
    required this.ready,
  });
  final String title, detail;
  final bool ready;
  @override
  Widget build(BuildContext context) => Card(
    child: Padding(
      padding: const EdgeInsets.all(20),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(
            ready ? Icons.check_circle_outline : Icons.info_outline,
            color: ready
                ? Theme.of(context).colorScheme.primary
                : Theme.of(context).colorScheme.error,
          ),
          const SizedBox(width: 16),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(title, style: Theme.of(context).textTheme.titleMedium),
                const SizedBox(height: 6),
                Text(detail),
              ],
            ),
          ),
        ],
      ),
    ),
  );
}

class ConnectPage extends StatefulWidget {
  const ConnectPage({super.key, this.address});
  final String? address;
  @override
  State<ConnectPage> createState() => _ConnectPageState();
}

class _ConnectPageState extends State<ConnectPage> {
  late final address = TextEditingController(text: widget.address);
  final password = TextEditingController();
  @override
  void dispose() {
    address.dispose();
    password.clear();
    password.dispose();
    super.dispose();
  }

  Future<void> connect() async {
    final controller = desk(context);
    final pending = controller.connect(address.text, password.text);
    password.clear(); // Clear immediately, including authentication/native failures.
    final session = await pending;
    if (mounted && session != null) context.go('/session/${session.id}');
  }

  @override
  Widget build(BuildContext context) {
    final controller = desk(context);
    return PageBody(
      title: 'Connect to a computer',
      subtitle: 'Use the host address on your private LAN or VPN.',
      children: [
        StatusCard(
          title: controller.initialized
              ? 'Engine connected'
              : 'Engine unavailable',
          detail: controller.initialized
              ? 'Enter the host address and its permanent password.'
              : 'The native engine must initialize before connecting. Check Permissions for details.',
          ready: controller.initialized,
        ),
        const SizedBox(height: 20),
        TextField(
          controller: address,
          decoration: const InputDecoration(
            labelText: 'Address',
            hintText: '192.168.1.10:47832',
          ),
          textInputAction: TextInputAction.next,
          autocorrect: false,
        ),
        const SizedBox(height: 16),
        TextField(
          key: const Key('connect-password'),
          controller: password,
          decoration: const InputDecoration(labelText: 'Host password'),
          obscureText: true,
          autocorrect: false,
          enableSuggestions: false,
          onSubmitted: controller.initialized && !controller.busy
              ? (_) => connect()
              : null,
        ),
        const SizedBox(height: 20),
        Align(
          alignment: Alignment.centerLeft,
          child: FilledButton.icon(
            key: const Key('connect-submit'),
            onPressed: controller.initialized && !controller.busy
                ? connect
                : null,
            icon: controller.busy
                ? const SizedBox(
                    width: 18,
                    height: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.arrow_forward),
            label: Text(controller.busy ? 'Connecting…' : 'Connect'),
          ),
        ),
        const SizedBox(height: 24),
        const Text(
          'Passwords are used for this request only and are never saved. A host must approve its required OS permissions.',
        ),
        if (controller.sessions.isNotEmpty) ...[
          const SizedBox(height: 24),
          Text('Sessions', style: Theme.of(context).textTheme.titleLarge),
          for (final session in controller.sessions.values)
            ListTile(
              title: Text(session.endpoint),
              subtitle: Text('${session.state} · ${session.transport}'),
              trailing: const Icon(Icons.arrow_forward),
              onTap: () => context.go('/session/${session.id}'),
            ),
        ],
      ],
    );
  }
}

class HostPage extends StatefulWidget {
  const HostPage({super.key});
  @override
  State<HostPage> createState() => _HostPageState();
}

class _HostPageState extends State<HostPage> {
  final password = TextEditingController();
  final bind = TextEditingController(text: '0.0.0.0:47832');
  final directory = TextEditingController();
  bool input = true, clipboard = false, files = false, busy = false;
  @override
  void dispose() {
    password.clear();
    password.dispose();
    bind.dispose();
    directory.dispose();
    super.dispose();
  }

  Future<void> setPassword() async {
    setState(() => busy = true);
    final pending = desk(context).hostPassword(password.text);
    password.clear();
    await pending;
    if (mounted) setState(() => busy = false);
  }

  Future<void> configure(bool enabled) async {
    setState(() => busy = true);
    await desk(context).configure(
      HostSettings(
        enabled: enabled,
        bindAddress: bind.text.trim(),
        allowInput: input,
        clipboard: clipboard,
        files: files,
        transferDirectory: directory.text.trim().isEmpty
            ? null
            : directory.text.trim(),
      ),
    );
    if (mounted) setState(() => busy = false);
  }

  @override
  Widget build(BuildContext context) {
    final controller = desk(context), host = desk(context).host;
    return PageBody(
      title: 'This Computer',
      subtitle: 'Share this computer only when its native host is ready.',
      children: [
        StatusCard(
          title: host?.state.replaceAll('_', ' ') ?? 'Host status unavailable',
          detail: host?.detail ?? 'Initialize the native engine to inspect host permissions and readiness.',
          ready: host?.running == true && host?.state == 'ready',
        ),
        if (host != null) ...[
          for (final address in host.addresses) SelectableText(address),
          const SizedBox(height: 12),
          Text(
            host.unattendedAvailable
                ? 'Installed host capability is available.'
                : 'Unattended hosting is unavailable. Native installation and sign-in qualification are required.',
          ),
        ],
        const SizedBox(height: 16),
        OutlinedButton.icon(
          onPressed: () => context.go('/permissions'),
          icon: const Icon(Icons.security),
          label: const Text('Permissions and readiness'),
        ),
        const SizedBox(height: 24),
        Text('Host password', style: Theme.of(context).textTheme.titleLarge),
        const SizedBox(height: 12),
        TextField(
          key: const Key('host-password'),
          controller: password,
          decoration: InputDecoration(
            labelText: host?.passwordConfigured == true
                ? 'New permanent password'
                : 'Set permanent password',
          ),
          obscureText: true,
          autocorrect: false,
          enableSuggestions: false,
        ),
        const SizedBox(height: 12),
        Align(
          alignment: Alignment.centerLeft,
          child: FilledButton(
            onPressed: controller.initialized && !busy ? setPassword : null,
            child: const Text('Set password'),
          ),
        ),
        const SizedBox(height: 24),
        TextField(
          controller: bind,
          decoration: const InputDecoration(labelText: 'Listen address'),
        ),
        const SizedBox(height: 12),
        SwitchListTile(
          title: const Text('Allow keyboard and pointer'),
          value: input,
          onChanged: busy ? null : (v) => setState(() => input = v),
        ),
        SwitchListTile(
          title: const Text('Text clipboard'),
          subtitle: const Text(
            'Available only in an authorized active user session.',
          ),
          value: clipboard,
          onChanged: busy ? null : (v) => setState(() => clipboard = v),
        ),
        SwitchListTile(
          title: const Text('File transfers'),
          subtitle: const Text('Unavailable while locked or signed out.'),
          value: files,
          onChanged: busy ? null : (v) => setState(() => files = v),
        ),
        if (files)
          TextField(
            controller: directory,
            decoration: const InputDecoration(
              labelText: 'Allowed receive directory',
            ),
          ),
        const SizedBox(height: 16),
        Wrap(
          spacing: 12,
          runSpacing: 12,
          children: [
            FilledButton.icon(
              onPressed: controller.initialized && !busy
                  ? () => configure(true)
                  : null,
              icon: const Icon(Icons.screen_share),
              label: const Text('Start sharing'),
            ),
            OutlinedButton(
              onPressed:
                  controller.initialized && !busy && host?.running == true
                  ? () => configure(false)
                  : null,
              child: const Text('Stop sharing'),
            ),
          ],
        ),
      ],
    );
  }
}

class DevicesPage extends StatefulWidget {
  const DevicesPage({super.key});
  @override
  State<DevicesPage> createState() => _DevicesPageState();
}

class _DevicesPageState extends State<DevicesPage> {
  final name = TextEditingController(), address = TextEditingController();
  @override
  void dispose() {
    name.dispose();
    address.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final controller = desk(context);
    return PageBody(
      title: 'Devices',
      subtitle: 'Keep names and addresses locally. Passwords are never stored.',
      children: [
        for (final device in controller.devices)
          Card(
            child: ListTile(
              minVerticalPadding: 16,
              title: Text(device.name),
              subtitle: Text(
                '${device.address}\n${device.identity ?? 'Identity will be verified at connection'}',
              ),
              isThreeLine: true,
              onTap: () => context.go(
                '/connect?address=${Uri.encodeQueryComponent(device.address)}',
              ),
              trailing: IconButton(
                tooltip: 'Forget ${device.name}',
                onPressed: () => controller.forget(device.address),
                icon: const Icon(Icons.delete_outline),
              ),
            ),
          ),
        if (controller.devices.isEmpty)
          const StatusCard(
            title: 'No saved devices',
            detail: 'Save a trusted computer below, then connect using its host password.',
            ready: false,
          ),
        const SizedBox(height: 24),
        TextField(
          controller: name,
          decoration: const InputDecoration(labelText: 'Device name'),
        ),
        const SizedBox(height: 12),
        TextField(
          controller: address,
          decoration: const InputDecoration(labelText: 'Device address'),
        ),
        const SizedBox(height: 16),
        Align(
          alignment: Alignment.centerLeft,
          child: FilledButton.icon(
            onPressed: controller.initialized
                ? () async {
                    await controller.save(name.text, address.text);
                    if (mounted && controller.error == null) {
                      name.clear();
                      address.clear();
                    }
                  }
                : null,
            icon: const Icon(Icons.bookmark_add_outlined),
            label: const Text('Save device'),
          ),
        ),
      ],
    );
  }
}

class TransfersPage extends StatelessWidget {
  const TransfersPage({super.key});
  @override
  Widget build(BuildContext context) {
    final controller = desk(context);
    return PageBody(
      title: 'Transfers',
      subtitle: 'Transfers require an authorized active desktop and an allowed destination.',
      children: [
        if (controller.transfers.isEmpty)
          const StatusCard(
            title: 'No transfers',
            detail: 'Open a connected session to send a file. Progress and checksum results appear here.',
            ready: false,
          ),
        for (final transfer in controller.transfers.values)
          Card(
            child: Padding(
              padding: const EdgeInsets.all(20),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    transfer.filename,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  const SizedBox(height: 8),
                  LinearProgressIndicator(
                    value: transfer.total > 0
                        ? (transfer.transferred / transfer.total).clamp(0, 1)
                        : null,
                  ),
                  const SizedBox(height: 8),
                  Text(
                    '${transfer.state} · ${transfer.transferred} / ${transfer.total} bytes',
                  ),
                  if (transfer.checksum.isNotEmpty)
                    SelectableText('Checksum: ${transfer.checksum}'),
                  if (![
                    'completed',
                    'cancelled',
                    'failed',
                  ].contains(transfer.state))
                    TextButton(
                      onPressed: () => controller.cancelTransfer(transfer.id),
                      child: const Text('Cancel transfer'),
                    ),
                ],
              ),
            ),
          ),
      ],
    );
  }
}

class SettingsPage extends StatelessWidget {
  const SettingsPage({super.key, required this.mode, required this.onMode});
  final ThemeMode mode;
  final ValueChanged<ThemeMode> onMode;
  @override
  Widget build(BuildContext context) => PageBody(
    title: 'Settings',
    subtitle: 'Private connections with explicit local control.',
    children: [
      DropdownButtonFormField<ThemeMode>(
        initialValue: mode,
        decoration: const InputDecoration(labelText: 'Appearance'),
        items: const [
          DropdownMenuItem(value: ThemeMode.system, child: Text('System')),
          DropdownMenuItem(value: ThemeMode.light, child: Text('Light')),
          DropdownMenuItem(value: ThemeMode.dark, child: Text('Dark')),
        ],
        onChanged: (v) {
          if (v != null) onMode(v);
        },
      ),
      const SizedBox(height: 24),
      const StatusCard(
        title: 'Direct transport',
        detail: 'QUIC on port 47832, with TLS/TCP fallback when UDP is blocked. Use a LAN address or an existing VPN. No external relay or account.',
        ready: true,
      ),
      const StatusCard(
        title: 'Release qualification pending',
        detail: 'Native sign-in, lock, permission revocation, physical platform and signing checks remain required before production use.',
        ready: false,
      ),
      ListTile(
        minVerticalPadding: 16,
        leading: const Icon(Icons.security),
        title: const Text('Permissions and native capabilities'),
        trailing: const Icon(Icons.arrow_forward),
        onTap: () => context.go('/permissions'),
      ),
    ],
  );
}

class PermissionsPage extends StatelessWidget {
  const PermissionsPage({super.key});
  @override
  Widget build(BuildContext context) {
    final controller = desk(context), host = desk(context).host;
    return PageBody(
      title: 'Permissions and readiness',
      subtitle: 'Readiness comes from the native host. OS permission prompts must be completed on the host.',
      children: [
        StatusCard(
          title:
              host?.state.replaceAll('_', ' ') ?? 'Native engine unavailable',
          detail:
              host?.detail ??
              controller.error ??
              'The engine has not initialized.',
          ready: host?.state == 'ready',
        ),
        const StatusCard(
          title: 'Windows',
          detail: 'Installed broker and restricted capture/input workers are required for sign-in, lock, logout, UAC and secure desktop. A normal GUI process cannot provide these capabilities.',
          ready: false,
        ),
        const StatusCard(
          title: 'macOS',
          detail: 'Screen Recording and Accessibility consent, signed installed helpers, and physical LoginWindow/Aqua tests are required. Permission revocation must stop access.',
          ready: false,
        ),
        const StatusCard(
          title: 'Ubuntu / Wayland',
          detail: 'An active connected display and qualified GDM/session capture and input helpers are required. Intel and AMD native proofs remain separate.',
          ready: false,
        ),
        const StatusCard(
          title: 'Android',
          detail: 'Hosting requires MediaProjection consent every session and a visible foreground service. Remote input requires explicit Accessibility permission. Protected screens stay protected.',
          ready: false,
        ),
        const SizedBox(height: 16),
        OutlinedButton.icon(
          onPressed: controller.initialized ? controller.refresh : null,
          icon: const Icon(Icons.refresh),
          label: const Text('Recheck native readiness'),
        ),
      ],
    );
  }
}
