"""Read-only native probe collection. Never promotes an observation to a P0 pass."""
import argparse
import datetime
import hashlib
import json
import pathlib
import platform
import subprocess

def classify_context(system, release, os_version, architecture, product=None):
    native_linux = system == "Linux" and "microsoft" not in release.lower() and "wsl" not in release.lower()
    mac_major = int(os_version.split('.')[0]) if os_version.split('.')[0].isdigit() else 0
    eligible = ((system == "Windows" and product == "workstation" and os_version == "11" and architecture.lower() in ("amd64", "x86_64"))
                or (system == "Darwin" and product == "macos" and mac_major >= 14 and architecture.lower() in ("arm64", "x86_64"))
                or (native_linux and product == "ubuntu" and os_version == "24.04" and architecture.lower() in ("amd64", "x86_64")))
    return {"system": system, "release": release, "os_version": os_version,
            "architecture": architecture, "product": product, "eligible_os": eligible,
            "hardware_or_session_proven": False,
            "qualification": "OS preflight only; GPU, desktop security context, installed signature and capture/input remain unproven"}

def context():
    version = platform.release()
    product = None
    if platform.system() == "Linux":
        try:
            release = platform.freedesktop_os_release()
            version = release.get("VERSION_ID", "unknown")
            product = release.get("ID")
        except OSError:
            version = "unknown"
    elif platform.system() == "Darwin":
        version = platform.mac_ver()[0]
        product = "macos"
    elif platform.system() == "Windows":
        # Windows may report release10 for Windows11; read the kernel build, not a marketing alias.
        import sys
        windows = sys.getwindowsversion()
        product = "workstation" if windows.product_type == 1 else "server"
        version = "11" if windows.build >= 22000 else platform.release()
    return classify_context(platform.system(), platform.release(), version, platform.machine(), product)

def collect(command, destination):
    destination = pathlib.Path(destination)
    destination.mkdir(mode=0o700, parents=False, exist_ok=False)
    result = {"schema_version": 1, "collected_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "context": context(), "command": command, "p0_status": "not_run",
              "inspection_status": "failed", "probe_exit": None,
              "qualification": "Collection does not run login cycles, grant permissions, install services or send input"}
    # Only invoke an operator-selected trusted local probe. No shell interpretation or network request.
    try:
        output = subprocess.run(command, capture_output=True, timeout=30, check=False)
        result["probe_exit"] = output.returncode
        (destination / "stdout.txt").write_bytes(output.stdout)
        (destination / "stderr.txt").write_bytes(output.stderr)
        if output.returncode == 0:
            try:
                value = json.loads(output.stdout)
                if isinstance(value, dict) and value.get("schema_version") == 1 and value.get("platform") in ("windows", "linux", "macos", "android"):
                    (destination / "inspection.json").write_text(json.dumps(value, indent=2), encoding="utf-8")
                    result["inspection_status"] = "observed"
                else:
                    result["error"] = "invalid inspection schema/platform"
            except (ValueError, UnicodeDecodeError) as error:
                result["error"] = str(error)
    except (OSError, subprocess.TimeoutExpired) as error:
        result["error"] = str(error)
    result["artifacts"] = [{"path": path.name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
                           for path in sorted(destination.iterdir()) if path.is_file()]
    (destination / "collection.json").write_text(json.dumps(result, indent=2), encoding="utf-8")
    return result

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--probe", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True, help="New directory under an existing parent")
    args = parser.parse_args()
    result = collect([str(args.probe.resolve(strict=True)), "inspect"], args.output)
    print(json.dumps(result, indent=2))
    return 0 if result["inspection_status"] == "observed" else 1

if __name__ == "__main__":
    raise SystemExit(main())
