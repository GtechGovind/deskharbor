"""Run the negative Linux checks on the no-display WSL vgem fixture. Emit JSON."""
import argparse
import datetime
import json
import os
from pathlib import Path
import platform
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--bin-dir", type=Path, required=True)
args = parser.parse_args()
bin_dir = args.bin_dir.resolve()
card = Path("/dev/dri/card0")
sysfs = Path("/sys/class/drm/card0").resolve()
if (os.geteuid() == 0 or "microsoft" not in platform.release().lower()
        or "vgem" not in sysfs.parts or os.access(card, os.R_OK)):
    raise SystemExit("requires an unprivileged WSL vgem card0 with denied read access")

def run(binary, *arguments, expected):
    result = subprocess.run(
        [str(bin_dir / binary), *arguments], capture_output=True,
        text=True, timeout=20, check=False,
    )
    if result.returncode != expected:
        raise RuntimeError(f"{binary} expected {expected}, observed {result.returncode}: {result.stderr}")
    return result

inspection = json.loads(run("dh-platform-probe", "inspect", expected=0).stdout)
if inspection["displays"]:
    raise SystemExit("requires no connected DRM displays")

checks = []
with tempfile.TemporaryDirectory() as tmp:
    target = Path(tmp) / "capture.bmp"
    for identifier, code in [("card0:1", "permission_denied"), ("primary", "no_readable_scanout")]:
        result = run("dh-platform-probe", "capture", "--display", identifier,
                     "--output", str(target), expected=1)
        if target.exists() or not result.stderr.startswith(code + ":"):
            raise RuntimeError(f"Unexpected capture result for {identifier}: {result.stderr}")
        checks.append({"display_argument": identifier, "exit_code": result.returncode,
                       "error": result.stderr.strip(), "output_created": target.exists()})

# No opt-in flag or input writes in this script.
no_opt_in = run("dh-uinput-tap", expected=2)
print(json.dumps({
    "observed_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "scope": "WSL negative diagnostic checks only",
    "card_sysfs_target": str(sysfs),
    "inspection": inspection,
    "captures": checks,
    "input_without_opt_in_exit": no_opt_in.returncode,
    "physical_cases": "not_run",
}, indent=2))
