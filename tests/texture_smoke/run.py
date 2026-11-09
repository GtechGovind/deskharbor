#!/usr/bin/env python3
"""Real Linux texture smoke on verified Xvfb. No physical screen capture or input."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import tempfile
import time


def run(command, **kwargs):
    subprocess.run([str(item) for item in command], check=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    display = re.fullmatch(r':(\d+)(?:\.0)?', os.environ.get('DISPLAY', ''))
    if not display:
        parser.error('Run under a local xvfb-run display, never the user desktop')
    # Only a runner explicitly created for this fixture is an acceptable target.
    if os.environ.get('DESKHARBOR_SYNTHETIC_DISPLAY') != '1':
        parser.error('Explicit disposable-display opt-in required')
    lock = Path(f'/tmp/.X{display[1]}-lock')
    try:
        server_pid = int(lock.read_text().strip())
        executable = Path(f'/proc/{server_pid}/exe').resolve(strict=True)
        if lock.stat().st_uid != os.geteuid() or executable.name != 'Xvfb':
            raise ValueError('Display is not an owned Xvfb process')
    except (OSError, ValueError) as error:
        parser.error(f'Cannot verify disposable Xvfb display: {error}')
    repo = Path(__file__).resolve().parents[2]
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    ignore = shutil.ignore_patterns('build', '.dart_tool', '.gradle', 'ephemeral',
                                    '.plugin_symlinks', '.flutter-plugins-dependencies',
                                    'local.properties', '.idea')
    with tempfile.TemporaryDirectory(prefix='deskharbor-texture-smoke-') as temporary:
        root = Path(temporary)
        project = root / 'project'
        app = project / 'apps/client'
        shutil.copytree(repo / 'apps/client', app, ignore=ignore)
        for package in ('engine_bridge', 'native_surfaces'):
            shutil.copytree(repo / 'packages' / package, project / 'packages' / package, ignore=ignore)
        shutil.copy2(repo / 'tests/texture_smoke/probe.dart', app / 'lib/texture_probe.dart')
        environment = os.environ.copy()
        environment['DESKHARBOR_RUST_LIBRARY'] = str(repo / 'target/release/libdh_ffi.so')
        run(['flutter', 'pub', 'get', '--enforce-lockfile'], cwd=app, env=environment)
        run(['flutter', 'analyze', '--no-pub', 'lib/texture_probe.dart'], cwd=app, env=environment)
        run(['flutter', 'build', 'linux', '--release', '--target', 'lib/texture_probe.dart'],
            cwd=app, env=environment)
        state = root / 'state'
        state.mkdir(mode=0o700)
        cli = repo / 'target/release/dh-cli'
        run([cli, '--data-dir', state, 'password'], input=b'DeskHarbor synthetic test only\n')
        with socket.socket() as reservation:
            reservation.bind(('127.0.0.1', 0))
            port = reservation.getsockname()[1]
        endpoint = f'127.0.0.1:{port}'
        run([cli, '--data-dir', state, 'enable', endpoint])
        environment['DESKHARBOR_SMOKE_ENDPOINT'] = endpoint
        finish = root / 'pixel-check-finished'
        environment['DESKHARBOR_SMOKE_DONE'] = str(finish)
        host = viewer = None
        try:
            with (output / 'host.log').open('w') as host_log, (output / 'viewer.log').open('w') as viewer_log:
                host = subprocess.Popen([str(repo / 'target/release/dh-hostd'), '--data-dir', str(state),
                                         '--synthetic-loopback'], stdout=host_log, stderr=subprocess.STDOUT)
                deadline = time.monotonic() + 20
                while True:
                    if host.poll() is not None:
                        raise RuntimeError('Synthetic host exited before listening')
                    try:
                        with socket.create_connection(('127.0.0.1', port), timeout=0.2):
                            break
                    except OSError:
                        if time.monotonic() > deadline:
                            raise TimeoutError('Synthetic host did not bind')
                        time.sleep(0.1)
                bundle = app / 'build/linux/x64/release/bundle'
                viewer = subprocess.Popen([str(bundle / 'deskharbor')], cwd=bundle, env=environment,
                                          stdout=viewer_log, stderr=subprocess.STDOUT)
                deadline = time.monotonic() + 45
                while 'DESKHARBOR_TEXTURE_READY' not in (output / 'viewer.log').read_text(errors='replace'):
                    if viewer.poll() is not None:
                        raise RuntimeError('Native viewer exited before texture readiness')
                    if time.monotonic() > deadline:
                        raise TimeoutError('Native renderer did not become ready')
                    time.sleep(0.1)
                # This display contains only the fixture's synthetic remote
                # pixels. No physical desktop or user clipboard is accessed.
                screenshot = output / 'synthetic-texture.png'
                run(['scrot', '--overwrite', screenshot])
                from PIL import Image
                with Image.open(screenshot) as image:
                    rgb = image.convert('RGB')
                    green = sum(1 for red, value, blue in rgb.getdata()
                                if red < 215 and 100 <= value <= 155)
                if green < 30000:
                    raise AssertionError(f'Decoded synthetic green plane absent: {green} matching pixels')
                finish.write_text('verified\n')
                if viewer.wait(timeout=20) != 0:
                    raise RuntimeError('Native viewer cleanup failed')
                log = (output / 'viewer.log').read_text(errors='replace')
                if 'DESKHARBOR_TEXTURE_CLEAN' not in log:
                    raise RuntimeError('Native cleanup acknowledgement missing')
                (output / 'result.json').write_text(json.dumps({
                    'synthetic_only': True, 'texture_cycles': 8,
                    'matching_decoded_pixels': green, 'native_cleanup': True,
                    'sign_in_qualified': False,
                }, indent=2) + '\n')
        finally:
            for process in (viewer, host):
                if process is not None and process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=10)


if __name__ == '__main__':
    main()
